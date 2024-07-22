use rspotify::clients::BaseClient;
use songbird::input::Compose;

use crate::prelude::*;

#[cfg(feature = "rvc")]
use crate::rvc;


pub enum SongLinkType {
    Youtube,
    Spotify(String),
    Search,
}

impl SongLinkType {
    pub fn new(s: &str) -> Self {
        let link_trim = s
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        let mut link_split = link_trim.split('/');
        
        if let Some(domain) = link_split.next() {
            if domain.contains("youtube") || domain.contains("youtu.be") {
                return SongLinkType::Youtube;
            }
            if domain.contains("spotify") {
                if let Some(track_url) = link_split.nth(1) {
                    let mut track_id = track_url;
                    if let Some(si_pos) = track_id.find("?si=") {
                        track_id = &track_url[..(si_pos)];
                    }
                    return SongLinkType::Spotify(track_id.to_string());
                };
            }
        }
        
        SongLinkType::Search
    }
}

pub enum Source {

    Chat(String),

    #[cfg(feature = "rvc")]
    RVC(rvc::RVCSong),
}

async fn youtubedl_get_title_async(mut youtubedl: songbird::input::YoutubeDl, optional_artist: Option<String>) -> Option<String> {
    if let Ok(metadata) = youtubedl.aux_metadata().await {
        if let Some(artist) = optional_artist.as_ref().or(metadata.artist.as_ref()) {
            if let Some(title) = metadata.title.as_ref() {
                return Some(format!("{} - {}", artist, title))
            }
        }
        if let Some(title) = metadata.title.as_ref() {
            return Some(title.clone())
        }
    }
    None
}

impl Source {
    pub async fn get_input(&self, ctx: &serenity::Context) -> Result<(songbird::input::Input, std::pin::Pin<Box<dyn futures::Future<Output = Option<String>> + Send>>), Error> {
        match self {
            Self::Chat(_) => {
                let source = self.get_youtube(ctx).await?;
                let title = youtubedl_get_title_async(source.clone(), None);
                Ok((source.into(), Box::pin(title)))
            },
            #[cfg(feature = "rvc")]
            Self::RVC(rvc_song) => {
                let file = rvc_song.file().await?;

                if !file.exists() {
                    return Err(Error::from("RVC Failed"));
                }

                let youtube_source = rvc_song.youtube.clone();
                let title = youtubedl_get_title_async(youtube_source, None);

                Ok((songbird::input::File::new(file).into(), Box::pin(title)))
            }
        }
    }

    pub async fn get_youtube(&self, ctx: &serenity::Context) -> Result<songbird::input::YoutubeDl, Error> {
        let shared = data::Shared::get(ctx).await;
        match self {
            Self::Chat(str) => {
                match SongLinkType::new(str) {
                    SongLinkType::Youtube => {
                        Ok(songbird::input::YoutubeDl::new(shared.http_client.clone(), str.clone()))
                    },
                    SongLinkType::Spotify(track_id) => {
                        let track_id = rspotify::model::TrackId::from_id(&track_id)?;
                        let track = loop {
                            match shared.spotify.track(track_id.clone(), None).await {
                                Ok(track) => break track,
                                Err(rspotify::ClientError::InvalidToken) => {
                                    shared.spotify.request_token().await?;
                                }
                                Err(err) => {
                                    return Err(Box::new(err));
                                }
                            }
                        };
                        let search_str = {
                            let mut search_str: String = String::with_capacity(64);
                            search_str.push_str("music ");
                            search_str.push_str(&track.artists.iter().map(|a| a.name.as_str()).collect::<Vec<&str>>().join(", "));
                            search_str.push_str(" - ");
                            search_str.push_str(&track.name);
                            search_str
                        };

                        Ok(songbird::input::YoutubeDl::new_search(shared.http_client.clone(), search_str))
                    },
                    SongLinkType::Search => {
                        Ok(songbird::input::YoutubeDl::new_search(shared.http_client.clone(), str.clone()))
                    },
                }
            },
            #[cfg(feature = "rvc")]
            Self::RVC(rvc_song) => {
                Ok(rvc_song.youtube.clone())
            }
        }
    }
}


pub struct Request {
    pub source: Source,
    pub guild_id: serenity::GuildId,
    pub author_id: serenity::UserId,
    pub channel_id: serenity::ChannelId,
    pub message_id: serenity::MessageId,
}

impl Request {
    pub const REACT_QUEUE: char = '🔖';
    pub const REACT_PLAYING: char = '🎵';
    pub const REACT_DONE: char = '✅';

    pub fn new(
        source: Source,
        guild_id: serenity::GuildId,
        author_id: serenity::UserId,
        channel_id: serenity::ChannelId,
        message_id: serenity::MessageId,
    ) -> Self {
        Self {
            source,
            guild_id,
            author_id,
            channel_id,
            message_id,
        }
    }

    pub async fn messge(&self, ctx: &serenity::Context) -> Option<serenity::Message> {
        ctx.http.get_message(self.channel_id, self.message_id).await.ok()
    }

    pub async fn react_queue(&self, ctx: &serenity::Context) -> Result<(), serenity::Error> {
        ctx.http
            .create_reaction(self.channel_id, self.message_id, &Self::REACT_QUEUE.into())
            .await
    }

    pub async fn remove_react_queue(&self, ctx: &serenity::Context) -> Result<(), serenity::Error> {
        ctx.http
            .delete_message_reaction_emoji(
                self.channel_id,
                self.message_id,
                &Self::REACT_QUEUE.into(),
            )
            .await       
    }

    pub async fn react_playing(&self, ctx: &serenity::Context) -> Result<(), serenity::Error> {
        ctx.http
            .delete_message_reaction_emoji(
                self.channel_id,
                self.message_id,
                &Self::REACT_QUEUE.into(),
            )
            .await?;
            ctx.http
                .create_reaction(
                    self.channel_id,
                    self.message_id,
                    &Self::REACT_PLAYING.into(),
                )
                .await
    }

    pub async fn react_done(&self, ctx: &serenity::Context) -> Result<(), serenity::Error> {
        ctx.http
            .delete_message_reaction_emoji(
                self.channel_id,
                self.message_id,
                &Self::REACT_PLAYING.into(),
            )
            .await?;
            ctx.http
                .create_reaction(self.channel_id, self.message_id, &Self::REACT_DONE.into())
                .await
    }
}

impl From<&serenity::Message> for Request {
    fn from(value: &serenity::Message) -> Self {
        Self::new(
            Source::Chat(value.content.clone()),
            value.guild_id.expect("Except message is in guild"),
            value.author.id,
            value.channel_id,
            value.id,
        )
    }
}

pub struct Now {
    pub track: songbird::tracks::TrackHandle,
    pub request: std::sync::Arc<Request>,
}

impl Now {
    pub fn new(track: songbird::tracks::TrackHandle, request: std::sync::Arc<Request>) -> Self {
        Self { track, request }
    }
}

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
    pub fn from_str(s: &str) -> Self {
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


// impl futures::Future for RVCSong {
//     type Output = ;
// }

pub enum Source {

    Chat(String),

    #[cfg(feature = "rvc")]
    RVC(rvc::RVCSong),
}

async fn youtubedl_get_title_async(mut youtubedl: songbird::input::YoutubeDl) -> Option<String> {
    if let Ok(metadata) = youtubedl.aux_metadata().await {
        if let Some(artist) = metadata.artist.as_ref() {
            if let Some(track) = metadata.track.as_ref() {
                return Some(format!("{} - {}", artist, track))
            }
        }
        if let Some(track) = metadata.track.as_ref() {
            return Some(track.clone())
        }
    }
    None
}

impl Source {
    pub async fn get_input(&self, ctx: &serenity::Context) -> Result<(songbird::input::Input, std::pin::Pin<Box<dyn futures::Future<Output = Option<String>> + Send>>), Error> {
        match self {
            Self::Chat(str) => {
                let shared = data::Shared::get(ctx).await;
                match SongLinkType::from_str(str) {
                    SongLinkType::Youtube => {
                        let source = songbird::input::YoutubeDl::new(shared.http_client.clone(), str.clone());
                        let title = youtubedl_get_title_async(source.clone());
                        Ok((source.into(), Box::pin(title)))
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

                        println!("spotify search_str = {}", &search_str);

                        let source = songbird::input::YoutubeDl::new_search(shared.http_client.clone(), search_str);
                        let title = youtubedl_get_title_async(source.clone());
                        Ok((source.into(), Box::pin(title)))
                    },
                    SongLinkType::Search => {
                        let source = songbird::input::YoutubeDl::new_search(shared.http_client.clone(), str.clone());
                        let title = youtubedl_get_title_async(source.clone());
                        Ok((source.into(), Box::pin(title)))
                    },
                }
            },
            #[cfg(feature = "rvc")]
            Self::RVC(rvc_song) => {
                Err(Error::from("in Development")) //Ok(songbird::input::File::new("test.wav").into())
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

    pub async fn messge(&self, ctx: &serenity::Context) -> serenity::Result<serenity::Message> {
        ctx.http.get_message(self.channel_id, self.message_id).await
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

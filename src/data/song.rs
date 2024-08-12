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

pub enum InputResult{
    Input(songbird::input::Input, std::pin::Pin<Box<dyn futures::Future<Output = Option<String>> + Send>>),
    Canceled
}

impl Source {
    pub async fn get_input(&self, ctx: &serenity::Context, #[allow(unused_variables)] locale: Option<&str>) -> Result<InputResult, Error> {
        match self {
            Self::Chat(_) => {
                let source = self.get_youtube(ctx).await?;
                let title = youtubedl_get_title_async(source.clone(), None);
                Ok(InputResult::Input(source.into(), Box::pin(title)))
            },
            #[cfg(feature = "rvc")]
            Self::RVC(rvc_song) => {
                match rvc_song.wait().await? {
                    rvc::RVCProcessorResult::Canceled => {
                        return Ok(InputResult::Canceled);
                    },
                    rvc::RVCProcessorResult::Error => {
                        return Err(Error::from("RVC Error"));
                    }
                    _ => {}
                }

                let file = rvc_song.file();

                if !file.exists() {
                    return Err(Error::from("RVC Failed"));
                }

                let title = rvc_song.title(locale);
                let title_future = async move {
                    Some(title)
                };

                Ok(InputResult::Input(songbird::input::File::new(file).into(), Box::pin(title_future)))
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

#[derive(Clone, Copy)]
pub enum RequestState {
    None,
    Queue,
    Playing,
    Done,
    Canceled,
    Skipped,
}

impl RequestState {
    pub fn emoji(&self) -> Option<char> {
        match self {
            RequestState::None => None,
            RequestState::Queue => Some('🔖'),
            RequestState::Playing => Some('🎵'),
            RequestState::Done => Some('✅'),
            RequestState::Canceled => Some('❌'),
            RequestState::Skipped => Some('💩'),
        }
    }
}

pub struct Request {
    pub source: Source,
    pub guild_id: serenity::GuildId,
    pub author_id: serenity::UserId,
    pub channel_id: serenity::ChannelId,
    pub message_id: serenity::MessageId,
    pub locale: Option<String>,
    pub state: std::sync::Arc<std::sync::Mutex<RequestState>>,
}

impl Request {

    pub fn new(
        source: Source,
        guild_id: serenity::GuildId,
        author_id: serenity::UserId,
        channel_id: serenity::ChannelId,
        message_id: serenity::MessageId,
        locale: Option<impl Into<String>>,
    ) -> Self {
        Self {
            source,
            guild_id,
            author_id,
            channel_id,
            message_id,
            locale: locale.map(Into::into),
            state: std::sync::Arc::new(std::sync::Mutex::new(RequestState::Queue))
        }
    }

    pub fn cancel(&self) {
        if let Source::RVC(song) = &self.source {
            song.cancel();
        }
    }

    pub async fn messge(&self, ctx: &serenity::Context) -> Option<serenity::Message> {
        ctx.http.get_message(self.channel_id, self.message_id).await.ok()
    }
    
    pub async fn remove_react(&self, ctx: &serenity::Context) -> Result<(), serenity::Error> {
        let emoji = self.state.lock().unwrap().emoji();
        if let Some(emoji) = emoji {
            ctx.http
            .delete_message_reaction_emoji(
                self.channel_id,
                self.message_id,
                &emoji.into(),
            )
            .await?;
        }
        Ok(())
    }
    
    pub async fn add_react(&self, ctx: &serenity::Context) -> Result<(), serenity::Error> {
        let emoji = self.state.lock().unwrap().emoji();
        if let Some(emoji) = emoji {
            ctx.http
            .create_reaction(self.channel_id, self.message_id, &emoji.into())
            .await?;
        }
        Ok(())
    }

    pub async fn set_state_async(&self, ctx: &serenity::Context, state: RequestState) -> Result<(), serenity::Error> {
        self.remove_react(ctx).await?;
        *self.state.lock().unwrap() = state;
        self.add_react(ctx).await?;
        Ok(())
    }

    pub fn set_state_nowait(self: std::sync::Arc<Request>, ctx: serenity::Context, state: RequestState) {
        tokio::spawn(async move { 
            self.set_state_async(&ctx, state).await.ok(); 
        });
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
            value.author.locale.as_ref(),
        )
    }
}

pub enum Now {
    Waiting {
        request: std::sync::Arc<Request>
    },
    Playing {
        track: songbird::tracks::TrackHandle,
        request: std::sync::Arc<Request>,
    }
}

impl Now {
    pub fn request(&self) -> std::sync::Arc<Request> {
        match self {
            Self::Waiting { request } => {
                request.clone()
            },
            Self::Playing { track: _, request } => {
                request.clone()
            }
        }
    }
}

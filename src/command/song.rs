use poise::serenity_prelude::async_trait;
use std::sync::Arc;

use crate::{data, prelude::*};

#[cfg(feature = "rvc")]
use crate::rvc;

use super::data::song;

#[derive(Debug)]
pub enum SongError {
    Guild,
    VoiceChannel,
    VoiceConnection,
}

impl std::fmt::Display for SongError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Guild => f.write_str("Song feature requires guild to use voice chat."),
            Self::VoiceChannel => f.write_str("Cannot find voice channel from request."),
            Self::VoiceConnection => f.write_str("Bot is not connected to the voice channel."),
        }
    }
}

impl std::error::Error for SongError {}



#[derive(Copy, Clone, Default, poise::ChoiceParameter)]
pub enum SongRequestTarget
{
    #[default]
    Play,
    Download,
    All,
}

impl SongRequestTarget {

    #[allow(dead_code)]
    pub fn should_play(&self) -> bool {
        match self {
            SongRequestTarget::Play => true,
            SongRequestTarget::Download => false,
            SongRequestTarget::All => true,
        }
    }

    #[allow(dead_code)]
    pub fn should_download(&self) -> bool {
        match self {
            SongRequestTarget::Play => false,
            SongRequestTarget::Download => true,
            SongRequestTarget::All => true,
        }
    }
}


struct TrackEndHandler {
    context: serenity::Context,
    guild_id: serenity::GuildId,
}

#[async_trait]
impl songbird::EventHandler for TrackEndHandler {
    async fn act(&self, _: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        let guild_data = data::Storage::guild(&self.context, self.guild_id).await;
        guild_data.lock().await.song_now_complete(&self.context);

        next_internal(&self.context, self.guild_id).await.ok();
        None
    }
}

struct DisconnectHandler {
    context: serenity::Context,
    guild_id: serenity::GuildId,
}

#[async_trait]
impl songbird::EventHandler for DisconnectHandler {
    async fn act(&self, _: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        self.context.shard.set_activity(None);
        let guild_data = data::Storage::guild(&self.context, self.guild_id).await;
        let mut guild_data = guild_data.lock().await;
        guild_data.song_now_cancel(&self.context);
        guild_data.song_queue_clear(&self.context);
        None
    }
}

pub enum SongCommandResult {
    Play,
    Queue,
}

pub async fn get_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Option<Arc<tokio::sync::Mutex<songbird::Call>>> {
    let manager = songbird::get(ctx).await.unwrap().clone();
    manager.get(guild_id)
}

pub async fn join_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    author_id: serenity::UserId,
) -> Result<Arc<tokio::sync::Mutex<songbird::Call>>, Error> {

    let (guild_id, channel_id) = {
        let guild = match ctx.cache.guild(guild_id) {
            Some(guild) => guild,
            None => return Err(SongError::Guild.into()),
        };
        let channel_id = guild
            .voice_states
            .get(&author_id)
            .and_then(|voice_state| voice_state.channel_id);
        (guild.id, channel_id)
    };

    match channel_id {
        Some(channel_id) => {
            let manager = songbird::get(ctx).await.unwrap().clone();
            if let Some(call) = manager.get(guild_id) {
                if call.lock().await.current_channel()
                    == Some(songbird::id::ChannelId::from(channel_id))
                {
                    return Ok(call);
                }
            }

            let call_ptr = manager.join(guild_id, channel_id).await?;

            // Initialize Call
            {
                let mut call = call_ptr.lock().await;
                call.deafen(true).await?;
                call.add_global_event(
                    songbird::Event::Core(songbird::CoreEvent::DriverDisconnect),
                    DisconnectHandler {
                        context: ctx.clone(),
                        guild_id,
                    },
                )
            }

            Ok(call_ptr)
        }
        None => Err(SongError::VoiceChannel.into()),
    }
}

pub async fn join_or_get(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    author_id: Option<serenity::UserId>,
) -> Result<Arc<tokio::sync::Mutex<songbird::Call>>, Error> {
    if let Some(author_id) = author_id {
        let call = join_internal(ctx, guild_id, author_id).await;
        if call.is_ok() {
            return call;
        }
    }

    get_internal(ctx, guild_id)
        .await
        .ok_or(SongError::VoiceConnection.into())
}



pub async fn play_internal(
    ctx: &serenity::Context,
    request: Arc<data::song::Request>,
) -> Result<Option<songbird::tracks::TrackHandle>, Error> {
    let guild_data = data::Storage::guild(ctx, request.guild_id).await;
    guild_data.lock().await.song_now = Some(data::song::Now::Waiting{
        request: request.clone()
    });

    let (handle, title_future) = {
   
        let (input, title_future) = match request.source.get_input(ctx, request.locale.as_deref()).await? {
            song::InputResult::Input(input, title_future) => (input, title_future),
            song::InputResult::Canceled => {
                return Ok(None)
            },
        };
        
        let handle = {
            let call = join_or_get(ctx, request.guild_id, Some(request.author_id)).await?;
            let mut call = call.lock().await;
            call.play_only_input(input)
        };
        
        guild_data.lock().await.song_now = Some(data::song::Now::Playing{
            track: handle.clone(),
            request: request.clone()
        });

        (handle, title_future)
    };

    if let Some(title) = title_future.await {
        ctx.set_activity(Some(serenity::ActivityData::listening(title)))
    } else {
        ctx.set_activity(Some(serenity::ActivityData::listening("")))
    }

    handle.add_event(
        songbird::Event::Track(songbird::TrackEvent::End),
        TrackEndHandler {
            context: ctx.clone(),
            guild_id: request.guild_id,
        },
    )?;

    request.set_state_nowait(ctx.clone(), song::RequestState::Playing);

    Ok(Some(handle))
}

pub async fn queue_internal(
    ctx: &serenity::Context,
    request: std::sync::Arc<data::song::Request>,
) -> Result<SongCommandResult, Error> {

    let guild_id = request.guild_id;
    let guild_data = data::Storage::guild(ctx, guild_id).await;

    let first_queue = {
        let mut guild_data = guild_data.lock().await;
        guild_data.song_queue.push_back(request.clone());
        guild_data.song_queue.len() == 1 && guild_data.song_now.is_none()
    };

    request.set_state_nowait(ctx.clone(), song::RequestState::Queue);

    if first_queue {
        next_internal(ctx, guild_id).await?;
        Ok(SongCommandResult::Play)
    } else {
        Ok(SongCommandResult::Queue)
    }
}

pub async fn cancel_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Result<(), Error> {
    let guild_data = data::Storage::guild(ctx, guild_id).await;
    guild_data.lock().await.song_now_cancel(ctx);

    if let Some(call) = get_internal(ctx, guild_id).await {
        let mut call = call.lock().await;
        call.stop();
    }

    ctx.set_activity(None);

    Ok(())
}

pub async fn next_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Result<(), Error> {
    cancel_internal(ctx, guild_id).await?;
    let guild_data = data::Storage::guild(ctx, guild_id).await;

    loop
    {
        let next = guild_data.lock().await.song_queue_take(ctx).await;

        match next {
            Some(next) => {
                match play_internal(ctx, next.clone()).await {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        if let Ok(message) = ctx.http.get_message(next.channel_id, next.message_id).await {
                            let error_message: String = format!("error : {:?}", e);
                            message.reply(ctx, error_message).await?;
                        }
                        next.set_state_nowait(ctx.clone(), song::RequestState::Canceled);
                    },
                }
            }
            None => {
                return Ok(());
            }
        }
    }
}

#[poise::command(
    slash_command,
    guild_only,
    subcommands("join", "leave", "stop", "next"),
    subcommand_required
)]
pub async fn song(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("join").await?;
    join_internal(
        ctx.serenity_context(),
        ctx.guild_id().unwrap(),
        ctx.author().id,
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(call) = get_internal(ctx.serenity_context(), ctx.guild_id().unwrap()).await {
        {
            let mut call = call.lock().await;
            call.leave().await?;
        }
        ctx.reply("leave").await?;
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("song stop").await?;
    cancel_internal
(ctx.serenity_context(), ctx.guild_id().unwrap()).await?;

    let guild_data = data::Storage::guild(ctx.serenity_context(), ctx.guild_id().unwrap()).await;
    guild_data.lock().await.song_queue_clear(ctx.serenity_context());
    Ok(())
}

#[poise::command(slash_command)]
pub async fn next(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("song next").await?;
    next_internal(ctx.serenity_context(), ctx.guild_id().unwrap()).await?;
    Ok(())
}

#[cfg(feature = "rvc")]
#[poise::command(slash_command)]
pub async fn ai(ctx: Context<'_>, singer: rvc::Model, song: String, pitch: Option<i32>, target: Option<SongRequestTarget>) -> Result<(), Error> {
    let reply = ctx.reply("...song ai fetching").await?;

    let target = target.unwrap_or_default();
    let youtube = data::song::Source::Chat(song).get_youtube(ctx.serenity_context()).await?;
    let rvc_song = rvc::RVCSong::new(singer, youtube, pitch, target.should_download()).await?;    
    
    let name = rvc_song.title(ctx.locale());
    let reply_builder: poise::CreateReply = poise::CreateReply {
        content: Some(name.clone()),
        ..Default::default()
    };
    reply.edit(ctx, reply_builder).await?;
    let message = reply.message().await?;
    
    let request = Arc::new(data::song::Request::new(
        data::song::Source::RVC(rvc_song),
        ctx.guild_id().expect("This command can only be used within guilds."), 
        ctx.author().id,
        ctx.channel_id(),
        message.id,
        ctx.locale()
    ));

    if target.should_play() {
        queue_internal(ctx.serenity_context(), request.clone()).await?;
    }
    
    if target.should_download() {
        if let data::song::Source::RVC(rvc_song) = &request.source {
            if let rvc::RVCProcessorResult::Good = rvc_song.wait().await? {
                let mp3 = tokio::fs::File::open(rvc_song.mp3()).await?;
                let message = serenity::CreateMessage::new()
                    .add_file(serenity::CreateAttachment::file(&mp3, format!("{}.mp3", name)).await?);
                ctx.channel_id().send_message(ctx, message).await?;
            }
        }
    }
    
    Ok(())
}
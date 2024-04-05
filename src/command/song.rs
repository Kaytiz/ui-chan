use poise::serenity_prelude::async_trait;
use songbird::input::Compose;
use std::sync::Arc;

use crate::{data, prelude::*};

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

struct TrackEndHandler {
    context: serenity::Context,
    guild_id: serenity::GuildId,
}

#[async_trait]
impl songbird::EventHandler for TrackEndHandler {
    async fn act(&self, _: &songbird::EventContext<'_>) -> Option<songbird::Event> {
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
        guild_data.song_now_complete(&self.context).await.ok();
        guild_data.song_queue_clear(&self.context).await.ok();
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
    request: data::song::Request,
) -> Result<songbird::tracks::TrackHandle, Error> {
    let shared = data::Shared::get(ctx).await;
    let guild_data = data::Storage::guild(ctx, request.guild_id).await;

    let (mut src, handle) = {
        let mut guild_data = guild_data.lock().await;

        let src = if request.url.starts_with("http") {
            songbird::input::YoutubeDl::new(shared.http_client.clone(), request.url.clone())
        } else {
            songbird::input::YoutubeDl::new_search(shared.http_client.clone(), request.url.clone())
        };

        let handle = {
            let call = join_or_get(ctx, request.guild_id, Some(request.author_id)).await?;
            let mut call = call.lock().await;
            call.play_only_input(src.clone().into())
        };

        guild_data.song_now_complete(ctx).await?;
        guild_data.song_now = Some(data::song::Now::new(handle.clone(), request.clone()));
        guild_data.save().await?;

        (src, handle)
    };

    let metadata = src.aux_metadata().await?;

    if let Some(title) = metadata.title.as_deref() {
        ctx.set_activity(Some(serenity::ActivityData::listening(title)))
    }

    handle.add_event(
        songbird::Event::Track(songbird::TrackEvent::End),
        TrackEndHandler {
            context: ctx.clone(),
            guild_id: request.guild_id,
        },
    )?;

    request.react_playing(ctx).await?;

    Ok(handle)
}

pub async fn queue_internal(
    ctx: &serenity::Context,
    message: &serenity::Message,
) -> Result<SongCommandResult, Error> {
    let request = data::song::Request::from(message);
    let guild_id = request.guild_id;
    let guild_data = data::Storage::guild(ctx, guild_id).await;

    let first_queue = {
        let mut guild_data = guild_data.lock().await;
        guild_data.song_queue.push_back(request.clone());
        guild_data.song_queue.len() == 1 && guild_data.song_now.is_none()
    };

    request.react_queue(ctx).await?;

    if first_queue {
        next_internal(ctx, guild_id).await?;
        Ok(SongCommandResult::Play)
    } else {
        Ok(SongCommandResult::Queue)
    }
}

pub async fn stop_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Result<(), Error> {
    let guild_data = data::Storage::guild(ctx, guild_id).await;
    let mut guild_data = guild_data.lock().await;

    let call = get_internal(ctx, guild_id)
        .await
        .ok_or(Box::new(SongError::VoiceConnection))?;
    let mut call = call.lock().await;
    call.stop();

    ctx.set_activity(None);
    guild_data.song_now_complete(ctx).await?;
    guild_data.song_queue_clear(ctx).await?;

    Ok(())
}

pub async fn next_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Result<(), Error> {
    let guild_data = data::Storage::guild(ctx, guild_id).await;

    let next = guild_data.lock().await.song_queue.pop_front();

    match next {
        Some(next) => {
            play_internal(ctx, next).await?;
        }
        None => {
            stop_internal(ctx, guild_id).await?;
        }
    }

    Ok(())
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
    join_internal(
        ctx.serenity_context(),
        ctx.guild_id().unwrap(),
        ctx.author().id,
    )
    .await?;
    ctx.reply("join").await?;
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
    stop_internal(ctx.serenity_context(), ctx.guild_id().unwrap()).await
}

#[poise::command(slash_command)]
pub async fn next(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("song next").await?;
    next_internal(ctx.serenity_context(), ctx.guild_id().unwrap()).await?;
    Ok(())
}

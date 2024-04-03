use std::sync::Arc;
use tokio::sync::Mutex;

use crate::prelude::*;
use songbird::{
    input::{Compose, YoutubeDl},
    Call,
};

#[derive(Debug)]
pub enum SongError {
    NoGuild,
    NoVoiceChannel,
    NoVoiceConnection,
}

impl std::fmt::Display for SongError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGuild => f.write_str("Song feature requires guild to use voice chat."),
            Self::NoVoiceChannel => f.write_str("Cannot find voice channel from request."),
            Self::NoVoiceConnection => f.write_str("Bot is not connected to the voice channel."),
        }
    }
}

impl std::error::Error for SongError {}

pub async fn get_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Option<Arc<Mutex<Call>>> {
    let manager = songbird::get(ctx).await.unwrap().clone();
    manager.get(guild_id)
}

pub async fn join_internal(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    author_id: serenity::UserId,
) -> Result<Arc<Mutex<Call>>, Error> {
    let (guild_id, channel_id) = {
        let guild = match ctx.cache.guild(guild_id) {
            Some(guild) => guild,
            None => return Err(SongError::NoGuild.into()),
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
            let call = manager.join(guild_id, channel_id).await?;
            init_call(call.clone()).await?;
            Ok(call)
        }
        None => Err(SongError::NoVoiceChannel.into()),
    }
}

pub async fn init_call(call: Arc<Mutex<Call>>) -> Result<(), Error> {
    let mut call = call.lock().await;
    call.deafen(true).await?;
    Ok(())
}

pub async fn handle_play(
    ctx: &serenity::Context,
    data: &Data,
    url: String,
    guild_id: serenity::GuildId,
    author_id: serenity::UserId,
) -> Result<(), Error> {
    let mut src = if url.starts_with("http") {
        YoutubeDl::new(data.http_client.clone(), url)
    } else {
        YoutubeDl::new_search(data.http_client.clone(), url)
    };

    {
        let call_mutex = join_internal(ctx, guild_id, author_id).await?;
        let mut call = call_mutex.lock().await;

        call.play_only_input(src.clone().into());
    }

    if let Some(title) = src
        .aux_metadata()
        .await
        .ok()
        .and_then(|metadata| metadata.title)
    {
        ctx.set_activity(Some(serenity::ActivityData::listening(title)));
    }

    Ok(())
}

#[poise::command(
    slash_command,
    guild_only,
    subcommands("join", "leave", "play", "stop", "skip"),
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
    Ok(())
}

#[poise::command(slash_command)]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(call) = get_internal(ctx.serenity_context(), ctx.guild_id().unwrap()).await {
        let mut call = call.lock().await;
        call.leave().await?;
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "유튜브 URL 또는 검색어"] url: String,
) -> Result<(), Error> {
    handle_play(
        ctx.serenity_context(),
        ctx.data(),
        url,
        ctx.guild_id().unwrap(),
        ctx.author().id,
    )
    .await
}

#[poise::command(slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let call = get_internal(ctx.serenity_context(), ctx.guild_id().unwrap())
        .await
        .ok_or(Box::new(SongError::NoVoiceConnection))?;
    let mut call = call.lock().await;
    call.stop();
    Ok(())
}

#[poise::command(slash_command)]
pub async fn skip(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

use crate::{command, data, prelude::*};

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    _: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.name);
        }
        serenity::FullEvent::CacheReady { guilds } => {
            println!("Cache ready! {:?}", guilds);
        }
        serenity::FullEvent::Ratelimit { data } => {
            println!("ratelilmit! {:?}", data);
        }
        serenity::FullEvent::Message { new_message } => {
            handle_queue_new_song(ctx, new_message).await;
        }
        serenity::FullEvent::MessageDelete {
            channel_id: _,
            deleted_message_id,
            guild_id: Some(guild_id),
        } => {
            handle_queue_delete(ctx, deleted_message_id, guild_id).await;
        }
        serenity::FullEvent::VoiceStateUpdate {
            old: Some(old),
            new: _,
        } => {
            handle_exit_when_nobody(ctx, old).await;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_queue_new_song(ctx: &serenity::Context, message: &serenity::Message) -> Option<()> {
    if message.author.id == ctx.cache.current_user().id {
        return None;
    }

    let guild_id = message.guild_id?;

    let is_song_channel = {
        let guild_data = data::Storage::guild(ctx, guild_id).await;
        let guild_data = guild_data.lock().await;
        guild_data.channel_song == Some(message.channel_id)
    };

    if is_song_channel {
        if let Err(err) = command::song::queue_internal(ctx, std::sync::Arc::new(data::song::Request::from(message))).await {
            message.reply(ctx, &format!("error: {}", err)).await.ok()?;
        }
    }

    Some(())
}

async fn handle_queue_delete(
    ctx: &serenity::Context,
    deleted_message_id: &serenity::MessageId,
    guild_id: &serenity::GuildId,
) -> Option<()> {
    let guild_data = data::Storage::guild(ctx, *guild_id).await;
    let mut guild_data = guild_data.lock().await;

    if guild_data
        .song_now
        .as_ref()
        .map_or(false, |now| now.request.message_id == *deleted_message_id)
    {
        drop(guild_data);
        command::song::next_internal(ctx, *guild_id).await.ok()?;
    } else if let Some(index) = {
        guild_data
            .song_queue
            .iter()
            .position(|queue| queue.message_id == *deleted_message_id)
    } {
        guild_data.song_queue.remove(index);
    }

    Some(())
}

async fn handle_exit_when_nobody(
    ctx: &serenity::Context,
    old: &serenity::VoiceState,
) -> Option<()> {
    let channel_id = old.channel_id?;
    let guild_id = old.guild_id?;

    let call = command::song::get_internal(ctx, guild_id).await?;

    let call_channel_id = call.lock().await.current_channel();
    if call_channel_id != Some(channel_id.into()) {
        return None;
    }

    let channel = ctx.http.get_channel(channel_id).await.ok()?;
    if let Some(guild_channel) = channel.guild() {
        let members = guild_channel.members(&ctx.cache).ok()?;
        let num_voice_members = members.iter().filter(|member| !member.user.bot).count();
        if num_voice_members == 0 {
            let mut call = call.lock().await;
            call.leave().await.ok()?;
        }
    }

    Some(())
}

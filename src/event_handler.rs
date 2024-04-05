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
        serenity::FullEvent::Message { new_message } => {
            if new_message.author.id != ctx.cache.current_user().id {
                if let Some(guild_id) = new_message.guild_id {
                    let is_song_channel = {
                        let guild_data = data::Storage::guild(ctx, guild_id).await;
                        let guild_data = guild_data.lock().await;
                        guild_data.channel_song == Some(new_message.channel_id)
                    };
                    if is_song_channel {
                        if let Err(err) = command::song::queue_internal(ctx, new_message).await {
                            new_message.reply(ctx, &format!("error: {}", err)).await?;
                        }
                    }
                }
            }
        }
        serenity::FullEvent::MessageDelete {
            channel_id: _,
            deleted_message_id,
            guild_id: Some(guild_id),
        } => {
            let guild_data = data::Storage::guild(ctx, *guild_id).await;
            let mut guild_data = guild_data.lock().await;

            if guild_data
                .song_now
                .as_ref()
                .map_or(false, |now| now.request.message_id == *deleted_message_id)
            {
                drop(guild_data);
                command::song::next_internal(ctx, *guild_id).await?;
            } else if let Some(index) = {
                guild_data
                    .song_queue
                    .iter()
                    .position(|queue| queue.message_id == *deleted_message_id)
            } {
                guild_data.song_queue.remove(index);
            }
        }
        serenity::FullEvent::Ratelimit { data } => {
            println!("ratelilmit! {:?}", data);
        }
        _ => {}
    }
    Ok(())
}

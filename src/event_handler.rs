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
                    let is_song = {
                        let storage = data::Storage::get(ctx).await;
                        let storage = storage.lock().await;
                        match storage.guild(guild_id) {
                            Some(guild) => guild.channel_song == Some(new_message.channel_id),
                            None => false,
                        }
                    };

                    if is_song {
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
            let storage = data::Storage::get(ctx).await;
            let mut storage = storage.lock().await;
            let guild = storage.guild_mut(*guild_id);
            if let Some(index) = {
                guild
                    .song_queue
                    .iter()
                    .position(|queue| queue.message_id == *deleted_message_id)
            } {
                guild.song_queue.remove(index);
            }
        }
        _ => {}
    }
    Ok(())
}

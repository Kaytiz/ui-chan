use std::sync::atomic::Ordering;

use crate::{command, data::channel, prelude::*};

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.name);
        }
        serenity::FullEvent::CacheReady { guilds } => {
            println!("cache ready! {:?}", guilds);
        }
        serenity::FullEvent::Message { new_message } => {
            if new_message.author.id != ctx.cache.current_user().id {
                if new_message.content.to_lowercase().contains("poise") {
                    let old_mentions = data.poise_mentions.fetch_add(1, Ordering::SeqCst);
                    new_message
                        .reply(
                            ctx,
                            format!("Poise has been mentioned {} times", old_mentions + 1),
                        )
                        .await?;
                }

                if let Some(guild_id) = new_message.guild_id {
                    let is_song = {
                        data.storage
                            .lock()
                            .unwrap()
                            .channel(guild_id, new_message.channel_id)
                            .map_or(false, |channel| channel.has_property(channel::is_song))
                    };

                    if is_song {
                        if let Err(err) = command::song::handle_play(ctx, data, new_message).await {
                            new_message.reply(ctx, &format!("error: {}", err)).await?;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

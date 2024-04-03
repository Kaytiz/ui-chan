use crate::prelude::*;

/// 명령어를 등록합니다.
#[poise::command(prefix_command, owners_only)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// 봇의 데이터를 저장합니다.
#[poise::command(slash_command, owners_only)]
pub async fn save(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data().save_default()?;
    ctx.say("saved!").await?;
    Ok(())
}

pub mod channel {
    use crate::{data, prelude::*};

    #[poise::command(
        slash_command,
        owners_only,
        guild_only,
        subcommands("info", "notify", "song", "attribute"),
        subcommand_required
    )]
    pub async fn channel(_: Context<'_>) -> Result<(), Error> {
        Ok(())
    }

    /// 현재 채널에 대한 정보를 받아옵니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn info(ctx: Context<'_>) -> Result<(), Error> {
        let response = {
            let storage = ctx.data().storage.lock().unwrap();
            if let Some(guild) = storage.guild(ctx.guild_id().unwrap()) {
                let channel = guild.channel(ctx.channel_id());
                match channel {
                    Some(channel) => {
                        let mut properties: Vec<String> = vec![];
                        if guild.channel_notify == Some(ctx.channel_id()) {
                            properties.push("Primary Notify".into());
                        }
                        properties.append(
                            &mut channel
                                .properties
                                .iter()
                                .map(|p| format!("{:?}", p))
                                .collect::<Vec<String>>(),
                        );
                        format!("channel properties : {:?}", properties)
                    }
                    None => String::from("no channel properties"),
                }
            } else {
                String::from("no channel properties")
            }
        };
        ctx.say(&response).await?;
        Ok(())
    }

    /// 현재 채널을 서버의 기본 알림 채널로 설정합니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn notify(ctx: Context<'_>) -> Result<(), Error> {
        {
            let mut storage = ctx.data().storage.lock().unwrap();
            let guild = storage.guild_mut(ctx.guild_id().unwrap());
            guild.channel_notify = Some(ctx.channel_id());
        };

        let response = format!(
            "Now, {} is the primary notification channel!",
            ctx.channel_id().name(ctx.http()).await?
        );
        ctx.say(&response).await?;
        ctx.data().save_default()?;

        Ok(())
    }

    /// 현재 채널을 음악 채널로 설정합니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn song(ctx: Context<'_>) -> Result<(), Error> {
        let response = {
            let mut storage = ctx.data().storage.lock().unwrap();
            let channel = storage.channel_mut(ctx.guild_id().unwrap(), ctx.channel_id());
            if channel.has_property(data::channel::is_song) {
                channel.remove_property(data::channel::is_song);
                "channel song off"
            } else {
                channel.set_property(data::channel::Property::Song);
                "channel song on"
            }
        };

        ctx.say(response).await?;
        ctx.data().save_default()?;

        Ok(())
    }

    /// 현재 채널의 attribute를 설정합니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn attribute(ctx: Context<'_>, attribute: String) -> Result<(), Error> {
        let response = {
            let mut storage = ctx.data().storage.lock().unwrap();
            let channel = storage.channel_mut(ctx.guild_id().unwrap(), ctx.channel_id());
            if attribute.is_empty() {
                channel.remove_property(data::channel::is_attribute);
                String::from("channel attribute removed")
            } else {
                channel.set_property(data::channel::Property::Attribute(attribute.clone()));
                format!("channel attribute set to {}", &attribute)
            }
        };

        ctx.say(&response).await?;
        ctx.data().save_default()?;

        Ok(())
    }
}

use crate::{data, prelude::*};

/// 명령어를 등록합니다.
#[poise::command(prefix_command, owners_only)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// 봇의 데이터를 저장합니다.
#[poise::command(slash_command, owners_only)]
pub async fn save(ctx: Context<'_>) -> Result<(), Error> {
    data::Storage::guild(ctx.serenity_context(), ctx.guild_id().unwrap())
        .await
        .lock()
        .await
        .save()
        .await?;
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
            let guild_id = ctx.guild_id().unwrap();
            let guild_data = data::Storage::guild(ctx.serenity_context(), guild_id).await;
            let guild_data = guild_data.lock().await;
            let channel_data = guild_data.channel(ctx.channel_id());
            match channel_data {
                Some(channel_data) => {
                    let mut properties: Vec<String> = vec![];
                    if guild_data.channel_notify == Some(ctx.channel_id()) {
                        properties.push("Primary Notify".into());
                    }
                    if guild_data.channel_song == Some(ctx.channel_id()) {
                        properties.push("Primary Song".into());
                    }
                    properties.append(
                        &mut channel_data
                            .properties
                            .iter()
                            .map(|p| format!("{}", p))
                            .collect::<Vec<String>>(),
                    );
                    if !properties.is_empty() {
                        format!("channel properties : {:?}", properties)
                    } else {
                        String::from("no channel properties")
                    }
                }
                None => String::from("no channel properties"),
            }
        };
        ctx.say(&response).await?;
        Ok(())
    }

    /// 현재 채널을 서버의 기본 알림 채널로 설정합니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn notify(ctx: Context<'_>) -> Result<(), Error> {
        {
            let guild_id = ctx.guild_id().unwrap();
            let guild_data = data::Storage::guild(ctx.serenity_context(), guild_id).await;
            let mut guild_data = guild_data.lock().await;
            guild_data.channel_notify = Some(ctx.channel_id());
            guild_data.save().await?;
        };

        let response = format!(
            "Now, {} is the primary notification channel!",
            ctx.channel_id().name(ctx.http()).await?
        );
        ctx.say(&response).await?;

        Ok(())
    }

    /// 현재 채널을 음악 채널로 설정합니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn song(ctx: Context<'_>) -> Result<(), Error> {
        {
            let guild_id = ctx.guild_id().unwrap();
            let guild_data = data::Storage::guild(ctx.serenity_context(), guild_id).await;
            let mut guild_data = guild_data.lock().await;
            guild_data.channel_song = Some(ctx.channel_id());
            guild_data.save().await?;
        };

        let response = format!(
            "Now, {} is the primary song channel!",
            ctx.channel_id().name(ctx.http()).await?
        );
        ctx.say(&response).await?;

        Ok(())
    }

    /// 현재 채널의 attribute를 설정합니다.
    #[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
    pub async fn attribute(ctx: Context<'_>, attribute: String) -> Result<(), Error> {
        let response = {
            let guild_id = ctx.guild_id().unwrap();
            let guild_data = data::Storage::guild(ctx.serenity_context(), guild_id).await;
            let mut guild_data = guild_data.lock().await;
            let channel_data = guild_data.channel_mut(ctx.channel_id());
            if attribute.is_empty() {
                channel_data.remove_property(data::channel::Property::is_attribute);
                guild_data.save().await?;
                String::from("channel attribute removed")
            } else {
                channel_data.set_property(data::channel::Property::Attribute(attribute.clone()));
                guild_data.save().await?;
                format!("channel attribute set to {}", &attribute)
            }
        };

        ctx.say(&response).await?;

        Ok(())
    }
}

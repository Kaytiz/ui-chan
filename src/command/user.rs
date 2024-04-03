use std::str::FromStr;

use chrono::NaiveDate;
use poise::Modal;

use crate::prelude::*;

#[poise::command(
    slash_command,
    guild_only,
    subcommands("info", "edit"),
    subcommand_required
)]
pub async fn user(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// 사용자의 정보를 표시합니다.
#[poise::command(slash_command)]
pub async fn info(ctx: Context<'_>, target: Option<serenity::UserId>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let user_id = target.unwrap_or(ctx.author().id);

    let user = {
        let storage = ctx.data().storage.lock().unwrap();
        storage.user(guild_id, user_id).cloned()
    };

    match user {
        Some(user) => ctx.reply(format!("{:?}", user)).await?,
        None => ctx.reply("no user data").await?,
    };

    Ok(())
}

#[derive(Debug, poise::Modal, Default)]
#[name = "유저 정보 (필수 아님 원하는것 일부만)"]
struct UserModal {
    #[name = "생일 (형식 : 2002-01-01)"]
    birthday: Option<String>,

    #[name = "전화번호"]
    phone_number: Option<String>,
}

/// 사용자의 정보를 수정합니다.
#[poise::command(slash_command)]
pub async fn edit(ctx: ApplicationContext<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let user_id = ctx.author().id;

    let defaults = {
        let storage = ctx.data().storage.lock().unwrap();
        match storage.user(guild_id, user_id) {
            Some(user) => {
                let birthday = user.birthday.map(|date| date.to_string());
                let phone_number = user.phone_number.to_owned();
                UserModal {
                    birthday,
                    phone_number,
                }
            }
            None => Default::default(),
        }
    };

    let data = UserModal::execute_with_defaults(ctx, defaults).await?;

    if let Some(data) = data {
        {
            let mut storage = ctx.data().storage.lock().unwrap();
            let user = storage.user_mut(guild_id, user_id);
            user.birthday = data
                .birthday
                .and_then(|date_str| NaiveDate::from_str(&date_str).ok());
            user.phone_number = data.phone_number;
        }
        ctx.data().save_default()?;
        ctx.reply("updated!").await?;
    }

    Ok(())
}

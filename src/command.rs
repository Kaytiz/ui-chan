use crate::prelude::*;

pub mod dev;
pub mod owner;
pub mod song;
pub mod user;

/// 도움말을 표시합니다.
#[poise::command(slash_command, track_edits)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "설명을 보고싶은 커맨드"]
    #[autocomplete = poise::builtins::autocomplete_command]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

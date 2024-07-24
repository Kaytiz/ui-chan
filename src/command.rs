use crate::prelude::*;

pub mod owner;
pub mod song;
pub mod user;

/// 도움말을 표시합니다.
#[poise::command(slash_command, track_edits)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "설명을 보고 싶은 커맨드"]
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


pub fn create_commands() -> Vec<poise::Command<Data, Error>> {
    let mut commands = vec![
        help(),
        owner::register(),
        owner::reload(),
        owner::save(),
        owner::channel::channel(),
        user::user(),
    ];

    #[allow(unused_mut)]
    let mut command_song = song::song();

    #[cfg(feature = "rvc")]
    command_song.subcommands.push(song::ai());

    commands.push(command_song);

    commands
}
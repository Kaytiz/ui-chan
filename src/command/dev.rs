use crate::prelude::*;
use rand::Rng;

/// dev age
#[poise::command(slash_command)]
pub async fn age(
    ctx: Context<'_>,
    #[description = "selected user"] user: Option<poise::serenity_prelude::User>,
) -> Result<(), Error> {
    let u = user.as_ref().unwrap_or_else(|| ctx.author());
    let response = format!("{}'s account was created at {}", u.name, u.created_at());
    ctx.say(&response).await?;
    Ok(())
}

/// dev rand
#[poise::command(slash_command)]
pub async fn rand(ctx: Context<'_>) -> Result<(), Error> {
    let random_number: i32 = {
        let mut rng = rand::thread_rng();
        rng.gen()
    };
    let response = format!("Your random number is {}", random_number);
    ctx.say(&response).await?;
    Ok(())
}

#[derive(Debug, poise::Modal)]
#[allow(dead_code)] // fields only used for Debug print
struct MyModal {
    first_input: String,
    second_input: Option<String>,
}

/// dev modal
#[poise::command(slash_command)]
pub async fn modal(ctx: poise::ApplicationContext<'_, Data, Error>) -> Result<(), Error> {
    use poise::Modal as _;

    let data = MyModal::execute(ctx).await?;
    println!("Got data: {:?}", data);

    Ok(())
}

/// Add two numbers
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn add(
    ctx: Context<'_>,
    #[description = "First operand"] a: f64,
    #[description = "Second operand"] b: f32,
) -> Result<(), Error> {
    ctx.say(format!("Result: {}", a + b as f64)).await?;

    Ok(())
}

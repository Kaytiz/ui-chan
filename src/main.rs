pub mod data;

#[cfg(feature = "rvc")]
pub mod rvc;

pub mod prelude;

mod command;
mod event_handler;

use std::sync::Arc;

use prelude::*;
use songbird::SerenityInit;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let temp_path = std::path::Path::new("temp");
    if temp_path.exists() {
        if let Ok(read_dir) = temp_path.read_dir() {
            for dir in read_dir.flatten() {
                let dir_path = dir.path();
                if dir_path.is_dir() {
                    std::fs::remove_dir_all(dir_path).ok();
                }
                else if dir_path.is_file() {
                    std::fs::remove_file(dir_path).ok();
                }
            }
        }
    }

    let token = std::env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("~!".into()),
                ..Default::default()
            },
            commands: command::create_commands(),
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler::event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|_ctx, _ready, _frameworkk| {
            Box::pin(async move {
                Ok(Data)
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::GUILD_PRESENCES
        | serenity::GatewayIntents::GUILD_MEMBERS
        | serenity::GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::Client::builder(&token, intents)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        
        data.insert::<data::SharedKey>(Arc::new(data::Shared {
            http_client: reqwest::Client::new(),
            spotify: {
                let creds = rspotify::Credentials::from_env().unwrap();
                rspotify::ClientCredsSpotify::new(creds)
            }
        }));

        data.insert::<data::StorageKey>(Arc::new(
            tokio::sync::Mutex::new(data::Storage::default()),
        ));
    }

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shard_manager.shutdown_all().await;
    });

    client
        .start()
        .await
        .unwrap_or_else(|err| panic!("Failed to start bot. {:?}", err));
}

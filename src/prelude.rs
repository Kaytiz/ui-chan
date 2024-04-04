pub use poise::serenity_prelude as serenity;

pub use crate::data::Data;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type ApplicationContext<'a> = poise::ApplicationContext<'a, Data, Error>;

pub const DEFAULT_DATA_FILENAME: &str = ".data";

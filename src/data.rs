use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    mem,
    sync::Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod song;

pub struct Data;

pub struct SharedKey;

impl serenity::prelude::TypeMapKey for SharedKey {
    type Value = Arc<Shared>;
}

pub struct Shared {
    pub http_client: reqwest::Client,
}

impl Shared {
    pub async fn get(ctx: &serenity::Context) -> Arc<Self> {
        ctx.data.read().await.get::<SharedKey>().unwrap().clone()
    }
}

pub struct StorageKey;

impl serenity::prelude::TypeMapKey for StorageKey {
    type Value = Arc<serenity::prelude::Mutex<Storage>>;
}

#[derive(Default)]
pub struct Storage {
    guilds: HashMap<serenity::GuildId, Arc<serenity::prelude::Mutex<Guild>>>,
}

impl Storage {
    pub async fn get(ctx: &serenity::Context) -> Arc<serenity::prelude::Mutex<Self>> {
        ctx.data.read().await.get::<StorageKey>().unwrap().clone()
    }

    pub async fn guild(
        ctx: &serenity::Context,
        guild_id: serenity::GuildId,
    ) -> Arc<serenity::prelude::Mutex<Guild>> {
        let storage = Self::get(ctx).await;
        let mut storage = storage.lock().await;

        match storage.guilds.entry(guild_id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => match Guild::load(guild_id).await {
                Ok(guild) => entry
                    .insert(Arc::new(serenity::prelude::Mutex::new(guild)))
                    .clone(),
                _ => Arc::new(serenity::prelude::Mutex::new(Guild::new(guild_id))),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Guild {
    #[serde(skip)]
    pub id: serenity::GuildId,

    pub channels: HashMap<serenity::ChannelId, Channel>,
    pub channel_notify: Option<serenity::ChannelId>,
    pub channel_song: Option<serenity::ChannelId>,
    pub users: HashMap<serenity::UserId, User>,

    #[serde(skip)]
    pub song_now: Option<song::Now>,

    #[serde(skip)]
    pub song_queue: VecDeque<song::Request>,
}

impl Guild {
    pub fn new(id: serenity::GuildId) -> Self {
        Self {
            id,
            channels: HashMap::new(),
            channel_notify: None,
            channel_song: None,
            users: HashMap::new(),
            song_now: None,
            song_queue: VecDeque::new(),
        }
    }

    const FILE_DIR: &'static str = "./data";

    fn file_name(guild_id: serenity::GuildId) -> String {
        guild_id.to_string()
    }

    pub async fn load(guild_id: serenity::GuildId) -> Result<Self, Error> {
        let file_dir = std::path::Path::new(Self::FILE_DIR);
        let file_name = Self::file_name(guild_id);
        let file_path = file_dir.join(&file_name);

        let mut file = tokio::fs::File::open(file_path).await?;
        let mut str = String::new();
        file.read_to_string(&mut str).await?;
        let mut data: Self = serde_json::from_str(&str)?;
        data.id = guild_id;
        Ok(data)
    }

    pub async fn save(&self) -> Result<(), Error> {
        let file_dir = std::path::Path::new(Self::FILE_DIR);
        let file_name = Self::file_name(self.id);
        let file_path = file_dir.join(&file_name);

        tokio::fs::create_dir_all(file_dir).await?;
        let mut file = tokio::fs::File::create(file_path).await?;
        let file_str = serde_json::to_string_pretty(&self)?;
        file.write_all(file_str.as_bytes()).await?;
        Ok(())
    }

    pub fn channel(&self, channel_id: serenity::ChannelId) -> Option<&Channel> {
        self.channels.get(&channel_id)
    }

    pub fn channel_mut(&mut self, channel_id: serenity::ChannelId) -> &mut Channel {
        self.channels.entry(channel_id).or_default()
    }

    pub fn user(&self, user_id: serenity::UserId) -> Option<&User> {
        self.users.get(&user_id)
    }

    pub fn user_mut(&mut self, user_id: serenity::UserId) -> &mut User {
        self.users.entry(user_id).or_default()
    }

    pub async fn song_now_complete(&mut self, ctx: &serenity::Context) -> Result<(), Error> {
        if let Some(now) = self.song_now.take() {
            now.request.react_done(ctx).await.ok();
        }
        Ok(())
    }

    pub async fn song_queue_clear(&mut self, ctx: &serenity::Context) -> Result<(), Error> {
        let song_queue = {
            // clear queue
            let mut song_queue: VecDeque<data::song::Request> = VecDeque::new();
            std::mem::swap(&mut song_queue, &mut self.song_queue);
            song_queue
        };

        for request in song_queue {
            request.remove_react_queue(ctx).await?;
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Channel {
    pub properties: Vec<channel::Property>,
}

impl Channel {
    pub fn has_property<F: Fn(&channel::Property) -> bool>(&self, filter: F) -> bool {
        self.properties.iter().any(filter)
    }

    pub fn get_property<F: Fn(&channel::Property) -> bool>(
        &self,
        filter: F,
    ) -> Option<&channel::Property> {
        self.properties.iter().find(|p| filter(p))
    }

    pub fn set_property(&mut self, property: channel::Property) {
        self.remove_property(|p| mem::discriminant(p) == mem::discriminant(&property));
        self.properties.push(property);
    }

    pub fn remove_property<F: Fn(&channel::Property) -> bool>(&mut self, filter: F) {
        self.properties.retain(|p| !filter(p))
    }
}

pub mod channel {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub enum Property {
        Attribute(String),
    }

    impl std::fmt::Display for Property {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Attribute(attr) => write!(f, "Attribute({attr})"),
            }
        }
    }

    impl Property {
        pub fn is_attribute(property: &Property) -> bool {
            matches!(property, Property::Attribute(_))
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct User {
    pub birthday: Option<chrono::NaiveDate>,
    pub phone_number: Option<String>,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut properties = Vec::new();
        if let Some(birthday) = self.birthday.as_ref() {
            properties.push(format!("birthday : {}", birthday));
        }
        if let Some(phone_number) = self.phone_number.as_ref() {
            properties.push(format!("phone_number : {}", phone_number));
        }
        write!(f, "{}", properties.join(", "))
    }
}

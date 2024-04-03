use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    mem,
    sync::{atomic::AtomicU32, Mutex},
};

pub struct Data {
    pub storage: Mutex<Storage>,
    pub http_client: reqwest::Client,
    pub poise_mentions: AtomicU32,
}

impl Data {
    pub fn load(filename: &str) -> Result<Self, Error> {
        Ok(Self {
            storage: Mutex::new(Storage::from_file(filename)?),
            ..Default::default()
        })
    }

    pub fn save(&self, filename: &str) -> Result<(), Error> {
        self.storage.lock().unwrap().to_file(filename)
    }

    pub fn load_default() -> Result<Self, Error> {
        Data::load(DEFAULT_DATA_FILENAME)
    }

    pub fn save_default(&self) -> Result<(), Error> {
        self.save(DEFAULT_DATA_FILENAME)
    }
}

impl Default for Data {
    fn default() -> Self {
        Data {
            storage: Mutex::new(Storage::default()),
            http_client: reqwest::Client::new(),
            poise_mentions: AtomicU32::new(0),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Storage {
    guilds: HashMap<serenity::GuildId, Guild>,
}

impl Storage {
    pub fn from_file(filename: &str) -> Result<Self, Error> {
        if std::path::Path::new(filename).exists() {
            let file = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(file);
            let data = serde_json::from_reader(reader)?;
            Ok(data)
        } else {
            Ok(Default::default())
        }
    }

    pub fn to_file(&self, filename: &str) -> Result<(), Error> {
        let file = std::fs::File::create(filename)?;
        let writer = std::io::BufWriter::new(file);
        let mut ser = serde_json::Serializer::pretty(writer);
        self.serialize(&mut ser)?;
        Ok(())
    }

    pub fn guild(&self, guild_id: serenity::GuildId) -> Option<&Guild> {
        self.guilds.get(&guild_id)
    }

    pub fn guild_mut(&mut self, guild_id: serenity::GuildId) -> &mut Guild {
        self.guilds.entry(guild_id).or_default()
    }

    pub fn channel(
        &self,
        guild_id: serenity::GuildId,
        channel_id: serenity::ChannelId,
    ) -> Option<&Channel> {
        self.guild(guild_id)
            .and_then(|guild| guild.channel(channel_id))
    }

    pub fn channel_mut(
        &mut self,
        guild_id: serenity::GuildId,
        channel_id: serenity::ChannelId,
    ) -> &mut Channel {
        self.guild_mut(guild_id).channel_mut(channel_id)
    }

    pub fn user(&self, guild_id: serenity::GuildId, user_id: serenity::UserId) -> Option<&User> {
        self.guild(guild_id).and_then(|guild| guild.user(user_id))
    }

    pub fn user_mut(
        &mut self,
        guild_id: serenity::GuildId,
        user_id: serenity::UserId,
    ) -> &mut User {
        self.guild_mut(guild_id).user_mut(user_id)
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Guild {
    pub channels: HashMap<serenity::ChannelId, Channel>,
    pub channel_notify: Option<serenity::ChannelId>,
    pub users: HashMap<serenity::UserId, User>,
}

impl Guild {
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
}

#[derive(Serialize, Deserialize, Default, Debug)]
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

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Property {
        Song,
        Attribute(String),
    }

    pub fn is_song(property: &Property) -> bool {
        matches!(property, Property::Song)
    }

    pub fn is_attribute(property: &Property) -> bool {
        matches!(property, Property::Attribute(_))
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct User {
    pub birthday: Option<chrono::NaiveDate>,
    pub phone_number: Option<String>,
}

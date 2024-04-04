use crate::prelude::*;

pub struct Request {
    pub url: String,
    pub guild_id: serenity::GuildId,
    pub author_id: serenity::UserId,
    pub channel_id: serenity::ChannelId,
    pub message_id: serenity::MessageId,
}

impl Request {
    const REACT_QUEUE: char = '🔖';
    const REACT_PLAYING: char = '🎵';
    const REACT_DONE: char = '✅';
    const REACTS_ALL: [char; 3] = [Self::REACT_QUEUE, Self::REACT_PLAYING, Self::REACT_DONE];

    pub fn new(
        url: impl Into<String>,
        guild_id: serenity::GuildId,
        author_id: serenity::UserId,
        channel_id: serenity::ChannelId,
        message_id: serenity::MessageId,
    ) -> Self {
        Self {
            url: url.into(),
            guild_id,
            author_id,
            channel_id,
            message_id,
        }
    }

    pub async fn messge(&self, ctx: &serenity::Context) -> serenity::Result<serenity::Message> {
        ctx.http.get_message(self.channel_id, self.message_id).await
    }

    pub async fn react_queue(&self, ctx: &serenity::Context) -> Result<(), Error> {
        self.messge(ctx)
            .await?
            .react(ctx, Self::REACT_QUEUE)
            .await?;
        Ok(())
    }

    pub async fn remove_react_queue(&self, ctx: &serenity::Context) -> Result<(), Error> {
        self.messge(ctx)
            .await?
            .delete_reaction(ctx, None, Self::REACT_QUEUE)
            .await?;
        Ok(())
    }

    pub async fn react_playing(&self, ctx: &serenity::Context) -> Result<(), Error> {
        let message = self.messge(ctx).await?;
        message
            .delete_reaction(ctx, None, Self::REACT_QUEUE)
            .await?;
        message.react(ctx, Self::REACT_PLAYING).await?;
        Ok(())
    }

    pub async fn react_done(&self, ctx: &serenity::Context) -> Result<(), Error> {
        let message = self.messge(ctx).await?;
        message
            .delete_reaction(ctx, None, Self::REACT_PLAYING)
            .await?;
        message.react(ctx, Self::REACT_DONE).await?;
        Ok(())
    }

    pub async fn clear_react(&self, ctx: &serenity::Context) -> Result<(), Error> {
        for react_type in Self::REACTS_ALL {
            self.messge(ctx)
                .await?
                .delete_reaction(ctx, None, react_type)
                .await?;
        }
        Ok(())
    }
}

impl From<&serenity::Message> for Request {
    fn from(value: &serenity::Message) -> Self {
        Self::new(
            value.content.clone(),
            value.guild_id.expect("Except message in guild"),
            value.author.id,
            value.channel_id,
            value.id,
        )
    }
}

pub struct Now {
    pub track: songbird::tracks::TrackHandle,
    pub request: Request,
}

impl Now {
    pub fn new(track: songbird::tracks::TrackHandle, request: Request) -> Self {
        Self { track, request }
    }
}

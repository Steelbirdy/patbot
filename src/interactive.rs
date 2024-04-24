use crate::prelude::{serenity::ComponentInteraction, *};
use poise::futures_util::StreamExt;
use std::time::Duration;

pub trait Interactive: Sized {
    async fn run(
        &mut self,
        ctx: ApplicationContext<'_>,
        cfg: Config,
        initial_message: InteractiveMessage,
    ) -> Result<serenity::MessageId> {
        initial_message.run(ctx, cfg, self).await
    }

    async fn process(
        &mut self,
        ctx: ApplicationContext<'_>,
        interaction: &ComponentInteraction,
    ) -> Result<ControlFlow>;

    fn update(&mut self, ctx: ApplicationContext<'_>, message: &mut InteractiveMessage);

    async fn finish(
        &mut self,
        ctx: ApplicationContext<'_>,
        message: &mut InteractiveMessage,
    ) -> Result<()>;
}

impl<T: Interactive> Interactive for &mut T {
    async fn run(
        &mut self,
        ctx: ApplicationContext<'_>,
        cfg: Config,
        initial_message: InteractiveMessage,
    ) -> Result<serenity::MessageId> {
        initial_message.run::<T>(ctx, cfg, self).await
    }

    async fn process(
        &mut self,
        ctx: ApplicationContext<'_>,
        interaction: &ComponentInteraction,
    ) -> Result<ControlFlow> {
        T::process(self, ctx, interaction).await
    }

    fn update(&mut self, ctx: ApplicationContext<'_>, message: &mut InteractiveMessage) {
        T::update(self, ctx, message);
    }

    async fn finish(
        &mut self,
        ctx: ApplicationContext<'_>,
        message: &mut InteractiveMessage,
    ) -> Result<()> {
        T::finish(self, ctx, message).await
    }
}

#[derive(Default)]
pub struct InteractiveMessage {
    pub content: Option<String>,
    pub embed: Option<serenity::CreateEmbed>,
    pub action_rows: Option<Vec<serenity::CreateActionRow>>,
}

#[derive(Copy, Clone)]
pub struct Config {
    pub duration: Duration,
}

pub enum ControlFlow {
    Continue { update: bool },
    Break,
}

#[allow(unused)]
impl InteractiveMessage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn embed(mut self, embed: serenity::CreateEmbed) -> Self {
        self.embed = Some(embed);
        self
    }

    pub fn action_rows(
        mut self,
        action_rows: impl IntoIterator<Item = serenity::CreateActionRow>,
    ) -> Self {
        self.action_rows = Some(action_rows.into_iter().collect());
        self
    }

    pub fn modify_content(&mut self, f: impl FnOnce(&mut String)) -> &mut Self {
        if let Some(content) = &mut self.content {
            f(content);
        }
        self
    }

    pub fn modify_embed(
        &mut self,
        f: impl FnOnce(serenity::CreateEmbed) -> serenity::CreateEmbed,
    ) -> &mut Self {
        if let Some(embed) = self.embed.take() {
            self.embed = Some(f(embed));
        }
        self
    }

    pub fn modify_action_rows(
        &mut self,
        f: impl FnOnce(Vec<serenity::CreateActionRow>) -> Vec<serenity::CreateActionRow>,
    ) -> &mut Self {
        if let Some(action_rows) = self.action_rows.take() {
            self.action_rows = Some(f(action_rows));
        }
        self
    }

    pub async fn run<T>(
        mut self,
        ctx: ApplicationContext<'_>,
        cfg: Config,
        interactive: &mut T,
    ) -> Result<serenity::MessageId>
    where
        T: Interactive,
    {
        let Config { duration } = cfg;

        let message = {
            let response = self.create_message();
            ctx.send(response).await?
        };

        let message_ref = message.message().await?;
        let mut interaction_stream = message_ref
            .as_ref()
            .await_component_interactions(ctx)
            .timeout(duration)
            .stream();

        while let Some(interaction) = interaction_stream.next().await {
            match interactive.process(ctx, &interaction).await? {
                ControlFlow::Continue { update: false } => {}
                ControlFlow::Continue { update: true } => {
                    interactive.update(ctx, &mut self);
                    let builder = self.response_update_message();
                    interaction
                        .create_response(
                            ctx,
                            serenity::CreateInteractionResponse::UpdateMessage(builder),
                        )
                        .await?;
                }
                ControlFlow::Break => {
                    interaction
                        .create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                        .await?;
                    break;
                }
            }
        }

        interactive.finish(ctx, &mut self).await?;
        let builder = self.edit_message();
        message.edit(ctx.into(), builder).await?;

        Ok(message_ref.as_ref().id)
    }

    fn create_message(&self) -> poise::CreateReply {
        let mut builder = poise::CreateReply::default();
        if let Some(content) = self.content.clone() {
            builder = builder.content(content);
        }
        if let Some(embed) = self.embed.clone() {
            builder = builder.embed(embed);
        }
        if let Some(action_rows) = self.action_rows.clone() {
            builder = builder.components(action_rows);
        }
        builder
    }

    fn edit_message(&self) -> poise::CreateReply {
        let mut builder = poise::CreateReply::default()
            .content(self.content.clone().unwrap_or_default())
            .components(self.action_rows.clone().unwrap_or_default());
        if let Some(embed) = self.embed.clone() {
            builder = builder.embed(embed);
        }
        builder
    }

    fn response_update_message(&self) -> serenity::CreateInteractionResponseMessage {
        let builder = serenity::CreateInteractionResponseMessage::default()
            .content(self.content.clone().unwrap_or_default())
            .components(self.action_rows.clone().unwrap_or_default());
        match self.embed.clone() {
            Some(embed) => builder.embed(embed),
            None => builder.embeds(Vec::new()),
        }
    }
}

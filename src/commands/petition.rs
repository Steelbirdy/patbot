use crate::{
    commands::poll,
    interactive::{Config as InteractiveConfig, ControlFlow, Interactive, InteractiveMessage},
    prelude::*,
};
use serenity::Mentionable;
use std::collections::HashMap;

/// Create a Frodge petition
#[poise::command(slash_command, guild_only)]
pub async fn petition(
    ctx: ApplicationContext<'_>,
    #[description = "The title of the petition"] title: String,
) -> Result {
    const PETITION_DURATION: std::time::Duration = std::time::Duration::from_secs(60 * 60 * 24);

    let guild = PatbotGuild::get(ctx).unwrap();
    if ctx.channel_id() != guild.congress_text_channel_id {
        reply_error!(
            ctx,
            "This command can only be used in {}.",
            guild.congress_text_channel_id.mention()
        );
    }

    if title.len() > 245 {
        reply_error!(
            ctx,
            "The petition title is too long. It cannot be longer than 245 characters."
        );
    }

    let author = poll::embed_author(ctx).await;
    let footer = poll::embed_footer(PETITION_DURATION);

    let embed_template = serenity::CreateEmbed::default()
        .color(serenity::Color::DARK_RED)
        .author(author)
        .footer(footer);

    let mut petition = Petition {
        title: title.clone(),
        embed_template: embed_template.clone(),
        vote_totals: [0; 3],
        votes: HashMap::default(),
        frodge_membership_count: crate::frodge_membership_count() as _,
    };

    let embed = embed_template
        .title(title)
        .fields(petition.build_embed_fields(false));

    let action_rows = petition_action_rows(ctx);

    let config = InteractiveConfig {
        duration: PETITION_DURATION,
    };

    let message = InteractiveMessage::default()
        .embed(embed)
        .action_rows(action_rows);

    let petition_message_id = petition.run(ctx, config, message).await?;

    if petition.passed() {
        guild
            .congress_text_channel_id
            .pin(ctx, petition_message_id)
            .await?;
    }

    let title = &petition.title;
    let content = if petition.passed() {
        format!("⚖️ The petition **{title}** passed!")
    } else if petition.total_votes() < petition.total_votes_needed() {
        format!("⚖️ The petition **{title}** failed because not enough people voted.")
    } else {
        format!("⚖️ The petition **{title}** failed because it did not receive enough support.")
    };

    guild
        .congress_text_channel_id
        .send_message(
            ctx,
            serenity::CreateMessage::default()
                .content(content)
                .reference_message((guild.congress_text_channel_id, petition_message_id)),
        )
        .await?;

    Ok(())
}

struct Petition {
    title: String,
    embed_template: serenity::CreateEmbed,
    vote_totals: [u32; 3],
    votes: HashMap<serenity::UserId, usize>,
    frodge_membership_count: u32,
}

impl Petition {
    fn total_votes_needed(&self) -> u32 {
        (self.frodge_membership_count as f64 * 0.75).ceil() as _
    }

    fn remaining_votes_needed(&self) -> u32 {
        self.total_votes_needed().saturating_sub(self.total_votes())
    }

    fn total_yays_needed(&self) -> u32 {
        (self.frodge_membership_count as f64 * 2. / 3.).ceil() as _
    }

    fn total_votes(&self) -> u32 {
        let [yays, nays, mehs] = self.vote_totals;
        yays + nays + mehs
    }

    fn passed(&self) -> bool {
        self.vote_totals[0] >= self.total_yays_needed()
    }

    fn build_embed_fields(&self, done: bool) -> impl IntoIterator<Item = (String, String, bool)> {
        let [yays, nays, mehs] = self.vote_totals;
        let remaining_votes = self.remaining_votes_needed();
        if done {
            vec![
                (String::from("Yay"), yays.to_string(), true),
                (String::from("Nay"), nays.to_string(), true),
                (String::from("Meh"), mehs.to_string(), true),
            ]
        } else {
            vec![
                (String::from("Yay"), yays.to_string(), true),
                (String::from("Nay"), nays.to_string(), true),
                (String::from("Meh"), mehs.to_string(), true),
                (
                    String::from("Votes Needed"),
                    remaining_votes.to_string(),
                    false,
                ),
            ]
        }
    }
}

impl Interactive for Petition {
    async fn process(
        &mut self,
        _ctx: ApplicationContext<'_>,
        interaction: &serenity::ComponentInteraction,
    ) -> Result<ControlFlow> {
        assert!(matches!(
            &interaction.data.kind,
            serenity::ComponentInteractionDataKind::Button
        ));
        let new_vote: usize = match interaction.data.custom_id.rsplit_once('_') {
            Some((_, "yay")) => 0,
            Some((_, "nay")) => 1,
            Some((_, "meh")) => 2,
            _ => unreachable!(),
        };

        self.vote_totals[new_vote] += 1;
        if let Some(prev_vote) = self.votes.get_mut(&interaction.user.id) {
            self.vote_totals[*prev_vote] -= 1;
            *prev_vote = new_vote;
        } else {
            self.votes.insert(interaction.user.id, new_vote);
        }

        Ok(if self.remaining_votes_needed() == 0 {
            ControlFlow::Break
        } else {
            ControlFlow::Continue { update: true }
        })
    }

    fn update(&mut self, _ctx: ApplicationContext<'_>, message: &mut InteractiveMessage) {
        let embed = self
            .embed_template
            .clone()
            .title(self.title.clone())
            .fields(self.build_embed_fields(false));
        message.modify_embed(|_| embed);
    }

    async fn finish(
        &mut self,
        _ctx: ApplicationContext<'_>,
        message: &mut InteractiveMessage,
    ) -> Result<()> {
        let passed = self.passed();
        let title_prefix = if passed { "Passed" } else { "Failed" };
        let title = format!("[{title_prefix}] {}", self.title);

        let embed = self
            .embed_template
            .clone()
            .title(title)
            .fields(self.build_embed_fields(true));
        message
            .modify_embed(|_| embed)
            .modify_action_rows(|_| Vec::new());

        Ok(())
    }
}

fn petition_action_rows(ctx: ApplicationContext<'_>) -> Vec<serenity::CreateActionRow> {
    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(custom_id(ctx, "yay"))
            .emoji("✔️".parse::<serenity::ReactionType>().unwrap())
            .label("Yay")
            .style(serenity::ButtonStyle::Success),
        serenity::CreateButton::new(custom_id(ctx, "nay"))
            .emoji("✖️".parse::<serenity::ReactionType>().unwrap())
            .label("Nay")
            .style(serenity::ButtonStyle::Danger),
        serenity::CreateButton::new(custom_id(ctx, "meh"))
            .emoji("❔".parse::<serenity::ReactionType>().unwrap())
            .label("Meh")
            .style(serenity::ButtonStyle::Secondary),
    ])]
}

fn custom_id(ctx: ApplicationContext<'_>, name: &str) -> String {
    let id = ctx.id();
    format!("{id}_petition_{name}")
}

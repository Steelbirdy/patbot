use crate::{data::PollMode, serenity, ApplicationContext, Result};
use parse_display::FromStr;
use poise::futures_util::StreamExt;
use std::collections::HashMap;

const CHOICE_EMOJIS: &[&str] = &["1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣", "6️⃣", "7️⃣", "8️⃣", "9️⃣", "🔟"];
const MAX_CHOICES: usize = CHOICE_EMOJIS.len();

#[poise::command(slash_command, owners_only)]
pub async fn set_poll_mode(ctx: ApplicationContext<'_>, mode: PollMode) -> Result {
    ctx.data().set_poll_mode(mode);
    ctx.send(
        poise::CreateReply::default()
            .content(":white_check_mark: Successfully changed the poll mode")
            .reply(true)
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command, guild_only)]
pub async fn poll(ctx: ApplicationContext<'_>) -> Result {
    // This command cannot be used in DMs
    if ctx.guild_id().is_none() {
        let _ = ctx.reply("This command cannot be used in a DM").await;
        return Ok(());
    }

    macro_rules! send_error {
        ($error_message:literal) => {{
            let _ = ctx
                .send(
                    poise::CreateReply::default()
                        .content(concat!(":x: ", $error_message))
                        .reply(true)
                        .ephemeral(true),
                )
                .await;
            return Ok(());
        }};
    }

    macro_rules! respond_ephemeral {
        ($ctx:ident, $interaction:ident, $content:expr) => {
            $interaction
                .create_response(
                    $ctx,
                    serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content($content)
                            .ephemeral(true),
                    ),
                )
                .await?;
        };
    }

    // Create the poll modal
    let Some(PollModal {
        title,
        choices,
        duration,
        privacy,
    }) = poise::Modal::execute(ctx).await?
    else {
        // If the user presses "Cancel", we don't follow up
        return Ok(());
    };

    let is_public = !privacy.is_some_and(|s| !s.is_empty());

    // Attempt to parse the poll duration entered by the user
    // TODO: Better error message
    let duration = duration.unwrap_or_else(|| String::from("24 hours"));
    let Ok(duration @ PollDuration { .. }) = duration.parse() else {
        send_error!("I didn't understand the duration that you entered.")
    };

    // If the user entered something like "0 minutes"
    if duration.amount == 0 {
        send_error!("The duration must be greater than 0.");
    }
    // If the user entered a duration over the maximum
    if !duration.check() {
        send_error!("The duration must be no longer than one week.");
    }

    // If the user entered more choices than we allow
    let choices: Vec<_> = choices.lines().collect();
    if choices.len() > MAX_CHOICES {
        send_error!("Polls can have at most 10 choices.");
    }

    // Build the poll message
    let mut embed = format_poll_embed(ctx, &title, &choices, duration, is_public).await;
    let action_rows = format_poll_action_rows(ctx, &choices);
    let message_builder = serenity::CreateMessage::default()
        .embed(embed.clone().into())
        .components(action_rows);
    let mut message = ctx.channel_id().send_message(ctx, message_builder).await?;

    let mut votes: HashMap<serenity::UserId, Vec<usize>> = HashMap::new();
    let mut vote_totals = vec![0_u32; choices.len()];

    macro_rules! update_vote_totals_if_public {
        () => {
            if is_public {
                let mut embed = embed.clone();
                update_vote_totals(&mut embed, &vote_totals);
                message
                    .edit(ctx, serenity::EditMessage::new().embed(embed.into()))
                    .await?;
            }
        };
    }

    update_vote_totals_if_public!();

    // Get a stream of interactions with the components attached to the poll
    let mut interaction_stream = message
        .await_component_interaction(ctx)
        .timeout(duration.into())
        .stream();

    while let Some(interaction) = interaction_stream.next().await {
        let new_votes = match &interaction.data.kind {
            serenity::ComponentInteractionDataKind::Button => {
                let custom_id = &interaction.data.custom_id;
                // The cancel button
                if custom_id.ends_with("_cancel") {
                    // Only allow the creator of the poll to cancel it
                    if interaction.user.id == ctx.author().id {
                        interaction
                            .create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                            .await?;
                        break;
                    } else {
                        respond_ephemeral!(
                            ctx,
                            interaction,
                            ":x: Only the person who started the poll can do this."
                        );
                        continue;
                    }
                } else {
                    // Indicates that a vote was cast using the buttons
                    interaction
                        .create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                        .await?;
                    let (_, option_number) = custom_id.rsplit_once('_').unwrap();
                    vec![option_number.parse::<usize>().unwrap()]
                }
            }
            serenity::ComponentInteractionDataKind::StringSelect { values } => {
                // Indicates that a vote was cast using the selection menu
                interaction
                    .create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                    .await?;
                values.iter().map(|s| s.parse().unwrap()).collect()
            }
            _ => unreachable!(),
        };

        // Update the vote totals
        let voter_id = interaction.user.id;
        let previous_votes = votes.entry(voter_id).or_default();
        for &vote in previous_votes.iter() {
            vote_totals[vote] -= 1;
        }
        for &vote in &new_votes {
            vote_totals[vote] += 1;
        }
        *previous_votes = new_votes;

        update_vote_totals_if_public!();
    }

    embed.title = format!("[CLOSED] {}", embed.title);
    update_vote_totals(&mut embed, &vote_totals);
    message
        .edit(
            ctx,
            serenity::EditMessage::new()
                .embed(embed.into())
                .components(Vec::new()),
        )
        .await?;

    Ok(())
}

fn update_vote_totals(embed: &mut PollEmbed, vote_totals: &[u32]) {
    for ((_, prev_votes, _), &total_votes) in embed.fields.iter_mut().zip(vote_totals) {
        *prev_votes = format!("{total_votes}");
    }
}

async fn format_poll_embed(
    ctx: ApplicationContext<'_>,
    title: &str,
    choices: &[&str],
    duration: PollDuration,
    is_public: bool,
) -> PollEmbed {
    // Format the author header as "Started by <USER>"
    let guild_id = ctx.guild_id().unwrap();
    let author = ctx.author();
    // Attempt to use the user's real name. If that fails, fall back on their nickname, otherwise just use their username
    let author_name = match crate::get_frodge_member(author.id) {
        Some(name) => format!("Started by {name}"),
        None => author
            .nick_in(ctx, guild_id)
            .await
            .unwrap_or_else(|| author.name.clone()),
    };
    // Attempt to use the user's avatar, otherwise fall back to a default Discord avatar
    let author_avatar_url = author
        .avatar_url()
        .unwrap_or_else(|| author.default_avatar_url());
    let author = serenity::CreateEmbedAuthor::new(author_name).icon_url(author_avatar_url);

    // Use emojis to assign each choice a number from 1 to 10
    let default_vote_count = if is_public {
        "0".to_string()
    } else {
        String::new()
    };
    let fields = choices
        .iter()
        .enumerate()
        .map(|(i, choice)| {
            (
                format!("{}  {choice}", CHOICE_EMOJIS[i]),
                default_vote_count.clone(),
                false,
            )
        })
        .collect();

    // Format the poll expiration time
    let time_zone = chrono::FixedOffset::west_opt(4 * 60 * 60).unwrap();
    let start_time = chrono::Utc::now().with_timezone(&time_zone);
    let expiration_time = start_time.checked_add_signed(duration.into()).unwrap();
    let formatted_expiration_time = expiration_time.format("%-I:%M %p EST on %e %B %Y");
    // Format the footer as "Closes at <EXPIRATION_TIME>"
    let footer = serenity::CreateEmbedFooter::new(format!("Closes at {formatted_expiration_time}"));

    PollEmbed {
        colour: ctx.data.bot_color(),
        title: format!("**{title}**"),
        author,
        fields,
        footer,
    }
}

fn format_poll_action_rows(
    ctx: ApplicationContext<'_>,
    choices: &[&str],
) -> Vec<serenity::CreateActionRow> {
    const MAX_BUTTONS_PER_ROW: usize = 5;

    fn take_button(button: &mut serenity::CreateButton) -> serenity::CreateButton {
        std::mem::replace(button, serenity::CreateButton::new(""))
    }

    let id = ctx.id();

    let mut rows: Vec<_> = match ctx.data().poll_mode() {
        PollMode::Buttons => {
            // Create all the choice buttons in a list
            let mut buttons: Vec<_> = choices
                .iter()
                .enumerate()
                .map(|(i, _)| format_poll_choice_button(id, i))
                .collect();

            // Split the choice buttons into rows
            buttons
                .chunks_mut(MAX_BUTTONS_PER_ROW)
                .map(|bt| {
                    let buttons = bt.iter_mut().map(take_button).collect();
                    serenity::CreateActionRow::Buttons(buttons)
                })
                .collect()
        }
        PollMode::Menu => {
            let select_menu = format_poll_select_menu(ctx, choices);
            vec![serenity::CreateActionRow::SelectMenu(select_menu)]
        }
    };

    // Add the cancel button in its own row
    let cancel_poll_button =
        serenity::CreateButton::new(format_poll_button_custom_id(id, "cancel", None))
            .emoji("✖️".parse::<serenity::ReactionType>().unwrap())
            .label("Cancel poll")
            .style(serenity::ButtonStyle::Danger);
    rows.push(serenity::CreateActionRow::Buttons(vec![cancel_poll_button]));

    rows
}

fn format_poll_select_menu(
    ctx: ApplicationContext<'_>,
    choices: &[&str],
) -> serenity::CreateSelectMenu {
    const LABEL_MAX_LEN: usize = 100;

    fn truncate_text(s: &str, max_len: usize) -> String {
        if s.len() > max_len {
            format!("{}...", &s[..max_len - 3])
        } else {
            s.to_string()
        }
    }

    let options: Vec<_> = choices
        .iter()
        .enumerate()
        .map(|(i, choice)| {
            serenity::CreateSelectMenuOption::new(
                truncate_text(choice, LABEL_MAX_LEN),
                i.to_string(),
            )
            .emoji(CHOICE_EMOJIS[i].parse::<serenity::ReactionType>().unwrap())
        })
        .collect();
    serenity::CreateSelectMenu::new(
        format!("{}_poll", ctx.id()),
        serenity::CreateSelectMenuKind::String { options },
    )
    .placeholder("Place your vote")
    .min_values(0)
    .max_values(1)
}

fn format_poll_choice_button(ctx_id: u64, index: usize) -> serenity::CreateButton {
    let custom_id = format_poll_button_custom_id(ctx_id, "choice", Some(index));
    let emoji = CHOICE_EMOJIS[index];
    serenity::CreateButton::new(custom_id)
        .emoji(emoji.parse::<serenity::ReactionType>().unwrap())
        .style(serenity::ButtonStyle::Primary)
}

fn format_poll_button_custom_id(ctx_id: u64, kind: &str, index: Option<usize>) -> String {
    match index {
        Some(i) => format!("{ctx_id}_poll_{kind}_{i}"),
        None => format!("{ctx_id}_poll_{kind}"),
    }
}

#[derive(Clone)]
struct PollEmbed {
    title: String,
    colour: serenity::Colour,
    author: serenity::CreateEmbedAuthor,
    fields: Vec<(String, String, bool)>,
    footer: serenity::CreateEmbedFooter,
}

impl From<PollEmbed> for serenity::CreateEmbed {
    fn from(value: PollEmbed) -> Self {
        let PollEmbed {
            title,
            colour,
            author,
            fields,
            footer,
        } = value;
        serenity::CreateEmbed::new()
            .title(title)
            .colour(colour)
            .author(author)
            .fields(fields)
            .footer(footer)
    }
}

#[derive(poise::Modal, Debug)]
struct PollModal {
    #[name = "Title"]
    #[placeholder = "Enter poll title here"]
    #[max_length = 250]
    title: String,
    #[name = "Options"]
    #[placeholder = "Enter poll options here, one on each line (maximum of 10)"]
    #[paragraph]
    choices: String,
    #[name = "Duration"]
    #[placeholder = r#""30 minutes", "6 hours", "1 day", etc. Defaults to 24 hours"#]
    duration: Option<String>,
    #[name = "Private"]
    #[placeholder = "Leave this empty to make the voting public"]
    #[max_length = 1]
    privacy: Option<String>,
}

#[derive(FromStr, Copy, Clone)]
#[display("{amount} {unit}")]
struct PollDuration {
    unit: PollDurationUnit,
    amount: u32,
}

impl PollDuration {
    fn to_minutes(self) -> u64 {
        let PollDuration { amount, unit } = self;
        let conversion = match unit {
            PollDurationUnit::Minute => 1,
            PollDurationUnit::Hour => 60,
            PollDurationUnit::Day => 60 * 24,
        };
        u64::from(amount) * conversion
    }

    fn check(self) -> bool {
        const MAX_MINUTES: u64 = 60 * 24 * 7;
        self.to_minutes() <= MAX_MINUTES
    }
}

impl std::fmt::Display for PollDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let amount = self.amount;
        let unit = self.unit.as_str_singular();
        if amount == 1 {
            write!(f, "1 {unit}")
        } else {
            write!(f, "{amount} {unit}s")
        }
    }
}

impl From<PollDuration> for chrono::Duration {
    fn from(value: PollDuration) -> Self {
        let PollDuration { amount, unit } = value;
        match unit {
            PollDurationUnit::Minute => chrono::Duration::minutes(amount.into()),
            PollDurationUnit::Hour => chrono::Duration::hours(amount.into()),
            PollDurationUnit::Day => chrono::Duration::days(amount.into()),
        }
    }
}

impl From<PollDuration> for std::time::Duration {
    fn from(value: PollDuration) -> Self {
        let minutes = value.to_minutes();
        std::time::Duration::from_secs(minutes * 60)
    }
}

#[derive(FromStr, Copy, Clone, Eq, PartialEq, Hash)]
enum PollDurationUnit {
    #[from_str(regex = "minutes?")]
    Minute,
    #[from_str(regex = "hours?")]
    Hour,
    #[from_str(regex = "days?")]
    Day,
}

impl PollDurationUnit {
    fn as_str_singular(self) -> &'static str {
        match self {
            Self::Minute => "minute",
            Self::Hour => "hour",
            Self::Day => "day",
        }
    }
}

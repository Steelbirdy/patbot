use crate::{data::PollMode, serenity, ApplicationContext, Result};
use parse_display::FromStr;

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

    // Create the poll modal
    let Some(PollModal {
        title,
        choices,
        duration,
    }) = poise::Modal::execute(ctx).await?
    else {
        // If the user presses "Cancel", we don't follow up
        return Ok(());
    };

    // Attempt to parse the poll duration entered by the user
    // TODO: Better error message
    let duration = duration.unwrap_or_else(|| String::from("24 hours"));
    let Ok(duration @ PollDuration { .. }) = duration.parse() else {
        send_error!("I didn't understand the duration that you entered.")
    };

    // If the user entered something like "0 minutes", we can't make the poll
    if duration.amount == 0 {
        send_error!("The duration must be greater than 0.");
    }

    // If the user entered more choices than we allow, we can't make the poll
    let choices: Vec<_> = choices.lines().collect();
    if choices.len() > MAX_CHOICES {
        send_error!("Polls can have at most 10 choices.");
    }

    let _ = ctx
        .send(
            poise::CreateReply::default()
                .content(":white_check_mark: Creating poll...")
                .reply(true)
                .ephemeral(true),
        )
        .await;

    // Build the poll message
    let embed = format_poll_embed(ctx, &title, &choices, duration).await;
    let action_rows = format_poll_action_rows(ctx, &choices);
    let message_builder = serenity::CreateMessage::default()
        .embed(embed)
        .components(action_rows);
    let _message = ctx.channel_id().send_message(ctx, message_builder).await?;

    // let mut votes = vec![0_u32; choices.len()];
    // let custom_id_prefix = ctx.id().to_string();
    // let collector = serenity::ComponentInteractionCollector::new(ctx)
    //     .author_id(ctx.author().id)
    //     .channel_id(ctx.channel_id())
    //     .timeout(duration.into())
    //     .filter(move |mci| mci.data.custom_id.starts_with(&custom_id_prefix))
    //     .

    Ok(())
}

async fn format_poll_embed(
    ctx: ApplicationContext<'_>,
    title: &str,
    choices: &[&str],
    duration: PollDuration,
) -> serenity::CreateEmbed {
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
    let fields = choices
        .iter()
        .enumerate()
        .map(|(i, choice)| (format!("{}  {choice}", CHOICE_EMOJIS[i]), "", false));

    // Format the poll expiration time
    let time_zone = chrono::FixedOffset::west_opt(4 * 60 * 60).unwrap();
    let start_time = chrono::Utc::now().with_timezone(&time_zone);
    let expiration_time = start_time.checked_add_signed(duration.into()).unwrap();
    let formatted_expiration_time = expiration_time.format("%-I:%M %p EST on %e %B %Y");
    // Format the footer as "Closes at <EXPIRATION_TIME>"
    let footer = serenity::CreateEmbedFooter::new(format!("Closes at {formatted_expiration_time}"));

    serenity::CreateEmbed::default()
        .color(ctx.data.bot_color())
        .title(format!("**{title}**"))
        .author(author)
        .fields(fields)
        .footer(footer)
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
}

#[derive(FromStr, Copy, Clone)]
#[display("{amount} {unit}")]
struct PollDuration {
    unit: PollDurationUnit,
    amount: u32,
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
        let PollDuration { amount, unit } = value;
        let conversion = match unit {
            PollDurationUnit::Minute => 60,
            PollDurationUnit::Hour => 60 * 60,
            PollDurationUnit::Day => 60 * 60 * 24,
        };
        std::time::Duration::from_secs(u64::from(amount) * conversion)
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

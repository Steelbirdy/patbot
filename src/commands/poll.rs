use crate::{serenity, ApplicationContext, Result};
use parse_display::FromStr;

const OPTION_EMOJIS: &[&str] = &[
    ":one:", ":two:", ":three:", ":four:", ":five:", ":six:", ":seven:", ":eight:", ":nine:",
    ":ten:",
];

#[poise::command(slash_command)]
pub async fn poll(ctx: ApplicationContext<'_>) -> Result {
    // This command cannot be used in DMs
    if ctx.guild_id().is_none() {
        let _ = ctx.reply(":x: This command cannot be used in a DM").await;
        return Ok(());
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
    let Ok(duration @ PollDuration { .. }) = duration.parse() else {
        let _ = ctx
            .send(
                poise::CreateReply::default()
                    .content(":x: I didn't understand the duration that you entered.")
                    .reply(true)
                    .ephemeral(true),
            )
            .await;
        return Ok(());
    };

    // If the user entered something like "0 minutes", we can't make the poll
    if duration.amount == 0 {
        let _ = ctx
            .send(
                poise::CreateReply::default()
                    .content(":x: The duration must be greater than 0.")
                    .reply(true)
                    .ephemeral(true),
            )
            .await;
        return Ok(());
    }

    let choices: Vec<_> = choices.lines().collect();
    let embed = format_poll_embed(ctx, &title, &choices, duration).await;

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
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
        .map(|(i, choice)| (format!("{} {choice}", OPTION_EMOJIS[i]), "", false));

    // Eastern Standard Time
    let time_zone = chrono::FixedOffset::west_opt(4 * 60 * 60).unwrap();
    // Get the current time in EST
    let start_time = chrono::Utc::now().with_timezone(&time_zone);
    // Add the poll duration to get the time at which the poll expires in EST
    let expiration_time = start_time.checked_add_signed(duration.into()).unwrap();
    // Format the expiration time like ex. 3:14 PM EST on 15 September 2024
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

#[derive(poise::Modal, Debug)]
struct PollModal {
    #[name = "Title"]
    #[placeholder = "Enter poll title here."]
    #[max_length = 250]
    title: String,
    #[name = "Options"]
    #[placeholder = "Enter poll options here, one on each line (maximum of 10)."]
    #[paragraph]
    choices: String,
    #[name = "Duration"]
    #[placeholder = r#""30 minutes", "6 hours", "1 day", etc. Defaults to 24 hours."#]
    duration: String,
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

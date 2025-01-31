use crate::{
    interactive::{Config as InteractiveConfig, ControlFlow, Interactive, InteractiveMessage},
    prelude::*,
};
use parse_display::FromStr;
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

/// Brings up a form for poll creation
#[poise::command(slash_command, guild_only)]
pub async fn poll(ctx: ApplicationContext<'_>) -> Result {
    // Create the poll modal
    let Some(PollModal {
        title,
        choices,
        duration,
        privacy,
        num_choices,
    }) = poise::Modal::execute(ctx).await?
    else {
        // If the user presses "Close", we don't follow up
        return Ok(());
    };

    // If the user entered more choices than we allow
    let choices: Vec<_> = choices.lines().collect();
    if choices.len() > MAX_CHOICES {
        reply_error!(ctx, "Polls can have at most {} choices.", MAX_CHOICES);
    }

    // Attempt to parse the poll duration entered by the user
    let duration = duration.unwrap_or_else(|| String::from("24 hours"));
    let Ok(duration @ PollDuration { .. }) = duration.parse() else {
        reply_error!(ctx, "I didn't understand the duration that you entered. Enter something like `# [minutes|hours|days]`, where each `#` is a number.");
    };

    // If the user entered something like "0 minutes"
    if duration.amount == 0 {
        reply_error!(ctx, "The duration must be greater than 0.");
    }
    // If the user entered a duration over the maximum
    if duration.to_minutes() > PollDuration::MAX_MINUTES {
        reply_error!(ctx, "The duration cannot be longer than one week.");
    }

    let public_voting = privacy.is_none_or(|s| s.is_empty());

    // Valid formats are "#" or "#-#"
    let max_allowed_choices = num_choices.map_or(Ok(1_u8), |s| s.parse());
    let max_allowed_choices = match max_allowed_choices {
        Err(_) => {
            reply_error!(ctx, "I didn't understand the number of choices that you entered. Enter something like `#` or `#-#`, where each `#` is a number.");
        }
        Ok(n) if usize::from(n) > choices.len() => {
            reply_error!(
                ctx,
                "Invalid number of choices: can't choose {} options from {} total.",
                n,
                choices.len()
            );
        }
        Ok(n) => n,
    };

    let mode = if max_allowed_choices == 1 {
        ctx.data().poll_mode()
    } else {
        PollMode::Menu
    };

    let options: Vec<_> = choices
        .iter()
        .enumerate()
        .map(|(i, choice)| {
            let emoji = CHOICE_EMOJIS[i];
            format!("{emoji}  {choice}")
        })
        .collect();

    let fields = options.iter().map(|name| {
        let value = if public_voting {
            String::from("0")
        } else {
            String::new()
        };
        (name.clone(), value, false)
    });

    let author = embed_author(ctx).await;
    let footer = embed_footer(duration.into());

    let embed_template = serenity::CreateEmbed::default()
        .color(ctx.data().bot_color())
        .author(author)
        .footer(footer);

    let embed = embed_template.clone().title(title.clone()).fields(fields);

    let action_rows = create_poll_action_rows(ctx, &choices, max_allowed_choices, mode);

    let mut poll = Poll {
        title,
        options,
        public_voting,
        embed_template,
        votes: HashMap::default(),
        vote_totals: vec![0; choices.len()],
    };

    let message = InteractiveMessage::default()
        .embed(embed)
        .action_rows(action_rows);

    let config = InteractiveConfig {
        duration: duration.into(),
    };

    message.run(ctx, config, &mut poll).await?;

    Ok(())
}

#[derive(poise::ChoiceParameter, Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum PollMode {
    #[default]
    #[name = "buttons"]
    Buttons,
    #[name = "menu"]
    Menu,
}

pub(in crate::commands) async fn embed_author(
    ctx: ApplicationContext<'_>,
) -> serenity::CreateEmbedAuthor {
    let author_name = crate::author_name(ctx.into()).await;
    let author = ctx.author();
    // Attempt to use the user's avatar, otherwise fall back to a default Discord avatar
    let author_avatar_url = author
        .avatar_url()
        .unwrap_or_else(|| author.default_avatar_url());
    serenity::CreateEmbedAuthor::new(format!("Started by {author_name}"))
        .icon_url(author_avatar_url)
}

pub(in crate::commands) fn embed_footer(
    duration: std::time::Duration,
) -> serenity::CreateEmbedFooter {
    // Format the poll expiration time
    let time_zone = chrono::FixedOffset::west_opt(4 * 60 * 60).unwrap();
    let start_time = chrono::Utc::now().with_timezone(&time_zone);
    let duration = chrono::Duration::from_std(duration).unwrap();
    let expiration_time = start_time.checked_add_signed(duration).unwrap();
    let formatted_expiration_time = expiration_time.format("%-I:%M %p EST on %e %B %Y");
    // Format the footer as "Closes at <EXPIRATION_TIME>"
    serenity::CreateEmbedFooter::new(format!("Closes at {formatted_expiration_time}"))
}

struct Poll {
    title: String,
    options: Vec<String>,
    public_voting: bool,
    embed_template: serenity::CreateEmbed,
    votes: HashMap<serenity::UserId, Vec<usize>>,
    vote_totals: Vec<u32>,
}

impl Interactive for Poll {
    async fn process(
        &mut self,
        ctx: ApplicationContext<'_>,
        interaction: &serenity::ComponentInteraction,
    ) -> Result<ControlFlow> {
        let custom_id = &interaction.data.custom_id;
        let user_id = interaction.user.id;
        let new_votes: Vec<usize> = match &interaction.data.kind {
            serenity::ComponentInteractionDataKind::Button if custom_id.ends_with("_close") => {
                return if user_id == ctx.author().id {
                    Ok(ControlFlow::Break)
                } else {
                    respond_to_interaction!(
                        ctx,
                        interaction,
                        ":x: Only the person who started the poll can do this."
                    )
                    .await?;
                    Ok(ControlFlow::Continue { update: false })
                };
            }
            serenity::ComponentInteractionDataKind::Button if custom_id.ends_with("_clear") => {
                Vec::new()
            }
            serenity::ComponentInteractionDataKind::Button => {
                let (_, option_number) = custom_id.rsplit_once('_').unwrap();
                let vote = option_number.parse().unwrap();
                vec![vote]
            }
            serenity::ComponentInteractionDataKind::StringSelect { values } => {
                values.iter().map(|s| s.parse().unwrap()).collect()
            }
            _ => unreachable!(),
        };

        let previous_votes = self.votes.entry(user_id).or_default();
        for &vote in &*previous_votes {
            self.vote_totals[vote] -= 1;
        }
        for &vote in &new_votes {
            self.vote_totals[vote] += 1;
        }
        *previous_votes = new_votes;

        Ok(ControlFlow::Continue {
            update: self.public_voting,
        })
    }

    fn update(&mut self, _ctx: ApplicationContext<'_>, message: &mut InteractiveMessage) {
        let fields = self
            .options
            .iter()
            .cloned()
            .zip(&self.vote_totals)
            .map(|(option, &votes)| (option, votes.to_string(), false));
        let embed = self
            .embed_template
            .clone()
            .title(self.title.clone())
            .fields(fields);
        message.modify_embed(|_| embed);
    }

    async fn finish(
        &mut self,
        _ctx: ApplicationContext<'_>,
        message: &mut InteractiveMessage,
    ) -> Result<()> {
        let fields = self
            .options
            .iter()
            .cloned()
            .zip(&self.vote_totals)
            .map(|(option, &votes)| (option, votes.to_string(), false));
        let embed = self
            .embed_template
            .clone()
            .title(format!("[CLOSED] {}", self.title))
            .fields(fields);
        message
            .modify_embed(|_| embed)
            .modify_action_rows(|_| Vec::new());
        Ok(())
    }
}

fn create_poll_action_rows(
    ctx: ApplicationContext<'_>,
    options: &[impl AsRef<str>],
    max_allowed_choices: u8,
    mode: PollMode,
) -> Vec<serenity::CreateActionRow> {
    const MAX_BUTTONS_PER_ROW: usize = 5;

    fn custom_id(id: u64, kind: &str, index: Option<usize>) -> String {
        match index {
            Some(i) => format!("{id}_poll_{kind}_{i}"),
            None => format!("{id}_poll_{kind}"),
        }
    }

    fn create_button(id: u64, row: usize, col: usize) -> serenity::CreateButton {
        let option_number = row * MAX_BUTTONS_PER_ROW + col;
        serenity::CreateButton::new(custom_id(id, "choice", Some(option_number)))
            .emoji(
                CHOICE_EMOJIS[option_number]
                    .parse::<serenity::ReactionType>()
                    .unwrap(),
            )
            .style(serenity::ButtonStyle::Primary)
    }

    fn create_poll_select_menu(
        id: u64,
        choices: &[impl AsRef<str>],
        max_allowed_choices: u8,
    ) -> serenity::CreateSelectMenu {
        const LABEL_MAX_LEN: usize = 100;

        let options = choices
            .iter()
            .enumerate()
            .map(|(i, choice)| {
                let choice = choice.as_ref();
                let label = if choice.len() > LABEL_MAX_LEN {
                    format!("{}...", &choice[..LABEL_MAX_LEN - 3])
                } else {
                    choice.to_string()
                };
                serenity::CreateSelectMenuOption::new(label, i.to_string())
                    .emoji(CHOICE_EMOJIS[i].parse::<serenity::ReactionType>().unwrap())
            })
            .collect();

        serenity::CreateSelectMenu::new(
            format!("{id}_poll_menu"),
            serenity::CreateSelectMenuKind::String { options },
        )
        .placeholder("Place your vote")
        .min_values(0)
        .max_values(max_allowed_choices)
    }

    let id = ctx.id();

    let mut rows: Vec<_> = match mode {
        PollMode::Buttons => options
            .chunks(MAX_BUTTONS_PER_ROW)
            .enumerate()
            .map(|(i, row)| {
                let row = (0..row.len()).map(|j| create_button(id, i, j)).collect();
                serenity::CreateActionRow::Buttons(row)
            })
            .collect(),
        PollMode::Menu => {
            let menu = create_poll_select_menu(id, options, max_allowed_choices);
            vec![serenity::CreateActionRow::SelectMenu(menu)]
        }
    };

    let clear_votes_button = serenity::CreateButton::new(custom_id(id, "clear", None))
        .emoji("✖️".parse::<serenity::ReactionType>().unwrap())
        .label("Clear my votes")
        .style(serenity::ButtonStyle::Primary);
    let close_poll_button = serenity::CreateButton::new(custom_id(id, "close", None))
        .emoji("🛑".parse::<serenity::ReactionType>().unwrap())
        .label("Close poll")
        .style(serenity::ButtonStyle::Danger);
    rows.push(serenity::CreateActionRow::Buttons(vec![
        clear_votes_button,
        close_poll_button,
    ]));

    rows
}

#[derive(poise::Modal, Debug)]
#[name = "Create a new poll"]
struct PollModal {
    #[name = "Title"]
    #[placeholder = "Enter poll title here"]
    #[max_length = 245]
    title: String,
    #[name = "Options"]
    #[placeholder = "Enter poll options here, one on each line (maximum of 10)"]
    #[paragraph]
    choices: String,
    #[name = "Duration"]
    #[placeholder = r#""30 minutes", "6 hours", "1 day", etc. Default is 1 day"#]
    duration: Option<String>,
    #[name = "Private"]
    #[placeholder = "Leave this empty to make the voting public"]
    #[max_length = 1]
    privacy: Option<String>,
    #[name = "Max Choices"]
    #[placeholder = r#"How many options can be chosen. Default is 1"#]
    num_choices: Option<String>,
}

#[derive(FromStr, Copy, Clone)]
#[display("{amount} {unit}")]
struct PollDuration {
    unit: PollDurationUnit,
    amount: u32,
}

impl PollDuration {
    const MAX_MINUTES: u64 = 60 * 24 * 7;

    fn to_minutes(self) -> u64 {
        let PollDuration { amount, unit } = self;
        let conversion = match unit {
            PollDurationUnit::Minute => 1,
            PollDurationUnit::Hour => 60,
            PollDurationUnit::Day => 60 * 24,
        };
        u64::from(amount) * conversion
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

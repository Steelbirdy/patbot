use crate::prelude::*;
use parse_display::FromStr;
use rand::Rng;

#[derive(FromStr, Debug, Copy, Clone, Eq, PartialEq)]
enum WildseaRoll {
    #[display("{dice} cut {cut}")]
    Cut {
        dice: u32,
        cut: u32,
    },
    #[display("{dice}")]
    NoCut {
        dice: u32,
    },
}

impl WildseaRoll {
    pub fn into_parts(self) -> (u32, u32) {
        match self {
            Self::Cut { dice, cut } => (dice, cut),
            Self::NoCut { dice } => (dice, 0),
        }
    }
}

/// Rolls dice in XdY format
#[poise::command(slash_command)]
pub async fn roll(
    ctx: Context<'_>,
    #[description = "The dice to roll"]
    #[rest]
    dice: Option<String>,
    #[description = "The roll will be visible only to you"]
    #[flag]
    private: bool,
) -> Result {
    let dice = dice.unwrap_or_else(|| String::from("1d20"));
    let roll = match rust_dice::roll(&dice) {
        Ok(roll) => roll,
        Err(err) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("Error: {err}"))
                    .ephemeral(true)
                    .reply(true),
            )
            .await?;
            return Ok(());
        }
    };

    let mut stringify = rust_dice::fmt::MarkdownStringifier::new();
    match stringify.stringify(&roll) {
        Ok(mut x) => {
            if x.len() > serenity::constants::MESSAGE_CODE_LIMIT {
                x = format!(
                    "{}... = `{}`",
                    &x[..serenity::constants::MESSAGE_CODE_LIMIT - 30],
                    roll.total().unwrap()
                );
            }

            ctx.send(
                poise::CreateReply::default()
                    .content(x)
                    .ephemeral(private)
                    .reply(true),
            )
            .await?;
        }
        Err(err) => {
            reply_error!(ctx, "Error: {}", err);
        }
    }

    Ok(())
}

#[poise::command(slash_command, rename = "wildsea")]
pub async fn roll_wildsea(
    ctx: Context<'_>,
    #[description = "The dice to roll. Use `X cut Y` format, or `X` if no cuts."]
    dice: String,
) -> Result {
    let Ok(dice) = dice.parse::<WildseaRoll>() else {
        reply_error!(ctx, "Error: please use the format `X cut Y`, or `X` if no cuts.");
    };
    let (dice, cut) = dice.into_parts();
    
    if cut >= dice {
        reply_error!(ctx, "Error: the number of dice to cut must be less than the number of dice to roll.");
    }
    
    let mut all_rolls: Vec<_> = {
        let mut rng = rand::rng();
        std::iter::repeat_with(|| rng.random_range(1_u32..=6_u32))
            .take(dice as usize)
            .collect()
    };
    all_rolls.sort_unstable();

    let rolls = &all_rolls[..all_rolls.len() - cut as usize];
    let highest_roll = rolls[rolls.len() - 1];
    let has_doubles = rolls
        .windows(2)
        .any(|w| w[0] == w[1]);
    
    let mut message = match highest_roll {
        1..=3 => "Disaster".to_string(),
        4..=5 => "Conflict".to_string(),
        6 => "Triumph".to_string(),
        _ => unreachable!(),
    };
    if has_doubles {
        message.push_str("... with a twist");
    }
    message.push('!');
    
    let rolls = all_rolls
        .iter()
        .enumerate()
        .fold(String::new(), |acc, (i, roll)| {
            if i >= rolls.len() {
                format!("{acc},~~{roll}~~")
            } else if acc.is_empty() {
                roll.to_string()
            } else {
                format!("{acc},{roll}")
            }
        });
    
    ctx.send(
        poise::CreateReply::default()
            .content(format!("Rolls: {rolls}\n{message}"))
            .reply(true)
    ).await?;
    
    Ok(())
}

#[test]
fn test_parse_wildsea_dice() {
    assert_eq!("3d6".parse::<WildseaRoll>().unwrap(), WildseaRoll::NoCut { dice: 3 });
    assert_eq!("3d6 cut 1".parse::<WildseaRoll>().unwrap(), WildseaRoll::Cut { dice: 3, cut: 1 });   
}

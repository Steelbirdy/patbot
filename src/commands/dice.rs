use crate::{serenity, Context, Result};

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
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("Error: {err}"))
                    .ephemeral(true)
                    .reply(true),
            )
            .await?;
        }
    }

    Ok(())
}

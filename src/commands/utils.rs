use crate::{Context, Result};

/// Pong!
#[poise::command(slash_command, ephemeral)]
pub async fn ping(ctx: Context<'_>) -> Result {
    ctx.say("Pong!").await?;
    Ok(())
}

// Shuts down the bot
#[poise::command(slash_command, owners_only, ephemeral)]
pub async fn quit(ctx: Context<'_>) -> Result {
    let _ = ctx.say("Shutting down!").await;

    let shard_manager = ctx.framework().shard_manager();
    shard_manager.shutdown_all().await;

    Ok(())
}

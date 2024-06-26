use crate::prelude::*;

/// Pong!
#[poise::command(slash_command, ephemeral)]
pub async fn ping(ctx: Context<'_>) -> Result {
    ctx.reply("Pong!").await?;
    Ok(())
}

/// Shuts down the bot
#[poise::command(slash_command, owners_only, ephemeral)]
pub async fn quit(ctx: Context<'_>) -> Result {
    let _ = ctx.reply("Shutting down!").await;

    let shard_manager = ctx.framework().shard_manager();
    shard_manager.shutdown_all().await;

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn register(ctx: Context<'_>) -> Result {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

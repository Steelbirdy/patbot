use crate::{Result, Context, serenity};

const GENERAL_VOICE_CHANNEL_ID: serenity::ChannelId = serenity::ChannelId::new(300755943912636418);
const BONK_CHANNEL_ID: serenity::ChannelId = serenity::ChannelId::new(643286466566291496);

async fn check(ctx: Context<'_>) -> Result<bool> {
    let is_frodge = ctx.guild_id().map_or(false, |id| id == crate::FRODGE_GUILD_ID);
    if !is_frodge {
        ctx.defer_ephemeral().await?;
        return Ok(false);
    }

    Ok(ctx.data()
        .buckets
        .check("bonk", ctx)
        .await
        .expect("expected bucket named `bonk`"))
}

/// __***BONK***__
#[poise::command(slash_command)]
pub async fn bonk(
    ctx: Context<'_>,
    #[description = "Mention the user to bonk"] user_id: serenity::Mention,
) -> Result {
    if !check(ctx).await? {
        return Ok(());
    }

    let serenity::Mention::User(user_id) = user_id
    else {
        ctx.send(poise::CreateReply::default()
            .content("You need to mention a user")
            .ephemeral(true))
            .await?;
        return Ok(());
    };

    let guild = ctx.guild().unwrap().id;
    guild
        .move_member(ctx, user_id, BONK_CHANNEL_ID)
        .await?;

    ctx.say("__***BONK***__").await?;
    Ok(())
}
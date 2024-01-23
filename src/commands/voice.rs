use crate::{Result, Context, serenity};
use rand::prelude::SliceRandom;

const GENERAL_VOICE_CHANNEL_ID: serenity::ChannelId = serenity::ChannelId::new(300755943912636418);
const BONK_CHANNEL_ID: serenity::ChannelId = serenity::ChannelId::new(643286466566291496);

async fn bonk_check(ctx: Context<'_>) -> Result<bool> {
    if !crate::is_frodge(ctx) {
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
    if !bonk_check(ctx).await? {
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

    let guild = ctx.guild_id().unwrap();
    guild
        .move_member(ctx, user_id, BONK_CHANNEL_ID)
        .await?;

    ctx.say("__***BONK***__").await?;
    Ok(())
}

async fn scatter_check(ctx: Context<'_>) -> Result<bool> {
    if !crate::is_frodge(ctx) {
        ctx.defer_ephemeral().await?;
        return Ok(false);
    }

    Ok(ctx
        .data()
        .buckets
        .check("scatter", ctx)
        .await
        .expect("expected bucket named `scatter`"))
}

/// __***SCATTER!!!***__
#[poise::command(slash_command)]
pub async fn scatter(ctx: Context<'_>) -> Result {
    if !scatter_check(ctx).await? {
        return Ok(());
    }


    let (vcs, voice_states) = {
        let guild = ctx.guild().unwrap();
        let vcs: Vec<_> = guild
            .channels
            .values()
            .filter(|ch| ch.kind == serenity::ChannelType::Voice && ch.id != GENERAL_VOICE_CHANNEL_ID)
            .map(|ch| ch.id)
            .collect();

        let voice_states: Vec<_> = guild
            .voice_states
            .values()
            .filter(|vs| vs.channel_id.as_ref() == Some(&GENERAL_VOICE_CHANNEL_ID))
            .map(|vs| vs.user_id)
            .collect();

        (vcs, voice_states)
    };

    let guild_id = ctx.guild_id().unwrap();

    for member in voice_states {
        let move_to = vcs.choose(&mut rand::thread_rng()).unwrap();
        guild_id.move_member(ctx, member, move_to).await?;
    }

    ctx.say("__***SCATTER!!!***__").await?;

    Ok(())
}
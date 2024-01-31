use crate::{
    serenity::{self, Mentionable},
    Context, Result,
};
use rand::prelude::SliceRandom;

fn general_voice_channel_id(ctx: Context<'_>) -> Option<serenity::ChannelId> {
    const FRODGE_GENERAL_VOICE_CHANNEL_ID: serenity::ChannelId =
        serenity::ChannelId::new(300755943912636418);
    const TESTING_GENERAL_VOICE_CHANNEL_ID: serenity::ChannelId =
        serenity::ChannelId::new(837063735645831188);

    if crate::is_frodge(ctx) {
        Some(FRODGE_GENERAL_VOICE_CHANNEL_ID)
    } else if crate::is_testing_server(ctx) {
        Some(TESTING_GENERAL_VOICE_CHANNEL_ID)
    } else {
        None
    }
}

fn bonk_channel_id(ctx: Context<'_>) -> Option<serenity::ChannelId> {
    const FRODGE_BONK_CHANNEL_ID: serenity::ChannelId =
        serenity::ChannelId::new(643286466566291496);
    const TESTING_BONK_CHANNEL_ID: serenity::ChannelId =
        serenity::ChannelId::new(1202374592983859210);

    if crate::is_frodge(ctx) {
        Some(FRODGE_BONK_CHANNEL_ID)
    } else if crate::is_testing_server(ctx) {
        Some(TESTING_BONK_CHANNEL_ID)
    } else {
        None
    }
}

async fn bucket_check(ctx: Context<'_>, bucket_name: &'static str) -> Result<bool> {
    if !crate::is_frodge_or_testing(ctx) {
        ctx.defer_ephemeral().await?;
        return Ok(false);
    }

    let res = ctx
        .data()
        .buckets
        .check(bucket_name, ctx)
        .unwrap_or_else(|| panic!("expected bucket named `{bucket_name}`"));

    Ok(match res {
        Ok(()) => true,
        Err(time_left) => {
            time_left.send_cooldown_message(ctx).await?;
            false
        }
    })
}

/// __***BONK***__
#[poise::command(slash_command)]
pub async fn bonk(
    ctx: Context<'_>,
    #[description = "Mention the user to bonk"] who: serenity::Mention,
) -> Result {
    if !bucket_check(ctx, "bonk").await? {
        return Ok(());
    }

    let serenity::Mention::User(user_id) = who else {
        ctx.send(
            poise::CreateReply::default()
                .content("You need to mention a user")
                .ephemeral(true)
                .reply(true),
        )
        .await?;
        return Ok(());
    };

    let guild = ctx.guild_id().unwrap();
    let bonk_channel_id = bonk_channel_id(ctx).unwrap();
    guild.move_member(ctx, user_id, bonk_channel_id).await?;

    ctx.reply("__***BONK***__").await?;
    ctx.data().buckets.record_usage("bonk", ctx);

    Ok(())
}

/// __***SCATTER!!!***__
#[poise::command(slash_command)]
pub async fn scatter(ctx: Context<'_>) -> Result {
    if !bucket_check(ctx, "scatter").await? {
        return Ok(());
    }

    let general_channel_id = general_voice_channel_id(ctx).unwrap();

    let (vcs, voice_states) = {
        let guild = ctx.guild().unwrap();
        let vcs: Vec<_> = guild
            .channels
            .values()
            .filter(|ch| ch.kind == serenity::ChannelType::Voice && ch.id != general_channel_id)
            .map(|ch| ch.id)
            .collect();

        let voice_states: Vec<_> = guild
            .voice_states
            .values()
            .filter(|vs| vs.channel_id.as_ref() == Some(&general_channel_id))
            .map(|vs| vs.user_id)
            .collect();

        (vcs, voice_states)
    };

    let author_id = ctx.author().id;
    let author_in_vc = voice_states.iter().any(|&user_id| user_id == author_id);

    if !author_in_vc {
        ctx.send(
            poise::CreateReply::default()
                .content(format!(
                    "You must be in {} to use `scatter`.",
                    general_channel_id.mention()
                ))
                .reply(true)
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let guild_id = ctx.guild_id().unwrap();

    for member in voice_states {
        let move_to = vcs.choose(&mut rand::thread_rng()).unwrap();
        guild_id.move_member(ctx, member, move_to).await?;
    }

    ctx.reply("__***SCATTER!!!***__").await?;
    ctx.data().buckets.record_usage("scatter", ctx);

    Ok(())
}

use crate::prelude::*;
use rand::prelude::SliceRandom;
use serenity::Mentionable;

async fn bucket_check(ctx: Context<'_>, bucket_name: &'static str) -> Result<bool> {
    if PatbotGuild::get(ctx).is_none() {
        ctx.defer_ephemeral().await?;
        return Ok(false);
    }

    let res = ctx
        .data()
        .use_buckets(|b| b.check(bucket_name, ctx))
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
#[poise::command(slash_command, guild_only)]
pub async fn bonk(
    ctx: Context<'_>,
    #[description = "Mention the user to bonk"]
    #[rest]
    who: String,
) -> Result {
    if !bucket_check(ctx, "bonk").await? {
        return Ok(());
    }

    let Some(user_id) = crate::parse_frodge_member(&who) else {
        reply_error!(ctx, "You need to specify who to bonk. This can be done by mentioning them or their name role, or just giving their name.");
    };

    let guild = PatbotGuild::get(ctx).unwrap();
    let bonk_channel_id = guild.bonk_voice_channel_id;
    guild.id.move_member(ctx, user_id, bonk_channel_id).await?;

    ctx.reply("__***BONK***__").await?;
    ctx.data().use_buckets_mut(|b| b.record_usage("bonk", ctx));

    Ok(())
}

/// __***SCATTER!!!***__
#[poise::command(slash_command, guild_only)]
pub async fn scatter(ctx: Context<'_>) -> Result {
    if !bucket_check(ctx, "scatter").await? {
        return Ok(());
    }

    let guild = PatbotGuild::get(ctx).unwrap();
    let general_channel_id = guild.general_voice_channel_id;

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
        reply_error!(
            ctx,
            "You must be in {} to use `scatter`.",
            general_channel_id.mention()
        );
    }

    for member in voice_states {
        let move_to = vcs.choose(&mut rand::thread_rng()).unwrap();
        guild.id.move_member(ctx, member, move_to).await?;
    }

    ctx.reply("__***SCATTER!!!***__").await?;
    ctx.data()
        .use_buckets_mut(|b| b.record_usage("scatter", ctx));

    Ok(())
}

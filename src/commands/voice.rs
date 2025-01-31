use crate::prelude::*;
use rand::prelude::IndexedRandom;
use serenity::Mentionable;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum BonkKind {
    Command,
    ContextMenu,
}

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

#[poise::command(context_menu_command = "Bonk", rename = "bonk")]
pub async fn bonk_context_menu(ctx: Context<'_>, who: serenity::User) -> Result {
    let guild = PatbotGuild::get(ctx).unwrap();
    bonk_impl(
        ctx,
        who.id,
        guild.bonk_text_channel_id,
        BonkKind::ContextMenu,
    )
    .await
}

/// __***BONK***__
#[poise::command(slash_command, guild_only)]
pub async fn bonk(
    ctx: Context<'_>,
    #[description = "The name of the person to bonk"]
    #[rest]
    who: String,
) -> Result {
    let Some(user_id) = crate::parse_frodge_member(&who) else {
        let bot_owner_id = {
            let bot_owners = &ctx.framework().options().owners;
            bot_owners.iter().next().copied().unwrap()
        };
        let bot_owner_name = crate::get_frodge_member(bot_owner_id).unwrap();
        reply_error!(
            ctx,
            r#"I did not understand who you wanted to bonk.
You can just use their name (ex. "{name}") or mention them (ex. "{id}").
Alternatively, you can right-click on their profile picture, then go to "Apps", then click "Bonk"."#,
            name = bot_owner_name,
            id = bot_owner_id.mention(),
        );
    };
    bonk_impl(ctx, user_id, ctx.channel_id(), BonkKind::Command).await
}

async fn bonk_impl(
    ctx: Context<'_>,
    user_id: serenity::UserId,
    channel_id: serenity::ChannelId,
    kind: BonkKind,
) -> Result {
    if !bucket_check(ctx, "bonk").await? {
        return Ok(());
    }

    let guild = PatbotGuild::get(ctx).unwrap();
    let bonk_channel_id = guild.bonk_voice_channel_id;

    if let Err(err) = guild.id.move_member(ctx, user_id, bonk_channel_id).await {
        tracing::info!("while moving user for `bonk`: {err:?}");
        reply_error!(ctx, "That user is not currently in a voice channel.");
    }

    let content = match kind {
        BonkKind::Command => String::from("__***BONK***__"),
        BonkKind::ContextMenu => {
            let author = crate::get_frodge_member(ctx.author().id).unwrap();
            format!("__***BONK***__ (used by {author})")
        }
    };

    channel_id
        .send_message(ctx, serenity::CreateMessage::default().content(content))
        .await?;
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
        let move_to = vcs.choose(&mut rand::rng()).unwrap();
        guild.id.move_member(ctx, member, move_to).await?;
    }

    ctx.reply("__***SCATTER!!!***__").await?;
    ctx.data()
        .use_buckets_mut(|b| b.record_usage("scatter", ctx));

    Ok(())
}

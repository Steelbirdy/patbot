use crate::{Context, Result};

#[poise::command(slash_command, subcommands("add", "get", "create", "delete"))]
pub async fn counter(ctx: Context<'_>, name: String) -> Result {
    get_inner(ctx, name).await
}

#[poise::command(slash_command)]
pub async fn add(ctx: Context<'_>, name: String, n: u32) -> Result {
    let new_value = &ctx.data().use_counters(|c| c.add(&name, n));

    match new_value {
        Some(value) => {
            ctx.reply(format!(
                "Added `{n}` to counter `{name}`. New value is `{value}`."
            ))
            .await?;
        }
        None => {
            ctx.send(poise::CreateReply::default()
                .content(format!("That counter does not exist. You can create it using `counter create {name}`."))
                .ephemeral(true)
                .reply(true)).await?;
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn get(ctx: Context<'_>, name: String) -> Result {
    get_inner(ctx, name).await
}

async fn get_inner(ctx: Context<'_>, name: String) -> Result {
    let value = ctx.data().use_counters(|c| c.get(&name));
    match value {
        Some(value) => {
            ctx.reply(format!(
                "The current value of counter `{name}` is `{value}`."
            ))
            .await?;
        }
        None => {
            ctx.send(poise::CreateReply::default()
                .content(format!("That counter does not exist. You can create it using `counter create {name}`."))
                .ephemeral(true)
                .reply(true))
                .await?;
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn create(ctx: Context<'_>, name: String) -> Result {
    let already_existed = ctx.data().use_counters(|c| !c.create(&name));
    if already_existed {
        ctx.send(
            poise::CreateReply::default()
                .content("That counter already exists!")
                .ephemeral(true)
                .reply(true),
        )
        .await?;
    } else {
        ctx.reply(format!("Created counter `{name}`.")).await?;
    }
    Ok(())
}

#[poise::command(slash_command, owners_only)]
pub async fn delete(ctx: Context<'_>, name: String) -> Result {
    let was_deleted = ctx.data().use_counters(|c| c.delete(&name));
    if was_deleted {
        ctx.reply(format!("Deleted counter `{name}`.")).await?;
    } else {
        ctx.send(
            poise::CreateReply::default()
                .content("That counter does not exist.")
                .ephemeral(true)
                .reply(true),
        )
        .await?;
    }

    Ok(())
}

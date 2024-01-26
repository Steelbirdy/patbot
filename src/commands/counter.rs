use crate::{Context, Result};

#[poise::command(slash_command, subcommands("add", "get", "create", "delete"))]
pub async fn counter(ctx: Context<'_>, name: String) -> Result {
    get_inner(ctx, name).await
}

#[poise::command(slash_command)]
pub async fn add(ctx: Context<'_>, name: String, n: u32) -> Result {
    let counters = &ctx.data().counters;
    let new_value = {
        let mut counters = counters.lock().unwrap();
        counters.add(&name, n)
    };

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
    let counters = &ctx.data().counters;
    let value = {
        let counters = counters.lock().unwrap();
        counters.get(&name)
    };
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
    let counters = &ctx.data().counters;
    let already_existed = {
        let mut counters = counters.lock().unwrap();
        !counters.create(&name)
    };
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
    let counters = &ctx.data().counters;
    let was_removed = {
        let mut counters = counters.lock().unwrap();
        counters.delete(&name)
    };
    if was_removed {
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

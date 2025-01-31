use crate::prelude::*;

#[poise::command(slash_command, subcommands("add", "get", "create", "delete"))]
pub async fn counter(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_name"] name: String,
) -> Result {
    get_inner(ctx, name).await
}

#[poise::command(slash_command)]
pub async fn add(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_name"] name: String,
    n: u32,
) -> Result {
    let new_value = &ctx.data().use_counters_mut(|c| c.add(&name, n));

    match new_value {
        Some(value) => {
            ctx.reply(format!(
                "Added `{n}` to counter `{name}`. New value is `{value}`."
            ))
            .await?;
        }
        None => {
            reply_error!(
                ctx,
                "That counter does not exist. You can create it using `counter create {}`.",
                name
            );
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn get(ctx: Context<'_>, #[autocomplete = "autocomplete_name"] name: String) -> Result {
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
            reply_error!(
                ctx,
                "That counter does not exist. You can create it using `counter create {}`.",
                name
            );
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn create(ctx: Context<'_>, name: String) -> Result {
    let already_existed = ctx.data().use_counters_mut(|c| !c.create(&name));
    if already_existed {
        reply_error!(ctx, "That counter already exists!");
    } else {
        ctx.reply(format!("Created counter `{name}`.")).await?;
    }
    Ok(())
}

#[poise::command(slash_command, owners_only)]
pub async fn delete(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_name"] name: String,
) -> Result {
    let was_deleted = ctx.data().use_counters_mut(|c| c.delete(&name));
    if was_deleted {
        ctx.reply(format!("Deleted counter `{name}`.")).await?;
    } else {
        reply_error!(ctx, "That counter does not exist.");
    }

    Ok(())
}

async fn autocomplete_name<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let counter_names: Vec<_> = ctx
        .data()
        .use_counters(|c| c.names().map(str::to_owned).collect());
    counter_names
        .into_iter()
        .filter(move |name| name.starts_with(partial))
}

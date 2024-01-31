mod commands;
mod data;

use data::Data;

use poise::serenity_prelude as serenity;
use shuttle_persist::PersistInstance;
use shuttle_secrets::SecretStore;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T = (), E = Error> = std::result::Result<T, E>;
type Context<'a> = poise::Context<'a, Data, Error>;

const FRODGE_GUILD_ID: serenity::GuildId = serenity::GuildId::new(300755943912636417);
const TESTING_GUILD_ID: serenity::GuildId = serenity::GuildId::new(765314921151332464);

fn is_frodge(ctx: Context<'_>) -> bool {
    ctx.guild_id() == Some(FRODGE_GUILD_ID)
}

fn is_testing_server(ctx: Context<'_>) -> bool {
    ctx.guild_id() == Some(TESTING_GUILD_ID)
}

fn is_frodge_or_testing(ctx: Context<'_>) -> bool {
    is_frodge(ctx) || is_testing_server(ctx)
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_persist::Persist] persist: PersistInstance,
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    let _ = dotenv::dotenv();

    let token = if let Ok(token) = std::env::var("DISCORD_TOKEN") {
        token
    } else {
        secret_store.get("DISCORD_TOKEN").expect(
            "expected a bot token in the environment. Add the `DISCORD_TOKEN` key to the Secrets.toml file",
        )
    };

    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::bonk(),
                commands::counter(),
                commands::ping(),
                commands::quit(),
                commands::roll(),
                commands::scatter(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Data::new(ctx, persist).await
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(client.into())
}

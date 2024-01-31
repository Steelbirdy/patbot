mod commands;
mod data;

use data::Data;
use std::time::Duration;

use poise::serenity_prelude as serenity;

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

async fn data(ctx: &serenity::Context) -> Result<Data> {
    const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);

    let mut data = Data::new(ctx).await?;

    data.buckets.insert("bonk", ONE_DAY);
    data.buckets.insert("scatter", ONE_DAY * 7);

    Ok(data)
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_secrets::Secrets] secret_store: shuttle_secrets::SecretStore,
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
                data(ctx).await
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("failed to create client");

    Ok(client.into())
}

mod commands;
mod data;

use std::time::Duration;
use data::Data;

use tracing_subscriber::{EnvFilter, FmtSubscriber};
use poise::serenity_prelude as serenity;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T = (), E = Error> = std::result::Result<T, E>;
type Context<'a> = poise::Context<'a, Data, Error>;

const FRODGE_GUILD_ID: serenity::GuildId = serenity::GuildId::new(300755943912636417);

async fn data(ctx: &serenity::Context) -> Result<Data> {
    const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);

    let mut data = Data::new(ctx).await?;

    data.buckets.insert("bonk", ONE_DAY);

    Ok(data)
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("failed to load .env file");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed to start the logger");

    let token = std::env::var("DISCORD_TOKEN")
        .expect("expected a bot token in the environment. Add the `DISCORD_TOKEN` key to the .env file");

    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::bonk(),
                commands::ping(),
                commands::quit(),
                commands::roll(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(data(ctx).await?)
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}

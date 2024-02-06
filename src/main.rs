mod commands;
mod data;

use data::Data;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

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

static FRODGE_MEMBERS: OnceLock<HashMap<String, serenity::UserId>> = OnceLock::new();

static FRODGE_ROLES: OnceLock<HashMap<serenity::RoleId, serenity::UserId>> = OnceLock::new();

fn parse_frodge_member(s: &str) -> Option<serenity::UserId> {
    match s.parse::<serenity::Mention>() {
        Ok(serenity::Mention::User(ret)) if is_frodge_member(ret) => return Some(ret),
        Ok(serenity::Mention::Role(key)) => {
            let role_map = FRODGE_ROLES.get().unwrap();
            return role_map.get(&key).copied();
        }
        _ => {}
    }

    let member_map = FRODGE_MEMBERS.get().unwrap();
    member_map.get(&s.to_ascii_lowercase()).copied()
}

fn is_frodge_member(user_id: serenity::UserId) -> bool {
    static FRODGE_MEMBER_IDS: OnceLock<HashSet<serenity::UserId>> = OnceLock::new();

    FRODGE_MEMBER_IDS
        .get_or_init(|| {
            let member_map = FRODGE_MEMBERS.get().unwrap();
            member_map.values().copied().collect()
        })
        .contains(&user_id)
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_persist::Persist] persist: PersistInstance,
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    let _ = dotenv::dotenv();

    #[derive(Debug)]
    struct EnvVarNotFound(String);

    let env_var = |key: &str| {
        if let Ok(value) = std::env::var(key) {
            return Ok(value);
        }
        if let Some(value) = secret_store.get(key) {
            return Ok(value);
        }
        Err(EnvVarNotFound(key.to_string()))
    };

    let token = env_var("DISCORD_TOKEN").unwrap();
    let prefix = env_var("BOT_PREFIX").unwrap();
    let frodge_members = env_var("FRODGE_MEMBERS").unwrap();
    FRODGE_MEMBERS
        .set(serde_json::from_str(&frodge_members).unwrap())
        .unwrap();
    let frodge_roles = env_var("FRODGE_ROLES").unwrap();
    FRODGE_ROLES
        .set(serde_json::from_str(&frodge_roles).unwrap())
        .unwrap();

    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(prefix),
                ..Default::default()
            },
            commands: vec![
                commands::bonk(),
                commands::counter(),
                commands::ping(),
                commands::quit(),
                commands::register(),
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

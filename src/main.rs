mod commands;
mod data;
mod interactive;
mod macros;

use data::Data;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use poise::serenity_prelude as serenity;
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T = (), E = Error> = std::result::Result<T, E>;
type Context<'a> = poise::Context<'a, Data, Error>;
type ApplicationContext<'a> = poise::ApplicationContext<'a, Data, Error>;

mod prelude {
    pub(crate) use crate::{
        macros::{reply_error, respond_to_interaction},
        serenity, ApplicationContext, Context, PatbotGuild, Result,
    };
}

#[derive(Copy, Clone)]
struct PatbotGuild {
    id: serenity::GuildId,
    general_voice_channel_id: serenity::ChannelId,
    bonk_voice_channel_id: serenity::ChannelId,
    congress_text_channel_id: serenity::ChannelId,
}

impl PatbotGuild {
    const ALL: &'static [PatbotGuild] = &[Self::FRODGE_GUILD, Self::TESTING_GUILD];

    const FRODGE_GUILD: PatbotGuild = PatbotGuild {
        id: serenity::GuildId::new(300755943912636417),
        general_voice_channel_id: serenity::ChannelId::new(300755943912636418),
        bonk_voice_channel_id: serenity::ChannelId::new(643286466566291496),
        congress_text_channel_id: serenity::ChannelId::new(384516853386706964),
    };

    const TESTING_GUILD: PatbotGuild = PatbotGuild {
        id: serenity::GuildId::new(765314921151332464),
        general_voice_channel_id: serenity::ChannelId::new(837063735645831188),
        bonk_voice_channel_id: serenity::ChannelId::new(1202374592983859210),
        congress_text_channel_id: serenity::ChannelId::new(765314921604710462),
    };

    fn get(ctx: impl ctx::PatbotGuildContext) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|guild| ctx.guild_id() == Some(guild.id))
    }
}

mod ctx {
    pub(crate) trait PatbotGuildContext {
        fn guild_id(&self) -> Option<crate::serenity::GuildId>;
    }

    impl PatbotGuildContext for crate::Context<'_> {
        fn guild_id(&self) -> Option<crate::serenity::GuildId> {
            crate::Context::guild_id(*self)
        }
    }

    impl PatbotGuildContext for crate::ApplicationContext<'_> {
        fn guild_id(&self) -> Option<crate::serenity::GuildId> {
            crate::ApplicationContext::guild_id(*self)
        }
    }
}

static FRODGE_MEMBERS: OnceLock<HashMap<String, serenity::UserId>> = OnceLock::new();
static FRODGE_ROLES: OnceLock<HashMap<serenity::RoleId, serenity::UserId>> = OnceLock::new();
static FRODGE_NONPREFERENTIAL_NAMES: OnceLock<HashSet<String>> = OnceLock::new();

fn frodge_membership_count() -> usize {
    let roles = FRODGE_ROLES.get().unwrap();
    roles.len()
}

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

fn get_frodge_member(user_id: serenity::UserId) -> Option<&'static str> {
    static FRODGE_MEMBERS_INVERSE: OnceLock<HashMap<serenity::UserId, String>> = OnceLock::new();

    let member_map = FRODGE_MEMBERS_INVERSE.get_or_init(|| {
        let member_map = FRODGE_MEMBERS.get().unwrap();
        let nonpreferential_names = FRODGE_NONPREFERENTIAL_NAMES.get().unwrap();

        member_map
            .iter()
            .filter_map(|(name, &user_id)| {
                let is_preferential = !nonpreferential_names.contains(name);
                if is_preferential {
                    let first_letter = name[..1].to_ascii_uppercase();
                    let rest = if name.len() == 1 { "" } else { &name[1..] };
                    let name = format!("{first_letter}{rest}");
                    Some((user_id, name))
                } else {
                    None
                }
            })
            .collect()
    });
    member_map.get(&user_id).map(String::as_str)
}

fn is_frodge_member(user_id: serenity::UserId) -> bool {
    get_frodge_member(user_id).is_some()
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_persist::Persist] persist: PersistInstance,
    #[shuttle_runtime::Secrets] secret_store: SecretStore,
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
    let frodge_nonpreferential_names = env_var("FRODGE_NONPREFERENTIAL_NAMES").unwrap();
    FRODGE_NONPREFERENTIAL_NAMES
        .set(serde_json::from_str(&frodge_nonpreferential_names).unwrap())
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
                commands::gazoo(),
                commands::petition(),
                commands::ping(),
                commands::poll(),
                commands::quit(),
                commands::register(),
                commands::roll(),
                commands::scatter(),
                commands::set_poll_mode(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    serenity::GuildId::new(765314921151332464),
                )
                .await?;
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

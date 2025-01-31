use crate::{
    commands::{PollMode, ReplyCommand, ReplyCommandResponse},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use serenity::Color;
use shuttle_persist::PersistInstance;
use std::{collections::HashMap, fmt, sync::Mutex};
use time::{Duration, OffsetDateTime};

pub struct Data {
    bot_color: Color,
    poll_mode: Mutex<PollMode>,
    writer: DataWriter,
}

mod writer {
    use super::{Buckets, Counters, ReplyCommands};
    use shuttle_persist::PersistInstance;
    use std::sync::Mutex;

    pub struct DataWriter {
        persist: PersistInstance,
        buckets: Mutex<Buckets>,
        counters: Mutex<Counters>,
        reply_commands: Mutex<ReplyCommands>,
    }

    impl DataWriter {
        pub fn new(
            persist: PersistInstance,
            buckets: Mutex<Buckets>,
            counters: Mutex<Counters>,
            reply_commands: Mutex<ReplyCommands>,
        ) -> Self {
            Self {
                persist,
                buckets,
                counters,
                reply_commands,
            }
        }

        pub fn use_counters<F, T>(&self, f: F) -> T
        where
            F: FnOnce(&Counters) -> T,
        {
            let counters = self.counters.lock().unwrap();
            f(&counters)
        }

        pub fn use_counters_mut<F, T>(&self, f: F) -> T
        where
            F: FnOnce(&mut Counters) -> T,
        {
            let (ret, counters_clone) = {
                let mut counters = self.counters.lock().unwrap();
                let ret = f(&mut counters);
                (ret, counters.clone())
            };
            self.persist.save("counters", counters_clone).unwrap();
            ret
        }

        pub fn use_buckets<F, T>(&self, f: F) -> T
        where
            F: FnOnce(&Buckets) -> T,
        {
            let buckets = self.buckets.lock().unwrap();
            f(&buckets)
        }

        pub fn use_buckets_mut<F, T>(&self, f: F) -> T
        where
            F: FnOnce(&mut Buckets) -> T,
        {
            let (ret, buckets_clone) = {
                let mut buckets = self.buckets.lock().unwrap();
                let ret = f(&mut buckets);
                (ret, buckets.clone())
            };
            self.persist.save("buckets", buckets_clone).unwrap();
            ret
        }

        pub fn use_reply_commands<F, T>(&self, f: F) -> T
        where
            F: FnOnce(&ReplyCommands) -> T,
        {
            let reply_commands = self.reply_commands.lock().unwrap();
            f(&reply_commands)
        }

        pub fn use_reply_commands_mut<F, T>(&self, f: F) -> T
        where
            F: FnOnce(&mut ReplyCommands) -> T,
        {
            let (ret, reply_commands_clone) = {
                let mut reply_commands = self.reply_commands.lock().unwrap();
                let ret = f(&mut reply_commands);
                (ret, reply_commands.clone())
            };
            self.persist
                .save("reply_commands", reply_commands_clone)
                .unwrap();
            ret
        }
    }
}

pub use writer::DataWriter;

impl Data {
    pub async fn new(ctx: &serenity::Context, persist: PersistInstance) -> Result<Self> {
        let bot_color = ctx
            .http
            .get_current_user()
            .await?
            .accent_colour
            .unwrap_or(Color::BLURPLE);

        let buckets = match persist.load::<Buckets>("buckets") {
            Ok(x) => Mutex::new(x),
            Err(_) => Default::default(),
        };
        let counters = match persist.load::<Counters>("counters") {
            Ok(x) => Mutex::new(x),
            Err(_) => Default::default(),
        };
        let reply_commands = match persist.load::<ReplyCommands>("reply_commands") {
            Ok(mut x) => {
                x.clear_ids();
                Mutex::new(x)
            }
            Err(_) => Default::default(),
        };

        Ok(Self {
            bot_color,
            poll_mode: Default::default(),
            writer: DataWriter::new(persist, buckets, counters, reply_commands),
        })
    }

    pub fn bot_color(&self) -> Color {
        self.bot_color
    }

    pub fn poll_mode(&self) -> PollMode {
        *self.poll_mode.lock().unwrap()
    }

    pub fn set_poll_mode(&self, mode: PollMode) {
        *self.poll_mode.lock().unwrap() = mode;
    }

    pub async fn register_reply_command(
        &self,
        ctx: Context<'_>,
        mut reply_command: ReplyCommand,
    ) -> Result {
        let new_command = reply_command.to_poise_command();
        let new_commands =
            poise::builtins::create_application_commands(std::slice::from_ref(&new_command));
        assert_eq!(new_commands.len(), 1);
        let new_command = &new_commands[0];
        for guild in PatbotGuild::all() {
            match guild.id.create_command(ctx, new_command.clone()).await {
                Ok(command) => reply_command.ids.push((guild.id.get(), command.id.get())),
                Err(err) => {
                    tracing::error!(
                        "failed to create reply command in guild {}: {err:?}",
                        guild.id
                    );
                    if Some(guild.id) == ctx.guild_id() {
                        let _ = ctx.reply(format!("Failed to create command: {err}")).await;
                    }
                }
            }
        }
        self.use_reply_commands_mut(|commands| commands.insert(reply_command));
        Ok(())
    }

    pub async fn delete_reply_command(&self, ctx: Context<'_>, name: &str) -> Result<bool> {
        let Some(command) = self.use_reply_commands_mut(|commands| commands.remove(name)) else {
            return Ok(false);
        };
        for (guild_id, command_id) in command.ids {
            serenity::GuildId::new(guild_id)
                .delete_command(ctx, serenity::CommandId::new(command_id))
                .await?;
        }
        Ok(true)
    }

    pub fn reply_command_response(&self, name: &str) -> Option<ReplyCommandResponse> {
        self.use_reply_commands(|commands| commands.command_response(name))
    }
}

impl std::ops::Deref for Data {
    type Target = DataWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Buckets {
    inner: HashMap<String, Bucket>,
}

impl Default for Buckets {
    fn default() -> Self {
        const ONE_DAY: Duration = Duration::new(60 * 60 * 24, 0);

        let mut inner = HashMap::default();
        inner.insert("bonk".to_string(), Bucket::new(ONE_DAY));
        inner.insert("scatter".to_string(), Bucket::new(ONE_DAY * 7));
        Self { inner }
    }
}

impl Buckets {
    pub fn check(&self, name: &'static str, ctx: Context<'_>) -> Option<Result<(), TimeLeft>> {
        let bucket = self.inner.get(name)?;
        let id = ctx.author().id.get();
        Some(bucket.check(id))
    }

    pub fn record_usage(&mut self, name: &'static str, ctx: Context<'_>) {
        let bucket = self
            .inner
            .get_mut(name)
            .unwrap_or_else(|| panic!("expected a bucket named `{name}`"));
        let id = ctx.author().id.get();
        bucket
            .record_usage(id)
            .expect("expected a valid command usage");
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bucket {
    last_usage: HashMap<u64, OffsetDateTime>,
    interval: Duration,
}

impl Bucket {
    pub fn new(interval: Duration) -> Self {
        Self {
            last_usage: Default::default(),
            interval,
        }
    }

    pub fn record_usage(&mut self, id: u64) -> Result<(), TimeLeft> {
        let ret = self.check(id);
        if ret.is_ok() {
            self.insert_now(id);
        }
        ret
    }

    pub fn check(&self, id: u64) -> Result<(), TimeLeft> {
        match self.time_passed(id) {
            Some(time_passed) if time_passed < self.interval => {
                Err(TimeLeft(self.interval - time_passed))
            }
            _ => Ok(()),
        }
    }

    fn time_passed(&self, id: u64) -> Option<Duration> {
        let last_usage = self.last_usage.get(&id)?;
        Some(OffsetDateTime::now_utc() - *last_usage)
    }

    fn insert_now(&mut self, id: u64) {
        self.last_usage.insert(id, OffsetDateTime::now_utc());
    }
}

#[derive(Debug)]
pub struct TimeLeft(Duration);

impl TimeLeft {
    pub async fn send_cooldown_message(&self, ctx: Context<'_>) -> Result {
        ctx.send(
            poise::CreateReply::default()
                .content(format!("You must wait `{self}` to use that command again."))
                .reply(true)
                .ephemeral(true),
        )
        .await?;
        Ok(())
    }
}

impl fmt::Display for TimeLeft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let duration = &self.0;

        let div_mod = |a: u64, b: u64| (a / b, a % b);

        let (minutes, seconds) = div_mod(duration.whole_seconds() as _, 60);
        let (hours, minutes) = div_mod(minutes, 60);
        let (days, hours) = div_mod(hours, 24);

        let mut parts = vec![];

        let format_part = |x: u64, s: &str| {
            let suffix = if x == 1 { "" } else { "s" };
            format!("{} {}{}", x, s, suffix)
        };

        if days > 0 {
            parts.push(format_part(days, "day"));
        }
        if hours > 0 {
            parts.push(format_part(hours, "hour"));
        }
        if minutes > 0 {
            parts.push(format_part(minutes, "minute"));
        }
        if seconds > 0 {
            parts.push(format_part(seconds, "second"));
        }

        if parts.is_empty() {
            f.write_str("0 seconds")
        } else {
            write!(f, "{}", parts.join(", "))
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Counters {
    inner: HashMap<String, u32>,
}

impl Counters {
    /// Returns true if the counter did not already exist
    pub fn create(&mut self, name: impl ToString) -> bool {
        let name = name.to_string();
        let is_new_counter = !self.inner.contains_key(&name);
        if is_new_counter {
            self.inner.insert(name, 0);
        }
        is_new_counter
    }

    pub fn delete(&mut self, name: impl AsRef<str>) -> bool {
        let ret = self.inner.remove(name.as_ref()).is_some();
        ret
    }

    /// Returns the new value
    pub fn add(&mut self, name: impl AsRef<str>, n: u32) -> Option<u32> {
        let name = name.as_ref();
        let value = {
            let counter = self.inner.get_mut(name)?;
            *counter += n;
            *counter
        };
        Some(value)
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<u32> {
        self.inner.get(name.as_ref()).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(String::as_str)
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ReplyCommands {
    commands: Vec<ReplyCommand>,
}

impl ReplyCommands {
    fn insert(&mut self, command: ReplyCommand) {
        match self
            .commands
            .iter_mut()
            .find(|cmd| cmd.name == command.name)
        {
            Some(prev) => {
                *prev = command;
            }
            None => {
                self.commands.push(command);
            }
        }
    }

    fn remove(&mut self, name: &str) -> Option<ReplyCommand> {
        let index = self.commands.iter().position(|cmd| cmd.name == name)?;
        Some(self.commands.swap_remove(index))
    }

    fn command_response(&self, name: &str) -> Option<ReplyCommandResponse> {
        let command = self.get(name)?;
        Some(command.response.clone())
    }

    fn clear_ids(&mut self) {
        for command in self.iter_mut() {
            command.ids.clear();
        }
    }

    pub fn get(&self, name: &str) -> Option<&ReplyCommand> {
        self.commands.iter().find(|cmd| cmd.name == name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReplyCommand> {
        self.commands.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ReplyCommand> {
        self.commands.iter_mut()
    }
}

use crate::{
    serenity::{self, model::Colour},
    Context, Result,
};
use serde::{Deserialize, Serialize};
use shuttle_persist::PersistInstance;
use std::{collections::HashMap, fmt, sync::Mutex};
use time::{Duration, OffsetDateTime};

pub struct Data {
    writer: PersistInstance,
    bot_color: Colour,
    poll_mode: Mutex<PollMode>,
    buckets: Mutex<Buckets>,
    counters: Mutex<Counters>,
}

#[derive(poise::ChoiceParameter, Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum PollMode {
    #[default]
    #[name = "buttons"]
    Buttons,
    #[name = "menu"]
    Menu,
}

impl Data {
    pub async fn new(ctx: &serenity::Context, persist: PersistInstance) -> Result<Self> {
        let bot_color = ctx
            .http
            .get_current_user()
            .await?
            .accent_colour
            .unwrap_or(Colour::BLURPLE);

        let counters = match persist.load("counters") {
            Ok(x) => Mutex::new(x),
            Err(_) => Default::default(),
        };
        let buckets = match persist.load("buckets") {
            Ok(x) => Mutex::new(x),
            Err(_) => Default::default(),
        };

        Ok(Self {
            writer: persist,
            bot_color,
            poll_mode: Default::default(),
            buckets,
            counters,
        })
    }

    pub fn bot_color(&self) -> Colour {
        self.bot_color
    }

    pub fn poll_mode(&self) -> PollMode {
        *self.poll_mode.lock().unwrap()
    }

    pub fn set_poll_mode(&self, mode: PollMode) {
        *self.poll_mode.lock().unwrap() = mode;
    }

    pub fn use_counters<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut Counters) -> T,
    {
        let (ret, counters_clone) = {
            let mut counters = self.counters.lock().unwrap();
            let ret = f(&mut counters);
            (ret, counters.clone())
        };
        self.writer.save("counters", counters_clone).unwrap();
        ret
    }

    pub fn use_buckets<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut Buckets) -> T,
    {
        let (ret, buckets_clone) = {
            let mut buckets = self.buckets.lock().unwrap();
            let ret = f(&mut buckets);
            (ret, buckets.clone())
        };
        self.writer.save("buckets", buckets_clone).unwrap();
        ret
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
}

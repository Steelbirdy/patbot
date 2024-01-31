use crate::{serenity, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    sync::Mutex,
    time::{Duration, Instant},
};

const COUNTERS_FILE: &str = "counters.json";

pub struct Data {
    pub buckets: Buckets,
    pub counters: Mutex<Counters>,
}

impl Data {
    pub async fn new(_ctx: &serenity::Context) -> Result<Self> {
        let counters = std::fs::read_to_string(COUNTERS_FILE)
            .ok()
            .and_then(|str| serde_json::from_str(&str).ok())
            .unwrap_or_default();

        Ok(Self {
            buckets: Buckets::default(),
            counters,
        })
    }
}

#[derive(Default)]
pub struct Buckets {
    inner: HashMap<&'static str, Bucket>,
}

impl Buckets {
    pub fn insert(&mut self, name: &'static str, interval: Duration) -> &mut Self {
        let bucket = Bucket::new(interval);
        self.inner.insert(name, bucket);
        self
    }

    pub fn check(&self, name: &'static str, ctx: Context<'_>) -> Option<Result<(), TimeLeft>> {
        let bucket = self.inner.get(name)?;
        let id = ctx.author().id.get();
        Some(bucket.check(id))
    }

    pub fn record_usage(&self, name: &'static str, ctx: Context<'_>) {
        let bucket = self
            .inner
            .get(name)
            .unwrap_or_else(|| panic!("expected a bucket named `{name}`"));
        let id = ctx.author().id.get();
        bucket
            .record_usage(id)
            .expect("expected a valid command usage");
    }
}

pub struct Bucket {
    last_usage: Mutex<HashMap<u64, Instant>>,
    interval: Duration,
}

impl Bucket {
    pub fn new(interval: Duration) -> Self {
        Self {
            last_usage: Mutex::default(),
            interval,
        }
    }

    pub fn record_usage(&self, id: u64) -> Result<(), TimeLeft> {
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
        let last_usage = {
            let lock = self.last_usage.lock().unwrap();
            lock.get(&id).copied()?
        };
        Some(last_usage.elapsed())
    }

    fn insert_now(&self, id: u64) {
        let mut lock = self.last_usage.lock().unwrap();
        lock.insert(id, Instant::now());
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

        let (minutes, seconds) = div_mod(duration.as_secs(), 60);
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

#[derive(Default, Serialize, Deserialize)]
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
            self.write_to_file();
        }
        is_new_counter
    }

    pub fn delete(&mut self, name: impl AsRef<str>) -> bool {
        let ret = self.inner.remove(name.as_ref()).is_some();
        self.write_to_file();
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
        self.write_to_file();
        Some(value)
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<u32> {
        self.inner.get(name.as_ref()).copied()
    }

    fn write_to_file(&self) {
        let serialized = serde_json::to_string(self).unwrap();
        std::fs::write(COUNTERS_FILE, serialized).unwrap();
    }
}

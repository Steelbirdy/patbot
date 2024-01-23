use crate::{serenity, Context, Result};
use std::{
    collections::HashMap,
    fmt,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct Data {
    pub buckets: Buckets,
}

impl Data {
    pub async fn new(_ctx: &serenity::Context) -> Result<Self> {
        Ok(Self {
            buckets: Buckets::default(),
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

    pub fn get(&self, name: &'static str) -> Option<&Bucket> {
        self.inner.get(name)
    }

    pub async fn check(&self, name: &'static str, ctx: Context<'_>) -> Option<bool> {
        let bucket = self.get(name)?;
        Some(bucket.check(ctx).await)
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

    pub fn can_use(&self, id: u64) -> bool {
        self.time_passed(id).map_or(false, |t| t >= self.interval)
    }

    pub fn record_usage(&self, id: u64) -> Result<(), TimeLeft> {
        match self.time_passed(id) {
            Some(time_passed) if time_passed < self.interval => {
                Err(TimeLeft(self.interval - time_passed))
            }
            _ => {
                self.insert_now(id);
                Ok(())
            }
        }
    }

    pub async fn check(&self, ctx: Context<'_>) -> bool {
        if let Err(time_left) = self.record_usage(ctx.author().id.get()) {
            let _ = ctx
                .send(
                    poise::CreateReply::default()
                        .content(format!(
                            "You must wait `{time_left}` to use that command again"
                        ))
                        .ephemeral(true),
                )
                .await;
            return false;
        }
        true
    }

    fn get(&self, id: u64) -> Option<Instant> {
        let lock = self.last_usage.lock().unwrap();
        lock.get(&id).copied()
    }

    fn time_passed(&self, id: u64) -> Option<Duration> {
        let last_usage = self.get(id)?;
        Some(Instant::now() - last_usage)
    }

    fn insert_now(&self, id: u64) {
        let mut lock = self.last_usage.lock().unwrap();
        lock.insert(id, Instant::now());
    }
}

pub struct TimeLeft(Duration);

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

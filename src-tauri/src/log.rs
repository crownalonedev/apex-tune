//! Logging: a rolling file on disk plus the in-app Activity Log.
//!
//! Anything logged with `target: "activity"` is a user-facing action and
//! appears in the Activity Log. Warnings and errors from this crate appear
//! there too, so a failure is never only in the file.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

pub const ACTIVITY_TARGET: &str = "activity";
const CRATE_TARGET: &str = "apex_tune_lib";
const LOG_FILES_KEPT: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub id: u64,
    pub at: DateTime<Utc>,
    pub level: ActivityLevel,
    pub message: String,
    /// The tweak or feature the entry is about, from a `tweak = "…"` field.
    pub subject: Option<String>,
}

type Sink = Box<dyn Fn(&ActivityEntry) + Send + Sync>;

/// Bounded in-memory history of activity entries, plus an optional sink that
/// forwards each new entry to the UI.
pub struct ActivityLog {
    entries: Mutex<VecDeque<ActivityEntry>>,
    capacity: usize,
    next_id: AtomicU64,
    sink: OnceLock<Sink>,
}

impl ActivityLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            next_id: AtomicU64::new(1),
            sink: OnceLock::new(),
        }
    }

    /// Sets where new entries are forwarded. Only the first call has an effect.
    pub fn set_sink(&self, sink: impl Fn(&ActivityEntry) + Send + Sync + 'static) {
        let _ = self.sink.set(Box::new(sink));
    }

    pub fn push(&self, level: ActivityLevel, message: String, subject: Option<String>) {
        let entry = ActivityEntry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            at: Utc::now(),
            level,
            message,
            subject,
        };
        {
            let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
            if entries.len() == self.capacity {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }
        if let Some(sink) = self.sink.get() {
            sink(&entry);
        }
    }

    /// Entries oldest-first.
    pub fn recent(&self) -> Vec<ActivityEntry> {
        let entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        entries.iter().cloned().collect()
    }
}

/// Forwards matching tracing events into an [`ActivityLog`].
struct ActivityLayer {
    log: Arc<ActivityLog>,
}

#[derive(Default)]
struct EventFields {
    message: String,
    subject: Option<String>,
}

impl Visit for EventFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_owned(),
            "tweak" => self.subject = Some(value.to_owned()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}"),
            "tweak" => self.subject = Some(format!("{value:?}").trim_matches('"').to_owned()),
            _ => {}
        }
    }
}

fn activity_level(level: &Level) -> ActivityLevel {
    match *level {
        Level::ERROR => ActivityLevel::Error,
        Level::WARN => ActivityLevel::Warn,
        _ => ActivityLevel::Info,
    }
}

fn is_user_facing(target: &str, level: &Level) -> bool {
    target == ACTIVITY_TARGET || (target.starts_with(CRATE_TARGET) && *level <= Level::WARN)
}

impl<S: Subscriber> Layer<S> for ActivityLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        if !is_user_facing(meta.target(), meta.level()) {
            return;
        }
        let mut fields = EventFields::default();
        event.record(&mut fields);
        self.log
            .push(activity_level(meta.level()), fields.message, fields.subject);
    }
}

/// Installs the global subscriber. Keep the returned guard alive for the
/// whole process, or buffered file output is lost on exit.
pub fn init(log_dir: &Path, activity: Arc<ActivityLog>) -> std::io::Result<WorkerGuard> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("apex-tune")
        .filename_suffix("log")
        .max_log_files(LOG_FILES_KEPT)
        .build(log_dir)
        .map_err(std::io::Error::other)?;
    let (file_writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_env("APEX_TUNE_LOG")
        .unwrap_or_else(|_| EnvFilter::new(format!("info,{CRATE_TARGET}=debug")));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false),
        )
        .with(cfg!(debug_assertions).then(tracing_subscriber::fmt::layer))
        .with(ActivityLayer { log: activity })
        .try_init()
        .map_err(std::io::Error::other)?;
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn keeps_only_the_newest_entries() {
        let log = ActivityLog::new(3);
        for i in 0..5 {
            log.push(ActivityLevel::Info, format!("entry {i}"), None);
        }
        let messages: Vec<_> = log.recent().into_iter().map(|e| e.message).collect();
        assert_eq!(messages, ["entry 2", "entry 3", "entry 4"]);
    }

    #[test]
    fn ids_increase_and_sink_sees_every_entry() {
        let log = ActivityLog::new(10);
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = seen.clone();
        log.set_sink(move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        log.push(ActivityLevel::Info, "a".into(), None);
        log.push(ActivityLevel::Error, "b".into(), Some("game_mode".into()));
        let entries = log.recent();
        assert!(entries[0].id < entries[1].id);
        assert_eq!(entries[1].subject.as_deref(), Some("game_mode"));
        assert_eq!(seen.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn layer_captures_activity_and_crate_warnings_only() {
        let log = Arc::new(ActivityLog::new(10));
        let subscriber = tracing_subscriber::registry().with(ActivityLayer { log: log.clone() });
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "activity", tweak = "hags", "Applied HAGS");
            tracing::info!(target: "apex_tune_lib::power", "internal detail");
            tracing::warn!(target: "apex_tune_lib::power", "plan missing");
            tracing::warn!(target: "some_dependency", "not ours");
        });
        let entries = log.recent();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "Applied HAGS");
        assert_eq!(entries[0].subject.as_deref(), Some("hags"));
        assert_eq!(entries[1].level, ActivityLevel::Warn);
    }
}

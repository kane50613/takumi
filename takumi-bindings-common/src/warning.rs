//! The warnings takumi logs, handed to a binding's console once each per process.

use std::{
  collections::HashSet,
  sync::{Mutex, OnceLock},
};

use log::{Level, LevelFilter, Log, Metadata, Record, kv::Key};

/// The prefix of every target takumi logs under.
const TARGET_PREFIX: &str = "takumi::";

/// The key a warning about one CSS property carries its name under, so that its warnings, at any
/// line, count as one.
const PROPERTY_KEY: &str = "property";

/// Hands each new takumi warning to a sink, such as `console.warn`.
pub struct WarningLog {
  sink: fn(String),
  /// The warnings already handed on, by property name or else by message.
  seen: Mutex<HashSet<String>>,
}

static WARNING_LOG: OnceLock<WarningLog> = OnceLock::new();

impl WarningLog {
  /// Sends takumi's warnings to `sink` from now on, unless the process already has a logger.
  pub fn install(sink: fn(String)) {
    let log = WARNING_LOG.get_or_init(|| Self {
      sink,
      seen: Mutex::default(),
    });

    if log::set_logger(log).is_ok() {
      log::set_max_level(LevelFilter::Warn);
    }
  }

  /// Whether no warning has had `key` before.
  fn is_new(&self, key: String) -> bool {
    self.seen.lock().is_ok_and(|mut seen| seen.insert(key))
  }
}

impl Log for WarningLog {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    metadata.level() <= Level::Warn && metadata.target().starts_with(TARGET_PREFIX)
  }

  fn log(&self, record: &Record<'_>) {
    if !self.enabled(record.metadata()) {
      return;
    }

    let message = record.args().to_string();
    let key = match record.key_values().get(Key::from_str(PROPERTY_KEY)) {
      Some(property) => format!("{PROPERTY_KEY}:{property}"),
      None => message.clone(),
    };

    if self.is_new(key) {
      (self.sink)(format!("takumi: {message}"));
    }
  }

  fn flush(&self) {}
}

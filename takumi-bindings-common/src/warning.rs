//! The warnings takumi logs, handed to a binding's console.

use std::sync::Once;

use log::{Level, LevelFilter, Log, Metadata, Record};

/// The prefix of every target takumi logs under.
const TARGET_PREFIX: &str = "takumi::";

/// Hands each takumi warning to a binding's sink, on the thread that logs it.
pub struct WarningLog(fn(String));

impl WarningLog {
  /// Sends takumi's warnings to `sink` from now on, unless the process already has a logger.
  pub fn install(sink: fn(String)) {
    static INSTALL: Once = Once::new();

    INSTALL.call_once(|| {
      if log::set_logger(Box::leak(Box::new(Self(sink)))).is_ok() {
        log::set_max_level(LevelFilter::Warn);
      }
    });
  }
}

impl Log for WarningLog {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    metadata.level() <= Level::Warn && metadata.target().starts_with(TARGET_PREFIX)
  }

  fn log(&self, record: &Record<'_>) {
    if self.enabled(record.metadata()) {
      (self.0)(format!("takumi: {}", record.args()));
    }
  }

  fn flush(&self) {}
}

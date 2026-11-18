//! The warnings takumi logs, which a binding prints once each on its JS thread.

use std::{cell::RefCell, collections::HashSet, sync::Once};

use log::{Level, LevelFilter, Log, Metadata, Record, kv::Key};

/// The prefix of every target takumi logs under.
const TARGET_PREFIX: &str = "takumi::";

/// The key a warning about one CSS property carries its name under, so that its warnings, at any
/// line, count as one.
const PROPERTY_KEY: &str = "property";

thread_local! {
  /// The keys of the warnings already printed on this thread.
  static PRINTED: RefCell<HashSet<String>> = RefCell::default();
}

/// A takumi warning, and the key a repeat of it shares.
pub struct Warning {
  pub key: String,
  pub message: String,
}

impl Warning {
  /// Whether no warning with this key printed on this thread before, the JS thread a binding prints
  /// on.
  pub fn is_first(&self) -> bool {
    PRINTED.with_borrow_mut(|printed| printed.insert(self.key.clone()))
  }
}

/// Hands each takumi warning to a binding's sink, on the thread that logs it.
pub struct WarningLog(fn(Warning));

impl WarningLog {
  /// Sends takumi's warnings to `sink` from now on, unless the process already has a logger.
  pub fn install(sink: fn(Warning)) {
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
    if !self.enabled(record.metadata()) {
      return;
    }

    let message = format!("takumi: {}", record.args());
    let key = match record.key_values().get(Key::from_str(PROPERTY_KEY)) {
      Some(property) => format!(
        "{PROPERTY_KEY}:{}",
        property.to_string().to_ascii_lowercase()
      ),
      None => message.clone(),
    };

    (self.0)(Warning { key, message });
  }

  fn flush(&self) {}
}

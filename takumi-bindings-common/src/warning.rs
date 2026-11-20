//! The warnings takumi logs, handed to a binding's console.

use std::sync::Once;

use log::{Level, LevelFilter, Log, Metadata, Record};
use takumi_core::resources::font::MISSING_FAMILY_LOG_TARGET;
#[cfg(feature = "wasm")]
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[cfg(feature = "wasm")]
#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(catch, js_namespace = console, js_name = warn)]
  fn console_warn(message: String) -> Result<(), JsValue>;
}

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

  /// Prints takumi's warnings with `console.warn` from now on, in a single-threaded wasm module.
  #[cfg(feature = "wasm")]
  pub fn print_to_console() {
    // A `console.warn` that throws must not fail the render that logged the warning.
    Self::install(|message| {
      let _ = console_warn(message);
    });
  }
}

impl Log for WarningLog {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    // A binding warns of unresolved families itself, from `RenderNeeds`.
    metadata.level() <= Level::Warn
      && metadata.target().starts_with(TARGET_PREFIX)
      && metadata.target() != MISSING_FAMILY_LOG_TARGET
  }

  fn log(&self, record: &Record<'_>) {
    if self.enabled(record.metadata()) {
      (self.0)(format!("takumi: {}", record.args()));
    }
  }

  fn flush(&self) {}
}

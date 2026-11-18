//! Takumi's warnings, printed with `console.warn` on the JS thread, once each.

use std::sync::OnceLock;

use napi::{
  Env,
  bindgen_prelude::*,
  threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use takumi_bindings_common::warning::{Warning, WarningLog};

/// Calls into the JS thread with a warning's key and message, without keeping its event loop alive.
type PrintWarning = ThreadsafeFunction<(String, String), (), (String, String), Status, false, true>;

/// The function warnings logged on any thread reach the JS thread of the first renderer through.
static PRINT: OnceLock<PrintWarning> = OnceLock::new();

/// Prints takumi's warnings, such as an unsupported CSS property, with `console.warn`.
pub(crate) fn forward_warnings(env: &Env) -> Result<()> {
  if PRINT.get().is_some() {
    return Ok(());
  }

  let print = env
    .create_function_from_closure("printTakumiWarning", |ctx| {
      let (key, message) = ctx.first_arg::<(String, String)>()?;
      let warning = Warning { key, message };

      // A `console.warn` that throws must not fail the render that logged the warning.
      if warning.is_first() {
        let _ = console_warn(ctx.env, warning.message);
      }

      Ok(())
    })?
    .build_threadsafe_function()
    .weak::<true>()
    .callee_handled::<false>()
    .build()?;

  if PRINT.set(print).is_ok() {
    WarningLog::install(send);
  }

  Ok(())
}

fn send(warning: Warning) {
  if let Some(print) = PRINT.get() {
    print.call(
      (warning.key, warning.message),
      ThreadsafeFunctionCallMode::NonBlocking,
    );
  }
}

fn console_warn(env: &Env, message: String) -> Result<()> {
  let console: Object = env.get_global()?.get_named_property("console")?;
  let warn: Function<String, Unknown> = console.get_named_property("warn")?;

  warn.apply(console, message).map(drop)
}

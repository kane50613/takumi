//! Takumi's warnings, printed with `console.warn` on the JS thread.

use std::sync::OnceLock;

use napi::{
  Env,
  bindgen_prelude::*,
  threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use takumi_bindings_common::warning::WarningLog;

/// Calls into the JS thread with a warning, without keeping its event loop alive.
type PrintWarning = ThreadsafeFunction<String, (), String, Status, false, true>;

/// The function warnings logged on any thread reach the JS thread of the first renderer through.
static PRINT: OnceLock<PrintWarning> = OnceLock::new();

/// Prints takumi's warnings, such as an unsupported CSS property, with `console.warn`.
pub(crate) fn forward_warnings(env: &Env) -> Result<()> {
  if PRINT.get().is_some() {
    return Ok(());
  }

  let print = env
    .create_function_from_closure("printTakumiWarning", |ctx| {
      // A `console.warn` that throws must not fail the render that logged the warning.
      let _ = console_warn(ctx.env, ctx.first_arg()?);

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

fn send(message: String) {
  if let Some(print) = PRINT.get() {
    print.call(message, ThreadsafeFunctionCallMode::NonBlocking);
  }
}

fn console_warn(env: &Env, message: String) -> Result<()> {
  let console: Object = env.get_global()?.get_named_property("console")?;
  let warn: Function<String, Unknown> = console.get_named_property("warn")?;

  warn.apply(console, message).map(drop)
}

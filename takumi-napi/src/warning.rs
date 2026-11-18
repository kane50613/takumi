//! Takumi's warnings, printed with `console.warn` on the JS thread.

use std::{ptr, sync::Arc};

use arc_swap::ArcSwapOption;
use napi::{
  Env,
  bindgen_prelude::*,
  sys,
  threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use takumi_bindings_common::warning::WarningLog;

/// Calls into the JS thread with a warning, without keeping its event loop alive.
type PrintWarning = ThreadsafeFunction<String, (), String, Status, false, true>;

/// The function warnings logged on any thread reach a live JS thread through.
static PRINT: ArcSwapOption<PrintWarning> = ArcSwapOption::const_empty();

/// Prints takumi's warnings, such as an unsupported CSS property, with `console.warn`.
pub(crate) fn forward_warnings(env: &Env) -> Result<()> {
  if PRINT.load().is_some() {
    return Ok(());
  }

  let print = Arc::new(
    env
      .create_function_from_closure("printTakumiWarning", |ctx| {
        // A `console.warn` that throws must not fail the render that logged the warning.
        let _ = console_warn(ctx.env, ctx.first_arg()?);

        Ok(())
      })?
      .build_threadsafe_function()
      .weak::<true>()
      .callee_handled::<false>()
      .build()?,
  );

  PRINT.store(Some(Arc::clone(&print)));
  // An exiting environment, such as a worker thread, takes its function back, so the next renderer
  // in a live one installs its own.
  env.add_env_cleanup_hook(Some(print), |print| {
    PRINT.compare_and_swap(&print, None);
  })?;
  WarningLog::install(send);

  Ok(())
}

fn send(message: String) {
  if let Some(print) = PRINT.load().as_ref() {
    print.call(message, ThreadsafeFunctionCallMode::NonBlocking);
  }
}

fn console_warn(env: &Env, message: String) -> Result<()> {
  let console: Object = env.get_global()?.get_named_property("console")?;
  let warn: Function<String, Unknown> = console.get_named_property("warn")?;

  if let Err(error) = warn.apply(console, message) {
    let mut exception = ptr::null_mut();

    // SAFETY: `env` is the live environment of this JS thread, where the failed call left its
    // exception pending.
    unsafe { sys::napi_get_and_clear_last_exception(env.raw(), &mut exception) };

    return Err(error);
  }

  Ok(())
}

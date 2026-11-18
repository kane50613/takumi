//! Takumi's warnings, printed with `console.warn` on the JS thread once a task settles.

use std::{mem::take, sync::Mutex};

use napi::{Env, bindgen_prelude::*};
use takumi_bindings_common::warning::WarningLog;

/// The warnings logged on worker threads that no task has printed yet.
static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn queue(message: String) {
  if let Ok(mut pending) = PENDING.lock() {
    pending.push(message);
  }
}

/// Queues takumi's warnings, such as an unsupported CSS property, for the next task to print.
pub(crate) fn forward_warnings() {
  WarningLog::install(queue);
}

/// Prints the queued warnings with `console.warn`.
fn print_warnings(env: &Env) -> Result<()> {
  let pending = PENDING
    .lock()
    .map(|mut pending| take(&mut *pending))
    .unwrap_or_default();

  if pending.is_empty() {
    return Ok(());
  }

  let console: Object = env.get_global()?.get_named_property("console")?;
  let warn: Function<String, Unknown> = console.get_named_property("warn")?;

  for message in pending {
    warn.apply(console, message)?;
  }

  Ok(())
}

/// A task that prints the warnings takumi logged once it settles.
pub struct Warned<T>(pub(crate) T);

impl<T: Task> Task for Warned<T> {
  type Output = T::Output;
  type JsValue = T::JsValue;

  fn compute(&mut self) -> Result<Self::Output> {
    self.0.compute()
  }

  fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
    self.0.resolve(env, output)
  }

  fn reject(&mut self, env: Env, error: Error) -> Result<Self::JsValue> {
    self.0.reject(env, error)
  }

  fn finally(self, env: Env) -> Result<()> {
    print_warnings(&env)?;
    self.0.finally(env)
  }
}

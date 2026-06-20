//! An owned rayon thread pool that is joined on N-API teardown.
//!
//! rayon's global pool spawns detached daemon workers and never joins them. On
//! Windows, `ExitProcess` force-terminates them mid-flight during process exit,
//! faulting with `0xC0000005` (#763). This pool keeps its workers'
//! [`JoinHandle`]s and joins them from an env cleanup hook, so none are alive
//! when the process exits. All parallel work runs through [`install`].

use std::{
  sync::{Arc, Mutex, OnceLock},
  thread::{Builder, JoinHandle},
};

use napi::Env;
use rayon::{ThreadPool, ThreadPoolBuilder};

struct Shared {
  pool: Option<Arc<ThreadPool>>,
  handles: Vec<JoinHandle<()>>,
}

static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();

/// Builds an owned pool whose worker `JoinHandle`s are retained for joining.
fn build() -> Shared {
  let mut handles = Vec::new();

  // `spawn_handler` is `FnMut` and called synchronously during `build`, so the
  // handles can be collected without interior mutability.
  let pool = ThreadPoolBuilder::new()
    .spawn_handler(|thread| {
      let mut builder = Builder::new();
      if let Some(name) = thread.name() {
        builder = builder.name(name.to_owned());
      }
      if let Some(size) = thread.stack_size() {
        builder = builder.stack_size(size);
      }
      handles.push(builder.spawn(|| thread.run())?);
      Ok(())
    })
    .build()
    .ok()
    .map(Arc::new);

  Shared { pool, handles }
}

/// Returns a clone of the current pool, rebuilding it if a previous N-API
/// environment shut it down on its own teardown.
fn pool() -> Option<Arc<ThreadPool>> {
  let cell = SHARED.get_or_init(|| Mutex::new(build()));
  let mut shared = cell.lock().ok()?;
  if shared.pool.is_none() {
    *shared = build();
  }
  shared.pool.clone()
}

/// Runs `op` with all nested rayon work dispatched to the owned pool.
pub(crate) fn install<OP, R>(op: OP) -> R
where
  OP: FnOnce() -> R + Send,
  R: Send,
{
  match pool() {
    Some(pool) => pool.install(op),
    None => op(),
  }
}

/// Joins the pool's workers so none outlive the N-API environment.
fn shutdown() {
  let Some(cell) = SHARED.get() else { return };
  let handles = {
    let Ok(mut shared) = cell.lock() else { return };
    // Drop our pool reference so `terminate()` signals the workers; with no
    // work in flight at teardown this is the last reference.
    shared.pool.take();
    std::mem::take(&mut shared.handles)
  };
  for handle in handles {
    let _ = handle.join();
  }
}

/// Joins the pool when the calling N-API environment tears down.
///
/// Registered per `Renderer`, not deduplicated: [`shutdown`] is idempotent, and
/// registering on every environment that builds a pool is what keeps a pool
/// rebuilt by one environment from outliving another's teardown.
pub(crate) fn register_cleanup(env: &Env) {
  let _ = env.add_env_cleanup_hook((), |_| shutdown());
}

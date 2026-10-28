//! An owned rayon thread pool that is joined on N-API teardown.
//!
//! rayon's global pool spawns detached daemon workers and never joins them. On
//! Windows, `ExitProcess` force-terminates them mid-flight during process exit,
//! faulting with `0xC0000005` (#763). This pool keeps its workers'
//! [`JoinHandle`]s and joins them from an env cleanup hook, so none are alive
//! when the process exits. All parallel work runs through [`install`].

use std::{
  sync::Arc,
  thread::{Builder, JoinHandle, yield_now},
};

use arc_swap::ArcSwapOption;
use napi::Env;
use rayon::{ThreadPool, ThreadPoolBuilder};

/// The pool and its workers' handles, joined once the pool is dropped.
struct Pool {
  pool: ThreadPool,
  handles: Vec<JoinHandle<()>>,
}

impl Pool {
  /// Builds an owned pool whose worker `JoinHandle`s are retained for joining.
  fn build() -> Option<Self> {
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
      .ok()?;

    Some(Self { pool, handles })
  }

  /// Drops the pool so its workers exit, then joins them.
  fn join(self) {
    drop(self.pool);
    for handle in self.handles {
      let _ = handle.join();
    }
  }
}

static SHARED: ArcSwapOption<Pool> = ArcSwapOption::const_empty();

/// The current pool, built again if a previous N-API environment shut it down on its own
/// teardown. Of two threads building at once, the one that loses joins its own pool.
fn pool() -> Option<Arc<Pool>> {
  if let Some(pool) = SHARED.load_full() {
    return Some(pool);
  }

  let built = Arc::new(Pool::build()?);
  let previous = SHARED.compare_and_swap(&None::<Arc<Pool>>, Some(Arc::clone(&built)));

  match &*previous {
    None => Some(built),
    Some(current) => {
      let current = Arc::clone(current);

      if let Ok(built) = Arc::try_unwrap(built) {
        built.join();
      }
      Some(current)
    }
  }
}

/// Runs `op` with all nested rayon work dispatched to the owned pool.
pub(crate) fn install<OP, R>(op: OP) -> R
where
  OP: FnOnce() -> R + Send,
  R: Send,
{
  match pool() {
    Some(pool) => pool.pool.install(op),
    None => op(),
  }
}

/// Joins the pool's workers so none outlive the N-API environment, first waiting for any work
/// still in flight to let go of the pool.
fn shutdown() {
  let Some(mut pool) = SHARED.swap(None) else {
    return;
  };

  loop {
    match Arc::try_unwrap(pool) {
      Ok(pool) => return pool.join(),
      Err(shared) => {
        pool = shared;
        yield_now();
      }
    }
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

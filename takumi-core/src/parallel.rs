//! Whether a render spreads its own work across the thread pool.
//!
//! A render with the machine to itself blurs and deflates on several cores and takes about half
//! the time. Renders that already fill the cores gain nothing from that and pay for the handoff:
//! on 14 cores with 14 renders at once, keeping each on one thread drew 4% to 30% more a second
//! and cut the slowest by a third, and it broke even with 10 renders at once, where at 8 spreading
//! still drew a tenth more. So a render's work spreads only while the renders in flight leave a
//! third of the pool idle. What a render draws never depends on it.

use std::{
  cell::Cell,
  marker::PhantomData,
  sync::atomic::{AtomicUsize, Ordering},
};

/// Renders in flight across the process.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
  /// The [`ActiveRender`]s this thread holds, so a render counts once however its calls nest.
  static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// A render in flight on this thread, counted from [`begin`](Self::begin) until it drops.
#[must_use = "a render counts only while this is held"]
pub struct ActiveRender {
  /// Whether this is the thread's outermost, the one the process count holds.
  counted: bool,
  /// The count is the thread's, so the render ends on the thread it began on.
  thread: PhantomData<*const ()>,
}

impl ActiveRender {
  /// Counts a render on this thread. A thread already rendering is not counted twice.
  pub fn begin() -> Self {
    let counted = DEPTH.with(|depth| {
      let held = depth.get();

      depth.set(held + 1);
      held == 0
    });

    if counted {
      ACTIVE.fetch_add(1, Ordering::Relaxed);
    }

    Self {
      counted,
      thread: PhantomData,
    }
  }
}

impl Drop for ActiveRender {
  fn drop(&mut self) {
    DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    if self.counted {
      ACTIVE.fetch_sub(1, Ordering::Relaxed);
    }
  }
}

/// Whether work that can run on one thread or many should take many: while the renders in flight
/// leave a third of the pool's threads idle.
pub fn spreads() -> bool {
  #[cfg(feature = "rayon")]
  {
    spreads_across(ACTIVE.load(Ordering::Relaxed), rayon::current_num_threads())
  }
  #[cfg(not(feature = "rayon"))]
  {
    false
  }
}

/// Whether `active` renders leave a third of `threads` idle.
#[cfg_attr(not(any(feature = "rayon", test)), allow(dead_code))]
fn spreads_across(active: usize, threads: usize) -> bool {
  active.saturating_mul(3) <= threads.saturating_mul(2)
}

#[cfg(test)]
mod tests {
  use super::{ACTIVE, ActiveRender, DEPTH, Ordering, spreads_across};

  #[test]
  fn work_spreads_while_a_third_of_the_pool_is_idle() {
    // No render counted, as a blur outside one, spreads.
    assert!(spreads_across(0, 1));
    assert!(!spreads_across(1, 1));
    assert!(spreads_across(1, 2));
    assert!(!spreads_across(2, 2));
    assert!(spreads_across(2, 4));
    assert!(!spreads_across(3, 4));
    assert!(spreads_across(9, 14));
    assert!(!spreads_across(10, 14));
  }

  #[test]
  fn nested_renders_count_once() {
    // Other tests render on their own threads, so only this thread's depth is exact.
    let outer = ActiveRender::begin();
    let inner = ActiveRender::begin();

    assert!(outer.counted && !inner.counted);
    drop(inner);
    assert!(ACTIVE.load(Ordering::Relaxed) >= 1);
    drop(outer);
    assert_eq!(DEPTH.get(), 0, "the thread still holds a render");

    let again = ActiveRender::begin();

    assert!(again.counted);
    drop(again);
  }
}

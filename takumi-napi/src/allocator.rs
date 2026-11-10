//! The allocator the binding uses on musl, and how long it keeps memory it was handed back.
//!
//! musl's own allocator is the slow path of every render there: see this crate's `Cargo.toml`.

use std::{ffi::c_long, sync::Once};

use libmimalloc_sys::{mi_option_set_default, mi_option_t};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// `mi_option_purge_delay` in mimalloc's `mi_option_t`, which `libmimalloc-sys` leaves out. It
/// sits between `mi_option_eager_commit_delay` (14) and `mi_option_use_numa_nodes` (16) in
/// mimalloc 2 and 3 alike.
const PURGE_DELAY: mi_option_t = 15;

/// Milliseconds freed memory waits before it goes back to the system. Rendering a card four at a
/// time drew 576 a second holding 244MiB at mimalloc's own second, 546 holding 178MiB at this, and
/// 515 holding 161MiB at none.
const PURGE_DELAY_MS: c_long = 2;

/// Sets how long freed memory waits, unless `MIMALLOC_PURGE_DELAY` already says.
pub(crate) fn tune() {
  static TUNED: Once = Once::new();

  TUNED.call_once(|| {
    // SAFETY: `PURGE_DELAY` is an option of the mimalloc this crate links, and a default leaves a
    // value the environment set alone.
    unsafe { mi_option_set_default(PURGE_DELAY, PURGE_DELAY_MS) };
  });
}

//! A process-global cache of shaped text-only inline layouts.

#[cfg(test)]
use std::cell::Cell;
use std::{
  hash::{Hash, Hasher},
  sync::{
    Arc, LazyLock,
    atomic::{AtomicUsize, Ordering},
  },
};

use quick_cache::{Weighter, sync::Cache};
use xxhash_rust::xxh3::Xxh3;

use crate::layout::inline::InlineLayout;

const DEFAULT_SHAPE_CACHE_MAX_BYTES: usize = 4 << 20; // 4 MiB
const ESTIMATED_BYTES_PER_CHAR: usize = 32;
const ENTRY_OVERHEAD: usize = 128;
const AVERAGE_ENTRY_CHARS: usize = 64;

static MAX_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_SHAPE_CACHE_MAX_BYTES);

/// Sets the byte budget for the shape cache. `0` stops caching. Call before the first render.
pub fn set_shape_cache_max_bytes(bytes: usize) {
  MAX_BYTES.store(bytes, Ordering::Relaxed);
}

type Entry = Arc<(InlineLayout, String)>;

#[derive(Clone)]
struct ByBytes;

impl Weighter<u64, Entry> for ByBytes {
  fn weight(&self, _key: &u64, entry: &Entry) -> u64 {
    let (_, text) = entry.as_ref();

    (text.len() * ESTIMATED_BYTES_PER_CHAR + ENTRY_OVERHEAD) as u64
  }
}

static SHARED: LazyLock<Cache<u64, Entry, ByBytes>> = LazyLock::new(|| {
  let max_bytes = MAX_BYTES.load(Ordering::Relaxed) as u64;
  let average_entry =
    (AVERAGE_ENTRY_CHARS * ESTIMATED_BYTES_PER_CHAR + ENTRY_OVERHEAD).max(1) as u64;
  let estimated_items = (max_bytes / average_entry).max(1) as usize;

  Cache::with_weighter(estimated_items, max_bytes, ByBytes)
});

/// What a shape-cache key is computed from, named so the three `u64`s can't be swapped at a call site.
pub(crate) struct CacheKeyParts {
  pub(crate) fingerprint: u64,
  pub(crate) revision: u64,
  pub(crate) fallback_signature: u64,
}

fn key(parts: CacheKeyParts) -> u64 {
  let mut hasher = Xxh3::new();

  parts.fingerprint.hash(&mut hasher);
  parts.revision.hash(&mut hasher);
  parts.fallback_signature.hash(&mut hasher);
  hasher.finish()
}

/// `expected`'s cached layout under `parts`, shaping and inserting it via `shape` on a miss.
pub(crate) fn get_or_shape(
  parts: CacheKeyParts,
  expected: &str,
  shape: impl FnOnce() -> (InlineLayout, String),
) -> (InlineLayout, String) {
  let cache_key = key(parts);

  if let Some(entry) = SHARED.get(&cache_key) {
    let (layout, text) = entry.as_ref();
    if text == expected {
      return (layout.clone(), text.clone());
    }
  }

  let shaped = shape();

  #[cfg(test)]
  LAST_INSERTED_KEY.with(|cell| cell.set(Some(cache_key)));

  SHARED.insert(cache_key, Arc::new(shaped.clone()));
  shaped
}

#[cfg(test)]
thread_local! {
  static LAST_INSERTED_KEY: Cell<Option<u64>> = const { Cell::new(None) };
}

/// The key this thread last inserted, or `None` since. Test-only; caller and render must share a thread.
#[cfg(test)]
pub(crate) fn take_last_inserted_key_for_test() -> Option<u64> {
  LAST_INSERTED_KEY.with(|cell| cell.take())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
  use super::*;

  fn parts(fingerprint: u64, revision: u64, fallback_signature: u64) -> CacheKeyParts {
    CacheKeyParts {
      fingerprint,
      revision,
      fallback_signature,
    }
  }

  #[test]
  fn a_cache_hit_returns_the_stored_entry_when_the_text_matches() {
    get_or_shape(parts(1, 1, 1), "hello", || {
      (InlineLayout::new(), "hello".to_owned())
    });

    let (_, text) = get_or_shape(parts(1, 1, 1), "hello", || panic!("must hit, not reshape"));
    assert_eq!(text, "hello");
  }

  #[test]
  fn a_text_mismatch_is_a_miss_even_on_a_fingerprint_collision() {
    get_or_shape(parts(2, 1, 1), "hello", || {
      (InlineLayout::new(), "hello".to_owned())
    });

    let (_, text) = get_or_shape(parts(2, 1, 1), "goodbye", || {
      (InlineLayout::new(), "goodbye".to_owned())
    });
    assert_eq!(text, "goodbye");
  }

  #[test]
  fn different_revisions_never_collide() {
    get_or_shape(parts(3, 1, 1), "hello", || {
      (InlineLayout::new(), "hello".to_owned())
    });

    let (_, text) = get_or_shape(parts(3, 2, 1), "hello", || {
      (InlineLayout::new(), "reshaped".to_owned())
    });
    assert_eq!(text, "reshaped");
  }

  #[test]
  fn different_fallback_signatures_never_collide() {
    get_or_shape(parts(4, 1, 1), "hello", || {
      (InlineLayout::new(), "hello".to_owned())
    });

    let (_, text) = get_or_shape(parts(4, 1, 2), "hello", || {
      (InlineLayout::new(), "reshaped".to_owned())
    });
    assert_eq!(text, "reshaped");
  }

  #[test]
  fn weight_scales_with_text_length() {
    let weighter = ByBytes;
    let short = weighter.weight(&0, &Arc::new((InlineLayout::new(), "hi".to_owned())));
    let long = weighter.weight(&0, &Arc::new((InlineLayout::new(), "x".repeat(1000))));

    assert!(long > short);
  }
}

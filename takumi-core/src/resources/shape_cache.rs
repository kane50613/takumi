//! A process-global cache of shaped text-only inline layouts, keyed by
//! content fingerprint plus the font registry's revision and resolved
//! fallback chain, so a stale entry is unreachable rather than explicitly
//! invalidated.

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

/// Sets the byte budget for the shape cache. `0` stops caching. Takes effect
/// for a cache not yet used; call it before the first render. Defaults to 4 MiB.
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

/// The key for `fingerprint` under `revision` and `fallback_signature`.
pub(crate) fn key(fingerprint: u64, revision: u64, fallback_signature: u64) -> u64 {
  let mut hasher = Xxh3::new();

  fingerprint.hash(&mut hasher);
  revision.hash(&mut hasher);
  fallback_signature.hash(&mut hasher);
  hasher.finish()
}

/// The cached layout for `key`, if its stored text still matches `expected`.
pub(crate) fn get(key: u64, expected: &str) -> Option<(InlineLayout, String)> {
  let entry = SHARED.get(&key)?;
  let (layout, text) = entry.as_ref();

  (text == expected).then(|| (layout.clone(), text.clone()))
}

pub(crate) fn insert(key: u64, shaped: (InlineLayout, String)) {
  #[cfg(test)]
  LAST_INSERTED_KEY.with(|cell| cell.set(Some(key)));

  SHARED.insert(key, Arc::new(shaped));
}

// Per-thread: this cache is global and shared by every concurrent render.
#[cfg(test)]
thread_local! {
  static LAST_INSERTED_KEY: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// The key this thread last inserted, or `None` since the last call. Test-only.
#[cfg(test)]
pub(crate) fn take_last_inserted_key_for_test() -> Option<u64> {
  LAST_INSERTED_KEY.with(|cell| cell.take())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
  use super::*;

  #[test]
  fn a_cache_hit_returns_the_stored_entry_when_the_text_matches() {
    let cache_key = key(1, 1, 1);
    insert(cache_key, (InlineLayout::new(), "hello".to_owned()));

    let (_, text) = get(cache_key, "hello").unwrap();
    assert_eq!(text, "hello");
  }

  #[test]
  fn a_text_mismatch_is_a_miss_even_on_a_fingerprint_collision() {
    let cache_key = key(2, 1, 1);
    insert(cache_key, (InlineLayout::new(), "hello".to_owned()));

    assert!(get(cache_key, "goodbye").is_none());
  }

  #[test]
  fn different_revisions_never_collide() {
    let fingerprint = 3;
    insert(
      key(fingerprint, 1, 1),
      (InlineLayout::new(), "hello".to_owned()),
    );

    assert!(get(key(fingerprint, 2, 1), "hello").is_none());
  }

  #[test]
  fn different_fallback_signatures_never_collide() {
    let fingerprint = 4;
    insert(
      key(fingerprint, 1, 1),
      (InlineLayout::new(), "hello".to_owned()),
    );

    assert!(get(key(fingerprint, 1, 2), "hello").is_none());
  }

  #[test]
  fn weight_scales_with_text_length() {
    let weighter = ByBytes;
    let short = weighter.weight(&0, &Arc::new((InlineLayout::new(), "hi".to_owned())));
    let long = weighter.weight(&0, &Arc::new((InlineLayout::new(), "x".repeat(1000))));

    assert!(long > short);
  }
}

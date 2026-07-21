//! Process-global glyph caches under one byte budget.
//!
//! Entries live in sharded concurrent caches shared by every worker thread, so
//! a glyph resolved on one thread is a hit on all of them and eviction is
//! global — no thread keeps a stale share the way per-thread maps did.

use std::sync::atomic::{AtomicUsize, Ordering};

use quick_cache::{Weighter, sync::Cache};

const DEFAULT_GLYPH_CACHE_MAX_BYTES: usize = 8 << 20; // 8 MiB

static MAX_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_GLYPH_CACHE_MAX_BYTES);

/// Sets the byte budget shared by the glyph caches. `0` stops caching. Takes
/// effect for caches not yet used; call it before the first render. Defaults
/// to 8 MiB.
pub fn set_glyph_cache_max_bytes(bytes: usize) {
  MAX_BYTES.store(bytes, Ordering::Relaxed);
}

/// Half of the configured budget: the mask and resolved-glyph caches each get
/// an equal share of the process-wide total.
pub fn glyph_cache_share_bytes() -> u64 {
  (MAX_BYTES.load(Ordering::Relaxed) / 2) as u64
}

#[derive(Clone)]
struct Entry<V> {
  value: V,
  bytes: u32,
}

#[derive(Clone)]
struct ByBytes;

impl<V: Clone> Weighter<u64, Entry<V>> for ByBytes {
  fn weight(&self, _key: &u64, entry: &Entry<V>) -> u64 {
    u64::from(entry.bytes).max(1)
  }
}

/// A weighted concurrent glyph cache: entries are charged the byte weight the
/// caller reports, and going over budget evicts cold entries globally.
pub struct GlyphCache<V: Clone> {
  cache: Cache<u64, Entry<V>, ByBytes>,
}

impl<V: Clone> GlyphCache<V> {
  /// Creates a cache holding at most `max_bytes` across its entries; `0`
  /// disables retention.
  pub fn new(max_bytes: u64) -> Self {
    // ~4 KiB average glyph entry ⇒ item-count hint for the budget.
    let estimated_items = (max_bytes / (4 << 10)).max(1) as usize;

    Self {
      cache: Cache::with_weighter(estimated_items, max_bytes, ByBytes),
    }
  }

  /// Returns a clone of the cached value.
  pub fn get(&self, key: u64) -> Option<V> {
    self.cache.get(&key).map(|entry| entry.value)
  }

  /// Caches `value` at `bytes` weight. Entries heavier than the whole budget
  /// are rejected by the cache rather than evicting everything else.
  pub fn insert(&self, key: u64, value: V, bytes: usize) {
    let entry = Entry {
      value,
      bytes: bytes.min(u32::MAX as usize) as u32,
    };

    self.cache.insert(key, entry);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn over_budget_evicts_down_to_the_ceiling() {
    let cache = GlyphCache::new(1024);

    for key in 0..8 {
      cache.insert(key, "entry", 300);
    }

    let live: usize = (0..8).filter(|key| cache.get(*key).is_some()).count();
    assert!((1..=3).contains(&live), "live entries: {live}");
  }

  #[test]
  fn zero_budget_disables_retention() {
    let cache = GlyphCache::new(0);

    cache.insert(1, "entry", 64);
    assert!(cache.get(1).is_none());
  }
}

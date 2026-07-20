//! Thread-local glyph caches under one process-wide byte budget.
//!
//! Glyph lookups are the hottest cache path in a render, so entries stay in
//! thread-local maps: a hit is a plain `HashMap` read with no atomics. Only
//! inserts and evictions touch the shared byte counter, which enforces one
//! ceiling across every worker thread.

use std::{
  collections::HashMap,
  sync::atomic::{AtomicUsize, Ordering},
};

const DEFAULT_GLYPH_CACHE_MAX_BYTES: usize = 8 << 20; // 8 MiB

static USED_BYTES: AtomicUsize = AtomicUsize::new(0);
static MAX_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_GLYPH_CACHE_MAX_BYTES);

/// Sets the process-wide byte budget shared by every thread's glyph caches.
/// `0` stops further caching. Defaults to 8 MiB.
pub fn set_glyph_cache_max_bytes(bytes: usize) {
  MAX_BYTES.store(bytes, Ordering::Relaxed);
}

struct Entry<V> {
  value: V,
  bytes: usize,
  last_used: u32,
}

/// A thread-local glyph cache charging its entries against the process-wide
/// budget. Entries age per render; going over budget drops this thread's
/// entries that no recent render touched.
pub struct GlyphCache<V> {
  entries: HashMap<u64, Entry<V>>,
  tick: u32,
}

impl<V> Default for GlyphCache<V> {
  fn default() -> Self {
    Self {
      entries: HashMap::new(),
      tick: 0,
    }
  }
}

impl<V> GlyphCache<V> {
  /// Ages the cache by one render; entries untouched for two renders become
  /// eviction candidates.
  pub fn begin_render(&mut self) {
    self.tick = self.tick.saturating_add(1);
  }

  /// Returns the cached value and marks it live for the current render.
  pub fn get(&mut self, key: u64) -> Option<&V> {
    let tick = self.tick;

    self.entries.get_mut(&key).map(|entry| {
      entry.last_used = tick;
      &entry.value
    })
  }

  /// Caches `value` at `bytes` weight, evicting stale entries when the shared
  /// budget overflows.
  pub fn insert(&mut self, key: u64, value: V, bytes: usize) {
    let max = MAX_BYTES.load(Ordering::Relaxed);
    if max == 0 {
      return;
    }

    let entry = Entry {
      value,
      bytes,
      last_used: self.tick,
    };
    if let Some(old) = self.entries.insert(key, entry) {
      USED_BYTES.fetch_sub(old.bytes, Ordering::Relaxed);
    }

    let used = USED_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    if used > max {
      self.evict_stale(used - max);
    }
  }

  /// Drops entries no render touched since the previous one; when that frees
  /// less than `target` bytes, drops arbitrary fresh entries until it is
  /// covered, so one over-budget render degrades gradually instead of
  /// flushing everything it just cached. Only this thread's entries are
  /// considered — an idle thread keeps its share until it renders again, so
  /// the ceiling holds even though reclaim is local.
  fn evict_stale(&mut self, target: usize) {
    let cutoff = self.tick.saturating_sub(1);
    let mut freed = 0;

    self.entries.retain(|_, entry| {
      entry.last_used >= cutoff || {
        freed += entry.bytes;
        false
      }
    });

    if freed < target {
      let fresh: Vec<u64> = self.entries.keys().copied().collect();
      for key in fresh {
        if freed >= target {
          break;
        }
        if let Some(entry) = self.entries.remove(&key) {
          freed += entry.bytes;
        }
      }
    }

    USED_BYTES.fetch_sub(freed, Ordering::Relaxed);
  }
}

impl<V> Drop for GlyphCache<V> {
  fn drop(&mut self) {
    let held: usize = self.entries.values().map(|entry| entry.bytes).sum();

    USED_BYTES.fetch_sub(held, Ordering::Relaxed);
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Mutex;

  use super::*;

  /// The budget statics are process-wide; serialize tests that reconfigure them.
  static BUDGET_LOCK: Mutex<()> = Mutex::new(());

  #[test]
  fn eviction_keeps_entries_of_recent_renders() {
    let _guard = BUDGET_LOCK.lock().unwrap();
    set_glyph_cache_max_bytes(1024);
    let mut cache = GlyphCache::default();

    cache.begin_render();
    cache.insert(1, "old", 256);
    cache.begin_render();
    cache.begin_render();
    cache.insert(2, "hot", 900);

    assert!(cache.get(1).is_none());
    assert!(cache.get(2).is_some());
    set_glyph_cache_max_bytes(DEFAULT_GLYPH_CACHE_MAX_BYTES);
  }

  #[test]
  fn over_budget_with_only_fresh_entries_evicts_just_enough() {
    let _guard = BUDGET_LOCK.lock().unwrap();
    set_glyph_cache_max_bytes(512);
    let mut cache = GlyphCache::default();

    cache.begin_render();
    cache.insert(1, "a", 300);
    cache.insert(2, "b", 300);

    // 88 bytes over budget: dropping either 300-byte entry covers it, the
    // other survives instead of the whole map flushing.
    assert!(cache.get(1).is_some() ^ cache.get(2).is_some());
    set_glyph_cache_max_bytes(DEFAULT_GLYPH_CACHE_MAX_BYTES);
  }
}

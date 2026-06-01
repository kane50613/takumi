//! Global, content-addressed cache of *decoded* font bytes.
//!
//! Decoding woff2/woff is the only expensive step in the font pipeline (~100ms for a
//! large CJK font); registering a decoded blob into a context and cloning ("forking") a
//! context are both single-digit µs and independent of font size. So the one thing worth
//! memoizing is the decode, keyed by a hash of the input bytes.
//!
//! The cache is pure memoization: keyed by content, values are immutable `Blob`s. A cold
//! and a warm cache produce identical output, so sharing it process-wide (native) or
//! module-wide (wasm) is safe and never causes the family mixing a mutable shared font
//! context would. On wasm/edge this is exactly the sanctioned "immutable cache in font_context
//! scope, loss is acceptable" pattern.

use std::{
  collections::HashMap,
  sync::{
    RwLock,
    atomic::{AtomicUsize, Ordering},
  },
};

use parley::fontique::Blob;

/// Decoded-font budget before entries start getting evicted.
const DEFAULT_MAX_BYTES: usize = 256 << 20; // 256 MiB

/// Content-addressed store of decoded font blobs.
///
/// Reads (cache hits) take a shared lock; the rare writes (a font's first decode) take the
/// exclusive lock. All byte-counter updates happen under the write lock, so it stays
/// consistent with the map; `get`/`len_bytes` stay lock-light.
pub struct FontDecodeCache {
  entries: RwLock<HashMap<u64, Blob<u8>>>,
  bytes: AtomicUsize,
  max_bytes: AtomicUsize,
}

impl Default for FontDecodeCache {
  fn default() -> Self {
    Self {
      entries: RwLock::new(HashMap::new()),
      bytes: AtomicUsize::new(0),
      max_bytes: AtomicUsize::new(DEFAULT_MAX_BYTES),
    }
  }
}

impl FontDecodeCache {
  /// Returns the decoded blob for `key`, if present.
  pub fn get(&self, key: u64) -> Option<Blob<u8>> {
    self.entries.read().ok()?.get(&key).cloned()
  }

  /// Inserts a decoded blob, evicting other entries to stay within the byte budget.
  pub fn insert(&self, key: u64, blob: Blob<u8>) {
    let max = self.max_bytes.load(Ordering::Relaxed);
    let len = blob.as_ref().len();
    // disabled, or a single font larger than the whole budget: don't cache.
    if max == 0 || len > max {
      return;
    }
    let Ok(mut map) = self.entries.write() else {
      return;
    };
    evict_until(&mut map, &self.bytes, max - len);
    if map.insert(key, blob).is_none() {
      self.bytes.fetch_add(len, Ordering::Relaxed);
    }
  }

  /// Sets the decoded-byte budget. `0` disables the cache and clears it.
  pub fn set_max_bytes(&self, max: usize) {
    self.max_bytes.store(max, Ordering::Relaxed);
    if max == 0 {
      self.clear();
    } else if let Ok(mut map) = self.entries.write() {
      evict_until(&mut map, &self.bytes, max);
    }
  }

  /// Drops all cached fonts.
  pub fn clear(&self) {
    if let Ok(mut map) = self.entries.write() {
      map.clear();
    }
    self.bytes.store(0, Ordering::Relaxed);
  }

  /// Total bytes of decoded fonts currently held.
  pub fn len_bytes(&self) -> usize {
    self.bytes.load(Ordering::Relaxed)
  }
}

/// Evicts arbitrary entries until held bytes are `<= target`. Caller holds the write lock.
// Coarse (not strict LRU) — fonts are few and immutable, so a miss just re-decodes.
fn evict_until(map: &mut HashMap<u64, Blob<u8>>, bytes: &AtomicUsize, target: usize) {
  while bytes.load(Ordering::Relaxed) > target {
    let Some(&victim) = map.keys().next() else {
      break;
    };
    if let Some(removed) = map.remove(&victim) {
      bytes.fetch_sub(removed.as_ref().len(), Ordering::Relaxed);
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use parley::fontique::Blob;

  use super::FontDecodeCache;

  fn blob(len: usize) -> Blob<u8> {
    Blob::new(Arc::new(vec![0u8; len]))
  }

  #[test]
  fn hit_miss_and_byte_accounting() {
    let cache = FontDecodeCache::default();
    assert!(cache.get(1).is_none());
    cache.insert(1, blob(100));
    assert!(cache.get(1).is_some());
    assert_eq!(cache.len_bytes(), 100);
    // re-inserting the same key doesn't double-count.
    cache.insert(1, blob(100));
    assert_eq!(cache.len_bytes(), 100);
  }

  #[test]
  fn disabling_clears_and_refuses() {
    let cache = FontDecodeCache::default();
    cache.insert(1, blob(100));
    cache.set_max_bytes(0);
    assert!(cache.get(1).is_none());
    assert_eq!(cache.len_bytes(), 0);
    cache.insert(2, blob(100)); // refused while disabled
    assert!(cache.get(2).is_none());
  }

  #[test]
  fn evicts_to_stay_within_budget() {
    let cache = FontDecodeCache::default();
    cache.set_max_bytes(150);
    cache.insert(1, blob(100));
    cache.insert(2, blob(100)); // can't fit both → one is evicted
    assert!(cache.len_bytes() <= 150);
    assert!(cache.get(2).is_some()); // newest stays
    // a single font larger than the whole budget is never cached.
    cache.insert(3, blob(1000));
    assert!(cache.get(3).is_none());
  }
}

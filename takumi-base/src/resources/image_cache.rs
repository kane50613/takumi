//! Global, content-addressed cache of *decoded* images.
//!
//! Decoding (png/jpeg/webp/gif/svg) is the expensive step; the decoded `ImageSource` is
//! Arc-backed, so a cache hit clones for a refcount bump. Like the font cache this is pure
//! memoization keyed by a hash of the input bytes — a cold and a warm cache render
//! identically, so sharing it process-wide (native) or module-wide (wasm) is safe.

use std::{
  collections::HashMap,
  sync::{
    RwLock,
    atomic::{AtomicUsize, Ordering},
  },
};

use xxhash_rust::xxh3::xxh3_64;

use crate::resources::image::{ImageResult, ImageSource};

/// Decoded-image budget before entries start getting evicted.
const DEFAULT_MAX_BYTES: usize = 64 << 20; // 64 MiB

/// Entry-count budget; unlimited by default (the byte budget is the real cap).
const DEFAULT_MAX_SIZE: usize = usize::MAX;

/// Content-addressed store of decoded images.
///
/// Reads (cache hits) take a shared lock; the rare writes (an image's first decode) take
/// the exclusive lock. All byte-counter updates happen under the write lock, so it stays
/// consistent with the map; `get`/`len_bytes` stay lock-light.
pub struct ImageCache {
  entries: RwLock<HashMap<u64, ImageSource>>,
  bytes: AtomicUsize,
  max_bytes: AtomicUsize,
  max_size: AtomicUsize,
}

impl Default for ImageCache {
  fn default() -> Self {
    Self {
      entries: RwLock::new(HashMap::new()),
      bytes: AtomicUsize::new(0),
      max_bytes: AtomicUsize::new(DEFAULT_MAX_BYTES),
      max_size: AtomicUsize::new(DEFAULT_MAX_SIZE),
    }
  }
}

impl ImageCache {
  /// Returns the decoded image for `bytes`, decoding and caching it on a miss.
  pub fn get_or_decode(&self, bytes: &[u8]) -> ImageResult {
    let key = xxh3_64(bytes);
    if let Some(source) = self.get(key) {
      return Ok(source);
    }
    let source = ImageSource::from_bytes(bytes)?;
    self.insert(key, source.clone());
    Ok(source)
  }

  /// Returns the decoded image for `key`, if present.
  pub fn get(&self, key: u64) -> Option<ImageSource> {
    self.entries.read().ok()?.get(&key).cloned()
  }

  /// Inserts a decoded image, evicting other entries to stay within both budgets.
  pub fn insert(&self, key: u64, source: ImageSource) {
    let max_bytes = self.max_bytes.load(Ordering::Relaxed);
    let max_size = self.max_size.load(Ordering::Relaxed);
    let len = source.estimated_bytes();
    // disabled, or a single image larger than the whole budget: don't cache.
    if max_bytes == 0 || max_size == 0 || len > max_bytes {
      return;
    }
    let Ok(mut map) = self.entries.write() else {
      return;
    };
    evict_until(&mut map, &self.bytes, max_bytes - len, max_size - 1);
    if map.insert(key, source).is_none() {
      self.bytes.fetch_add(len, Ordering::Relaxed);
    }
  }

  /// Sets the decoded-byte budget. `0` disables the cache and clears it.
  pub fn set_max_bytes(&self, max: usize) {
    self.max_bytes.store(max, Ordering::Relaxed);
    if max == 0 {
      self.clear();
    } else if let Ok(mut map) = self.entries.write() {
      evict_until(
        &mut map,
        &self.bytes,
        max,
        self.max_size.load(Ordering::Relaxed),
      );
    }
  }

  /// Sets the entry-count budget. `0` disables the cache and clears it.
  pub fn set_max_size(&self, max: usize) {
    self.max_size.store(max, Ordering::Relaxed);
    if max == 0 {
      self.clear();
    } else if let Ok(mut map) = self.entries.write() {
      evict_until(
        &mut map,
        &self.bytes,
        self.max_bytes.load(Ordering::Relaxed),
        max,
      );
    }
  }

  /// Drops all cached images.
  pub fn clear(&self) {
    if let Ok(mut map) = self.entries.write() {
      map.clear();
    }
    self.bytes.store(0, Ordering::Relaxed);
  }

  /// Total bytes of decoded images currently held.
  pub fn len_bytes(&self) -> usize {
    self.bytes.load(Ordering::Relaxed)
  }
}

/// Evicts arbitrary entries until held bytes are `<= max_bytes` and count is `<= max_size`.
/// Caller holds the write lock.
// Coarse (not strict LRU) — a miss just re-decodes.
fn evict_until(
  map: &mut HashMap<u64, ImageSource>,
  bytes: &AtomicUsize,
  max_bytes: usize,
  max_size: usize,
) {
  while bytes.load(Ordering::Relaxed) > max_bytes || map.len() > max_size {
    let Some(&victim) = map.keys().next() else {
      break;
    };
    if let Some(removed) = map.remove(&victim) {
      bytes.fetch_sub(removed.estimated_bytes(), Ordering::Relaxed);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::ImageCache;
  use crate::resources::{image::ImageSource, image_buffer::ImageBuffer};

  /// Builds a bitmap image of exactly `bytes` decoded size (premultiplied RGBA, 1px tall).
  fn image(bytes: usize) -> ImageSource {
    let width = (bytes / 4).max(1) as u32;
    ImageSource::from(ImageBuffer::new(width, 1).unwrap())
  }

  #[test]
  fn hit_miss_and_byte_accounting() {
    let cache = ImageCache::default();
    assert!(cache.get(1).is_none());
    cache.insert(1, image(100));
    assert!(cache.get(1).is_some());
    assert_eq!(cache.len_bytes(), 100);
    // re-inserting the same key doesn't double-count.
    cache.insert(1, image(100));
    assert_eq!(cache.len_bytes(), 100);
  }

  #[test]
  fn disabling_clears_and_refuses() {
    let cache = ImageCache::default();
    cache.insert(1, image(100));
    cache.set_max_bytes(0);
    assert!(cache.get(1).is_none());
    assert_eq!(cache.len_bytes(), 0);
    cache.insert(2, image(100)); // refused while disabled
    assert!(cache.get(2).is_none());
  }

  #[test]
  fn evicts_to_stay_within_budget() {
    let cache = ImageCache::default();
    cache.set_max_bytes(150);
    cache.insert(1, image(100));
    cache.insert(2, image(100)); // can't fit both → one is evicted
    assert!(cache.len_bytes() <= 150);
    assert!(cache.get(2).is_some()); // newest stays
    // a single image larger than the whole budget is never cached.
    cache.insert(3, image(1000));
    assert!(cache.get(3).is_none());
  }

  #[test]
  fn evicts_to_stay_within_entry_count() {
    let cache = ImageCache::default();
    cache.set_max_size(2);
    cache.insert(1, image(10));
    cache.insert(2, image(10));
    cache.insert(3, image(10)); // third entry evicts one of the first two
    assert!(cache.get(3).is_some()); // newest stays
    let held = [1, 2, 3]
      .iter()
      .filter(|&&k| cache.get(k).is_some())
      .count();
    assert_eq!(held, 2);
    // shrinking the budget evicts down to it.
    cache.set_max_size(1);
    let held = [1, 2, 3]
      .iter()
      .filter(|&&k| cache.get(k).is_some())
      .count();
    assert_eq!(held, 1);
    // disabling clears.
    cache.set_max_size(0);
    assert_eq!(cache.len_bytes(), 0);
  }
}

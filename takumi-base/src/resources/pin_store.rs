//! Per-renderer store of pinned decoded images, keyed by `src`.
//!
//! Unlike [`ImageCache`](crate::resources::image_cache::ImageCache) — which is content-addressed
//! and evictable — entries here are pinned for the renderer's lifetime so an image's bytes need
//! cross the FFI boundary only once. The interior `RwLock` mirrors `ImageCache`: it works under a
//! shared `&self` guard, compiles to wasm, and on the single-threaded wasm target never contends.

use std::{collections::HashMap, sync::Arc, sync::RwLock};

use crate::resources::image::ImageSource;

/// Per-renderer store of pinned decoded images, keyed by `src`.
#[derive(Default)]
pub struct PinStore {
  entries: RwLock<HashMap<Arc<str>, ImageSource>>,
}

impl PinStore {
  /// Pins a decoded image under `src`, replacing any previous entry.
  pub fn insert(&self, src: Arc<str>, image: ImageSource) {
    if let Ok(mut map) = self.entries.write() {
      map.insert(src, image);
    }
  }

  /// Clones each pinned entry into `out`, but only where the key is absent so
  /// caller-provided entries win.
  pub fn snapshot_into(&self, out: &mut HashMap<Arc<str>, ImageSource>) {
    let Ok(map) = self.entries.read() else {
      return;
    };
    for (src, image) in map.iter() {
      out.entry(src.clone()).or_insert_with(|| image.clone());
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{collections::HashMap, sync::Arc};

  use super::PinStore;
  use crate::resources::{image::ImageSource, image_buffer::ImageBuffer};

  fn image(width: u32) -> ImageSource {
    ImageSource::from(ImageBuffer::new(width, 1).unwrap())
  }

  #[test]
  fn snapshot_fills_absent_keys_only() {
    let store = PinStore::default();
    store.insert(Arc::from("a"), image(10));
    store.insert(Arc::from("b"), image(20));

    let mut out: HashMap<Arc<str>, ImageSource> = HashMap::new();
    let caller_a = image(99);
    out.insert(Arc::from("a"), caller_a.clone());
    store.snapshot_into(&mut out);

    // caller-provided "a" wins; "b" is pulled from the store.
    assert_eq!(out.len(), 2);
    assert_eq!(out["a"].estimated_bytes(), caller_a.estimated_bytes());
    assert_eq!(out["b"].estimated_bytes(), image(20).estimated_bytes());
  }

  #[test]
  fn insert_replaces_existing() {
    let store = PinStore::default();
    store.insert(Arc::from("a"), image(10));
    store.insert(Arc::from("a"), image(40));

    let mut out: HashMap<Arc<str>, ImageSource> = HashMap::new();
    store.snapshot_into(&mut out);

    assert_eq!(out.len(), 1);
    assert_eq!(out["a"].estimated_bytes(), image(40).estimated_bytes());
  }
}

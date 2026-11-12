//! Resolved glyphs and the masks rasterized from them and from shapes, sharing one byte budget so
//! the split follows the workload.

use std::sync::{
  Arc,
  atomic::{AtomicU64, Ordering},
};

use quick_cache::{
  Weighter,
  sync::{Cache as Store, DefaultLifecycle},
};
use rustc_hash::FxBuildHasher;

use crate::{
  geometry::Placement,
  resources::{cache::CacheOptions, glyph::ResolvedGlyph},
};

/// The glyph budget of caches built without one of their own.
static DEFAULT_MAX_BYTES: AtomicU64 = AtomicU64::new(CacheOptions::DEFAULT_GLYPHS);

/// Sets the glyph budget of the caches built after this call without one of their own, which
/// includes [`Cache::shared`](crate::Cache::shared) when nothing used it yet. `0` stops caching.
#[deprecated(note = "use `Cache::builder().glyphs(bytes)`")]
pub fn set_glyph_cache_max_bytes(bytes: usize) {
  DEFAULT_MAX_BYTES.store(bytes as u64, Ordering::Relaxed);
}

/// The glyph budget of a cache built without one of its own.
pub fn default_max_bytes() -> u64 {
  DEFAULT_MAX_BYTES.load(Ordering::Relaxed)
}

/// Bytes charged for an entry's own bookkeeping on top of its payload.
pub(crate) const ENTRY_OVERHEAD: usize = 64;

/// The share of the budget one entry may take: a 32nd, what one shard of the cache holds on an
/// eight-core machine, so a single raster cannot crowd out the glyphs.
const ENTRY_SHARE: u64 = 32;

#[derive(Clone)]
enum CachedGlyph {
  Resolved(Arc<ResolvedGlyph>),
  Mask(Arc<Vec<u8>>, Placement),
}

#[derive(Clone)]
struct Entry {
  value: CachedGlyph,
  bytes: u32,
}

/// Which stage a slot holds. Part of the key rather than a bit folded into it,
/// so a caller keeps its full 64 bits and the two stages cannot alias.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum GlyphKind {
  Resolved,
  Mask,
}

type Key = (u64, GlyphKind);

#[derive(Clone)]
struct ByBytes;

impl Weighter<Key, Entry> for ByBytes {
  fn weight(&self, _key: &Key, entry: &Entry) -> u64 {
    u64::from(entry.bytes).max(1)
  }
}

/// The glyphs and masks a [`Cache`](crate::Cache) keeps.
pub struct GlyphCache {
  store: Store<Key, Entry, ByBytes, FxBuildHasher>,
}

impl GlyphCache {
  /// A cache holding at most `max_bytes`.
  pub(crate) fn new(max_bytes: u64) -> Self {
    // ~4 KiB average entry ⇒ item-count hint for the budget.
    let estimated_items = (max_bytes / (4 << 10)).max(1) as usize;

    Self {
      store: Store::with(
        estimated_items,
        max_bytes,
        ByBytes,
        FxBuildHasher,
        DefaultLifecycle::default(),
      ),
    }
  }

  fn get_or_insert(
    &self,
    key: Key,
    f: impl FnOnce() -> Option<(CachedGlyph, usize)>,
  ) -> Option<CachedGlyph> {
    self
      .store
      .get_or_insert_with(&key, || {
        f()
          .map(|(value, bytes)| Entry {
            value,
            bytes: bytes.min(u32::MAX as usize) as u32,
          })
          .ok_or(())
      })
      .ok()
      .map(|entry| entry.value)
  }

  /// The resolved outline or bitmap for `key`, resolved once across concurrent misses.
  pub(crate) fn resolved_glyph(
    &self,
    key: u64,
    resolve: impl Fn() -> Option<ResolvedGlyph>,
  ) -> Option<Arc<ResolvedGlyph>> {
    let cached = self.get_or_insert((key, GlyphKind::Resolved), || {
      resolve().map(|glyph| {
        let bytes = glyph.estimated_bytes();

        (CachedGlyph::Resolved(Arc::new(glyph)), bytes)
      })
    });

    match cached {
      Some(CachedGlyph::Resolved(glyph)) => Some(glyph),
      _ => None,
    }
  }

  /// The most bytes one cached mask should take, a fixed share of the budget.
  pub fn max_entry_bytes(&self) -> u64 {
    self.store.capacity() / ENTRY_SHARE
  }

  /// The rasterized mask for `key`, rendered once across concurrent misses.
  pub fn glyph_mask(
    &self,
    key: u64,
    render: impl Fn() -> (Vec<u8>, Placement),
  ) -> (Arc<Vec<u8>>, Placement) {
    let cached = self.get_or_insert((key, GlyphKind::Mask), || {
      let (mask, placement) = render();
      let bytes = mask.capacity() + ENTRY_OVERHEAD;

      Some((CachedGlyph::Mask(Arc::new(mask), placement), bytes))
    });

    match cached {
      Some(CachedGlyph::Mask(mask, placement)) => (mask, placement),
      _ => {
        let (mask, placement) = render();

        (Arc::new(mask), placement)
      }
    }
  }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
  use super::*;
  use crate::resources::glyph::ResolvedOutlineGlyph;

  fn outline(cache_signature: u64) -> ResolvedGlyph {
    ResolvedGlyph::Outline(ResolvedOutlineGlyph::Plain {
      paths: Vec::new(),
      embolden: None,
      cache_signature,
      ink: None,
    })
  }

  fn signature_of(glyph: &ResolvedGlyph) -> Option<u64> {
    match glyph {
      ResolvedGlyph::Outline(outline) => Some(outline.cache_signature()),
      ResolvedGlyph::Bitmap(_) => None,
    }
  }

  fn placement() -> Placement {
    Placement {
      left: 0,
      top: 0,
      width: 1,
      height: 1,
    }
  }

  fn cache() -> GlyphCache {
    GlyphCache::new(8 << 20)
  }

  #[test]
  fn the_two_kinds_do_not_alias_on_the_same_key() {
    let cache = cache();
    let key = 0x5eed_1234_5678_9abc;

    let mask = cache.glyph_mask(key, || (vec![7; 4], placement()));
    let resolved = cache.resolved_glyph(key, || Some(outline(42))).unwrap();

    // Same raw key, disjoint entries: the mask must survive resolving, and each
    // side must get back its own kind.
    assert_eq!(*mask.0, vec![7; 4]);
    assert_eq!(signature_of(&resolved), Some(42));
    assert_eq!(
      *cache.glyph_mask(key, || (vec![0; 4], placement())).0,
      vec![7; 4]
    );
  }

  #[test]
  fn the_whole_key_is_kept() {
    let cache = cache();
    // Two keys that differ only in the top bit, which a key packed into a shifted
    // u64 would have dropped.
    let low = cache.resolved_glyph(0, || Some(outline(1))).unwrap();
    let high = cache.resolved_glyph(1 << 63, || Some(outline(2))).unwrap();

    assert_eq!(signature_of(&low), Some(1));
    assert_eq!(signature_of(&high), Some(2));
  }

  #[test]
  fn a_failed_resolve_caches_nothing_and_retries() {
    let cache = cache();
    let key = 0xfa11_0000_0000_0001;

    assert!(cache.resolved_glyph(key, || None).is_none());
    let resolved = cache.resolved_glyph(key, || Some(outline(7))).unwrap();
    assert_eq!(signature_of(&resolved), Some(7));
  }
}

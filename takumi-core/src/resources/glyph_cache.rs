//! Resolved glyphs and the masks rasterized from them and from shapes, as one kind of data in a
//! [`Cache`](crate::Cache)'s pool.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use crate::{
  geometry::Placement,
  resources::{
    cache::{ENTRY_OVERHEAD, KindStore, Weighed},
    glyph::ResolvedGlyph,
    image::ImageCacheMode,
  },
};

/// Whether caches built without a glyph mode of their own keep glyphs.
static KEEPS_GLYPHS: AtomicBool = AtomicBool::new(true);

/// Keeps no glyphs in the caches built after this call without a glyph mode of their own when
/// `bytes` is `0`, which includes [`Cache::shared`](crate::Cache::shared) when nothing used it
/// yet. Any other value keeps them, in the pool every kind shares.
#[deprecated(note = "use `CacheOptions::builder().glyphs(ImageCacheMode::None)`")]
pub fn set_glyph_cache_max_bytes(bytes: usize) {
  KEEPS_GLYPHS.store(bytes > 0, Ordering::Relaxed);
}

/// The glyph mode of a cache built without one of its own.
pub(crate) fn default_mode() -> ImageCacheMode {
  if KEEPS_GLYPHS.load(Ordering::Relaxed) {
    ImageCacheMode::Auto
  } else {
    ImageCacheMode::None
  }
}

/// The share of the glyph bytes one entry may take: a 32nd, what one shard of the cache holds on
/// an eight-core machine, so a single raster cannot crowd out the glyphs.
const ENTRY_SHARE: u64 = 32;

/// The glyph bytes an entry's share is taken from, the old default glyph budget.
const GLYPH_BYTES: u64 = 8 << 20;

#[derive(Clone)]
pub(crate) enum CachedGlyph {
  Resolved(Arc<ResolvedGlyph>),
  Mask(Arc<Vec<u8>>, Placement),
}

/// Which stage a slot holds. Part of the key rather than a bit folded into it,
/// so a caller keeps its full 64 bits and the two stages cannot alias.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GlyphKind {
  Resolved,
  Mask,
}

pub(crate) type GlyphKey = (u64, GlyphKind);

/// The glyphs and masks a [`Cache`](crate::Cache) keeps.
#[doc(hidden)]
pub struct GlyphCache {
  store: KindStore<CachedGlyph>,
}

impl GlyphCache {
  pub(crate) fn new(store: KindStore<CachedGlyph>) -> Self {
    Self { store }
  }

  fn get_or_insert(
    &self,
    key: GlyphKey,
    f: impl FnOnce() -> Option<(CachedGlyph, usize)>,
  ) -> Option<CachedGlyph> {
    self
      .store
      .get_or_insert_with(key, || {
        f()
          .map(|(value, bytes)| Weighed::new(value, bytes))
          .ok_or(())
      })
      .ok()
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

  /// The most bytes one cached mask should take, a fixed share of the pool up to
  /// [`GLYPH_BYTES`].
  pub fn max_entry_bytes(&self) -> u64 {
    self.store.max_bytes().min(GLYPH_BYTES) / ENTRY_SHARE
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
  use crate::{Cache, resources::glyph::ResolvedOutlineGlyph};

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

  fn cache() -> Cache {
    Cache::default()
  }

  #[test]
  fn the_two_kinds_do_not_alias_on_the_same_key() {
    let cache = cache();
    let cache = cache.glyphs();
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
    let cache = cache.glyphs();
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
    let cache = cache.glyphs();
    let key = 0xfa11_0000_0000_0001;

    assert!(cache.resolved_glyph(key, || None).is_none());
    let resolved = cache.resolved_glyph(key, || Some(outline(7))).unwrap();
    assert_eq!(signature_of(&resolved), Some(7));
  }
}

//! One process-global cache for glyph work, under a single byte budget.
//!
//! Resolved outlines and the masks rasterized from them are two stages of the
//! same pipeline, and their byte ratio is set by the content: a 24px CJK outline
//! outweighs its mask about six to one, while a 96px heading inverts that. They
//! share one pool so the split follows the workload instead of a fixed fraction.
//!
//! The pool is sharded and shared by every worker thread, so a glyph resolved on
//! one thread is a hit on all of them and eviction is global.

use std::sync::{
  Arc, LazyLock,
  atomic::{AtomicUsize, Ordering},
};

use quick_cache::{Weighter, sync::Cache};

use crate::{geometry::Placement, resources::glyph::ResolvedGlyph};

const DEFAULT_GLYPH_CACHE_MAX_BYTES: usize = 8 << 20; // 8 MiB

/// Bytes charged for an entry's own bookkeeping on top of its payload.
const ENTRY_OVERHEAD: usize = 64;

static MAX_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_GLYPH_CACHE_MAX_BYTES);

/// Sets the byte budget for the glyph cache. `0` stops caching. Takes effect for
/// a cache not yet used; call it before the first render. Defaults to 8 MiB.
pub fn set_glyph_cache_max_bytes(bytes: usize) {
  MAX_BYTES.store(bytes, Ordering::Relaxed);
}

#[derive(Clone)]
enum CachedGlyph {
  Resolved(Arc<ResolvedGlyph>),
  Mask(Arc<[u8]>, Placement),
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

static SHARED: LazyLock<Cache<Key, Entry, ByBytes>> = LazyLock::new(|| {
  let max_bytes = MAX_BYTES.load(Ordering::Relaxed) as u64;
  // ~4 KiB average entry ⇒ item-count hint for the budget.
  let estimated_items = (max_bytes / (4 << 10)).max(1) as usize;

  Cache::with_weighter(estimated_items, max_bytes, ByBytes)
});

fn get_or_insert(
  key: Key,
  f: impl FnOnce() -> Option<(CachedGlyph, usize)>,
) -> Option<CachedGlyph> {
  SHARED
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

/// The resolved outline or bitmap for `key`, resolving on a miss. Concurrent
/// misses for the same key resolve once. `resolve` returning `None` caches
/// nothing and a later call retries.
pub(crate) fn resolved_glyph(
  key: u64,
  resolve: impl Fn() -> Option<ResolvedGlyph>,
) -> Option<Arc<ResolvedGlyph>> {
  let cached = get_or_insert((key, GlyphKind::Resolved), || {
    resolve().map(|glyph| {
      let bytes = glyph.estimated_bytes();

      (CachedGlyph::Resolved(Arc::new(glyph)), bytes)
    })
  });

  match cached {
    Some(CachedGlyph::Resolved(glyph)) => Some(glyph),
    Some(CachedGlyph::Mask(..)) => None,
    None => None,
  }
}

/// The rasterized mask for `key`, rendering on a miss. Concurrent misses for the
/// same key rasterize once. The mask is stored as a boxed slice, so the cache
/// retains exactly its length.
pub fn glyph_mask(key: u64, render: impl Fn() -> (Vec<u8>, Placement)) -> (Arc<[u8]>, Placement) {
  let cached = get_or_insert((key, GlyphKind::Mask), || {
    let (mask, placement) = render();
    let mask: Arc<[u8]> = mask.into();
    let bytes = mask.len() + ENTRY_OVERHEAD;

    Some((CachedGlyph::Mask(mask, placement), bytes))
  });

  match cached {
    Some(CachedGlyph::Mask(mask, placement)) => (mask, placement),
    _ => {
      let (mask, placement) = render();

      (mask.into(), placement)
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

  #[test]
  fn the_two_kinds_do_not_alias_on_the_same_key() {
    let key = 0x5eed_1234_5678_9abc;

    let mask = glyph_mask(key, || (vec![7; 4], placement()));
    let resolved = resolved_glyph(key, || Some(outline(42))).unwrap();

    // Same raw key, disjoint entries: the mask must survive resolving, and each
    // side must get back its own kind.
    assert_eq!(*mask.0, vec![7; 4]);
    assert_eq!(signature_of(&resolved), Some(42));
    assert_eq!(*glyph_mask(key, || (vec![0; 4], placement())).0, vec![7; 4]);
  }

  #[test]
  fn the_whole_key_is_kept() {
    // Two keys that differ only in the top bit, which a key packed into a shifted
    // u64 would have dropped.
    let low = resolved_glyph(0, || Some(outline(1))).unwrap();
    let high = resolved_glyph(1 << 63, || Some(outline(2))).unwrap();

    assert_eq!(signature_of(&low), Some(1));
    assert_eq!(signature_of(&high), Some(2));
  }

  #[test]
  fn a_failed_resolve_caches_nothing_and_retries() {
    let key = 0xfa11_0000_0000_0001;

    assert!(resolved_glyph(key, || None).is_none());
    let resolved = resolved_glyph(key, || Some(outline(7))).unwrap();
    assert_eq!(signature_of(&resolved), Some(7));
  }
}

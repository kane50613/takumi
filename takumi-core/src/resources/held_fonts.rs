//! The font files a host holds in a [`Cache`](crate::Cache), by the id of their decoded bytes, and
//! the leases that keep the ones a render uses from eviction, after RocksDB's block cache handles
//! with `strict_capacity_limit` off: a leased font still counts toward the cache's bytes, and the
//! cache can hold more than its size while leases last.

use std::{
  convert::Infallible,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
};

use crate::resources::{
  cache::{KindStore, Weighed},
  image::ImageCacheMode,
};

/// The font files a host holds, each evicted on its own once no lease or pin keeps it. The cache
/// never touches a registry. The host takes what was evicted with [`Self::take_evicted`] and
/// unregisters it.
pub struct HeldFonts {
  store: HeldStore,
}

/// One held font file, by the id of its decoded bytes.
pub(crate) struct HeldFont {
  id: u64,
  leases: AtomicUsize,
  pinned: AtomicBool,
}

impl HeldFont {
  /// Whether a lease or a pin keeps it from eviction.
  pub(crate) fn is_held(&self) -> bool {
    self.leases.load(Ordering::Acquire) > 0 || self.pinned.load(Ordering::Acquire)
  }
}

/// The store of held fonts, and whether it keeps a font once nothing holds it.
#[derive(Clone)]
struct HeldStore {
  fonts: KindStore<Arc<HeldFont>>,
  keeps: bool,
}

impl HeldStore {
  /// Lets go of `font` once no lease or pin holds it: evicts what the cache holds past its size,
  /// or the font itself when the cache keeps no fonts.
  fn release(&self, font: &HeldFont) {
    if self.keeps {
      self.fonts.trim();
    } else {
      self.fonts.evict(font.id);
    }
  }
}

/// Keeps one held font from eviction until it drops. Once the last lease of a font drops, the cache
/// evicts what it holds past its size.
pub struct FontLease {
  font: Arc<HeldFont>,
  store: HeldStore,
}

impl FontLease {
  fn new(font: Arc<HeldFont>, store: &HeldStore) -> Self {
    font.leases.fetch_add(1, Ordering::AcqRel);

    Self {
      font,
      store: store.clone(),
    }
  }
}

impl Drop for FontLease {
  fn drop(&mut self) {
    if self.font.leases.fetch_sub(1, Ordering::AcqRel) == 1 {
      self.store.release(&self.font);
    }
  }
}

impl HeldFonts {
  pub(crate) fn new(store: KindStore<Arc<HeldFont>>, mode: ImageCacheMode) -> Self {
    Self {
      store: HeldStore {
        fonts: store,
        keeps: matches!(mode, ImageCacheMode::Auto),
      },
    }
  }

  /// Holds the font file whose decoded bytes `id` names, `bytes` long, leased. A file the cache
  /// already holds, under any key, counts once.
  pub fn keep(&self, id: u64, bytes: u64) -> FontLease {
    loop {
      let mut kept = None;
      let font = self
        .store
        .fonts
        .get_or_insert_with(id, || {
          // Leased before it is kept, so a cache too small for it still keeps it.
          let font = Arc::new(HeldFont {
            id,
            leases: AtomicUsize::new(1),
            pinned: AtomicBool::new(false),
          });

          kept = Some(Arc::clone(&font));

          Ok::<_, Infallible>(Weighed::new(
            font,
            usize::try_from(bytes).unwrap_or(usize::MAX),
          ))
        })
        .unwrap_or_else(|never| match never {});

      if let Some(font) = kept {
        return FontLease {
          font,
          store: self.store.clone(),
        };
      }

      if let Some(lease) = self.lease_found(font, id) {
        return lease;
      }
    }
  }

  /// A lease on the font `id` names, none once it was evicted.
  pub fn lease(&self, id: u64) -> Option<FontLease> {
    self.lease_found(self.store.fonts.get(id)?, id)
  }

  /// A lease on `font`, found under `id`, none once it was evicted.
  fn lease_found(&self, font: Arc<HeldFont>, id: u64) -> Option<FontLease> {
    let lease = FontLease::new(font, &self.store);

    self.still_holds(&lease.font, id).then_some(lease)
  }

  /// Whether a lease or a pin keeps the font `id` names from eviction, which counts as no lookup.
  pub fn is_held(&self, id: u64) -> bool {
    self.store.fonts.peek(id).is_some_and(|font| font.is_held())
  }

  /// Keeps the font `id` names from eviction, or lets it go, until [`Self::forget`]; false once it
  /// was evicted.
  pub fn pin(&self, id: u64, pinned: bool) -> bool {
    self
      .store
      .fonts
      .peek(id)
      .is_some_and(|font| self.pin_found(&font, id, pinned))
  }

  /// Pins or unpins `font`, found under `id`; false once it was evicted.
  fn pin_found(&self, font: &Arc<HeldFont>, id: u64, pinned: bool) -> bool {
    font.pinned.store(pinned, Ordering::Release);

    if !pinned {
      self.store.release(font);
    }

    self.still_holds(font, id)
  }

  /// Whether the cache still holds `font` under `id` once a lease or pin protects it, as an
  /// eviction between finding it and protecting it takes it.
  fn still_holds(&self, font: &Arc<HeldFont>, id: u64) -> bool {
    self
      .store
      .fonts
      .peek(id)
      // Identity, not the id: the same bytes kept again after an eviction make a new entry.
      .is_some_and(|held| Arc::ptr_eq(&held, font))
  }

  /// Drops the font `id` names, which counts as no eviction, as a host does when it unregisters it.
  pub fn forget(&self, id: u64) {
    self.store.fonts.remove(id);
  }

  /// The ids of the fonts evicted since the last call, other than ones kept again since, which the
  /// host unregisters.
  pub fn take_evicted(&self) -> Vec<u64> {
    self.store.fonts.take_evicted()
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    Cache,
    resources::{cache::CacheOptions, image::ImageCacheMode},
  };

  const FONT: u64 = 64 << 10;

  /// Whether `cache` holds the font `id` names, which counts as no lookup.
  fn holds(cache: &Cache, id: u64) -> bool {
    cache.fonts().store.fonts.peek(id).is_some()
  }

  fn cache(max_bytes: u64, fonts: ImageCacheMode) -> Cache {
    Cache::from(CacheOptions {
      max_bytes,
      fonts,
      ..CacheOptions::default()
    })
  }

  /// A cache that keeps no font once nothing holds it.
  fn keeping_none() -> Cache {
    cache(1 << 20, ImageCacheMode::None)
  }

  #[test]
  fn the_same_bytes_count_once() {
    let cache = cache(1 << 20, ImageCacheMode::Auto);
    let first = cache.fonts().keep(1, FONT);
    let second = cache.fonts().keep(1, FONT);

    drop((first, second));

    let stats = cache.stats().fonts;

    assert_eq!(stats.current, FONT);
    assert_eq!((stats.hits, stats.misses), (1, 1));
  }

  #[test]
  fn a_lease_keeps_a_font_until_it_drops_when_the_cache_keeps_none() {
    let cache = keeping_none();
    let lease = cache.fonts().keep(1, FONT);

    assert!(holds(&cache, 1));
    assert_eq!(cache.stats().fonts.current, FONT);
    assert!(cache.fonts().take_evicted().is_empty());

    let again = cache.fonts().lease(1).unwrap();

    drop(lease);
    assert!(holds(&cache, 1));
    drop(again);

    assert!(!holds(&cache, 1));
    assert_eq!(cache.fonts().take_evicted(), [1]);
    assert!(cache.fonts().take_evicted().is_empty());
    assert_eq!(cache.stats().fonts.evictions, 1);
  }

  #[test]
  fn a_lease_keeps_a_font_past_the_size_until_it_drops() {
    let cache = cache(0, ImageCacheMode::Auto);
    let lease = cache.fonts().keep(1, FONT);

    assert!(holds(&cache, 1));
    drop(lease);

    assert!(!holds(&cache, 1));
    assert_eq!(cache.fonts().take_evicted(), [1]);
  }

  #[test]
  fn fonts_past_the_size_go_least_recently_used_first() {
    let cache = cache(3 * FONT, ImageCacheMode::Auto);

    for id in 0..8 {
      drop(cache.fonts().keep(id, FONT));
      drop(cache.fonts().lease(0));
    }

    let mut evicted = cache.fonts().take_evicted();

    evicted.sort_unstable();

    assert!(cache.stats().fonts.current <= 3 * FONT);
    assert!(holds(&cache, 0));
    assert_eq!(evicted.len(), 5);
    assert!(!evicted.contains(&0));
  }

  #[test]
  fn a_font_is_held_while_leased_or_pinned() {
    let cache = cache(1 << 20, ImageCacheMode::Auto);
    let lease = cache.fonts().keep(1, FONT);

    assert!(cache.fonts().is_held(1));
    drop(lease);
    assert!(!cache.fonts().is_held(1));
    cache.fonts().pin(1, true);
    assert!(cache.fonts().is_held(1));
    assert!(!cache.fonts().is_held(2));
  }

  #[test]
  fn a_pinned_font_stays_until_it_is_let_go() {
    let cache = keeping_none();

    let lease = cache.fonts().keep(2, FONT);

    cache.fonts().pin(2, true);
    drop(lease);
    drop(cache.fonts().keep(3, FONT));

    assert!(holds(&cache, 2));

    cache.fonts().pin(2, false);

    assert!(!holds(&cache, 2));
  }

  #[test]
  fn a_font_evicted_before_its_lease_counts_gets_none() {
    let cache = keeping_none();
    let lease = cache.fonts().keep(1, FONT);
    let found = cache.fonts().store.fonts.peek(1).unwrap();

    drop(lease);

    assert!(cache.fonts().lease_found(found, 1).is_none());
    assert_eq!(cache.fonts().take_evicted(), [1]);
  }

  #[test]
  fn a_font_evicted_before_its_pin_counts_is_not_pinned() {
    let cache = keeping_none();
    let lease = cache.fonts().keep(1, FONT);
    let found = cache.fonts().store.fonts.peek(1).unwrap();

    drop(lease);

    assert!(!cache.fonts().pin_found(&found, 1, true));
    assert!(!holds(&cache, 1));
  }

  #[test]
  fn a_font_kept_again_is_not_reported_evicted() {
    let cache = keeping_none();

    drop(cache.fonts().keep(1, FONT));

    let lease = cache.fonts().keep(1, FONT);

    assert!(cache.fonts().take_evicted().is_empty());
    drop(lease);
    assert_eq!(cache.fonts().take_evicted(), [1]);
  }

  #[test]
  fn a_forgotten_font_counts_as_no_eviction() {
    let cache = cache(1 << 20, ImageCacheMode::Auto);

    drop(cache.fonts().keep(1, FONT));
    cache.fonts().forget(1);

    assert!(!holds(&cache, 1));
    assert!(cache.fonts().take_evicted().is_empty());
    assert_eq!(cache.stats().fonts.evictions, 0);
  }
}

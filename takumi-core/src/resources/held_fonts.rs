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

use crate::resources::cache::{KindStore, Weighed};

/// The font files a host holds, each evicted on its own once no lease or pin keeps it. The cache
/// never touches a registry. The host takes what was evicted with [`Self::take_evicted`] and
/// unregisters it.
pub struct HeldFonts {
  store: KindStore<Arc<HeldFont>>,
}

/// One held font file, by its decoded size.
pub(crate) struct HeldFont {
  bytes: u64,
  leases: AtomicUsize,
  pinned: AtomicBool,
}

impl HeldFont {
  /// Whether a lease or a pin keeps it from eviction.
  pub(crate) fn is_held(&self) -> bool {
    self.leases.load(Ordering::Acquire) > 0 || self.pinned.load(Ordering::Acquire)
  }
}

impl Weighed for Arc<HeldFont> {
  fn bytes(&self) -> usize {
    usize::try_from(self.bytes).unwrap_or(usize::MAX)
  }
}

/// Keeps one held font from eviction until it drops. Once the last lease of a font drops, the cache
/// evicts what it holds past its size.
pub struct FontLease {
  font: Arc<HeldFont>,
  store: KindStore<Arc<HeldFont>>,
}

impl FontLease {
  fn new(font: Arc<HeldFont>, store: &KindStore<Arc<HeldFont>>) -> Self {
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
      self.store.trim();
    }
  }
}

impl HeldFonts {
  pub(crate) fn new(store: KindStore<Arc<HeldFont>>) -> Self {
    Self { store }
  }

  /// Holds the font file whose decoded bytes `id` names, `bytes` long, leased. A file the cache
  /// already holds, under any key, counts once.
  pub fn keep(&self, id: u64, bytes: u64) -> FontLease {
    let _still = self.store.hold_still();

    loop {
      let mut kept = None;
      let font = self
        .store
        .get_or_insert_with(id, || {
          // Leased before it is kept, so a cache too small for it still keeps it.
          let font = Arc::new(HeldFont {
            bytes: bytes.max(1),
            leases: AtomicUsize::new(1),
            pinned: AtomicBool::new(false),
          });

          kept = Some(Arc::clone(&font));

          Ok::<_, Infallible>(font)
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
    let _still = self.store.hold_still();

    self.lease_found(self.store.get(id)?, id)
  }

  /// A lease on `font`, found under `id`, none once it was evicted.
  fn lease_found(&self, font: Arc<HeldFont>, id: u64) -> Option<FontLease> {
    let lease = FontLease::new(font, &self.store);

    self.still_holds(&lease.font, id).then_some(lease)
  }

  /// Whether the cache holds the font `id` names, which counts as no lookup.
  pub fn holds(&self, id: u64) -> bool {
    self.store.peek(id).is_some()
  }

  /// Keeps the font `id` names from eviction, or lets it go, until [`Self::forget`]; false once it
  /// was evicted.
  pub fn pin(&self, id: u64, pinned: bool) -> bool {
    let _still = self.store.hold_still();

    self
      .store
      .peek(id)
      .is_some_and(|font| self.pin_found(&font, id, pinned))
  }

  /// Pins or unpins `font`, found under `id`; false once it was evicted.
  fn pin_found(&self, font: &Arc<HeldFont>, id: u64, pinned: bool) -> bool {
    font.pinned.store(pinned, Ordering::Release);

    if !pinned {
      self.store.trim();
    }

    self.still_holds(font, id)
  }

  /// Whether the cache still holds `font` under `id` once a lease or pin protects it, as an
  /// eviction between finding it and protecting it takes it.
  fn still_holds(&self, font: &Arc<HeldFont>, id: u64) -> bool {
    self
      .store
      .peek(id)
      .is_some_and(|held| Arc::ptr_eq(&held, font))
  }

  /// Drops the font `id` names, which counts as no eviction, as a host does when it unregisters it.
  pub fn forget(&self, id: u64) {
    let _still = self.store.hold_still();

    self.store.remove(id);
  }

  /// The ids of the fonts evicted since the last call, other than ones kept again since, which the
  /// host unregisters.
  pub fn take_evicted(&self) -> Vec<u64> {
    self.store.take_evicted()
  }
}

#[cfg(test)]
mod tests {
  use std::thread;

  use crate::{Cache, resources::cache::CacheOptions};

  const FONT: u64 = 64 << 10;

  fn cache(fonts: Option<u64>) -> Cache {
    Cache::builder().max_bytes(1 << 20).fonts_opt(fonts).build()
  }

  #[test]
  fn the_same_bytes_count_once() {
    let cache = cache(None);
    let first = cache.fonts().keep(1, FONT);
    let second = cache.fonts().keep(1, FONT);

    drop((first, second));

    let stats = cache.stats().fonts;

    assert_eq!(stats.current, FONT);
    assert_eq!((stats.hits, stats.misses), (1, 1));
  }

  #[test]
  fn a_lease_keeps_a_font_past_the_size_until_it_drops() {
    let cache = cache(Some(0));
    let lease = cache.fonts().keep(1, FONT);

    assert!(cache.fonts().holds(1));
    assert_eq!(cache.stats().fonts.current, FONT);
    assert!(cache.fonts().take_evicted().is_empty());

    let again = cache.fonts().lease(1).unwrap();

    drop(lease);
    assert!(cache.fonts().holds(1));
    drop(again);

    assert!(!cache.fonts().holds(1));
    assert_eq!(cache.fonts().take_evicted(), [1]);
    assert!(cache.fonts().take_evicted().is_empty());
    assert_eq!(cache.stats().fonts.evictions, 1);
  }

  #[test]
  fn fonts_past_the_budget_go_least_recently_used_first() {
    let cache = cache(Some(3 * FONT));

    for id in 0..8 {
      drop(cache.fonts().keep(id, FONT));
      drop(cache.fonts().lease(0));
    }

    let mut evicted = cache.fonts().take_evicted();

    evicted.sort_unstable();

    assert!(cache.stats().fonts.current <= 3 * FONT);
    assert!(cache.fonts().holds(0));
    assert_eq!(evicted.len(), 5);
    assert!(!evicted.contains(&0));
  }

  #[test]
  fn a_pinned_font_stays_until_it_is_let_go() {
    let cache = cache(Some(0));

    let lease = cache.fonts().keep(2, FONT);

    cache.fonts().pin(2, true);
    drop(lease);
    drop(cache.fonts().keep(3, FONT));

    assert!(cache.fonts().holds(2));

    cache.fonts().pin(2, false);

    assert!(!cache.fonts().holds(2));
  }

  #[test]
  fn a_font_evicted_before_its_lease_counts_gets_none() {
    let cache = cache(Some(0));
    let lease = cache.fonts().keep(1, FONT);
    let found = cache.fonts().store.peek(1).unwrap();

    drop(lease);

    assert!(cache.fonts().lease_found(found, 1).is_none());
    assert_eq!(cache.fonts().take_evicted(), [1]);
  }

  #[test]
  fn a_font_evicted_before_its_pin_counts_is_not_pinned() {
    let cache = cache(Some(0));
    let lease = cache.fonts().keep(1, FONT);
    let found = cache.fonts().store.peek(1).unwrap();

    drop(lease);

    assert!(!cache.fonts().pin_found(&found, 1, true));
    assert!(!cache.fonts().holds(1));
  }

  #[test]
  fn a_font_kept_again_is_not_reported_evicted() {
    let cache = cache(Some(0));

    drop(cache.fonts().keep(1, FONT));

    let lease = cache.fonts().keep(1, FONT);

    assert!(cache.fonts().take_evicted().is_empty());
    drop(lease);
    assert_eq!(cache.fonts().take_evicted(), [1]);
  }

  #[test]
  fn a_forgotten_font_counts_as_no_eviction() {
    let cache = cache(None);

    drop(cache.fonts().keep(1, FONT));
    cache.fonts().forget(1);

    assert!(!cache.fonts().holds(1));
    assert!(cache.fonts().take_evicted().is_empty());
    assert_eq!(cache.stats().fonts.evictions, 0);
  }

  #[test]
  fn held_fonts_move_with_a_budget_they_gain_or_lose() {
    let cache = cache(None);
    let lease = cache.fonts().keep(1, FONT);

    drop(cache.fonts().keep(2, FONT));
    cache.set_options(CacheOptions {
      max_bytes: 1 << 20,
      fonts: Some(0),
      ..CacheOptions::default()
    });

    assert!(cache.fonts().holds(1));
    assert!(!cache.fonts().holds(2));
    assert_eq!(cache.fonts().take_evicted(), [2]);

    cache.set_options(CacheOptions {
      max_bytes: 1 << 20,
      ..CacheOptions::default()
    });

    assert!(cache.fonts().holds(1));
    drop(lease);
    assert!(cache.fonts().holds(1));
    assert_eq!(cache.stats().fonts.current, FONT);
  }

  /// Fonts kept while the fonts budget comes and goes stay in whichever store the cache moved them
  /// to, so none is dropped without an eviction.
  #[test]
  fn fonts_kept_while_the_budget_changes_stay_held() {
    let cache = Cache::default();
    let fonts = cache.fonts();
    let keeper = thread::scope(|scope| {
      let keeper = scope.spawn(|| {
        (0..2000)
          .map(|id| (id, fonts.keep(id, 64)))
          .collect::<Vec<_>>()
      });

      for round in 0..400 {
        cache.set_options(CacheOptions {
          fonts: (round % 2 == 0).then_some(64 << 20),
          ..CacheOptions::default()
        });
      }

      keeper.join().unwrap()
    });
    let lost = keeper.iter().filter(|(id, _)| !fonts.holds(*id)).count();

    assert_eq!(lost, 0);
  }
}

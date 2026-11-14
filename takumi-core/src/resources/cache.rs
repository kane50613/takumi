//! What renders keep for the renders after them, in one pool of bytes every kind of data shares.

use std::{
  array,
  convert::Infallible,
  marker::PhantomData,
  mem::take,
  ops::{Index, IndexMut},
  ptr,
  sync::{
    Arc, LazyLock, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard,
    atomic::{AtomicU64, Ordering},
  },
  thread::available_parallelism,
};

use arc_swap::ArcSwapOption;
use quick_cache::{
  DefaultHashBuilder, Lifecycle, OptionsBuilder, Weighter,
  sync::{Cache as Store, DefaultLifecycle, GuardResult},
};
use typed_builder::TypedBuilder;
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

#[cfg(feature = "font-face")]
use crate::resources::font::{CachedRegistry, DocumentKey, DocumentRegistries};
#[cfg(feature = "svg")]
use crate::resources::image::SvgPicture;
use crate::{
  layout::intercept::{CachedSpans, InterceptCache, InterceptKey},
  resources::{
    glyph_cache::{self, GlyphCache, GlyphEntry, GlyphKey},
    held_fonts::{HeldFont, HeldFonts},
    image::{ImageCacheMode, ImageError, ImageResult, ImageSource},
    image_buffer::ImageBuffer,
  },
  style::{
    ImageScalingAlgorithm, StyleSheet,
    tw::{CompiledClasses, TailwindCache},
  },
};

/// Bytes each shard holds, enough for one decoded photo, since `quick_cache` rejects an entry over
/// 97% of its shard.
const SHARD_BYTES: u64 = 64 << 20;

/// The step a budget's charge against the pool rounds up to, so the pool resizes only when the
/// bytes a budgeted kind holds cross one.
const CHARGE_STEP: u64 = 1 << 20;

/// What renders keep across renders: decoded images and the rasters drawn from them, parsed
/// stylesheets and Tailwind classes, glyphs and their masks, the faces of `@font-face` rules, and
/// the font files a host holds. Every kind shares one pool of bytes. Clones share the same data.
#[derive(Clone)]
pub struct Cache(Arc<Caches>);

struct Caches {
  pool: Arc<Pool>,
  images: KindStore<ImageSource>,
  sized_images: KindStore<Arc<ImageBuffer>>,
  #[cfg(feature = "svg")]
  pictures: KindStore<Arc<SvgPicture>>,
  stylesheets: KindStore<SheetEntry>,
  tailwind: TailwindCache,
  glyphs: GlyphCache,
  intercepts: InterceptCache,
  #[cfg(feature = "font-face")]
  font_faces: DocumentRegistries,
  fonts: HeldFonts,
}

/// The bytes a [`Cache`] keeps, and the most bytes each kind of data may hold within them. A kind
/// without a budget competes for the whole pool. `0` keeps nothing, of every kind or of one.
#[derive(TypedBuilder, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[builder(
  builder_type(name = CacheBuilder),
  build_method(into = Cache),
  field_defaults(default, setter(strip_option(ignore_invalid, fallback_suffix = "_opt")))
)]
pub struct CacheOptions {
  /// The bytes every kind of data shares. Approximate: entries are weighed by estimated size.
  #[builder(default = CacheOptions::DEFAULT_MAX_BYTES)]
  pub max_bytes: u64,
  /// Decoded images, and the SVG rasters and downscaled bitmaps drawn from them.
  pub images: Option<u64>,
  /// SVG images laid out to drawing ops, which vector backends draw and rasters are drawn from.
  #[cfg(feature = "svg")]
  pub pictures: Option<u64>,
  /// Parsed stylesheets.
  pub stylesheets: Option<u64>,
  /// The rules Tailwind classes compile to.
  pub tailwind: Option<u64>,
  /// Resolved glyphs, and the masks rasterized from glyphs and shapes. Without one, the budget the
  /// deprecated [`set_glyph_cache_max_bytes`](glyph_cache::set_glyph_cache_max_bytes) set.
  pub glyphs: Option<u64>,
  /// Where glyph outlines cross a decoration line, for `text-decoration-skip-ink`.
  pub intercepts: Option<u64>,
  /// The faces `@font-face` rules add, weighed by the font files they hold.
  #[cfg(feature = "font-face")]
  pub font_faces: Option<u64>,
  /// The decoded font files a host holds through [`Cache::fonts`].
  pub fonts: Option<u64>,
}

impl CacheOptions {
  /// The bytes a cache keeps unless its builder sets `max_bytes`.
  pub const DEFAULT_MAX_BYTES: u64 = 128 << 20;

  /// The budget of each kind, each capped at `max_bytes`.
  fn budgets(&self) -> PerKind<Option<u64>> {
    PerKind::from_fn(|kind| {
      let budget = match kind {
        Kind::Images => self.images,
        #[cfg(feature = "svg")]
        Kind::Pictures => self.pictures,
        #[cfg(not(feature = "svg"))]
        Kind::Pictures => None,
        Kind::Stylesheets => self.stylesheets,
        Kind::Tailwind => self.tailwind,
        Kind::Glyphs => self.glyphs.or_else(glyph_cache::default_budget),
        Kind::Intercepts => self.intercepts,
        #[cfg(feature = "font-face")]
        Kind::FontFaces => self.font_faces,
        #[cfg(not(feature = "font-face"))]
        Kind::FontFaces => None,
        Kind::Fonts => self.fonts,
      };

      budget.map(|budget| budget.min(self.max_bytes))
    })
  }
}

impl Default for CacheOptions {
  fn default() -> Self {
    Self {
      max_bytes: Self::DEFAULT_MAX_BYTES,
      images: None,
      #[cfg(feature = "svg")]
      pictures: None,
      stylesheets: None,
      tailwind: None,
      glyphs: None,
      intercepts: None,
      #[cfg(feature = "font-face")]
      font_faces: None,
      fonts: None,
    }
  }
}

impl From<CacheOptions> for Cache {
  fn from(options: CacheOptions) -> Self {
    let pool = Arc::new(Pool::new(options));

    Self(Arc::new(Caches {
      images: KindStore::new(&pool),
      sized_images: KindStore::new(&pool),
      #[cfg(feature = "svg")]
      pictures: KindStore::new(&pool),
      stylesheets: KindStore::new(&pool),
      tailwind: TailwindCache::new(KindStore::new(&pool)),
      glyphs: GlyphCache::new(KindStore::new(&pool)),
      intercepts: InterceptCache::new(KindStore::new(&pool)),
      #[cfg(feature = "font-face")]
      font_faces: DocumentRegistries::new(KindStore::new(&pool)),
      fonts: HeldFonts::new(KindStore::new(&pool)),
      pool,
    }))
  }
}

impl Default for Cache {
  fn default() -> Self {
    Self::builder().build()
  }
}

impl Cache {
  /// A builder for a cache with its own size and budgets.
  pub fn builder() -> CacheBuilder {
    CacheOptions::builder()
  }

  /// The cache every render without one of its own shares, with the default size.
  pub fn shared() -> Self {
    static SHARED: LazyLock<Cache> = LazyLock::new(Cache::default);

    SHARED.clone()
  }

  /// Gives the cache `options` in place of the ones it has, evicting at once what a smaller size
  /// or budget no longer fits. A kind that gains or loses a budget drops what it held. A store
  /// keeps the shards it was built with, so after a large cache shrinks, the largest entry it keeps
  /// can be smaller than in a cache built at the new size.
  pub fn set_options(&self, options: CacheOptions) {
    self.0.pool.set_options(options);
  }

  /// What the cache holds and how its lookups went since it was built.
  pub fn stats(&self) -> CacheStats {
    self.0.pool.stats()
  }

  /// The image `bytes` hold, decoded once across concurrent misses, and kept unless `mode` is
  /// [`ImageCacheMode::None`].
  pub fn decode_image(&self, bytes: &[u8], mode: ImageCacheMode) -> ImageResult {
    let hash = xxh3_64(bytes);

    if matches!(mode, ImageCacheMode::None) {
      return match self.0.images.get(hash) {
        Some(source) => Ok(source),
        None => ImageSource::from_bytes_uncached(bytes),
      };
    }

    self
      .0
      .images
      .get_or_insert_with(hash, || ImageSource::from_bytes_keyed(bytes, hash))
  }

  /// The stylesheet `sources` parse to, parsed once across renders that send the same CSS.
  pub fn parse_stylesheet(&self, sources: Vec<String>) -> Arc<StyleSheet> {
    let mut hasher = Xxh3::new();

    for source in &sources {
      hasher.update(source.as_bytes());
      hasher.update(&(source.len() as u64).to_le_bytes());
    }

    // Parsed rules retain roughly this much beyond the source text.
    let bytes = sources
      .iter()
      .map(String::len)
      .sum::<usize>()
      .saturating_mul(3);

    self
      .0
      .stylesheets
      .get_or_insert_with(hasher.digest(), || {
        Ok::<_, Infallible>(SheetEntry {
          sheet: Arc::new(StyleSheet::parse_owned_list_loosy(sources)),
          bytes,
        })
      })
      .unwrap_or_else(|never| match never {})
      .sheet
  }

  /// The old `ResourceCache::new`, a cache of `max_bytes`.
  #[deprecated(note = "use `Cache::builder().max_bytes(max_bytes)`")]
  pub fn new(max_bytes: u64) -> Self {
    Self::builder().max_bytes(max_bytes).build()
  }

  /// The old name of [`Self::decode_image`].
  #[deprecated(note = "use `decode_image`")]
  pub fn get_or_decode(&self, bytes: &[u8], mode: ImageCacheMode) -> ImageResult {
    self.decode_image(bytes, mode)
  }

  /// The old name of [`Self::parse_stylesheet`].
  #[deprecated(note = "use `parse_stylesheet`")]
  pub fn get_or_parse_stylesheet(&self, sources: Vec<String>) -> Arc<StyleSheet> {
    self.parse_stylesheet(sources)
  }

  /// The picture kept under `key`.
  #[cfg(feature = "svg")]
  pub(crate) fn picture(&self, key: u64) -> Option<Arc<SvgPicture>> {
    self.0.pictures.get(key)
  }

  /// `picture`, kept under its key when it has one.
  #[cfg(feature = "svg")]
  pub(crate) fn keep_picture(&self, picture: SvgPicture) -> Arc<SvgPicture> {
    let picture = Arc::new(picture);

    if let Some(key) = picture.key() {
      self.0.pictures.insert(key, picture.clone());
    }

    picture
  }

  /// The glyphs and masks the cache keeps.
  pub fn glyphs(&self) -> &GlyphCache {
    &self.0.glyphs
  }

  pub(crate) fn tailwind(&self) -> &TailwindCache {
    &self.0.tailwind
  }

  pub(crate) fn intercepts(&self) -> &InterceptCache {
    &self.0.intercepts
  }

  #[cfg(feature = "font-face")]
  pub(crate) fn font_faces(&self) -> &DocumentRegistries {
    &self.0.font_faces
  }

  /// The font files a host holds, such as the fonts a binding registers.
  pub fn fonts(&self) -> &HeldFonts {
    &self.0.fonts
  }

  /// The buffer an image with the cache key `source` draws at a size, `compute`d once across
  /// concurrent misses. A source with no key computes on every call.
  pub(crate) fn sized(
    &self,
    source: Option<u64>,
    sized: SizedKey,
    compute: impl FnOnce() -> Result<Arc<ImageBuffer>, ImageError>,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    let Some(source) = source else {
      return compute();
    };

    self
      .0
      .sized_images
      .get_or_insert_with((source, sized), compute)
  }
}

/// What a [`Cache`] holds of every kind together and of each kind, and how its lookups went since
/// it was built. Sizes are the estimates the cache weighs entries by, not process memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheStats {
  /// Every kind together.
  pub memory: CacheMemoryStats,
  /// [`CacheOptions::images`].
  pub images: CacheKindStats,
  /// [`CacheOptions::pictures`].
  #[cfg(feature = "svg")]
  pub pictures: CacheKindStats,
  /// [`CacheOptions::stylesheets`].
  pub stylesheets: CacheKindStats,
  /// [`CacheOptions::tailwind`].
  pub tailwind: CacheKindStats,
  /// [`CacheOptions::glyphs`].
  pub glyphs: CacheKindStats,
  /// [`CacheOptions::intercepts`].
  pub intercepts: CacheKindStats,
  /// [`CacheOptions::font_faces`].
  #[cfg(feature = "font-face")]
  pub font_faces: CacheKindStats,
  /// [`CacheOptions::fonts`].
  pub fonts: CacheKindStats,
}

/// The bytes a [`Cache`] holds of every kind together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheMemoryStats {
  /// The bytes held now.
  pub current: u64,
  /// The most bytes held at once since the cache was built, as measured after each entry it kept.
  pub high: u64,
  /// [`CacheOptions::max_bytes`].
  pub max: u64,
}

/// What a [`Cache`] holds of one kind of data, and how lookups of that kind went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheKindStats {
  /// The bytes held now.
  pub current: u64,
  /// The kind's budget, none when it shares the pool.
  pub max: Option<u64>,
  /// Lookups that found an entry.
  pub hits: u64,
  /// Lookups that found none.
  pub misses: u64,
  /// Entries dropped to stay within the bytes, counting entries too large to keep at all.
  pub evictions: u64,
}

/// A kind of data a budget covers, one for each budget of [`CacheOptions`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  Images,
  Pictures,
  Stylesheets,
  Tailwind,
  Glyphs,
  Intercepts,
  FontFaces,
  Fonts,
}

const KINDS: usize = 8;

impl Kind {
  const ALL: [Self; KINDS] = [
    Self::Images,
    Self::Pictures,
    Self::Stylesheets,
    Self::Tailwind,
    Self::Glyphs,
    Self::Intercepts,
    Self::FontFaces,
    Self::Fonts,
  ];

  /// Moves what `from` holds of this kind into `to` when it is [`Kind::Fonts`], whose host must
  /// hear of each font before it goes, so a budget they gain or lose keeps them where every other
  /// kind starts empty.
  fn move_held(self, from: &PoolStore, to: &PoolStore) {
    if self != Self::Fonts {
      return;
    }

    for (key, entry) in from.iter() {
      if key.kind() == self {
        to.insert(key, entry);
      }
    }
  }

  /// The approximate bytes of one entry, a hint for how many a budget holds.
  fn entry_bytes(self) -> u64 {
    match self {
      Self::Images => 64 << 10,
      Self::Pictures | Self::Stylesheets => 16 << 10,
      Self::Tailwind => 256,
      Self::Glyphs => 4 << 10,
      Self::Intercepts => 64,
      Self::FontFaces | Self::Fonts => 256 << 10,
    }
  }
}

/// One value for each kind of data.
struct PerKind<T>([T; KINDS]);

impl<T> PerKind<T> {
  fn from_fn(mut value: impl FnMut(Kind) -> T) -> Self {
    Self(array::from_fn(|index| value(Kind::ALL[index])))
  }
}

impl<T> Index<Kind> for PerKind<T> {
  type Output = T;

  fn index(&self, kind: Kind) -> &T {
    &self.0[kind as usize]
  }
}

impl<T> IndexMut<Kind> for PerKind<T> {
  fn index_mut(&mut self, kind: Kind) -> &mut T {
    &mut self.0[kind as usize]
  }
}

/// How lookups of one kind went, and how many of its entries were evicted.
#[derive(Default)]
struct KindCounts {
  hits: AtomicU64,
  misses: AtomicU64,
  evictions: AtomicU64,
}

impl KindCounts {
  fn looked_up(&self, found: bool) {
    let counter = if found { &self.hits } else { &self.misses };

    counter.fetch_add(1, Ordering::Relaxed);
  }

  fn stats(&self, current: u64, max: Option<u64>) -> CacheKindStats {
    CacheKindStats {
      current,
      max,
      hits: self.hits.load(Ordering::Relaxed),
      misses: self.misses.load(Ordering::Relaxed),
      evictions: self.evictions.load(Ordering::Relaxed),
    }
  }
}

type PoolStore = Store<PoolKey, PoolEntry, ByBytes, DefaultHashBuilder, PoolLifecycle>;

/// The bytes every kind shares: a store for the kinds without a budget, which gives up what the
/// budgets hold, and a store of its own for each kind with one.
struct Pool {
  store: PoolStore,
  max_bytes: AtomicU64,
  /// The most bytes the pool and its budgets held together after an entry was kept.
  high: AtomicU64,
  budgets: PerKind<ArcSwapOption<Budget>>,
  lifecycle: PoolLifecycle,
  /// The bytes budgets hold, each rounded up to a [`CHARGE_STEP`]. Held while budgets resize.
  charged: Mutex<u64>,
  /// Held exclusively while fonts move between the shared store and a budget, and shared while a
  /// font is kept, leased, pinned, or forgotten, so none lands in a store the move already left.
  fonts_moving: RwLock<()>,
}

impl Pool {
  fn new(options: CacheOptions) -> Self {
    let lifecycle = PoolLifecycle::default();
    let budgets = options.budgets();

    Self {
      // ~4 KiB average entry, as glyphs and masks outnumber the rest ⇒ an item-count hint.
      store: store(options.max_bytes, 4 << 10, &lifecycle),
      max_bytes: AtomicU64::new(options.max_bytes),
      high: AtomicU64::new(0),
      budgets: PerKind::from_fn(|kind| {
        ArcSwapOption::new(
          budgets[kind].map(|bytes| Arc::new(Budget::new(kind, bytes, &lifecycle))),
        )
      }),
      lifecycle,
      charged: Mutex::new(0),
      fonts_moving: RwLock::new(()),
    }
  }

  fn counts(&self) -> &PerKind<KindCounts> {
    &self.lifecycle.counts
  }

  fn lock_charged(&self) -> MutexGuard<'_, u64> {
    self.charged.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// `budget`'s store, or the shared one without a budget.
  fn store_for<'a>(&'a self, budget: Option<&'a Budget>) -> &'a PoolStore {
    budget.map_or(&self.store, |budget| &budget.store)
  }

  /// Charges what `kind`'s `budget` holds after it kept an entry, and records the bytes held.
  fn kept(&self, kind: Kind, budget: Option<&Budget>) {
    if let Some(budget) = budget {
      self.charge(kind, budget);
    }

    self.high.fetch_max(self.held(), Ordering::Relaxed);
  }

  /// The bytes the pool and its budgets hold together.
  fn held(&self) -> u64 {
    Kind::ALL
      .into_iter()
      .filter_map(|kind| {
        self.budgets[kind]
          .load()
          .as_ref()
          .map(|budget| budget.store.weight())
      })
      .sum::<u64>()
      + self.store.weight()
  }

  /// Evicts what `kind` holds past its store's capacity, as an entry that was pinned when it was kept
  /// no longer is.
  fn trim(&self, kind: Kind) {
    let budget = self.budgets[kind].load();
    let store = self.store_for(budget.as_deref());

    if store.weight() <= store.capacity() {
      return;
    }

    let mut charged = self.lock_charged();

    if let Some(budget) = &*self.budgets[kind].load() {
      self.fit(&mut charged, budget);
    }

    self.resize(*charged);
  }

  /// Charges what `budget` holds against the pool once it crossed a [`CHARGE_STEP`], after RocksDB's
  /// [`CacheReservationManager`](https://github.com/facebook/rocksdb/blob/main/cache/cache_reservation_manager.h),
  /// unless `kind` no longer has that budget.
  fn charge(&self, kind: Kind, budget: &Budget) {
    // A budget the others squeezed refits on every insert, so it grows back once they shrink.
    let squeezed = budget.store.capacity() < budget.max_bytes.load(Ordering::Relaxed);

    if !squeezed && budget.held() == budget.charged.load(Ordering::Relaxed) {
      return;
    }

    let mut charged = self.lock_charged();
    let current = self.budgets[kind].load();

    if current
      .as_deref()
      .is_some_and(|current| ptr::eq(current, budget))
    {
      self.fit(&mut charged, budget);
      self.resize(*charged);
    }
  }

  /// Fits `budget` into what the other budgets leave of the pool, then gives the pool what the
  /// budgets leave, so together they hold at most `max_bytes`.
  fn fit(&self, charged: &mut u64, budget: &Budget) {
    let max_bytes = self.max_bytes.load(Ordering::Relaxed);
    let others = *charged - budget.charged.load(Ordering::Relaxed);

    budget.store.set_capacity(
      budget
        .max_bytes
        .load(Ordering::Relaxed)
        .min(max_bytes.saturating_sub(others)),
    );

    let held = budget.held();

    budget.charged.store(held, Ordering::Relaxed);
    *charged = others + held;
  }

  /// Gives the shared store what the budgets leave of `max_bytes`.
  fn resize(&self, charged: u64) {
    self.store.set_capacity(
      self
        .max_bytes
        .load(Ordering::Relaxed)
        .saturating_sub(charged),
    );
  }

  /// Resizes the stores to `options` in place. A render that started before keeps its entries
  /// where the old options put them, until they are evicted.
  fn set_options(&self, options: CacheOptions) {
    let _moving = self
      .fonts_moving
      .write()
      .unwrap_or_else(PoisonError::into_inner);
    let mut charged = self.lock_charged();
    let budgets = options.budgets();

    self.max_bytes.store(options.max_bytes, Ordering::Relaxed);

    for kind in Kind::ALL {
      let slot = &self.budgets[kind];

      match (slot.load_full(), budgets[kind]) {
        (Some(budget), Some(bytes)) => budget.max_bytes.store(bytes, Ordering::Relaxed),
        (Some(budget), None) => {
          slot.store(None);
          *charged -= budget.charged.load(Ordering::Relaxed);
          kind.move_held(&budget.store, &self.store);
          budget.store.clear();
        }
        (None, Some(bytes)) => {
          let budget = Budget::new(kind, bytes, &self.lifecycle);

          kind.move_held(&self.store, &budget.store);
          slot.store(Some(Arc::new(budget)));
          self.store.retain(|key, _| key.kind() != kind);
        }
        (None, None) => {}
      }
    }

    // Budgets that give bytes back fit first, so the budgets that grow see what they released.
    for shrinks in [true, false] {
      for kind in Kind::ALL {
        if let Some(budget) = &*self.budgets[kind].load()
          && (budget.charged.load(Ordering::Relaxed) > budget.max_bytes.load(Ordering::Relaxed))
            == shrinks
        {
          self.fit(&mut charged, budget);
        }
      }
    }

    self.resize(*charged);
  }

  fn stats(&self) -> CacheStats {
    let budgets = PerKind::from_fn(|kind| self.budgets[kind].load_full());
    let mut held = PerKind::from_fn(|kind| {
      budgets[kind]
        .as_ref()
        .map_or(0, |budget| budget.store.weight())
    });

    for (key, entry) in self.store.iter() {
      held[key.kind()] += ByBytes.weight(&key, &entry);
    }

    let current = held.0.iter().sum();
    let kind = |kind: Kind| {
      self.counts()[kind].stats(
        held[kind],
        budgets[kind]
          .as_ref()
          .map(|budget| budget.max_bytes.load(Ordering::Relaxed)),
      )
    };

    CacheStats {
      memory: CacheMemoryStats {
        current,
        high: self.high.fetch_max(current, Ordering::Relaxed).max(current),
        max: self.max_bytes.load(Ordering::Relaxed),
      },
      images: kind(Kind::Images),
      #[cfg(feature = "svg")]
      pictures: kind(Kind::Pictures),
      stylesheets: kind(Kind::Stylesheets),
      tailwind: kind(Kind::Tailwind),
      glyphs: kind(Kind::Glyphs),
      intercepts: kind(Kind::Intercepts),
      #[cfg(feature = "font-face")]
      font_faces: kind(Kind::FontFaces),
      fonts: kind(Kind::Fonts),
    }
  }
}

/// A kind's own store, holding at most its budget, whose weight the pool gives up.
struct Budget {
  store: PoolStore,
  /// The most the kind may hold, which its store's capacity shrinks from while other budgets fill
  /// the pool.
  max_bytes: AtomicU64,
  /// The bytes this budget last charged the pool.
  charged: AtomicU64,
}

impl Budget {
  /// A store for `kind` holding at most `max_bytes`.
  fn new(kind: Kind, max_bytes: u64, lifecycle: &PoolLifecycle) -> Self {
    Self {
      store: store(max_bytes, kind.entry_bytes(), lifecycle),
      max_bytes: AtomicU64::new(max_bytes),
      charged: AtomicU64::new(0),
    }
  }

  /// The bytes the store holds, rounded up to a [`CHARGE_STEP`].
  fn held(&self) -> u64 {
    self.store.weight().next_multiple_of(CHARGE_STEP)
  }
}

/// Where one kind of data is kept: the shared pool, or a store of its own under a budget.
pub(crate) struct KindStore<V> {
  pool: Arc<Pool>,
  kind: PhantomData<fn() -> V>,
}

impl<V> Clone for KindStore<V> {
  fn clone(&self) -> Self {
    Self {
      pool: Arc::clone(&self.pool),
      kind: PhantomData,
    }
  }
}

impl<V: Kept> KindStore<V> {
  fn new(pool: &Arc<Pool>) -> Self {
    Self {
      pool: Arc::clone(pool),
      kind: PhantomData,
    }
  }

  fn counts(&self) -> &KindCounts {
    &self.pool.counts()[V::KIND]
  }

  fn budget_slot(&self) -> &ArcSwapOption<Budget> {
    &self.pool.budgets[V::KIND]
  }

  /// The budget's bytes, none for a kind that shares the pool.
  pub(crate) fn budget(&self) -> Option<u64> {
    self
      .budget_slot()
      .load()
      .as_ref()
      .map(|budget| budget.max_bytes.load(Ordering::Relaxed))
  }

  /// The bytes the whole cache keeps.
  pub(crate) fn max_bytes(&self) -> u64 {
    self.pool.max_bytes.load(Ordering::Relaxed)
  }

  /// The value kept under `key`.
  pub(crate) fn get(&self, key: V::Key) -> Option<V> {
    let budget = self.budget_slot().load();
    let value = self
      .pool
      .store_for(budget.as_deref())
      .get(&V::pool_key(key))
      .and_then(V::from_entry);

    self.counts().looked_up(value.is_some());

    value
  }

  /// The value kept under `key`, without counting a lookup or marking it used.
  pub(crate) fn peek(&self, key: V::Key) -> Option<V> {
    let budget = self.budget_slot().load();

    self
      .pool
      .store_for(budget.as_deref())
      .peek(&V::pool_key(key))
      .and_then(V::from_entry)
  }

  /// Drops the value kept under `key`, which counts as no eviction, and returns what its budget
  /// no longer holds to the pool.
  pub(crate) fn remove(&self, key: V::Key) {
    let budget = self.budget_slot().load();

    self
      .pool
      .store_for(budget.as_deref())
      .remove(&V::pool_key(key));

    if let Some(budget) = budget.as_deref() {
      self.pool.charge(V::KIND, budget);
    }
  }

  /// Evicts what the kind holds past its capacity, once values pinned when they were kept are not.
  pub(crate) fn trim(&self) {
    self.pool.trim(V::KIND);
  }

  /// Keeps `value` under `key`.
  pub(crate) fn insert(&self, key: V::Key, value: V) {
    let budget = self.budget_slot().load();

    self
      .pool
      .store_for(budget.as_deref())
      .insert(V::pool_key(key), value.into_entry());
    self.pool.kept(V::KIND, budget.as_deref());
  }

  /// The value kept under `key`, `compute`d once across concurrent misses and kept unless it fails.
  pub(crate) fn get_or_insert_with<E>(
    &self,
    key: V::Key,
    compute: impl FnOnce() -> Result<V, E>,
  ) -> Result<V, E> {
    let budget = self.budget_slot().load();
    let found = self
      .pool
      .store_for(budget.as_deref())
      .get_value_or_guard(&V::pool_key(key), None);

    self
      .counts()
      .looked_up(matches!(found, GuardResult::Value(_)));

    match found {
      GuardResult::Value(entry) => V::from_entry(entry).map_or_else(compute, Ok),
      GuardResult::Guard(guard) => {
        let value = compute()?;
        let _ = guard.insert(value.clone().into_entry());

        self.pool.kept(V::KIND, budget.as_deref());
        Ok(value)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => compute(),
    }
  }
}

impl KindStore<Arc<HeldFont>> {
  /// Keeps fonts from moving between stores while the guard lives.
  pub(crate) fn hold_still(&self) -> RwLockReadGuard<'_, ()> {
    self
      .pool
      .fonts_moving
      .read()
      .unwrap_or_else(PoisonError::into_inner)
  }

  /// The ids of the fonts evicted since the last call, without the ones kept again since.
  pub(crate) fn take_evicted(&self) -> Vec<u64> {
    let mut evicted = take(
      &mut *self
        .pool
        .lifecycle
        .evicted_fonts
        .lock()
        .unwrap_or_else(PoisonError::into_inner),
    );

    evicted.sort_unstable();
    evicted.dedup();
    evicted.retain(|&id| self.peek(id).is_none());

    evicted
  }
}

/// Counts the entries each kind loses to eviction, keeps a held font from eviction and records the
/// fonts evicted, and releases entries as `quick_cache`'s default lifecycle does.
#[derive(Clone)]
struct PoolLifecycle {
  counts: Arc<PerKind<KindCounts>>,
  /// The ids of the fonts evicted since [`HeldFonts::take_evicted`] last took them.
  evicted_fonts: Arc<Mutex<Vec<u64>>>,
  release: DefaultLifecycle<PoolKey, PoolEntry>,
}

impl Default for PoolLifecycle {
  fn default() -> Self {
    Self {
      counts: Arc::new(PerKind::from_fn(|_| KindCounts::default())),
      evicted_fonts: Arc::default(),
      release: DefaultLifecycle::default(),
    }
  }
}

impl Lifecycle<PoolKey, PoolEntry> for PoolLifecycle {
  type RequestState =
    <DefaultLifecycle<PoolKey, PoolEntry> as Lifecycle<PoolKey, PoolEntry>>::RequestState;

  fn is_pinned(&self, _key: &PoolKey, entry: &PoolEntry) -> bool {
    matches!(entry, PoolEntry::Font(font) if font.is_held())
  }

  fn on_evict(&self, state: &mut Self::RequestState, key: PoolKey, entry: PoolEntry) {
    self.counts[key.kind()]
      .evictions
      .fetch_add(1, Ordering::Relaxed);

    if let PoolKey::Font(id) = key {
      self
        .evicted_fonts
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(id);
    }

    self.release.on_evict(state, key, entry);
  }
}

/// A store holding at most `max_bytes`, sized for entries of about `entry_bytes`, with the
/// lifecycle every store of the pool shares.
fn store(max_bytes: u64, entry_bytes: u64, lifecycle: &PoolLifecycle) -> PoolStore {
  let estimated_items = (max_bytes / entry_bytes).max(1) as usize;
  let parallelism = available_parallelism().map_or(1, |n| n.get() as u64);
  let shards = (max_bytes / SHARD_BYTES).clamp(1, parallelism);
  let shards = if shards.is_power_of_two() {
    shards
  } else {
    shards.next_power_of_two() / 2
  } as usize;
  let options = OptionsBuilder::new()
    .estimated_items_capacity(estimated_items)
    .weight_capacity(max_bytes)
    .shards(shards)
    .build();
  match options {
    Ok(options) => Store::with_options(
      options,
      ByBytes,
      DefaultHashBuilder::default(),
      lifecycle.clone(),
    ),
    Err(_) => Store::with(
      estimated_items,
      max_bytes,
      ByBytes,
      DefaultHashBuilder::default(),
      lifecycle.clone(),
    ),
  }
}

/// One kind of data the pool keeps, as a variant of its key and of its entry.
pub(crate) trait Kept: Clone {
  /// What finds a value of this kind.
  type Key;
  /// The budget that covers values of this kind.
  const KIND: Kind;

  fn pool_key(key: Self::Key) -> PoolKey;
  fn into_entry(self) -> PoolEntry;
  fn from_entry(entry: PoolEntry) -> Option<Self>;
}

/// Declares the kinds of data the pool keeps: the variants of [`PoolKey`] and [`PoolEntry`], and
/// the [`Kept`] impl that moves each kind in and out of them.
macro_rules! kinds {
  ($($(#[$attr:meta])* $variant:ident($key:ty) => $value:ty, $kind:ident;)*) => {
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub(crate) enum PoolKey {
      $($(#[$attr])* $variant($key),)*
    }

    impl PoolKey {
      fn kind(&self) -> Kind {
        match self {
          $($(#[$attr])* Self::$variant(_) => Kind::$kind,)*
        }
      }
    }

    #[derive(Clone)]
    pub(crate) enum PoolEntry {
      $($(#[$attr])* $variant($value),)*
    }

    impl Weighed for PoolEntry {
      fn bytes(&self) -> usize {
        match self {
          $($(#[$attr])* Self::$variant(value) => value.bytes(),)*
        }
      }
    }

    $(
      $(#[$attr])*
      impl Kept for $value {
        type Key = $key;
        const KIND: Kind = Kind::$kind;

        fn pool_key(key: $key) -> PoolKey {
          PoolKey::$variant(key)
        }

        fn into_entry(self) -> PoolEntry {
          PoolEntry::$variant(self)
        }

        fn from_entry(entry: PoolEntry) -> Option<Self> {
          match entry {
            PoolEntry::$variant(value) => Some(value),
            _ => None,
          }
        }
      }
    )*
  };
}

kinds! {
  Image(u64) => ImageSource, Images;
  SizedImage((u64, SizedKey)) => Arc<ImageBuffer>, Images;
  #[cfg(feature = "svg")]
  Picture(u64) => Arc<SvgPicture>, Pictures;
  Stylesheet(u64) => SheetEntry, Stylesheets;
  Tailwind(u64) => CompiledClasses, Tailwind;
  Glyph(GlyphKey) => GlyphEntry, Glyphs;
  Intercept(InterceptKey) => CachedSpans, Intercepts;
  #[cfg(feature = "font-face")]
  FontFaces(DocumentKey) => CachedRegistry, FontFaces;
  Font(u64) => Arc<HeldFont>, Fonts;
}

/// How large an image draws: its buffer size and the filter it scales with.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SizedKey {
  /// A hash of whatever else the buffer depends on, such as an SVG raster's host.
  variant: u64,
  width: u32,
  height: u32,
  algorithm: u8,
}

impl SizedKey {
  pub(crate) fn new(
    variant: u64,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Self {
    Self {
      variant,
      width,
      height,
      algorithm: match algorithm {
        ImageScalingAlgorithm::Smooth => 1,
        ImageScalingAlgorithm::Pixelated => 2,
        _ => 0,
      },
    }
  }
}

#[derive(Clone)]
pub(crate) struct SheetEntry {
  sheet: Arc<StyleSheet>,
  bytes: usize,
}

/// An entry's approximate retained size in bytes.
pub(crate) trait Weighed {
  fn bytes(&self) -> usize;
}

impl Weighed for ImageSource {
  fn bytes(&self) -> usize {
    self.estimated_bytes()
  }
}

impl Weighed for Arc<ImageBuffer> {
  fn bytes(&self) -> usize {
    self.data().len()
  }
}

#[cfg(feature = "svg")]
impl Weighed for Arc<SvgPicture> {
  fn bytes(&self) -> usize {
    self.estimated_bytes()
  }
}

impl Weighed for SheetEntry {
  fn bytes(&self) -> usize {
    self.bytes
  }
}

#[derive(Clone)]
struct ByBytes;

impl Weighter<PoolKey, PoolEntry> for ByBytes {
  fn weight(&self, _key: &PoolKey, entry: &PoolEntry) -> u64 {
    (entry.bytes() as u64).max(1)
  }
}

#[cfg(all(test, feature = "png"))]
mod tests {
  use std::{sync::Arc, thread};

  use super::*;
  #[cfg(feature = "svg-raster")]
  use crate::{
    Fonts, context::RenderContext, resources::image::RenderedImage, style::SizingContext,
    viewport::Viewport,
  };

  /// PNG bytes that decode to a tiny bitmap.
  fn png_bytes() -> Vec<u8> {
    ImageBuffer::new(2, 2).unwrap().encode_png().unwrap()
  }

  fn sized(width: u32, height: u32) -> SizedKey {
    SizedKey::new(0, width, height, ImageScalingAlgorithm::Auto)
  }

  /// Keeps a `side`×`side` buffer for `source`, `width × height × 4` bytes.
  fn keep_buffer(cache: &Cache, source: u64, side: u32) {
    let _ = cache.sized(Some(source), sized(side, side), || {
      Ok(Arc::new(ImageBuffer::new(side, side).unwrap()))
    });
  }

  /// Keeps a stylesheet of about `bytes` bytes, unique to `seed`.
  fn keep_sheet(cache: &Cache, seed: u64, bytes: usize) -> Arc<StyleSheet> {
    let mut source = format!(".s{seed} {{ color: red }}");

    source.push_str(&" ".repeat(bytes / 3 - source.len()));
    cache.parse_stylesheet(vec![source])
  }

  fn pool_weight(cache: &Cache) -> u64 {
    cache.0.pool.store.weight()
  }

  fn pool_capacity(cache: &Cache) -> u64 {
    cache.0.pool.store.capacity()
  }

  /// Whether `cache` still keeps the buffer [`keep_buffer`] kept for `source`.
  fn keeps_buffer(cache: &Cache, source: u64, side: u32) -> bool {
    cache
      .0
      .sized_images
      .get((source, sized(side, side)))
      .is_some()
  }

  #[test]
  fn decodes_and_reuses_on_hit() {
    let cache = Cache::default();
    let bytes = png_bytes();

    let first = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();
    let second = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();

    match (&first, &second) {
      (ImageSource::Encoded(x), ImageSource::Encoded(y)) => assert!(Arc::ptr_eq(x, y)),
      _ => panic!("expected encoded bitmaps"),
    }
  }

  #[test]
  fn an_uncached_decode_keeps_nothing() {
    let cache = Cache::default();
    let bytes = png_bytes();

    let a = cache.decode_image(&bytes, ImageCacheMode::None).unwrap();
    let b = cache.decode_image(&bytes, ImageCacheMode::None).unwrap();

    match (&a, &b) {
      (ImageSource::Bitmap(x), ImageSource::Bitmap(y)) => assert!(!Arc::ptr_eq(x, y)),
      _ => panic!("expected bitmaps"),
    }
  }

  /// The raster an SVG draws is kept in the cache of the render that draws it, not the one that
  /// decoded it.
  #[cfg(feature = "svg-raster")]
  #[test]
  fn an_svg_raster_is_kept_by_the_render_that_draws_it() {
    let decoding = Cache::default();
    let drawing = Cache::default();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#;
    let source = decoding.decode_image(svg, ImageCacheMode::Auto).unwrap();
    let raster = |cache: &Cache| {
      let host = RenderContext::builder()
        .fonts(Fonts::default().snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::new((4, 4)))
            .build(),
        )
        .cache(cache.clone())
        .build();

      match source
        .render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0, &host)
        .unwrap()
      {
        RenderedImage::Rasterized(buffer) => buffer,
        _ => panic!("expected rasterized"),
      }
    };

    assert!(Arc::ptr_eq(&raster(&drawing), &raster(&drawing)));
    assert!(!Arc::ptr_eq(&raster(&drawing), &raster(&Cache::default())));
  }

  #[test]
  fn a_stylesheet_is_parsed_once() {
    let cache = Cache::default();
    let sources = vec![".a { color: red; }".to_string()];

    let first = cache.parse_stylesheet(sources.clone());
    let second = cache.parse_stylesheet(sources);

    assert!(Arc::ptr_eq(&first, &second));
  }

  #[test]
  fn a_sized_entry_near_the_size_is_retained() {
    let cache = Cache::builder().max_bytes(16 << 20).build();
    let buffer = Arc::new(ImageBuffer::new(2400, 1601).unwrap());
    let kept = cache
      .sized(Some(1), sized(2400, 1601), || Ok(buffer.clone()))
      .unwrap();

    assert!(Arc::ptr_eq(
      &kept,
      &cache
        .sized(Some(1), sized(2400, 1601), || unreachable!())
        .unwrap()
    ));
  }

  #[test]
  fn an_uneven_size_keeps_a_whole_shard_per_entry() {
    let cache = Cache::builder().max_bytes(192 << 20).build();
    let buffer = Arc::new(ImageBuffer::new(4000, 3900).unwrap());

    cache
      .sized(Some(1), sized(4000, 3900), || Ok(buffer.clone()))
      .unwrap();

    assert!(
      cache
        .sized(Some(1), sized(4000, 3900), || unreachable!())
        .is_ok()
    );
  }

  #[test]
  fn a_zero_size_keeps_nothing() {
    let cache = Cache::builder().max_bytes(0).images(1 << 20).build();
    let bytes = png_bytes();
    let sheet = || cache.parse_stylesheet(vec![".a { color: red }".to_owned()]);

    let a = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();
    let b = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();

    match (&a, &b) {
      (ImageSource::Encoded(x), ImageSource::Encoded(y)) => assert!(!Arc::ptr_eq(x, y)),
      _ => panic!("expected encoded bitmaps"),
    }
    assert!(!Arc::ptr_eq(&sheet(), &sheet()));
  }

  #[test]
  fn a_zero_budget_keeps_none_of_its_kind() {
    let cache = Cache::builder().images(0).build();
    let bytes = png_bytes();
    let sheet = || cache.parse_stylesheet(vec![".a { color: red }".to_owned()]);

    let a = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();
    let b = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();

    match (&a, &b) {
      (ImageSource::Encoded(x), ImageSource::Encoded(y)) => assert!(!Arc::ptr_eq(x, y)),
      _ => panic!("expected encoded bitmaps"),
    }
    assert!(Arc::ptr_eq(&sheet(), &sheet()));
  }

  #[test]
  fn eviction_stays_within_the_size() {
    let max_bytes = 4096;
    let cache = Cache::builder().max_bytes(max_bytes).build();

    for source in 0..64 {
      keep_buffer(&cache, source, 16);
    }

    assert!(pool_weight(&cache) <= max_bytes);
  }

  #[test]
  fn kinds_share_the_pool() {
    let max_bytes = 4 << 20;
    let cache = Cache::builder().max_bytes(max_bytes).build();
    let images_kept = || {
      (0..4)
        .filter(|&source| {
          cache
            .0
            .sized_images
            .get((source, sized(512, 512)))
            .is_some()
        })
        .count()
    };

    for source in 0..4 {
      keep_buffer(&cache, source, 512);
    }
    assert!(pool_weight(&cache) > 3 << 20);

    for seed in 0..64 {
      keep_sheet(&cache, seed, 64 << 10);
    }

    assert!(images_kept() < 4);
    assert!(Arc::ptr_eq(
      &keep_sheet(&cache, 63, 64 << 10),
      &keep_sheet(&cache, 63, 64 << 10)
    ));
    assert!(pool_weight(&cache) <= max_bytes);
  }

  #[test]
  fn a_budget_caps_its_kind_and_charges_only_what_it_holds() {
    let max_bytes = 16 << 20;
    let budget = 4 << 20;
    let cache = Cache::builder().max_bytes(max_bytes).images(budget).build();
    let images = || cache.stats().images.current;

    assert_eq!(pool_capacity(&cache), max_bytes);

    keep_buffer(&cache, 0, 512);
    assert_eq!(pool_capacity(&cache), max_bytes - (1 << 20));

    for source in 1..16 {
      keep_buffer(&cache, source, 512);
    }

    assert!(images() <= budget);
    assert!(images() >= 3 << 20);
    assert_eq!(
      pool_capacity(&cache),
      max_bytes - images().next_multiple_of(CHARGE_STEP)
    );
  }

  #[test]
  fn budgets_together_hold_at_most_the_size() {
    let max_bytes = 4 << 20;
    let cache = Cache::builder()
      .max_bytes(max_bytes)
      .images(3 << 20)
      .stylesheets(3 << 20)
      .build();
    for source in 0..3 {
      keep_buffer(&cache, source, 512);
    }
    for seed in 0..64 {
      keep_sheet(&cache, seed, 64 << 10);
    }

    let stats = cache.stats();
    let images = stats.images.current;
    let sheets = stats.stylesheets.current;

    assert!(images >= 2 << 20, "{images}");
    assert!(
      images + sheets <= max_bytes + CHARGE_STEP,
      "{images} + {sheets}"
    );
  }

  #[test]
  fn a_budget_is_capped_at_the_size() {
    let cache = Cache::builder().max_bytes(1 << 20).images(8 << 20).build();

    assert_eq!(cache.0.images.budget(), Some(1 << 20));
  }

  /// A budget the others squeezed to nothing gets room once a shrinking budget releases it, in the
  /// same `set_options` call and on its next insert.
  #[test]
  fn a_squeezed_budget_grows_into_what_another_releases() {
    let max_bytes = 4 << 20;
    let options = |stylesheets| CacheOptions {
      max_bytes,
      images: Some(2 << 20),
      stylesheets: Some(stylesheets),
      ..CacheOptions::default()
    };
    let cache = Cache::builder().max_bytes(max_bytes).build();
    let images_held = || cache.stats().images.current;

    cache.set_options(options(max_bytes));

    for seed in 0..64 {
      keep_sheet(&cache, seed, 64 << 10);
    }

    cache.set_options(options(1 << 20));
    keep_buffer(&cache, 0, 512);

    assert!(images_held() >= 1 << 20, "{}", images_held());
  }

  /// The shared store resizes once every budget fits the new options, so it never evicts entries
  /// that fit them on the way there.
  #[test]
  fn new_options_shrink_the_shared_store_only_to_their_final_size() {
    let mib = 1 << 20;
    let cache = Cache::builder()
      .max_bytes(16 * mib)
      .images(4 * mib)
      .stylesheets(4 * mib)
      .build();

    for source in 0..4 {
      keep_buffer(&cache, source, 512);
    }
    for seed in 0..64 {
      keep_sheet(&cache, seed, 64 << 10);
    }
    for seed in 0..32 {
      cache
        .tailwind()
        .compile(&format!("w-{seed} {}", " ".repeat(64 << 10)));
    }

    let shared = cache.stats().tailwind.current;

    assert!(shared >= 2 * mib, "{shared}");

    cache.set_options(CacheOptions {
      max_bytes: 6 * mib,
      images: Some(mib),
      stylesheets: Some(mib),
      ..CacheOptions::default()
    });

    assert_eq!(cache.stats().tailwind.evictions, 0);
    assert_eq!(cache.stats().tailwind.current, shared);
  }

  #[test]
  fn a_smaller_size_evicts_at_once() {
    let cache = Cache::builder().max_bytes(4 << 20).build();

    for source in 0..3 {
      keep_buffer(&cache, source, 512);
    }
    assert!(pool_weight(&cache) >= 3 << 20);

    cache.set_options(CacheOptions {
      max_bytes: 1 << 20,
      ..CacheOptions::default()
    });

    assert!(pool_weight(&cache) <= 1 << 20);
    assert!(cache.stats().images.evictions >= 2);
    assert_eq!(cache.stats().memory.max, 1 << 20);
  }

  #[test]
  fn a_larger_size_keeps_what_the_cache_holds() {
    let cache = Cache::builder().max_bytes(2 << 20).build();

    keep_buffer(&cache, 0, 512);
    cache.set_options(CacheOptions {
      max_bytes: 8 << 20,
      ..CacheOptions::default()
    });

    assert!(keeps_buffer(&cache, 0, 512));

    for source in 1..6 {
      keep_buffer(&cache, source, 512);
    }

    assert!((0..6).all(|source| keeps_buffer(&cache, source, 512)));
  }

  #[test]
  fn a_new_budget_drops_its_kind_from_the_pool() {
    let cache = Cache::builder().max_bytes(16 << 20).build();
    let sheet = keep_sheet(&cache, 0, 64 << 10);

    keep_buffer(&cache, 0, 512);
    cache.set_options(CacheOptions {
      max_bytes: 16 << 20,
      images: Some(2 << 20),
      ..CacheOptions::default()
    });

    assert!(!keeps_buffer(&cache, 0, 512));
    assert!(Arc::ptr_eq(&sheet, &keep_sheet(&cache, 0, 64 << 10)));

    for source in 0..4 {
      keep_buffer(&cache, source, 512);
    }

    let images = cache.stats().images;

    assert_eq!(images.max, Some(2 << 20));
    assert!(images.current <= 2 << 20);
    assert_eq!(pool_capacity(&cache), (16 << 20) - (2 << 20));
  }

  #[test]
  fn a_removed_budget_returns_its_kind_to_the_pool() {
    let max_bytes = 16 << 20;
    let cache = Cache::builder()
      .max_bytes(max_bytes)
      .images(4 << 20)
      .build();

    keep_buffer(&cache, 0, 512);
    assert_eq!(pool_capacity(&cache), max_bytes - (1 << 20));

    cache.set_options(CacheOptions {
      max_bytes,
      ..CacheOptions::default()
    });

    assert_eq!(pool_capacity(&cache), max_bytes);
    assert_eq!(cache.stats().images.max, None);
    assert!(!keeps_buffer(&cache, 0, 512));

    keep_buffer(&cache, 0, 512);

    assert!(keeps_buffer(&cache, 0, 512));
    assert!(pool_weight(&cache) >= 1 << 20);
  }

  #[test]
  fn a_removed_entry_returns_its_charge_to_the_pool() {
    let max_bytes = 16 << 20;
    let cache = Cache::builder().max_bytes(max_bytes).fonts(4 << 20).build();

    drop(cache.fonts().keep(1, 64 << 10));
    assert_eq!(pool_capacity(&cache), max_bytes - (1 << 20));

    cache.fonts().forget(1);

    assert_eq!(pool_capacity(&cache), max_bytes);
  }

  #[test]
  fn a_smaller_budget_evicts_at_once() {
    let max_bytes = 16 << 20;
    let cache = Cache::builder()
      .max_bytes(max_bytes)
      .images(4 << 20)
      .build();

    for source in 0..3 {
      keep_buffer(&cache, source, 512);
    }

    cache.set_options(CacheOptions {
      max_bytes,
      images: Some(1 << 20),
      ..CacheOptions::default()
    });

    let images = cache.stats().images;

    assert!(images.current <= 1 << 20, "{}", images.current);
    assert!(images.evictions >= 2);
    assert_eq!(pool_capacity(&cache), max_bytes - (1 << 20));
  }

  #[test]
  fn stats_count_each_kind_apart() {
    let cache = Cache::builder().max_bytes(1 << 20).build();
    let bytes = png_bytes();

    for _ in 0..2 {
      cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();
      keep_sheet(&cache, 0, 64 << 10);
    }

    let stats = cache.stats();

    assert_eq!((stats.images.hits, stats.images.misses), (1, 1));
    assert_eq!((stats.stylesheets.hits, stats.stylesheets.misses), (1, 1));
    assert_eq!(stats.stylesheets.evictions, 0);
    assert!(stats.stylesheets.current >= 60 << 10);
    assert_eq!(
      stats.memory.current,
      stats.images.current + stats.stylesheets.current
    );

    for source in 0..8 {
      keep_buffer(&cache, source, 256);
    }

    let stats = cache.stats();

    assert!(stats.images.evictions >= 4, "{}", stats.images.evictions);
    assert!(stats.memory.current <= 1 << 20);
    assert!(stats.memory.high >= stats.memory.current);
  }

  #[test]
  fn the_high_mark_outlasts_a_smaller_size() {
    let cache = Cache::builder().max_bytes(4 << 20).build();

    for seed in 0..3 {
      keep_sheet(&cache, seed, 64 << 10);
    }

    let held = cache.stats().memory;

    cache.set_options(CacheOptions {
      max_bytes: 0,
      ..CacheOptions::default()
    });

    let memory = cache.stats().memory;

    assert!(held.current >= 3 * (60 << 10), "{}", held.current);
    assert_eq!(held.high, held.current);
    assert_eq!(memory.current, 0);
    assert_eq!(memory.high, held.high);
  }

  #[test]
  fn options_change_while_renders_keep_entries() {
    let max_bytes = 8 << 20;
    let cache = Cache::builder().max_bytes(max_bytes).build();
    let budgeted = CacheOptions {
      max_bytes,
      images: Some(2 << 20),
      stylesheets: Some(1 << 20),
      ..CacheOptions::default()
    };

    thread::scope(|scope| {
      for worker in 0..4 {
        let cache = &cache;

        scope.spawn(move || {
          for round in 0..64 {
            keep_buffer(cache, worker * 64 + round, 256);
            keep_sheet(cache, worker * 64 + round, 16 << 10);
          }
        });
      }

      for round in 0..32 {
        cache.set_options(if round % 2 == 0 {
          budgeted
        } else {
          CacheOptions {
            max_bytes: max_bytes / 2,
            ..CacheOptions::default()
          }
        });
      }
    });

    cache.set_options(budgeted);

    let stats = cache.stats();

    assert!(stats.images.current <= 2 << 20);
    assert!(stats.stylesheets.current <= 1 << 20);
    assert!(stats.memory.current <= max_bytes + CHARGE_STEP);
    assert_eq!(
      pool_capacity(&cache),
      max_bytes
        - (stats.images.current.next_multiple_of(CHARGE_STEP)
          + stats.stylesheets.current.next_multiple_of(CHARGE_STEP))
    );
  }

  /// Sets the old default glyph budget, which every cache built after it in this process takes.
  #[test]
  fn the_old_glyph_setter_gives_later_caches_a_glyph_budget() {
    let mask_bytes = |cache: Cache| cache.glyphs().max_entry_bytes();

    assert_eq!(
      mask_bytes(Cache::builder().glyphs(1 << 20).build()),
      32 << 10
    );

    #[allow(deprecated)]
    glyph_cache::set_glyph_cache_max_bytes(4 << 20);

    assert_eq!(mask_bytes(Cache::default()), 128 << 10);
    assert_eq!(
      mask_bytes(Cache::builder().glyphs(1 << 20).build()),
      32 << 10
    );
  }

  #[test]
  fn a_shared_cache_is_one_cache() {
    let bytes = png_bytes();
    let first = Cache::shared()
      .decode_image(&bytes, ImageCacheMode::Auto)
      .unwrap();
    let second = Cache::shared()
      .decode_image(&bytes, ImageCacheMode::Auto)
      .unwrap();

    match (&first, &second) {
      (ImageSource::Encoded(x), ImageSource::Encoded(y)) => assert!(Arc::ptr_eq(x, y)),
      _ => panic!("expected encoded bitmaps"),
    }
  }
}

//! What renders keep for the renders after them, in one pool of bytes every kind of data shares.

use std::{
  array,
  convert::Infallible,
  marker::PhantomData,
  mem::take,
  ops::{Index, IndexMut},
  sync::{
    Arc, LazyLock, Mutex, MutexGuard, PoisonError,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  thread::available_parallelism,
};

use quick_cache::{
  DefaultHashBuilder, Lifecycle, OptionsBuilder, Weighter,
  sync::{Cache as Store, DefaultLifecycle, GuardResult},
};
use typed_builder::TypedBuilder;
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

#[cfg(feature = "font-face")]
use crate::resources::font::{DocumentKey, DocumentRegistries, FontRegistry};
#[cfg(feature = "svg")]
use crate::{geometry::Size, resources::image::SvgPicture, svg::HostReads};
use crate::{
  layout::intercept::{InterceptCache, InterceptKey, Spans},
  resources::{
    glyph_cache::{self, CachedGlyph, GlyphCache, GlyphKey},
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
  #[cfg(feature = "svg")]
  host_reads: KindStore<HostReads>,
  stylesheets: KindStore<Arc<StyleSheet>>,
  tailwind: TailwindCache,
  glyphs: GlyphCache,
  intercepts: InterceptCache,
  #[cfg(feature = "font-face")]
  font_faces: DocumentRegistries,
  fonts: HeldFonts,
}

/// The bytes a [`Cache`] keeps, and which kinds of data it keeps in them. `0` keeps nothing.
#[derive(TypedBuilder, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[builder(build_method(into = Cache))]
pub struct CacheOptions {
  /// The bytes every kind of data shares. Approximate: entries are weighed by estimated size.
  #[builder(default = CacheOptions::DEFAULT_MAX_BYTES)]
  pub max_bytes: u64,
  /// Whether to keep decoded images, and the SVG rasters and downscaled bitmaps drawn from them.
  #[builder(default)]
  pub images: ImageCacheMode,
  /// Whether to keep resolved glyphs, and the masks rasterized from glyphs and shapes. Without one,
  /// what the deprecated [`set_glyph_cache_max_bytes`](glyph_cache::set_glyph_cache_max_bytes)
  /// set.
  #[builder(default = glyph_cache::default_mode())]
  pub glyphs: ImageCacheMode,
  /// Whether to keep the decoded font files a host holds, such as the fonts a binding registers,
  /// once no render leases or pins them.
  #[builder(default)]
  pub fonts: ImageCacheMode,
}

impl CacheOptions {
  /// The bytes a cache keeps unless its builder sets `max_bytes`.
  pub const DEFAULT_MAX_BYTES: u64 = 128 << 20;

  /// Whether each kind is kept.
  fn kept(&self) -> PerKind<bool> {
    PerKind::from_fn(|kind| {
      let mode = match kind {
        Kind::Images => self.images,
        Kind::Glyphs => self.glyphs,
        // A leased font is held whatever the mode, which `HeldFonts` applies once it is not.
        Kind::Fonts | Kind::Other => ImageCacheMode::Auto,
      };

      matches!(mode, ImageCacheMode::Auto)
    })
  }
}

impl Default for CacheOptions {
  fn default() -> Self {
    Self {
      max_bytes: Self::DEFAULT_MAX_BYTES,
      images: ImageCacheMode::default(),
      glyphs: glyph_cache::default_mode(),
      fonts: ImageCacheMode::default(),
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
      #[cfg(feature = "svg")]
      host_reads: KindStore::new(&pool),
      stylesheets: KindStore::new(&pool),
      tailwind: KindStore::new(&pool),
      glyphs: GlyphCache::new(KindStore::new(&pool)),
      intercepts: KindStore::new(&pool),
      #[cfg(feature = "font-face")]
      font_faces: KindStore::new(&pool),
      fonts: HeldFonts::new(KindStore::new(&pool), options.fonts),
      pool,
    }))
  }
}

impl Default for Cache {
  fn default() -> Self {
    CacheOptions::builder().build()
  }
}

impl Cache {
  /// The cache every render without one of its own shares, with the default size.
  pub fn shared() -> Self {
    static SHARED: LazyLock<Cache> = LazyLock::new(Cache::default);

    SHARED.clone()
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

    self.0.images.get_or_insert_with(hash, || {
      let source = ImageSource::from_bytes_keyed(bytes, hash)?;
      let bytes = source.estimated_bytes();

      Ok(Weighed::new(source, bytes))
    })
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
        Ok::<_, Infallible>(Weighed::new(
          Arc::new(StyleSheet::parse_owned_list_loosy(sources)),
          bytes,
        ))
      })
      .unwrap_or_else(|never| match never {})
  }

  /// The picture kept under `key`, `lay_out` once across concurrent misses.
  #[cfg(feature = "svg")]
  pub(crate) fn picture(&self, key: u64, lay_out: impl FnOnce() -> SvgPicture) -> Arc<SvgPicture> {
    self
      .0
      .pictures
      .get_or_insert_with(key, || {
        let picture = Arc::new(lay_out());
        let bytes = picture.estimated_bytes();

        Ok::<_, Infallible>(Weighed::new(picture, bytes))
      })
      .unwrap_or_else(|never| match never {})
  }

  /// What the layout of the SVG source `source` at `viewport` read from its host, once one said,
  /// for the content of an `svg` element when `svg_element`.
  #[cfg(feature = "svg")]
  pub(crate) fn host_reads(
    &self,
    source: u64,
    viewport: Size<f32>,
    svg_element: bool,
  ) -> Option<HostReads> {
    self
      .0
      .host_reads
      .get(host_reads_key(source, viewport, svg_element))
  }

  /// Keeps what the layout of the SVG source `source` at `viewport` read from its host, for the
  /// content of an `svg` element when `svg_element`.
  #[cfg(feature = "svg")]
  pub(crate) fn keep_host_reads(
    &self,
    source: u64,
    viewport: Size<f32>,
    svg_element: bool,
    reads: HostReads,
  ) {
    self.0.host_reads.insert(
      host_reads_key(source, viewport, svg_element),
      Weighed::new(reads, HOST_READS_BYTES),
    );
  }

  /// The glyphs and masks the cache keeps.
  #[doc(hidden)]
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
  #[doc(hidden)]
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

    self.0.sized_images.get_or_insert_with((source, sized), || {
      let buffer = compute()?;
      let bytes = buffer.data().len();

      Ok(Weighed::new(buffer, bytes))
    })
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
  /// [`CacheOptions::glyphs`].
  pub glyphs: CacheKindStats,
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
}

/// What a [`Cache`] holds of one kind of data, and how lookups of that kind went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheKindStats {
  /// The bytes held now.
  pub current: u64,
  /// Lookups that found an entry.
  pub hits: u64,
  /// Lookups that found none.
  pub misses: u64,
  /// Entries dropped to stay within the bytes, counting entries too large to keep at all.
  pub evictions: u64,
}

/// A kind of data, one for each mode of [`CacheOptions`], and the data no mode covers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  Images,
  Glyphs,
  Fonts,
  /// SVG pictures, stylesheets, Tailwind classes, decoration intercepts, and `@font-face` faces,
  /// which are always kept.
  Other,
}

const KINDS: usize = 4;

impl Kind {
  const ALL: [Self; KINDS] = [Self::Images, Self::Glyphs, Self::Fonts, Self::Other];
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

  fn stats(&self, current: u64) -> CacheKindStats {
    CacheKindStats {
      current,
      hits: self.hits.load(Ordering::Relaxed),
      misses: self.misses.load(Ordering::Relaxed),
      evictions: self.evictions.load(Ordering::Relaxed),
    }
  }
}

type PoolStore = Store<PoolKey, Weighed<PoolEntry>, ByBytes, DefaultHashBuilder, PoolLifecycle>;

/// The bytes every kind shares, in one store keyed by kind.
struct Pool {
  store: PoolStore,
  max_bytes: u64,
  kept: PerKind<bool>,
  /// The most bytes the pool held after an entry was kept.
  high: AtomicU64,
  lifecycle: PoolLifecycle,
}

impl Pool {
  fn new(options: CacheOptions) -> Self {
    let lifecycle = PoolLifecycle::default();

    Self {
      store: store(options.max_bytes, &lifecycle),
      max_bytes: options.max_bytes,
      kept: options.kept(),
      high: AtomicU64::new(0),
      lifecycle,
    }
  }

  /// Evicts what the pool holds past its size, as an entry that was pinned when it was kept no
  /// longer is.
  fn trim(&self) {
    let capacity = self.store.capacity();

    if self.store.weight() > capacity {
      self.store.set_capacity(capacity);
    }
  }

  /// Records the bytes held after an entry was kept.
  fn kept_one(&self) {
    self.high.fetch_max(self.store.weight(), Ordering::Relaxed);
  }

  fn stats(&self) -> CacheStats {
    let kinds = self.kind_stats();
    let current = kinds.0.iter().map(|kind| kind.current).sum();

    CacheStats {
      memory: CacheMemoryStats {
        current,
        high: self.high.fetch_max(current, Ordering::Relaxed).max(current),
      },
      images: kinds[Kind::Images],
      glyphs: kinds[Kind::Glyphs],
      fonts: kinds[Kind::Fonts],
    }
  }

  /// What each kind holds and how its lookups went.
  fn kind_stats(&self) -> PerKind<CacheKindStats> {
    let mut held = PerKind::from_fn(|_| 0);

    for (key, entry) in self.store.iter() {
      held[key.kind()] += ByBytes.weight(&key, &entry);
    }

    PerKind::from_fn(|kind| self.lifecycle.counts[kind].stats(held[kind]))
  }
}

/// Where one kind of data is kept in the pool, keeping nothing when its mode is
/// [`ImageCacheMode::None`].
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

  /// Whether the cache keeps this kind.
  fn kept(&self) -> bool {
    self.pool.kept[V::KIND]
  }

  fn counts(&self) -> &KindCounts {
    &self.pool.lifecycle.counts[V::KIND]
  }

  /// The bytes the whole cache keeps.
  pub(crate) fn max_bytes(&self) -> u64 {
    self.pool.max_bytes
  }

  /// The value kept under `key`.
  pub(crate) fn get(&self, key: V::Key) -> Option<V> {
    if !self.kept() {
      return None;
    }

    let value = self
      .pool
      .store
      .get(&V::pool_key(key))
      .and_then(|entry| V::from_entry(entry.value));

    self.counts().looked_up(value.is_some());

    value
  }

  /// The value kept under `key`, without counting a lookup or marking it used.
  pub(crate) fn peek(&self, key: V::Key) -> Option<V> {
    self
      .pool
      .store
      .peek(&V::pool_key(key))
      .and_then(|entry| V::from_entry(entry.value))
  }

  /// Drops the value kept under `key`, which counts as no eviction.
  pub(crate) fn remove(&self, key: V::Key) {
    self.pool.store.remove(&V::pool_key(key));
  }

  /// Evicts what the pool holds past its size, once values pinned when they were kept are not.
  pub(crate) fn trim(&self) {
    self.pool.trim();
  }

  /// Keeps `value` under `key`.
  pub(crate) fn insert(&self, key: V::Key, value: Weighed<V>) {
    if !self.kept() {
      return;
    }

    self.pool.store.insert(V::pool_key(key), value.into_entry());
    self.pool.kept_one();
  }

  /// The value kept under `key`, `compute`d once across concurrent misses and kept unless it fails.
  pub(crate) fn get_or_insert_with<E>(
    &self,
    key: V::Key,
    compute: impl FnOnce() -> Result<Weighed<V>, E>,
  ) -> Result<V, E> {
    if !self.kept() {
      return compute().map(|weighed| weighed.value);
    }

    let found = self.pool.store.get_value_or_guard(&V::pool_key(key), None);

    self
      .counts()
      .looked_up(matches!(found, GuardResult::Value(_)));

    match found {
      GuardResult::Value(entry) => match V::from_entry(entry.value) {
        Some(value) => Ok(value),
        None => compute().map(|weighed| weighed.value),
      },
      GuardResult::Guard(guard) => {
        let weighed = compute()?;
        let value = weighed.value.clone();
        let _ = guard.insert(weighed.into_entry());

        self.pool.kept_one();
        Ok(value)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => compute().map(|weighed| weighed.value),
    }
  }
}

impl KindStore<Arc<HeldFont>> {
  /// Evicts the font kept under `id` unless a lease or pin holds it again, and reports it as
  /// evicted, as a cache that keeps no fonts does once the last lease drops.
  pub(crate) fn evict(&self, id: u64) {
    let removed = self
      .pool
      .store
      .remove_if(&PoolKey::Font(id), |entry| match &entry.value {
        PoolEntry::Font(font) => !font.is_held(),
        _ => false,
      });

    if removed.is_some() {
      self.counts().evictions.fetch_add(1, Ordering::Relaxed);
      self.pool.lifecycle.evicted_fonts.push(id);
    }
  }

  /// The ids of the fonts evicted since the last call, without the ones kept again since.
  pub(crate) fn take_evicted(&self) -> Vec<u64> {
    let mut evicted = self.pool.lifecycle.evicted_fonts.take();

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
  evicted_fonts: Arc<EvictedFonts>,
  release: DefaultLifecycle<PoolKey, Weighed<PoolEntry>>,
}

/// The ids of the fonts evicted since [`HeldFonts::take_evicted`] last took them, which evictions
/// on any thread push and the host takes.
#[derive(Default)]
struct EvictedFonts {
  /// Whether `ids` holds any, so a host that finds none, as on most calls, takes no lock.
  any: AtomicBool,
  ids: Mutex<Vec<u64>>,
}

impl EvictedFonts {
  fn lock(&self) -> MutexGuard<'_, Vec<u64>> {
    self.ids.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn push(&self, id: u64) {
    let mut ids = self.lock();

    ids.push(id);
    self.any.store(true, Ordering::Release);
  }

  fn take(&self) -> Vec<u64> {
    if !self.any.load(Ordering::Acquire) {
      return Vec::new();
    }

    let mut ids = self.lock();

    self.any.store(false, Ordering::Relaxed);
    take(&mut *ids)
  }
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

impl Lifecycle<PoolKey, Weighed<PoolEntry>> for PoolLifecycle {
  type RequestState = <DefaultLifecycle<PoolKey, Weighed<PoolEntry>> as Lifecycle<
    PoolKey,
    Weighed<PoolEntry>,
  >>::RequestState;

  fn is_pinned(&self, _key: &PoolKey, entry: &Weighed<PoolEntry>) -> bool {
    matches!(&entry.value, PoolEntry::Font(font) if font.is_held())
  }

  fn on_evict(&self, state: &mut Self::RequestState, key: PoolKey, entry: Weighed<PoolEntry>) {
    self.counts[key.kind()]
      .evictions
      .fetch_add(1, Ordering::Relaxed);

    if let PoolKey::Font(id) = key {
      self.evicted_fonts.push(id);
    }

    self.release.on_evict(state, key, entry);
  }
}

/// A store holding at most `max_bytes`, with the pool's `lifecycle`.
fn store(max_bytes: u64, lifecycle: &PoolLifecycle) -> PoolStore {
  // ~4 KiB average entry, as glyphs and masks outnumber the rest ⇒ an item-count hint.
  let estimated_items = (max_bytes / (4 << 10)).max(1) as usize;
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
  /// The mode that covers values of this kind.
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
  Picture(u64) => Arc<SvgPicture>, Other;
  #[cfg(feature = "svg")]
  HostReads(HostReadsKey) => HostReads, Other;
  Stylesheet(u64) => Arc<StyleSheet>, Other;
  Tailwind(u64) => CompiledClasses, Other;
  Glyph(GlyphKey) => CachedGlyph, Glyphs;
  Intercept(InterceptKey) => Spans, Other;
  #[cfg(feature = "font-face")]
  FontFaces(DocumentKey) => Arc<FontRegistry>, Other;
  Font(u64) => Arc<HeldFont>, Fonts;
}

/// The bytes one entry of what SVG layouts read holds, its key and bookkeeping included.
#[cfg(feature = "svg")]
const HOST_READS_BYTES: usize = size_of::<(HostReadsKey, HostReads)>() + ENTRY_OVERHEAD;

/// An SVG source, the bits of the width and height of a viewport it is laid out at, and whether it
/// is the content of an `svg` element, which passes the host's `color` in.
#[cfg(feature = "svg")]
type HostReadsKey = (u64, u32, u32, bool);

#[cfg(feature = "svg")]
fn host_reads_key(source: u64, viewport: Size<f32>, svg_element: bool) -> HostReadsKey {
  (
    source,
    viewport.width.to_bits(),
    viewport.height.to_bits(),
    svg_element,
  )
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

/// Bytes charged for an entry's own bookkeeping on top of its payload.
pub(crate) const ENTRY_OVERHEAD: usize = 64;

/// A cached value and its approximate retained size in bytes.
#[derive(Clone)]
pub(crate) struct Weighed<T> {
  value: T,
  bytes: usize,
}

impl<T> Weighed<T> {
  pub(crate) fn new(value: T, bytes: usize) -> Self {
    Self { value, bytes }
  }
}

impl<V: Kept> Weighed<V> {
  fn into_entry(self) -> Weighed<PoolEntry> {
    Weighed::new(self.value.into_entry(), self.bytes)
  }
}

#[derive(Clone)]
struct ByBytes;

impl<K, V> Weighter<K, Weighed<V>> for ByBytes {
  fn weight(&self, _key: &K, entry: &Weighed<V>) -> u64 {
    (entry.bytes as u64).max(1)
  }
}

#[cfg(all(test, feature = "png"))]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::geometry::Placement;
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

  /// Keeps a glyph mask of about `bytes` bytes, unique to `seed`.
  fn keep_mask(cache: &Cache, seed: u64, bytes: usize) {
    let placement = Placement {
      left: 0,
      top: 0,
      width: 1,
      height: 1,
    };

    cache
      .glyphs()
      .glyph_mask(seed, || (vec![0; bytes - ENTRY_OVERHEAD], placement));
  }

  /// What `cache` holds of the kinds no mode covers, such as stylesheets.
  fn other(cache: &Cache) -> CacheKindStats {
    cache.0.pool.kind_stats()[Kind::Other]
  }

  fn pool_weight(cache: &Cache) -> u64 {
    cache.0.pool.store.weight()
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
    let cache = CacheOptions::builder().max_bytes(16 << 20).build();
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
    let cache = CacheOptions::builder().max_bytes(192 << 20).build();
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
    let cache = CacheOptions::builder().max_bytes(0).build();
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
  fn a_kind_of_mode_none_keeps_none_of_its_kind() {
    let cache = CacheOptions::builder().images(ImageCacheMode::None).build();
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
    let cache = CacheOptions::builder().max_bytes(max_bytes).build();

    for source in 0..64 {
      keep_buffer(&cache, source, 16);
    }

    assert!(pool_weight(&cache) <= max_bytes);
  }

  #[test]
  fn kinds_share_the_pool() {
    let max_bytes = 4 << 20;
    let cache = CacheOptions::builder().max_bytes(max_bytes).build();
    let images_kept = || {
      (0..4)
        .filter(|&source| keeps_buffer(&cache, source, 512))
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
  fn stats_count_each_kind_apart() {
    let cache = CacheOptions::builder().max_bytes(1 << 20).build();
    let bytes = png_bytes();

    for _ in 0..2 {
      cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();
      keep_sheet(&cache, 0, 64 << 10);
    }

    let stats = cache.stats();

    assert_eq!((stats.images.hits, stats.images.misses), (1, 1));
    let sheets = other(&cache);

    assert_eq!((sheets.hits, sheets.misses), (1, 1));
    assert_eq!(sheets.evictions, 0);
    assert!(sheets.current >= 60 << 10);
    assert_eq!(stats.memory.current, stats.images.current + sheets.current);

    for source in 0..8 {
      keep_buffer(&cache, source, 256);
    }

    let stats = cache.stats();

    assert!(stats.images.evictions >= 4, "{}", stats.images.evictions);
    assert!(stats.memory.current <= 1 << 20);
    assert!(stats.memory.high >= stats.memory.current);
  }

  /// Sets the old process-wide glyph size, which every cache built after it in this process takes.
  #[test]
  fn the_old_glyph_setter_turns_glyphs_off_at_zero() {
    let keeps_glyphs = |cache: Cache| {
      keep_mask(&cache, 0, 4 << 10);
      cache.stats().glyphs.current > 0
    };

    #[allow(deprecated)]
    glyph_cache::set_glyph_cache_max_bytes(0);

    let off = keeps_glyphs(Cache::default());
    let kept = keeps_glyphs(CacheOptions::builder().glyphs(ImageCacheMode::Auto).build());

    #[allow(deprecated)]
    glyph_cache::set_glyph_cache_max_bytes(8 << 20);

    assert!(!off);
    assert!(kept);
    assert!(keeps_glyphs(Cache::default()));
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

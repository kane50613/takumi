//! What renders keep for the renders after them, in one pool of bytes every kind of data shares.

use std::{
  convert::Infallible,
  marker::PhantomData,
  sync::{
    Arc, LazyLock, Mutex, PoisonError,
    atomic::{AtomicU64, Ordering},
  },
  thread::available_parallelism,
};

use quick_cache::{
  DefaultHashBuilder, OptionsBuilder, Weighter,
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
/// stylesheets and Tailwind classes, glyphs and their masks, and the faces of `@font-face` rules.
/// Every kind shares one pool of bytes. Clones share the same data.
#[derive(Clone)]
pub struct Cache(Arc<Caches>);

struct Caches {
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
}

/// The bytes a [`Cache`] keeps, and the most bytes each kind of data may hold within them. A kind
/// without a budget competes for the whole pool. `0` keeps nothing, of every kind or of one.
#[derive(TypedBuilder)]
#[builder(
  builder_type(name = CacheBuilder),
  build_method(into = Cache),
  field_defaults(default, setter(strip_option(ignore_invalid, fallback_suffix = "_opt")))
)]
pub struct CacheOptions {
  /// The bytes every kind of data shares. Approximate: entries are weighed by estimated size.
  #[builder(default = CacheOptions::DEFAULT_MAX_BYTES)]
  max_bytes: u64,
  /// Decoded images, and the SVG rasters and downscaled bitmaps drawn from them.
  images: Option<u64>,
  /// SVG images laid out to drawing ops, which vector backends draw and rasters are drawn from.
  #[cfg(feature = "svg")]
  pictures: Option<u64>,
  /// Parsed stylesheets.
  stylesheets: Option<u64>,
  /// The rules Tailwind classes compile to.
  tailwind: Option<u64>,
  /// Resolved glyphs, and the masks rasterized from glyphs and shapes. Without one, the budget the
  /// deprecated [`set_glyph_cache_max_bytes`](glyph_cache::set_glyph_cache_max_bytes) set.
  glyphs: Option<u64>,
  /// Where glyph outlines cross a decoration line, for `text-decoration-skip-ink`.
  intercepts: Option<u64>,
  /// The faces `@font-face` rules add, weighed by the font files they hold.
  #[cfg(feature = "font-face")]
  font_faces: Option<u64>,
}

impl CacheOptions {
  /// The bytes a cache keeps unless its builder sets `max_bytes`.
  pub const DEFAULT_MAX_BYTES: u64 = 128 << 20;
}

impl From<CacheOptions> for Cache {
  fn from(options: CacheOptions) -> Self {
    let pool = Arc::new(Pool::new(options.max_bytes));
    let images = KindStore::new(&pool, options.images);

    Self(Arc::new(Caches {
      sized_images: images.sharing(),
      images,
      #[cfg(feature = "svg")]
      pictures: KindStore::new(&pool, options.pictures),
      stylesheets: KindStore::new(&pool, options.stylesheets),
      tailwind: TailwindCache::new(KindStore::new(&pool, options.tailwind)),
      glyphs: GlyphCache::new(KindStore::new(
        &pool,
        options.glyphs.or_else(glyph_cache::default_budget),
      )),
      intercepts: InterceptCache::new(KindStore::new(&pool, options.intercepts)),
      #[cfg(feature = "font-face")]
      font_faces: DocumentRegistries::new(KindStore::new(&pool, options.font_faces)),
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

/// The bytes every kind without a budget shares, shrunk by what budgeted kinds hold.
struct Pool {
  store: Store<PoolKey, PoolEntry, ByBytes>,
  max_bytes: u64,
  /// The bytes budgets hold, each rounded up to a [`CHARGE_STEP`].
  charged: Mutex<u64>,
}

impl Pool {
  fn new(max_bytes: u64) -> Self {
    Self {
      // ~4 KiB average entry, as glyphs and masks outnumber the rest ⇒ an item-count hint.
      store: store(max_bytes, 4 << 10),
      max_bytes,
      charged: Mutex::new(0),
    }
  }

  /// Charges what `budget` holds against the pool once it crossed a [`CHARGE_STEP`], after RocksDB's
  /// [`CacheReservationManager`](https://github.com/facebook/rocksdb/blob/main/cache/cache_reservation_manager.h).
  fn charge(&self, budget: &Budget) {
    let held = || budget.store.weight().next_multiple_of(CHARGE_STEP);

    if held() == budget.charged.load(Ordering::Relaxed) {
      return;
    }

    let mut charged = self.charged.lock().unwrap_or_else(PoisonError::into_inner);
    let held = held();

    *charged = *charged + held - budget.charged.swap(held, Ordering::Relaxed);
    self
      .store
      .set_capacity(self.max_bytes.saturating_sub(*charged));
  }
}

/// A kind's own store, holding at most its budget, whose weight the pool gives up.
struct Budget {
  store: Store<PoolKey, PoolEntry, ByBytes>,
  /// The bytes this budget last charged the pool.
  charged: AtomicU64,
}

/// Where one kind of data is kept: the shared pool, or a store of its own under a budget.
pub(crate) struct KindStore<V> {
  pool: Arc<Pool>,
  budget: Option<Arc<Budget>>,
  kind: PhantomData<fn() -> V>,
}

impl<V: Kept> KindStore<V> {
  /// A kind in `pool`, held in a store of its own when it has a `budget`, which `pool` caps.
  fn new(pool: &Arc<Pool>, budget: Option<u64>) -> Self {
    Self {
      pool: Arc::clone(pool),
      budget: budget.map(|budget| {
        Arc::new(Budget {
          store: store(budget.min(pool.max_bytes), V::ENTRY_BYTES),
          charged: AtomicU64::new(0),
        })
      }),
      kind: PhantomData,
    }
  }

  /// Another kind kept where this one is, under the same budget.
  fn sharing<W>(&self) -> KindStore<W> {
    KindStore {
      pool: Arc::clone(&self.pool),
      budget: self.budget.clone(),
      kind: PhantomData,
    }
  }

  fn store(&self) -> &Store<PoolKey, PoolEntry, ByBytes> {
    self
      .budget
      .as_ref()
      .map_or(&self.pool.store, |budget| &budget.store)
  }

  /// The budget's bytes, none for a kind that shares the pool.
  pub(crate) fn budget(&self) -> Option<u64> {
    self.budget.as_ref().map(|budget| budget.store.capacity())
  }

  /// The bytes the whole cache keeps.
  pub(crate) fn max_bytes(&self) -> u64 {
    self.pool.max_bytes
  }

  /// The value kept under `key`.
  pub(crate) fn get(&self, key: V::Key) -> Option<V> {
    self.store().get(&V::pool_key(key)).and_then(V::from_entry)
  }

  /// Keeps `value` under `key`.
  pub(crate) fn insert(&self, key: V::Key, value: V) {
    self.store().insert(V::pool_key(key), value.into_entry());
    self.charge();
  }

  /// The value kept under `key`, `compute`d once across concurrent misses and kept unless it fails.
  pub(crate) fn get_or_insert_with<E>(
    &self,
    key: V::Key,
    compute: impl FnOnce() -> Result<V, E>,
  ) -> Result<V, E> {
    match self.store().get_value_or_guard(&V::pool_key(key), None) {
      GuardResult::Value(entry) => V::from_entry(entry).map_or_else(compute, Ok),
      GuardResult::Guard(guard) => {
        let value = compute()?;
        let _ = guard.insert(value.clone().into_entry());

        self.charge();
        Ok(value)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => compute(),
    }
  }

  fn charge(&self) {
    if let Some(budget) = &self.budget {
      self.pool.charge(budget);
    }
  }
}

/// A store holding at most `max_bytes`, sized for entries of about `entry_bytes`.
fn store(max_bytes: u64, entry_bytes: u64) -> Store<PoolKey, PoolEntry, ByBytes> {
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
      DefaultLifecycle::default(),
    ),
    Err(_) => Store::with_weighter(estimated_items, max_bytes, ByBytes),
  }
}

/// One kind of data the pool keeps, as a variant of its key and of its entry.
pub(crate) trait Kept: Clone {
  /// What finds a value of this kind.
  type Key;
  /// The approximate bytes of one value, a hint for how many a budget holds.
  const ENTRY_BYTES: u64;

  fn pool_key(key: Self::Key) -> PoolKey;
  fn into_entry(self) -> PoolEntry;
  fn from_entry(entry: PoolEntry) -> Option<Self>;
}

/// Declares the kinds of data the pool keeps: the variants of [`PoolKey`] and [`PoolEntry`], and
/// the [`Kept`] impl that moves each kind in and out of them.
macro_rules! kinds {
  ($($(#[$attr:meta])* $kind:ident($key:ty) => $value:ty, $entry_bytes:expr;)*) => {
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub(crate) enum PoolKey {
      $($(#[$attr])* $kind($key),)*
    }

    #[derive(Clone)]
    pub(crate) enum PoolEntry {
      $($(#[$attr])* $kind($value),)*
    }

    impl Weighed for PoolEntry {
      fn bytes(&self) -> usize {
        match self {
          $($(#[$attr])* Self::$kind(value) => value.bytes(),)*
        }
      }
    }

    $(
      $(#[$attr])*
      impl Kept for $value {
        type Key = $key;
        const ENTRY_BYTES: u64 = $entry_bytes;

        fn pool_key(key: $key) -> PoolKey {
          PoolKey::$kind(key)
        }

        fn into_entry(self) -> PoolEntry {
          PoolEntry::$kind(self)
        }

        fn from_entry(entry: PoolEntry) -> Option<Self> {
          match entry {
            PoolEntry::$kind(value) => Some(value),
            _ => None,
          }
        }
      }
    )*
  };
}

kinds! {
  Image(u64) => ImageSource, 64 << 10;
  SizedImage((u64, SizedKey)) => Arc<ImageBuffer>, 64 << 10;
  #[cfg(feature = "svg")]
  Picture(u64) => Arc<SvgPicture>, 16 << 10;
  Stylesheet(u64) => SheetEntry, 16 << 10;
  Tailwind(u64) => CompiledClasses, 256;
  Glyph(GlyphKey) => GlyphEntry, 4 << 10;
  Intercept(InterceptKey) => CachedSpans, 64;
  #[cfg(feature = "font-face")]
  FontFaces(DocumentKey) => CachedRegistry, 256 << 10;
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
  use std::sync::Arc;

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
    cache.0.images.pool.store.weight()
  }

  fn pool_capacity(cache: &Cache) -> u64 {
    cache.0.images.pool.store.capacity()
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
    let images = cache.0.images.budget.as_ref().unwrap();

    assert_eq!(pool_capacity(&cache), max_bytes);

    keep_buffer(&cache, 0, 512);
    assert_eq!(pool_capacity(&cache), max_bytes - (1 << 20));

    for source in 1..16 {
      keep_buffer(&cache, source, 512);
    }

    assert!(images.store.weight() <= budget);
    assert!(images.store.weight() >= 3 << 20);
    assert_eq!(
      pool_capacity(&cache),
      max_bytes - images.store.weight().next_multiple_of(CHARGE_STEP)
    );
  }

  #[test]
  fn a_budget_is_capped_at_the_size() {
    let cache = Cache::builder().max_bytes(1 << 20).images(8 << 20).build();

    assert_eq!(cache.0.images.budget(), Some(1 << 20));
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

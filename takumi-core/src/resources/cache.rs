//! What renders keep for the renders after them, each kind of data under its own byte budget.

use std::{
  hash::Hash,
  sync::{Arc, LazyLock},
  thread::available_parallelism,
};

use quick_cache::{
  DefaultHashBuilder, OptionsBuilder, Weighter,
  sync::{Cache as Store, DefaultLifecycle, GuardResult},
};
use typed_builder::TypedBuilder;
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

#[cfg(feature = "font-face")]
use crate::resources::font::DocumentRegistries;
use crate::{
  layout::intercept::InterceptCache,
  resources::{
    glyph_cache::{self, GlyphCache},
    image::{ImageCacheMode, ImageError, ImageResult, ImageSource},
    image_buffer::ImageBuffer,
  },
  style::{ImageScalingAlgorithm, StyleSheet, tw::TailwindCache},
};

/// Bytes each shard holds, enough for one decoded photo, since `quick_cache` rejects an entry over
/// 97% of its shard.
const SHARD_BYTES: u64 = 64 << 20;

/// What renders keep across renders: decoded images and the rasters drawn from them, parsed
/// stylesheets and Tailwind classes, glyphs and their masks, and the faces of `@font-face` rules.
/// Clones share the same data.
#[derive(Clone)]
pub struct Cache(Arc<Caches>);

struct Caches {
  images: Store<ImageKey, ImageEntry, ByBytes>,
  stylesheets: Store<u64, SheetEntry, ByBytes>,
  tailwind: TailwindCache,
  glyphs: GlyphCache,
  intercepts: InterceptCache,
  #[cfg(feature = "font-face")]
  font_faces: DocumentRegistries,
}

/// The byte budget of each kind of data a [`Cache`] keeps, where `0` keeps none of that kind.
#[derive(TypedBuilder)]
#[builder(builder_type(name = CacheBuilder), build_method(into = Cache))]
pub struct CacheOptions {
  /// Decoded images, and the SVG rasters and downscaled bitmaps drawn from them.
  #[builder(default = CacheOptions::DEFAULT_IMAGES)]
  images: u64,
  /// Parsed stylesheets.
  #[builder(default = CacheOptions::DEFAULT_STYLESHEETS)]
  stylesheets: u64,
  /// The rules Tailwind classes compile to.
  #[builder(default = CacheOptions::DEFAULT_TAILWIND)]
  tailwind: u64,
  /// Resolved glyphs, and the masks rasterized from glyphs and shapes.
  #[builder(default = glyph_cache::default_max_bytes())]
  glyphs: u64,
  /// Where glyph outlines cross a decoration line, for `text-decoration-skip-ink`.
  #[builder(default = CacheOptions::DEFAULT_INTERCEPTS)]
  intercepts: u64,
  /// The faces `@font-face` rules add, weighed by the font files they hold.
  #[builder(default = CacheOptions::DEFAULT_FONT_FACES)]
  #[cfg_attr(not(feature = "font-face"), allow(dead_code))]
  font_faces: u64,
}

impl CacheOptions {
  /// The default budget for images.
  pub const DEFAULT_IMAGES: u64 = 64 << 20;
  /// The default budget for stylesheets.
  pub const DEFAULT_STYLESHEETS: u64 = 4 << 20;
  /// The default budget for tailwind.
  pub const DEFAULT_TAILWIND: u64 = 1 << 20;
  /// The default budget for glyphs.
  pub const DEFAULT_GLYPHS: u64 = 8 << 20;
  /// The default budget for intercepts.
  pub const DEFAULT_INTERCEPTS: u64 = 1 << 20;
  /// The default budget for font faces.
  pub const DEFAULT_FONT_FACES: u64 = 16 << 20;
}

impl From<CacheOptions> for Cache {
  fn from(options: CacheOptions) -> Self {
    Self(Arc::new(Caches {
      // ~64 KiB average decoded image ⇒ an item-count hint for the budget.
      images: store(options.images, 64 << 10),
      // ~16 KiB average sheet ⇒ an item-count hint for the budget.
      stylesheets: store(options.stylesheets, 16 << 10),
      tailwind: TailwindCache::new(options.tailwind),
      glyphs: GlyphCache::new(options.glyphs),
      intercepts: InterceptCache::new(options.intercepts),
      #[cfg(feature = "font-face")]
      font_faces: DocumentRegistries::new(options.font_faces),
    }))
  }
}

impl Default for Cache {
  fn default() -> Self {
    Self::builder().build()
  }
}

impl Cache {
  /// A builder for a cache with its own budgets.
  pub fn builder() -> CacheBuilder {
    CacheOptions::builder()
  }

  /// The cache every render without one of its own shares, with the default budgets.
  pub fn shared() -> Self {
    static SHARED: LazyLock<Cache> = LazyLock::new(Cache::default);

    SHARED.clone()
  }

  /// The image `bytes` hold, decoded once across concurrent misses, and kept unless `mode` is
  /// [`ImageCacheMode::None`].
  pub fn decode_image(&self, bytes: &[u8], mode: ImageCacheMode) -> ImageResult {
    let hash = xxh3_64(bytes);
    let key = ImageKey::source(hash);

    if matches!(mode, ImageCacheMode::None) {
      return match self.0.images.get(&key) {
        Some(ImageEntry::Source(source)) => Ok(source),
        _ => ImageSource::from_bytes_uncached(bytes),
      };
    }

    match self.0.images.get_value_or_guard(&key, None) {
      GuardResult::Value(ImageEntry::Source(source)) => Ok(source),
      GuardResult::Guard(guard) => {
        let source = ImageSource::from_bytes_keyed(bytes, hash)?;
        let _ = guard.insert(ImageEntry::Source(source.clone()));
        Ok(source)
      }
      // `None` timeout never times out.
      GuardResult::Value(_) | GuardResult::Timeout => ImageSource::from_bytes_keyed(bytes, hash),
    }
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
    let parse = || Arc::new(StyleSheet::parse_owned_list_loosy(sources));

    match self
      .0
      .stylesheets
      .get_value_or_guard(&hasher.digest(), None)
    {
      GuardResult::Value(entry) => entry.sheet,
      GuardResult::Guard(guard) => {
        let sheet = parse();
        let _ = guard.insert(SheetEntry {
          sheet: sheet.clone(),
          bytes,
        });
        sheet
      }
      // `None` timeout never times out.
      GuardResult::Timeout => parse(),
    }
  }

  /// The old `ResourceCache::new`. Approximate: gives images and stylesheets a budget of
  /// `max_bytes` each, where the old cache shared one between them.
  #[deprecated(note = "use `Cache::builder().images(max_bytes).build()`")]
  pub fn new(max_bytes: u64) -> Self {
    Self::builder()
      .images(max_bytes)
      .stylesheets(max_bytes)
      .build()
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
    let key = ImageKey::sized(source, sized);

    match self.0.images.get_value_or_guard(&key, None) {
      GuardResult::Value(ImageEntry::Sized(buffer)) => Ok(buffer),
      GuardResult::Guard(guard) => {
        let buffer = compute()?;
        let _ = guard.insert(ImageEntry::Sized(buffer.clone()));
        Ok(buffer)
      }
      // `None` timeout never times out.
      GuardResult::Value(_) | GuardResult::Timeout => compute(),
    }
  }
}

/// A store holding at most `max_bytes`, sized for entries of about `entry_bytes`.
pub(crate) fn store<K, V>(max_bytes: u64, entry_bytes: u64) -> Store<K, V, ByBytes>
where
  K: Eq + Hash + Clone,
  V: Clone + Weighed,
{
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

/// An image source, or one of its buffers at a size.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ImageKey {
  source: u64,
  sized: Option<SizedKey>,
}

impl ImageKey {
  fn source(source: u64) -> Self {
    Self {
      source,
      sized: None,
    }
  }

  fn sized(source: u64, sized: SizedKey) -> Self {
    Self {
      source,
      sized: Some(sized),
    }
  }
}

#[derive(Clone)]
enum ImageEntry {
  Source(ImageSource),
  Sized(Arc<ImageBuffer>),
}

#[derive(Clone)]
struct SheetEntry {
  sheet: Arc<StyleSheet>,
  bytes: usize,
}

/// An entry's approximate retained size in bytes.
pub(crate) trait Weighed {
  fn bytes(&self) -> usize;
}

impl Weighed for ImageEntry {
  fn bytes(&self) -> usize {
    match self {
      Self::Source(source) => source.estimated_bytes(),
      Self::Sized(buffer) => buffer.data().len(),
    }
  }
}

impl Weighed for SheetEntry {
  fn bytes(&self) -> usize {
    self.bytes
  }
}

#[derive(Clone)]
pub(crate) struct ByBytes;

impl<K, V: Weighed> Weighter<K, V> for ByBytes {
  fn weight(&self, _key: &K, entry: &V) -> u64 {
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
  fn a_sized_entry_near_the_budget_is_retained() {
    let cache = Cache::builder().images(16 << 20).build();
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
  fn an_uneven_budget_keeps_a_whole_shard_per_entry() {
    let cache = Cache::builder().images(192 << 20).build();
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
  fn a_zero_budget_keeps_nothing() {
    let cache = Cache::builder().images(0).build();
    let bytes = png_bytes();

    let a = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();
    let b = cache.decode_image(&bytes, ImageCacheMode::Auto).unwrap();

    match (&a, &b) {
      (ImageSource::Encoded(x), ImageSource::Encoded(y)) => assert!(!Arc::ptr_eq(x, y)),
      _ => panic!("expected encoded bitmaps"),
    }
  }

  #[test]
  fn eviction_stays_within_the_byte_budget() {
    let max_bytes = 4096;
    let cache = Cache::builder().images(max_bytes).build();

    for source in 0..64 {
      let _ = cache.sized(Some(source), sized(16, 16), || {
        Ok(Arc::new(ImageBuffer::new(16, 16).unwrap()))
      });
    }

    assert!(cache.0.images.weight() <= max_bytes);
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

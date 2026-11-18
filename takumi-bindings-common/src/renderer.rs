//! The options a renderer is constructed with, which every binding takes, and the cache statistics
//! every binding reports.

use serde::{Deserialize, Serialize};
use takumi_core::resources::{
  cache::{
    Cache, CacheKindStats as PoolKindStats, CacheMemoryStats as PoolMemoryStats,
    CacheOptions as PoolOptions,
  },
  glyph_cache,
  image::ImageCacheMode,
};

/// Options for constructing a renderer.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RendererOptions {
  /// The memory the renderer keeps across renders.
  pub cache: Option<CacheOptions>,
  /// The old name of [`CacheOptions::max_bytes`].
  pub cache_max_bytes: Option<f64>,
  /// The old bound on the bytes of fonts, which `0` sets [`CacheOptions::fonts`] to
  /// [`ImageCacheMode::None`] for, unless `fonts` is set.
  pub font_cache_max_bytes: Option<f64>,
}

/// The memory a renderer keeps across renders, in JavaScript byte counts, and which kinds of data
/// it keeps.
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CacheOptions {
  /// The bytes every kind of data shares, [`PoolOptions::DEFAULT_MAX_BYTES`] when absent.
  pub max_bytes: Option<f64>,
  /// [`PoolOptions`]'s `images`.
  pub images: Option<ImageCacheMode>,
  /// [`PoolOptions`]'s `glyphs`, what the last [`set_default_glyph_mode`] set when absent.
  pub glyphs: Option<ImageCacheMode>,
  /// [`PoolOptions`]'s `fonts`, the decoded font files registered fonts and `@font-face` files hold.
  pub fonts: Option<ImageCacheMode>,
}

impl RendererOptions {
  /// The cache a renderer built with these options keeps.
  pub fn cache(&self) -> Cache {
    let cache = self.cache.unwrap_or_default();
    let mut options = PoolOptions::default();

    options.max_bytes = cache
      .max_bytes
      .or(self.cache_max_bytes)
      .map_or(PoolOptions::DEFAULT_MAX_BYTES, bytes);

    if let Some(images) = cache.images {
      options.images = images;
    }

    if let Some(glyphs) = cache.glyphs {
      options.glyphs = glyphs;
    }

    if let Some(fonts) = cache.fonts {
      options.fonts = fonts;
    } else if self.font_cache_max_bytes == Some(0.0) {
      options.fonts = ImageCacheMode::None;
    }

    Cache::from(options)
  }
}

/// What a renderer's caches hold and how their lookups went, under the keys [`CacheOptions`] uses.
/// Sizes are estimates, not process memory. JavaScript numbers mirror the core's `u64` counts,
/// which napi serializes only up to `u32::MAX`.
#[derive(Serialize)]
pub struct CacheStats {
  memory: CacheMemoryStats,
  images: CacheKindStats,
  glyphs: CacheKindStats,
  fonts: CacheKindStats,
}

/// The bytes the pool holds of every kind together.
#[derive(Serialize)]
struct CacheMemoryStats {
  current: f64,
  high: f64,
}

/// What one kind of data holds and how its lookups went, in JavaScript numbers.
#[derive(Serialize)]
pub struct CacheKindStats {
  /// The bytes held now.
  pub current: f64,
  /// Lookups that found an entry.
  pub hits: f64,
  /// Lookups that found none.
  pub misses: f64,
  /// Entries released for space.
  pub evictions: f64,
}

impl From<PoolMemoryStats> for CacheMemoryStats {
  fn from(stats: PoolMemoryStats) -> Self {
    Self {
      current: stats.current as f64,
      high: stats.high as f64,
    }
  }
}

impl From<PoolKindStats> for CacheKindStats {
  fn from(stats: PoolKindStats) -> Self {
    Self {
      current: stats.current as f64,
      hits: stats.hits as f64,
      misses: stats.misses as f64,
      evictions: stats.evictions as f64,
    }
  }
}

impl CacheStats {
  /// What `cache` holds.
  pub fn new(cache: &Cache) -> Self {
    let stats = cache.stats();

    Self {
      memory: stats.memory.into(),
      images: stats.images.into(),
      glyphs: stats.glyphs.into(),
      fonts: stats.fonts.into(),
    }
  }
}

/// Keeps no glyphs in the renderers created after this call when `value` is `0`, and glyphs again
/// otherwise.
pub fn set_default_glyph_mode(value: f64) {
  #[allow(deprecated)]
  glyph_cache::set_glyph_cache_max_bytes(bytes(value) as usize);
}

/// A JavaScript byte count, negative ones read as zero.
fn bytes(value: f64) -> u64 {
  value.max(0.0) as u64
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;

  fn options(json: &str) -> RendererOptions {
    serde_json::from_str(json).unwrap()
  }

  /// Whether a cache built from `json` keeps a stylesheet across renders.
  fn keeps_stylesheets(json: &str) -> bool {
    let cache = options(json).cache();
    let sheet = || cache.parse_stylesheet(vec!["p { color: red }".to_owned()]);

    Arc::ptr_eq(&sheet(), &sheet())
  }

  // JavaScript's `{ cache: undefined }` reaches serde as a unit value, as JSON's `null` does.
  #[test]
  fn an_absent_cache_keeps_every_default() {
    let options = options(r#"{ "cache": null }"#);

    assert!(options.cache.is_none());
    assert!(keeps_stylesheets(r#"{ "cache": null }"#));
    assert!(keeps_stylesheets(r#"{ "cache": { "images": null } }"#));
  }

  #[test]
  fn max_bytes_sets_the_pool() {
    assert!(keeps_stylesheets(r#"{ "cache": { "maxBytes": 1024 } }"#));
    assert!(!keeps_stylesheets(r#"{ "cache": { "maxBytes": 0 } }"#));
  }

  #[test]
  fn a_mode_of_none_keeps_none_of_its_kind() {
    let json = r#"{ "cache": { "images": "none", "glyphs": "none" } }"#;
    let cache = options(json).cache();

    cache
      .glyphs()
      .glyph_mask(0, || (vec![0; 64], Default::default()));

    assert_eq!(cache.stats().glyphs.current, 0);
    assert!(keeps_stylesheets(json));
  }

  #[test]
  fn the_old_cache_size_sets_max_bytes() {
    assert!(!keeps_stylesheets(r#"{ "cacheMaxBytes": 0 }"#));
    assert!(keeps_stylesheets(
      r#"{ "cacheMaxBytes": 0, "cache": { "maxBytes": 2048 } }"#
    ));
  }

  #[test]
  fn an_old_font_bound_of_zero_keeps_no_fonts() {
    let keeps_fonts = |json| {
      let cache = options(json).cache();

      drop(cache.fonts().keep(1, 64));
      cache.stats().fonts.current > 0
    };

    assert!(keeps_fonts(r#"{ "fontCacheMaxBytes": 2 }"#));
    assert!(!keeps_fonts(r#"{ "fontCacheMaxBytes": 0 }"#));
    assert!(keeps_fonts(
      r#"{ "cache": { "fonts": "auto" }, "fontCacheMaxBytes": 0 }"#
    ));
    assert!(!keeps_fonts(r#"{ "cache": { "fonts": "none" } }"#));
  }

  #[test]
  fn stats_report_under_the_option_keys() {
    let options = options(r#"{ "cache": { "images": "none" } }"#);
    let stats = serde_json::to_value(CacheStats::new(&options.cache())).unwrap();

    assert_eq!(stats["memory"]["current"], 0.0);
    assert_eq!(stats["images"]["current"], 0.0);
    assert_eq!(stats["fonts"]["hits"], 0.0);
    assert_eq!(stats["glyphs"]["hits"], 0.0);
  }
}

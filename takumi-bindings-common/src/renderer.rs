//! The options a renderer is constructed with, which every binding takes, and the cache statistics
//! every binding reports.

use serde::{Deserialize, Serialize};
use takumi_core::resources::{
  cache::{Cache, CacheKindStats as PoolKindStats, CacheOptions as PoolOptions},
  glyph_cache,
};

/// Options for constructing a renderer.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RendererOptions {
  /// The memory the renderer keeps across renders.
  pub cache: Option<CacheOptions>,
  /// The old name of [`CacheOptions::max_bytes`].
  pub cache_max_bytes: Option<f64>,
  /// The old name of [`CacheBudgets::fonts`].
  pub font_cache_max_bytes: Option<f64>,
}

/// The memory a renderer keeps across renders, in JavaScript byte counts.
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CacheOptions {
  /// The bytes every kind of data but fonts shares, [`PoolOptions::DEFAULT_MAX_BYTES`] when absent.
  pub max_bytes: Option<f64>,
  /// The most bytes each kind of data may hold.
  pub budgets: Option<CacheBudgets>,
}

/// The most bytes each kind of data a renderer keeps may hold, in JavaScript byte counts. An
/// absent budget leaves its kind in the shared pool, except `glyphs`, which takes the budget the
/// last [`set_glyph_cache_max_bytes`] set, and `fonts`, which leaves the fonts unbounded.
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CacheBudgets {
  /// [`PoolOptions`]'s `images`.
  pub images: Option<f64>,
  /// [`PoolOptions`]'s `pictures`, ignored without the `svg` feature.
  pub pictures: Option<f64>,
  /// [`PoolOptions`]'s `stylesheets`.
  pub stylesheets: Option<f64>,
  /// [`PoolOptions`]'s `tailwind`.
  pub tailwind: Option<f64>,
  /// [`PoolOptions`]'s `glyphs`.
  pub glyphs: Option<f64>,
  /// [`PoolOptions`]'s `intercepts`.
  pub intercepts: Option<f64>,
  /// [`PoolOptions`]'s `font_faces`.
  pub font_faces: Option<f64>,
  /// Bytes of decoded fonts that renders' `fonts` lists may keep registered, apart from the pool.
  pub fonts: Option<f64>,
}

impl CacheOptions {
  /// The options of the cache that holds every kind but fonts. A glyph budget they leave out is the
  /// one the last [`set_glyph_cache_max_bytes`] set.
  pub fn pool(&self) -> PoolOptions {
    let budgets = self.budgets.unwrap_or_default();
    let mut options = PoolOptions::default();

    options.max_bytes = self.max_bytes.map_or(PoolOptions::DEFAULT_MAX_BYTES, bytes);
    options.images = budgets.images.map(bytes);
    #[cfg(feature = "svg")]
    {
      options.pictures = budgets.pictures.map(bytes);
    }
    options.stylesheets = budgets.stylesheets.map(bytes);
    options.tailwind = budgets.tailwind.map(bytes);
    options.glyphs = budgets.glyphs.map(bytes);
    options.intercepts = budgets.intercepts.map(bytes);
    options.font_faces = budgets.font_faces.map(bytes);

    options
  }

  /// The font budget, unbounded without one.
  pub fn font_bytes(&self) -> Option<u64> {
    self.budgets.and_then(|budgets| budgets.fonts).map(bytes)
  }
}

impl RendererOptions {
  /// The cache options, with the old names filling what `cache` leaves out.
  fn cache_options(&self) -> CacheOptions {
    let cache = self.cache.unwrap_or_default();
    let budgets = cache.budgets.unwrap_or_default();

    CacheOptions {
      max_bytes: cache.max_bytes.or(self.cache_max_bytes),
      budgets: Some(CacheBudgets {
        fonts: budgets.fonts.or(self.font_cache_max_bytes),
        ..budgets
      }),
    }
  }

  /// The font budget, unbounded when neither name sets one.
  pub fn font_cache_bytes(&self) -> Option<u64> {
    self.cache_options().font_bytes()
  }

  /// The cache a renderer built with these options keeps.
  pub fn cache(&self) -> Cache {
    Cache::from(self.cache_options().pool())
  }
}

/// What a renderer's caches hold and how their lookups went, under the keys [`CacheOptions`] uses.
/// Sizes are estimates, not process memory.
#[derive(Serialize)]
pub struct CacheStats {
  memory: CacheMemoryStats,
  budgets: CacheBudgetStats,
}

/// The bytes the pool holds of every kind but fonts.
#[derive(Serialize)]
struct CacheMemoryStats {
  current: f64,
  high: f64,
  max: f64,
}

/// What each kind of data holds, by its [`CacheBudgets`] key.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheBudgetStats {
  images: CacheKindStats,
  pictures: CacheKindStats,
  stylesheets: CacheKindStats,
  tailwind: CacheKindStats,
  glyphs: CacheKindStats,
  intercepts: CacheKindStats,
  font_faces: CacheKindStats,
  fonts: CacheKindStats,
}

/// What one kind of data holds and how its lookups went, in JavaScript numbers.
#[derive(Serialize, Default)]
pub struct CacheKindStats {
  /// The bytes held now.
  pub current: f64,
  /// The kind's budget, none when it has no bound of its own.
  pub max: Option<f64>,
  /// Lookups that found an entry.
  pub hits: f64,
  /// Lookups that found none.
  pub misses: f64,
  /// Entries released for space.
  pub evictions: f64,
}

impl From<PoolKindStats> for CacheKindStats {
  fn from(stats: PoolKindStats) -> Self {
    Self {
      current: stats.current as f64,
      max: stats.max.map(|max| max as f64),
      hits: stats.hits as f64,
      misses: stats.misses as f64,
      evictions: stats.evictions as f64,
    }
  }
}

impl CacheStats {
  /// What `cache` holds, with `fonts` for the fonts it does not.
  pub fn new(cache: &Cache, fonts: CacheKindStats) -> Self {
    let stats = cache.stats();

    Self {
      memory: CacheMemoryStats {
        current: stats.memory.current as f64,
        high: stats.memory.high as f64,
        max: stats.memory.max as f64,
      },
      budgets: CacheBudgetStats {
        images: stats.images.into(),
        #[cfg(feature = "svg")]
        pictures: stats.pictures.into(),
        #[cfg(not(feature = "svg"))]
        pictures: CacheKindStats::default(),
        stylesheets: stats.stylesheets.into(),
        tailwind: stats.tailwind.into(),
        glyphs: stats.glyphs.into(),
        intercepts: stats.intercepts.into(),
        font_faces: stats.font_faces.into(),
        fonts,
      },
    }
  }
}

/// Sets the glyph budget of the renderers created after this call, from a JavaScript byte count.
pub fn set_glyph_cache_max_bytes(value: f64) {
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
  use crate::font_cache::FontCache;

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
    assert_eq!(
      options.cache_options().pool().max_bytes,
      PoolOptions::DEFAULT_MAX_BYTES
    );
    assert_eq!(options.font_cache_bytes(), None);
    assert!(keeps_stylesheets(r#"{ "cache": null }"#));
    assert!(keeps_stylesheets(r#"{ "cache": { "budgets": null } }"#));
  }

  #[test]
  fn max_bytes_sets_the_pool() {
    assert_eq!(
      options(r#"{ "cache": { "maxBytes": 1024 } }"#)
        .cache_options()
        .pool()
        .max_bytes,
      1024
    );
    assert!(!keeps_stylesheets(r#"{ "cache": { "maxBytes": 0 } }"#));
  }

  #[test]
  fn a_budget_of_zero_keeps_none_of_its_kind() {
    assert!(!keeps_stylesheets(
      r#"{ "cache": { "budgets": { "stylesheets": 0 } } }"#
    ));
    assert!(keeps_stylesheets(
      r#"{ "cache": { "budgets": { "images": 0 } } }"#
    ));
  }

  #[test]
  fn the_old_cache_size_sets_max_bytes() {
    let max_bytes = |json| options(json).cache_options().pool().max_bytes;

    assert_eq!(max_bytes(r#"{ "cacheMaxBytes": 1024 }"#), 1024);
    assert_eq!(
      max_bytes(r#"{ "cacheMaxBytes": 1024, "cache": { "maxBytes": 2048 } }"#),
      2048
    );
    assert!(!keeps_stylesheets(r#"{ "cacheMaxBytes": 0 }"#));
  }

  #[test]
  fn the_fonts_budget_wins_over_the_old_name() {
    assert_eq!(
      options(r#"{ "fontCacheMaxBytes": 2 }"#).font_cache_bytes(),
      Some(2)
    );
    assert_eq!(
      options(r#"{ "cache": { "budgets": { "fonts": 1 } }, "fontCacheMaxBytes": 2 }"#)
        .font_cache_bytes(),
      Some(1)
    );
  }

  #[test]
  fn new_options_apply_to_a_built_cache() {
    let cache = options("{}").cache();
    let sheet = || cache.parse_stylesheet(vec!["p { color: red }".to_owned()]);

    assert!(Arc::ptr_eq(&sheet(), &sheet()));

    let shrunk: CacheOptions = serde_json::from_str(r#"{ "maxBytes": 0 }"#).unwrap();

    cache.set_options(shrunk.pool());

    assert_eq!(cache.stats().memory.current, 0);
    assert!(!Arc::ptr_eq(&sheet(), &sheet()));
  }

  #[test]
  fn stats_report_under_the_option_keys() {
    let options = options(r#"{ "cache": { "budgets": { "images": 1024, "fonts": 2048 } } }"#);
    let fonts = FontCache::new(options.font_cache_bytes());
    let stats = serde_json::to_value(CacheStats::new(&options.cache(), fonts.stats())).unwrap();

    assert_eq!(
      stats["memory"]["max"],
      PoolOptions::DEFAULT_MAX_BYTES as f64
    );
    assert_eq!(stats["budgets"]["images"]["max"], 1024.0);
    assert_eq!(stats["budgets"]["fonts"]["max"], 2048.0);
    assert!(stats["budgets"]["fontFaces"]["max"].is_null());
    assert_eq!(stats["budgets"]["glyphs"]["hits"], 0.0);
  }
}

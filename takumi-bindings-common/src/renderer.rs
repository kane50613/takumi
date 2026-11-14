//! The options a renderer is constructed with, which every binding takes.

use serde::Deserialize;
use takumi_core::resources::{
  cache::{Cache, CacheOptions as PoolOptions},
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

impl RendererOptions {
  fn budgets(&self) -> CacheBudgets {
    self
      .cache
      .and_then(|cache| cache.budgets)
      .unwrap_or_default()
  }

  /// The font budget, unbounded when neither name sets one.
  pub fn font_cache_bytes(&self) -> Option<u64> {
    self
      .budgets()
      .fonts
      .or(self.font_cache_max_bytes)
      .map(bytes)
  }

  /// The bytes the cache keeps, from `cache.maxBytes` or the old `cacheMaxBytes`.
  fn max_bytes(&self) -> u64 {
    self
      .cache
      .and_then(|cache| cache.max_bytes)
      .or(self.cache_max_bytes)
      .map_or(PoolOptions::DEFAULT_MAX_BYTES, bytes)
  }

  /// The cache a renderer built with these options keeps. A glyph budget it leaves out is the one
  /// the last [`set_glyph_cache_max_bytes`] set.
  pub fn cache(&self) -> Cache {
    let budgets = self.budgets();
    let builder = Cache::builder()
      .max_bytes(self.max_bytes())
      .images_opt(budgets.images.map(bytes))
      .stylesheets_opt(budgets.stylesheets.map(bytes))
      .tailwind_opt(budgets.tailwind.map(bytes))
      .glyphs_opt(budgets.glyphs.map(bytes))
      .intercepts_opt(budgets.intercepts.map(bytes))
      .font_faces_opt(budgets.font_faces.map(bytes));
    #[cfg(feature = "svg")]
    let builder = builder.pictures_opt(budgets.pictures.map(bytes));

    builder.build()
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
    assert_eq!(options.max_bytes(), PoolOptions::DEFAULT_MAX_BYTES);
    assert_eq!(options.font_cache_bytes(), None);
    assert!(keeps_stylesheets(r#"{ "cache": null }"#));
    assert!(keeps_stylesheets(r#"{ "cache": { "budgets": null } }"#));
  }

  #[test]
  fn max_bytes_sets_the_pool() {
    assert_eq!(
      options(r#"{ "cache": { "maxBytes": 1024 } }"#).max_bytes(),
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
    assert_eq!(options(r#"{ "cacheMaxBytes": 1024 }"#).max_bytes(), 1024);
    assert_eq!(
      options(r#"{ "cacheMaxBytes": 1024, "cache": { "maxBytes": 2048 } }"#).max_bytes(),
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
}

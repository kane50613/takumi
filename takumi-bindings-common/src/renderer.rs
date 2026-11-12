//! The options a renderer is constructed with, which every binding takes.

use serde::Deserialize;
use takumi_core::resources::{
  cache::{Cache, CacheOptions},
  glyph_cache,
};

/// Options for constructing a renderer.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RendererOptions {
  /// The byte budget of each kind of data the renderer keeps across renders.
  #[serde(default)]
  pub cache: CacheBudgets,
  /// The old name of [`CacheBudgets::images`].
  pub cache_max_bytes: Option<f64>,
  /// The old name of [`CacheBudgets::fonts`].
  pub font_cache_max_bytes: Option<f64>,
}

/// The byte budget of each kind of data a renderer keeps, in JavaScript byte counts, each the
/// default of [`CacheOptions`] when absent.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CacheBudgets {
  /// [`CacheOptions`]'s `images`.
  pub images: Option<f64>,
  /// [`CacheOptions`]'s `pictures`.
  pub pictures: Option<f64>,
  /// [`CacheOptions`]'s `stylesheets`.
  pub stylesheets: Option<f64>,
  /// [`CacheOptions`]'s `tailwind`.
  pub tailwind: Option<f64>,
  /// [`CacheOptions`]'s `glyphs`.
  pub glyphs: Option<f64>,
  /// [`CacheOptions`]'s `intercepts`.
  pub intercepts: Option<f64>,
  /// [`CacheOptions`]'s `font_faces`.
  pub font_faces: Option<f64>,
  /// Bytes of decoded fonts that renders' `fonts` lists may keep registered, unbounded when
  /// absent.
  pub fonts: Option<f64>,
}

impl RendererOptions {
  /// The font budget, unbounded when neither name sets one.
  pub fn font_cache_bytes(&self) -> Option<u64> {
    self.cache.fonts.or(self.font_cache_max_bytes).map(bytes)
  }

  /// The cache a renderer built with these options keeps. A glyph budget it leaves out is the one
  /// the last [`set_glyph_cache_max_bytes`] set.
  pub fn cache(&self) -> Cache {
    let budgets = &self.cache;
    let budget = |value: Option<f64>, default: u64| value.map_or(default, bytes);

    Cache::builder()
      .images(budget(
        budgets.images.or(self.cache_max_bytes),
        CacheOptions::DEFAULT_IMAGES,
      ))
      .pictures(budget(budgets.pictures, CacheOptions::DEFAULT_PICTURES))
      .stylesheets(budget(
        budgets.stylesheets,
        CacheOptions::DEFAULT_STYLESHEETS,
      ))
      .tailwind(budget(budgets.tailwind, CacheOptions::DEFAULT_TAILWIND))
      .glyphs(budget(budgets.glyphs, glyph_cache::default_max_bytes()))
      .intercepts(budget(budgets.intercepts, CacheOptions::DEFAULT_INTERCEPTS))
      .font_faces(budget(budgets.font_faces, CacheOptions::DEFAULT_FONT_FACES))
      .build()
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

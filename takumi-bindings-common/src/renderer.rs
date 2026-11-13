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
  pub cache: Option<CacheBudgets>,
  /// The old budget of the one store that held images, SVG rasters, and stylesheets. It sets the
  /// `images` and `stylesheets` budgets and caps `pictures`, as the deprecated `Cache::new` does.
  pub cache_max_bytes: Option<f64>,
  /// The old name of [`CacheBudgets::fonts`].
  pub font_cache_max_bytes: Option<f64>,
}

/// The byte budget of each kind of data a renderer keeps, in JavaScript byte counts. An absent
/// budget takes the default of [`CacheOptions`], except `glyphs`, which takes the budget the last
/// [`set_glyph_cache_max_bytes`] set, and `fonts`, which leaves the fonts unbounded.
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CacheBudgets {
  /// [`CacheOptions`]'s `images`.
  pub images: Option<f64>,
  /// [`CacheOptions`]'s `pictures`, ignored without the `svg` feature.
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
    self
      .cache
      .and_then(|budgets| budgets.fonts)
      .or(self.font_cache_max_bytes)
      .map(bytes)
  }

  /// The cache a renderer built with these options keeps. A glyph budget it leaves out is the one
  /// the last [`set_glyph_cache_max_bytes`] set.
  pub fn cache(&self) -> Cache {
    let budgets = self.cache.unwrap_or_default();
    let budget = |value: Option<f64>, default: u64| value.map_or(default, bytes);
    let old = self.cache_max_bytes.map(bytes);
    let old_or = |default: u64| old.unwrap_or(default);

    let builder = Cache::builder()
      .images(budget(budgets.images, old_or(CacheOptions::DEFAULT_IMAGES)))
      .stylesheets(budget(
        budgets.stylesheets,
        old_or(CacheOptions::DEFAULT_STYLESHEETS),
      ))
      .tailwind(budget(budgets.tailwind, CacheOptions::DEFAULT_TAILWIND))
      .glyphs(budget(budgets.glyphs, glyph_cache::default_max_bytes()))
      .intercepts(budget(budgets.intercepts, CacheOptions::DEFAULT_INTERCEPTS))
      .font_faces(budget(budgets.font_faces, CacheOptions::DEFAULT_FONT_FACES));
    #[cfg(feature = "svg")]
    let builder = builder.pictures(budget(
      budgets.pictures,
      old.map_or(CacheOptions::DEFAULT_PICTURES, |old| {
        old.min(CacheOptions::DEFAULT_PICTURES)
      }),
    ));

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

  // JavaScript's `{ cache: undefined }` reaches serde as a unit value, as JSON's `null` does.
  #[test]
  fn an_absent_cache_keeps_every_default() {
    let options: RendererOptions = serde_json::from_str(r#"{ "cache": null }"#).unwrap();

    assert!(options.cache.is_none());
    assert_eq!(options.font_cache_bytes(), None);
  }

  #[test]
  fn the_cache_fonts_budget_wins_over_the_old_name() {
    let options: RendererOptions =
      serde_json::from_str(r#"{ "cache": { "fonts": 1 }, "fontCacheMaxBytes": 2 }"#).unwrap();

    assert_eq!(options.font_cache_bytes(), Some(1));
  }

  #[test]
  fn the_old_cache_budget_covers_stylesheets() {
    let kept = |json: &str| {
      let cache = serde_json::from_str::<RendererOptions>(json)
        .unwrap()
        .cache();
      let sheet = || cache.parse_stylesheet(vec!["p { color: red }".to_owned()]);

      Arc::ptr_eq(&sheet(), &sheet())
    };

    assert!(kept("{}"));
    assert!(!kept(r#"{ "cacheMaxBytes": 0 }"#));
    assert!(kept(
      r#"{ "cacheMaxBytes": 0, "cache": { "stylesheets": 1048576 } }"#
    ));
  }
}

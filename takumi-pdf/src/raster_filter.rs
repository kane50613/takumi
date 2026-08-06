//! Rasterized fallback for filters a vector PDF cannot express.
//!
//! `filter: blur()` and `drop-shadow()` are convolutions, so the filtered
//! stacking context renders through takumi-raster like Chromium renders a
//! filtered layer offscreen, and the result embeds as an image. Every filter
//! function in the list applies during that raster pass, keeping the order
//! CSS requires.

use takumi_core::{
  context::RenderContext,
  style::{BlurType, Filter, Length},
};

/// Whether this filter list needs pixels: `blur()` and `drop-shadow()` have
/// no vector equivalent in PDF.
pub(crate) fn needs_raster(filters: &[Filter]) -> bool {
  filters
    .iter()
    .any(|filter| matches!(filter, Filter::Blur(_) | Filter::DropShadow(_)))
}

/// How far the filters paint outside the subtree's bounds, in CSS px.
pub(crate) fn bleed(filters: &[Filter], context: &RenderContext) -> f32 {
  let px = |length: Length| length.to_px(&context.sizing, 1.0).max(0.0);

  filters
    .iter()
    .map(|filter| match filter {
      Filter::Blur(radius) => px(*radius) * BlurType::Filter.extent_multiplier(),
      Filter::DropShadow(shadow) => {
        let offset = px(shadow.offset_x).abs().max(px(shadow.offset_y).abs());

        offset + px(shadow.blur_radius) * BlurType::Shadow.extent_multiplier()
      }
      _ => 0.0,
    })
    .fold(0.0, f32::max)
    .ceil()
}

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! Raster painting backend for takumi, built on tiny-skia: canvas, drawing,
//! filters, and the [`render`] entry point.
//!
//! Use it through the `takumi` umbrella. The render functions are re-exported at
//! the umbrella's crate root, and the crate itself is `takumi::unstable::raster`.
//! Core types are not re-exported here; reach them through `takumi_core` or
//! `takumi::unstable::base`.

use takumi_core::*;

/// Background and color drawing functions
mod background_drawing;
mod blend;
/// Canvas operations and image blending
mod canvas;
mod components;
/// Debug drawing utilities
mod debug_drawing;
mod dithering;
/// Filter rasterization (blur, drop-shadow, backdrop, pixel filters)
mod filter;
/// Image drawing functions
mod image_drawing;
pub(crate) mod inline_drawing;
/// Box-decoration painting (backgrounds, borders, outlines, box-shadows)
mod node_paint;
mod path;
/// Main image renderer and viewport management
mod render;
mod stacking_context;
/// Text drawing functions
mod text_drawing;
mod webp;
mod write;

pub(crate) use background_drawing::*;
pub(crate) use blend::*;
pub(crate) use canvas::*;
pub(crate) use components::*;
pub(crate) use debug_drawing::*;
pub use dithering::*;
pub(crate) use filter::*;
pub(crate) use image_drawing::*;
pub(crate) use node_paint::*;
pub(crate) use path::*;
pub use render::*;
pub(crate) use text_drawing::*;
use tiny_skia::{IntSize, Pixmap, PixmapRef};
pub use write::*;

use crate::resources::image_buffer::ImageBuffer;
pub(crate) use crate::{
  context::RenderContext,
  font_style::*,
  layout::inline::scale_text_fit_x,
  style::math::{fast_div_255, fast_div_255_u32},
};

/// Borrows an [`ImageBuffer`] as a zero-copy `tiny_skia` pixmap view.
pub(crate) fn pixmap_ref_from_buffer(buffer: &ImageBuffer) -> Option<PixmapRef<'_>> {
  PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height())
}

/// Copies an [`ImageBuffer`] into an owned `tiny_skia` pixmap.
pub(crate) fn pixmap_from_buffer(buffer: &ImageBuffer) -> Option<Pixmap> {
  let size = IntSize::from_wh(buffer.width(), buffer.height())?;
  Pixmap::from_vec(buffer.data().to_vec(), size)
}

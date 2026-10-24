#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! Raster painting backend for takumi, built on tiny-skia.
//!
//! [`render`] paints a node tree into a [`Bitmap`], and [`write_image`] encodes it as PNG,
//! JPEG, WebP, or ICO. [`write_animation`] encodes animated WebP, APNG, or GIF.
//!
//! Use it through the `takumi` crate, which re-exports these functions at its root. The whole
//! crate is `takumi::unstable::raster`. This crate does not re-export core types; reach them
//! through `takumi_core` or `takumi::unstable::base`.

use takumi_core::*;

/// Background and color drawing functions
mod background_drawing;
mod blend;
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
mod node_paint;
mod palette;
mod path;
/// Main image renderer and viewport management
mod render;
#[cfg(all(test, feature = "pages"))]
mod render_pages_tests;
mod scanline;
mod simd;
mod stacking_context;
/// Text drawing functions
mod text_drawing;
mod webp;
mod write;
mod zlib;

pub(crate) use background_drawing::*;
pub(crate) use canvas::*;
pub(crate) use components::*;
pub(crate) use debug_drawing::*;
pub use dithering::*;
pub(crate) use filter::*;
pub(crate) use image_drawing::*;
pub(crate) use node_paint::*;
pub(crate) use path::*;
pub use render::*;
pub(crate) use takumi_core::geometry::Placement;
pub(crate) use text_drawing::*;
use tiny_skia::PixmapRef;
#[cfg(any(feature = "svg", test))]
use tiny_skia::{IntSize, Pixmap};
pub use write::*;

use crate::resources::image_buffer::ImageBuffer;
pub(crate) use crate::{
  context::RenderContext,
  font_style::*,
  style::math::{div_255, mul_div_255},
};

/// Borrows an [`ImageBuffer`] as a zero-copy `tiny_skia` pixmap view.
pub(crate) fn pixmap_ref_from_buffer(buffer: &ImageBuffer) -> Option<PixmapRef<'_>> {
  PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height())
}

/// Copies an [`ImageBuffer`] into an owned `tiny_skia` pixmap.
#[cfg(any(feature = "svg", test))]
pub(crate) fn pixmap_from_buffer(buffer: &ImageBuffer) -> Option<Pixmap> {
  let size = IntSize::from_wh(buffer.width(), buffer.height())?;
  Pixmap::from_vec(buffer.data().to_vec(), size)
}

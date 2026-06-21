#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![allow(missing_docs)]
//! Raster (tiny-skia) painting backend for takumi: canvas, drawing, filters, and
//! the [`render`] entry point. Used via the `takumi` umbrella (`takumi::raster`) or
//! directly.
//!
//! Imports the `takumi-core` root privately so painting code resolves
//! `crate::layout`, `crate::resources`, `crate::Result`, etc. against the shared
//! core. Base types are _not_ re-exported from here; reach them through
//! `takumi::base` (or `takumi_core` directly).

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

use tiny_skia::{IntSize, Pixmap, PixmapRef};

use crate::resources::image_buffer::ImageBuffer;

pub(crate) use crate::font_style::*;
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
pub use write::*;

pub(crate) use crate::layout::style::{fast_div_255, fast_div_255_u32};

pub(crate) use crate::{context::RenderContext, layout::inline::scale_text_fit_x};

/// Borrows an [`ImageBuffer`] as a zero-copy `tiny_skia` pixmap view.
pub(crate) fn pixmap_ref_from_buffer(buffer: &ImageBuffer) -> Option<PixmapRef<'_>> {
  PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height())
}

/// Copies an [`ImageBuffer`] into an owned `tiny_skia` pixmap.
pub(crate) fn pixmap_from_buffer(buffer: &ImageBuffer) -> Option<Pixmap> {
  let size = IntSize::from_wh(buffer.width(), buffer.height())?;
  Pixmap::from_vec(buffer.data().to_vec(), size)
}

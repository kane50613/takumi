#![deny(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::unreachable,
  clippy::todo,
  clippy::unimplemented
)]
#![cfg_attr(
  test,
  allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
  )
)]
#![deny(missing_docs)]
//! Raster painting backend for takumi, built on ports of Chromium's Skia scan converters and shaders.
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
mod coverage;
/// Debug drawing utilities
mod debug_drawing;
mod dithering;
/// Filter rasterization (blur, drop-shadow, backdrop, pixel filters)
mod filter;
#[cfg(feature = "svg")]
mod gradient;
/// Image drawing functions
mod image_drawing;
pub(crate) mod inline_drawing;
mod node_paint;
mod palette;
mod path;
mod pixmap;
/// Main image renderer and viewport management
mod render;
#[cfg(all(test, feature = "pages"))]
mod render_pages_tests;
mod scanline;
mod shader;
mod simd;
mod stacking_context;
#[cfg(feature = "svg")]
mod svg;
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
#[cfg(feature = "svg")]
pub use svg::SvgPixmapRasterizer;
pub(crate) use takumi_core::geometry::Placement;
pub(crate) use text_drawing::*;
pub use write::*;

pub(crate) use crate::{
  context::RenderContext,
  font_style::*,
  style::math::{div_255, mul_div_255},
};

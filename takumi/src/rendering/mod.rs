/// Background and color drawing functions
mod background_drawing;
mod blend;
/// Canvas operations and image blending
mod canvas;
mod components;
/// Debug drawing utilities
mod debug_drawing;
mod dithering;
/// Image drawing functions
mod image_drawing;
pub(crate) mod inline_drawing;
mod path;
/// Main image renderer and viewport management
mod render;
mod stacking_context;
/// Text drawing functions
mod text_drawing;
mod webp;
mod write;

use std::borrow::Cow;
use std::{collections::HashMap, rc::Rc, sync::Arc};

use image::RgbaImage;
use taffy::Size;
use tiny_skia::{IntSize, Pixmap};

pub(crate) use background_drawing::*;
pub(crate) use blend::*;
pub(crate) use canvas::*;
pub(crate) use components::*;
pub(crate) use debug_drawing::*;
pub use dithering::*;
pub(crate) use image_drawing::*;
pub(crate) use path::*;
pub use render::*;
pub(crate) use text_drawing::*;
pub use write::*;

use crate::{
  GlobalContext,
  layout::{
    Viewport,
    style::{Affine, CalcArena, Color, ComputedStyle, StyleSheet},
  },
  resources::image::ImageSource,
};

/// The sizing context used for length value resolving.
#[derive(Clone)]
pub(crate) struct Sizing {
  /// The viewport for the image renderer.
  pub(crate) viewport: Viewport,
  /// The nearest query container size (content box) in device pixels.
  pub(crate) container_size: Size<Option<f32>>,
  /// The font size in pixels.
  pub(crate) font_size: f32,
  /// The calc arena shared by the current layout tree.
  pub(crate) calc_arena: Rc<CalcArena>,
}

impl Sizing {
  pub(crate) fn query_container_width(&self) -> f32 {
    self
      .container_size
      .width
      .unwrap_or(self.viewport.size.width.unwrap_or_default() as f32)
  }

  pub(crate) fn query_container_height(&self) -> f32 {
    self
      .container_size
      .height
      .unwrap_or(self.viewport.size.height.unwrap_or_default() as f32)
  }
}

/// The context for the internal rendering. You should not construct this directly.
#[derive(Clone)]
pub(crate) struct RenderContext<'g> {
  /// The global context.
  pub(crate) global: &'g GlobalContext,
  /// The scale factor for the image renderer.
  pub(crate) transform: Affine,
  /// The sizing context.
  pub(crate) sizing: Sizing,
  /// What the `currentColor` value is resolved to.
  pub(crate) current_color: Color,
  /// The style after inheritance.
  pub(crate) style: Box<ComputedStyle>,
  /// The active time for animation sampling.
  pub(crate) time: u64,
  /// Whether to draw debug borders.
  pub(crate) draw_debug_border: bool,
  /// The resources fetched externally.
  pub(crate) fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
  /// The stylesheets to apply before layout/rendering.
  pub(crate) stylesheet: Rc<StyleSheet>,
}

impl<'g> RenderContext<'g> {
  pub(crate) fn new(
    global: &'g GlobalContext,
    viewport: Viewport,
    fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
    stylesheet: Rc<StyleSheet>,
    time: u64,
  ) -> Self {
    Self {
      global,
      sizing: Sizing {
        viewport,
        container_size: Size::NONE,
        font_size: viewport.font_size,
        calc_arena: Rc::new(CalcArena::default()),
      },
      transform: Affine::IDENTITY,
      current_color: Color::black(),
      style: Box::default(),
      time,
      draw_debug_border: false,
      fetched_resources,
      stylesheet,
    }
  }

  /// Internal, only used in tests.
  #[cfg(test)]
  pub(crate) fn new_test(global: &'g GlobalContext, viewport: Viewport) -> Self {
    Self::new(global, viewport, Default::default(), Default::default(), 0)
  }
}

#[inline(always)]
pub(crate) fn fast_div_255(v: u32) -> u8 {
  fast_div_255_u32(v) as u8
}

/// Fast division by 255 by approximating `v / 255` using bitwise operations.
#[inline(always)]
pub(crate) fn fast_div_255_u32(v: u32) -> u32 {
  ((v.wrapping_add(128).wrapping_add(v >> 8)) >> 8).min(255)
}

#[inline(always)]
pub(crate) fn write_premultiplied_rgba(dst: &mut [u8], src: &[u8]) {
  for (dst_px, src_px) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
    let alpha = src_px[3];
    if alpha == u8::MAX {
      dst_px.copy_from_slice(src_px);
      continue;
    }
    if alpha == 0 {
      dst_px.copy_from_slice(&[0, 0, 0, 0]);
      continue;
    }

    let alpha_u32 = alpha as u32;
    dst_px[0] = fast_div_255(src_px[0] as u32 * alpha_u32);
    dst_px[1] = fast_div_255(src_px[1] as u32 * alpha_u32);
    dst_px[2] = fast_div_255(src_px[2] as u32 * alpha_u32);
    dst_px[3] = alpha;
  }
}

#[inline(always)]
fn has_opaque_alpha(raw: &[u8]) -> bool {
  raw.chunks_exact(4).all(|pixel| pixel[3] == u8::MAX)
}

#[inline(always)]
fn premultiply_rgba_in_place(raw: &mut [u8]) {
  for pixel in raw.chunks_exact_mut(4) {
    let alpha = pixel[3];
    if alpha == u8::MAX {
      continue;
    }
    if alpha == 0 {
      pixel[0] = 0;
      pixel[1] = 0;
      pixel[2] = 0;
      continue;
    }
    let alpha_u32 = alpha as u32;
    pixel[0] = fast_div_255(pixel[0] as u32 * alpha_u32);
    pixel[1] = fast_div_255(pixel[1] as u32 * alpha_u32);
    pixel[2] = fast_div_255(pixel[2] as u32 * alpha_u32);
  }
}

pub(crate) fn premultiplied_pixmap_from_rgba(source: Cow<'_, RgbaImage>) -> Option<Pixmap> {
  let (width, height, premultiplied) = match source {
    Cow::Owned(image) => {
      let width = image.width();
      let height = image.height();
      let mut raw = image.into_raw();
      if !has_opaque_alpha(&raw) {
        premultiply_rgba_in_place(&mut raw);
      }
      (width, height, raw)
    }
    Cow::Borrowed(image) => {
      let width = image.width();
      let height = image.height();
      let raw = image.as_raw();
      if has_opaque_alpha(raw) {
        (width, height, raw.to_vec())
      } else {
        let mut premultiplied = vec![0u8; raw.len()];
        write_premultiplied_rgba(&mut premultiplied, raw);
        (width, height, premultiplied)
      }
    }
  };

  let size = IntSize::from_wh(width, height)?;
  Pixmap::from_vec(premultiplied, size)
}

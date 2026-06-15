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

use crate::layout::tree::RenderNode;
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

pub(crate) use crate::context::RenderContext;

pub(crate) fn text_fit_x_correction(
  scale: f32,
  static_inline_prefix: f32,
  line_alignment_correction: f32,
) -> f32 {
  static_inline_prefix * (1.0 - scale) + line_alignment_correction
}

pub(crate) fn scale_text_fit_x(
  x: f32,
  origin_x: f32,
  scale: f32,
  static_inline_prefix: f32,
  line_alignment_correction: f32,
) -> f32 {
  if (scale - 1.0).abs() <= f32::EPSILON {
    return x;
  }

  text_fit_x_correction(scale, static_inline_prefix, line_alignment_correction)
    + origin_x
    + (x - origin_x) * scale
}

/// Borrows an [`ImageBuffer`] as a zero-copy `tiny_skia` pixmap view.
pub(crate) fn pixmap_ref_from_buffer(buffer: &ImageBuffer) -> Option<PixmapRef<'_>> {
  PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height())
}

/// Copies an [`ImageBuffer`] into an owned `tiny_skia` pixmap.
pub(crate) fn pixmap_from_buffer(buffer: &ImageBuffer) -> Option<Pixmap> {
  let size = IntSize::from_wh(buffer.width(), buffer.height())?;
  Pixmap::from_vec(buffer.data().to_vec(), size)
}

pub(crate) fn get_node_mut_by_path<'a, 'g>(
  root: &'a mut RenderNode<'g>,
  path: &[usize],
) -> Option<&'a mut RenderNode<'g>> {
  let mut current = root;
  for &index in path {
    let children = current.children.as_deref_mut()?;
    current = children.get_mut(index)?;
  }
  Some(current)
}

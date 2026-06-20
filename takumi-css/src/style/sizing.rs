use std::rc::Rc;

use taffy::Size;
use typed_builder::TypedBuilder;

use crate::Viewport;
use crate::style::CalcArena;

/// The sizing context used for length value resolving.
#[derive(Clone, TypedBuilder)]
pub struct SizingContext {
  /// The viewport for the image renderer.
  pub viewport: Viewport,
  /// The nearest query container size (content box) in device pixels.
  #[builder(default)]
  pub container_size: Size<Option<f32>>,
  /// The font size in pixels.
  #[builder(default = viewport.font_size * viewport.device_pixel_ratio)]
  pub font_size: f32,
  /// Computed `font-size` of the root element in device pixels. `None` before
  /// the root has been resolved; readers should fall back to `viewport.font_size`.
  /// https://www.w3.org/TR/css-values-4/#rem
  #[builder(default)]
  pub root_font_size: Option<f32>,
  /// Pixel basis for the `lh` unit.
  #[builder(default = viewport.font_size * viewport.device_pixel_ratio)]
  pub line_height: f32,
  /// Pixel basis for the `rlh` unit; `None` before root is resolved.
  #[builder(default)]
  pub root_line_height: Option<f32>,
  /// The calc arena shared by the current layout tree.
  #[builder(default)]
  pub calc_arena: Rc<CalcArena>,
}

impl SizingContext {
  /// Device-pixel basis for the `rem` unit.
  pub fn rem_basis(&self) -> f32 {
    self
      .root_font_size
      .unwrap_or(self.viewport.font_size * self.viewport.device_pixel_ratio)
  }

  pub fn root_line_height_basis(&self) -> f32 {
    self.root_line_height.unwrap_or(self.line_height)
  }

  pub fn query_container_width(&self) -> f32 {
    self
      .container_size
      .width
      .unwrap_or(self.viewport.size.width.unwrap_or_default() as f32)
  }

  pub fn query_container_height(&self) -> f32 {
    self
      .container_size
      .height
      .unwrap_or(self.viewport.size.height.unwrap_or_default() as f32)
  }
}

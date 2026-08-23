use std::{cell::Cell, rc::Rc};

use typed_builder::TypedBuilder;

use crate::{geometry::Size, style::CalcArena, viewport::Viewport};

/// The sizing context used for length value resolving.
#[derive(Clone, TypedBuilder)]
pub struct SizingContext {
  /// The viewport for the image renderer.
  pub viewport: Viewport,
  /// The nearest query container size (content box) in device pixels.
  #[builder(default, setter(skip))]
  pub(crate) container_size: Size<Option<f32>>,
  /// Set when a length resolves against the query container, so a caller can
  /// tell whether its result depends on one.
  #[builder(default, setter(skip))]
  pub(crate) container_read: Cell<bool>,
  /// The font size in pixels.
  #[builder(default = viewport.to_device(viewport.font_size))]
  pub font_size: f32,
  /// Computed `font-size` of the document root in device pixels, set only when
  /// the tree is a parsed document whose outermost node is an `<html>` element.
  /// A tree built in code is content rather than a document, so it leaves this
  /// `None` and `rem` resolves against the viewport.
  /// <https://www.w3.org/TR/css-values-4/#rem>
  #[builder(default)]
  pub root_font_size: Option<f32>,
  /// Pixel basis for the `lh` unit.
  #[builder(default = viewport.to_device(viewport.font_size))]
  pub line_height: f32,
  /// Pixel basis for the `rlh` unit, set alongside [`Self::root_font_size`].
  #[builder(default)]
  pub root_line_height: Option<f32>,
  /// The calc arena shared by the current layout tree.
  #[builder(default, setter(skip))]
  pub(crate) calc_arena: Rc<CalcArena>,
}

impl SizingContext {
  /// Converts an author-space CSS-pixel value into device pixels via
  /// [`crate::viewport::Viewport::to_device`], the single dpr boundary.
  #[inline]
  pub fn to_device(&self, css_px: f32) -> f32 {
    self.viewport.to_device(css_px)
  }

  /// Converts a device-pixel value back into author-space CSS pixels.
  #[inline]
  pub(crate) fn to_css(&self, device_px: f32) -> f32 {
    self.viewport.to_css(device_px)
  }

  /// Returns a copy with the font and line-height metrics overridden, inheriting
  /// the viewport, container size, and shared calc arena.
  pub(crate) fn with_font_metrics(
    &self,
    font_size: f32,
    root_font_size: Option<f32>,
    line_height: f32,
    root_line_height: Option<f32>,
  ) -> Self {
    Self {
      font_size,
      root_font_size,
      line_height,
      root_line_height,
      ..self.clone()
    }
  }

  /// Resolves an interned `calc(...)` handle against this context's arena.
  pub(crate) fn resolve_calc(&self, value: *const (), basis: f32) -> f32 {
    self.calc_arena.resolve_calc_value(value, basis)
  }

  /// Device-pixel basis for the `rem` unit.
  pub(crate) fn rem_basis(&self) -> f32 {
    self
      .root_font_size
      .unwrap_or(self.to_device(self.viewport.font_size))
  }

  /// Device-pixel basis for the `rlh` unit.
  pub(crate) fn root_line_height_basis(&self) -> f32 {
    self
      .root_line_height
      .unwrap_or(self.to_device(self.viewport.font_size))
  }

  /// Sets the nearest query container size (content box), in device pixels.
  pub fn set_container_size(&mut self, width: Option<f32>, height: Option<f32>) {
    self.container_size = Size { width, height };
  }

  /// Query container width in device pixels, falling back to the viewport.
  pub(crate) fn query_container_width(&self) -> f32 {
    self.container_read.set(true);
    self
      .container_size
      .width
      .unwrap_or(self.viewport.size.width.unwrap_or_default() as f32)
  }

  /// Query container height in device pixels, falling back to the viewport.
  pub(crate) fn query_container_height(&self) -> f32 {
    self.container_read.set(true);
    self
      .container_size
      .height
      .unwrap_or(self.viewport.size.height.unwrap_or_default() as f32)
  }
}

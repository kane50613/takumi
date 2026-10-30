use std::cell::Cell;

use typed_builder::TypedBuilder;

use crate::{geometry::Size, viewport::Viewport};

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
  /// Pixel basis for the `ch` unit: the advance of the `0` glyph, as Blink's
  /// `FontSizes::Ch` reads it from `PrimaryFontWithDigitZero`.
  #[builder(default = viewport.to_device(viewport.font_size) / 2.0)]
  pub(crate) ch: f32,
  /// Pixel basis for the `ex` unit: the x-height of the primary font, or half the font size when
  /// it has none, as Blink's `FontSizes::Ex`.
  #[builder(default = viewport.to_device(viewport.font_size) / 2.0)]
  pub(crate) ex: f32,
}

/// The pixel bases the `ch` and `ex` units read from the primary font.
#[derive(Clone, Copy)]
pub(crate) struct FontUnits {
  pub(crate) ch: f32,
  pub(crate) ex: f32,
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

  /// A copy that resolves lengths in CSS pixels, at a device pixel ratio of one.
  #[cfg(feature = "svg")]
  pub(crate) fn in_css_pixels(&self) -> Self {
    let css_size =
      |device: Option<u32>| device.map(|device| self.to_css(device as f32).round() as u32);

    Self {
      viewport: Viewport {
        size: (
          css_size(self.viewport.size.width),
          css_size(self.viewport.size.height),
        )
          .into(),
        device_pixel_ratio: 1.0,
        unit_reference: Some(Size {
          width: self.to_css(self.viewport.unit_width()),
          height: self.to_css(self.viewport.unit_height()),
        }),
        ..self.viewport
      },
      container_size: self
        .container_size
        .map(|size| size.map(|size| self.to_css(size))),
      container_read: Cell::new(false),
      font_size: self.to_css(self.font_size),
      root_font_size: self.root_font_size.map(|size| self.to_css(size)),
      line_height: self.to_css(self.line_height),
      root_line_height: self.root_line_height.map(|size| self.to_css(size)),
      ch: self.to_css(self.ch),
      ex: self.to_css(self.ex),
    }
  }

  /// Whether lengths resolve the same against both, whatever either read of a query container.
  pub(crate) fn resolves_like(&self, other: &Self) -> bool {
    self.viewport == other.viewport
      && self.container_size == other.container_size
      && self.font_size == other.font_size
      && self.root_font_size == other.root_font_size
      && self.line_height == other.line_height
      && self.root_line_height == other.root_line_height
      && self.ch == other.ch
      && self.ex == other.ex
  }

  /// A copy with its font and line-height metrics replaced, keeping its viewport and container.
  pub(crate) fn with_font_metrics(
    &self,
    font_size: f32,
    root_font_size: Option<f32>,
    line_height: f32,
    root_line_height: Option<f32>,
    units: FontUnits,
  ) -> Self {
    Self {
      font_size,
      root_font_size,
      line_height,
      root_line_height,
      ch: units.ch,
      ex: units.ex,
      ..self.clone()
    }
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

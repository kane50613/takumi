use crate::geometry::{AvailableSpace, Size};

/// The default font size in pixels.
pub(crate) const DEFAULT_FONT_SIZE: f32 = 16.0;

/// The default device pixel ratio.
pub const DEFAULT_DEVICE_PIXEL_RATIO: f32 = 1.0;

/// What `@media` matches a stylesheet's media type against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaTarget {
  /// Raster and SVG output.
  #[default]
  Screen,
  /// PDF output.
  Print,
}

/// The viewport for the image renderer.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
  /// Size of the viewport
  pub size: ViewportSize,
  /// Initial font size in pixels, used as the fallback when no root element
  /// has set its computed `font-size` (see CSS Values 4 §6.1 for `rem`).
  pub font_size: f32,
  /// The device pixel ratio.
  pub device_pixel_ratio: f32,
  /// The media type `@media` queries resolve against.
  pub media_target: MediaTarget,
  /// What viewport-percentage units resolve against when it differs from
  /// `size`: in paged output the content column lays out at unbounded height
  /// while `vh` takes the page area (CSS Values 4 §6.1.2, print media).
  pub unit_reference: Option<Size<f32>>,
}

impl From<Viewport> for Size<AvailableSpace> {
  fn from(value: Viewport) -> Self {
    Size::new(value.size.width, value.size.height).map(|length| {
      length.map_or(AvailableSpace::MaxContent, |length| {
        AvailableSpace::Definite(length as f32)
      })
    })
  }
}

impl Default for Viewport {
  fn default() -> Self {
    Self::new((None, None))
  }
}

impl Viewport {
  /// Creates a new viewport with the default font size.
  pub fn new(size: impl Into<ViewportSize>) -> Self {
    Self {
      size: size.into(),
      font_size: DEFAULT_FONT_SIZE,
      device_pixel_ratio: DEFAULT_DEVICE_PIXEL_RATIO,
      media_target: MediaTarget::Screen,
      unit_reference: None,
    }
  }

  /// Sets the font size in pixels.
  pub const fn with_font_size(mut self, font_size: f32) -> Self {
    self.font_size = font_size;
    self
  }

  /// Sets the device pixel ratio.
  pub const fn with_device_pixel_ratio(mut self, device_pixel_ratio: f32) -> Self {
    self.device_pixel_ratio = device_pixel_ratio;
    self
  }

  /// Sets the media type `@media` queries resolve against.
  pub const fn with_media_target(mut self, media_target: MediaTarget) -> Self {
    self.media_target = media_target;
    self
  }

  /// Sets the size viewport-percentage units resolve against.
  pub const fn with_unit_reference(mut self, size: Size<f32>) -> Self {
    self.unit_reference = Some(size);
    self
  }

  /// The width `vw` resolves against: the unit reference, else the viewport.
  pub fn unit_width(self) -> f32 {
    self
      .unit_reference
      .map_or(self.size.width.unwrap_or_default() as f32, |size| {
        size.width
      })
  }

  /// The height `vh` resolves against: the unit reference, else the viewport.
  pub fn unit_height(self) -> f32 {
    self
      .unit_reference
      .map_or(self.size.height.unwrap_or_default() as f32, |size| {
        size.height
      })
  }

  /// The effective device-pixel ratio, treating a non-positive value as `1.0`.
  #[inline]
  pub(crate) fn effective_dpr(self) -> f32 {
    if self.device_pixel_ratio > 0.0 {
      self.device_pixel_ratio
    } else {
      1.0
    }
  }

  /// Converts an author-space CSS-pixel value into device pixels, the engine's
  /// canonical unit (`viewport.size`, layout boxes and everything downstream of
  /// [`crate::style::Length::to_px`] are device pixels). The only place the
  /// device-pixel ratio is multiplied in; never write `value * dpr` elsewhere.
  #[inline]
  pub fn to_device(self, css_px: f32) -> f32 {
    css_px * self.effective_dpr()
  }

  /// Converts a device-pixel value back into author-space CSS pixels. The
  /// inverse of [`Self::to_device`]; the only place the ratio is divided out.
  #[inline]
  pub(crate) fn to_css(self, device_px: f32) -> f32 {
    device_px / self.effective_dpr()
  }
}

/// Represents Viewport size
#[derive(Debug, Clone, Copy, Default)]
pub struct ViewportSize {
  /// The width of the viewport in pixels.
  pub width: Option<u32>,
  /// The height of the viewport in pixels.
  pub height: Option<u32>,
}

impl From<(u32, u32)> for ViewportSize {
  fn from(value: (u32, u32)) -> Self {
    Self {
      width: Some(value.0),
      height: Some(value.1),
    }
  }
}

impl From<(Option<u32>, u32)> for ViewportSize {
  fn from(value: (Option<u32>, u32)) -> Self {
    Self {
      width: value.0,
      height: Some(value.1),
    }
  }
}

impl From<(u32, Option<u32>)> for ViewportSize {
  fn from(value: (u32, Option<u32>)) -> Self {
    Self {
      width: Some(value.0),
      height: value.1,
    }
  }
}

impl From<(Option<u32>, Option<u32>)> for ViewportSize {
  fn from(value: (Option<u32>, Option<u32>)) -> Self {
    Self {
      width: value.0,
      height: value.1,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_viewport_new_defaults() {
    let v = Viewport::new((800, 600));
    assert_eq!(v.size.width, Some(800));
    assert_eq!(v.size.height, Some(600));
    assert_eq!(v.font_size, DEFAULT_FONT_SIZE);
  }
}

use taffy::{AvailableSpace, Size};

/// The default font size in pixels.
pub const DEFAULT_FONT_SIZE: f32 = 16.0;

/// The default device pixel ratio.
pub const DEFAULT_DEVICE_PIXEL_RATIO: f32 = 1.0;

/// The viewport for the image renderer.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Viewport {
  /// The width of the viewport in pixels.
  pub width: Option<u32>,
  /// The height of the viewport in pixels.
  pub height: Option<u32>,
  /// The font size in pixels, used for em and rem units.
  pub font_size: f32,
  /// The device pixel ratio.
  pub device_pixel_ratio: f32,
}

impl From<Viewport> for Size<AvailableSpace> {
  fn from(value: Viewport) -> Self {
    Self {
      width: if let Some(width) = value.width {
        AvailableSpace::Definite(width as f32)
      } else {
        AvailableSpace::MaxContent
      },
      height: if let Some(height) = value.height {
        AvailableSpace::Definite(height as f32)
      } else {
        AvailableSpace::MaxContent
      },
    }
  }
}

impl From<(u32, u32)> for Viewport {
  fn from((width, height): (u32, u32)) -> Self {
    Self::new(Some(width), Some(height))
  }
}

impl Default for Viewport {
  fn default() -> Self {
    Self::new(None, None)
  }
}

impl Viewport {
  /// Creates a new viewport with the default font size.
  pub const fn new(width: Option<u32>, height: Option<u32>) -> Self {
    Self {
      width,
      height,
      font_size: DEFAULT_FONT_SIZE,
      device_pixel_ratio: DEFAULT_DEVICE_PIXEL_RATIO,
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
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_viewport_new_defaults() {
    let v = Viewport::new(Some(800), Some(600));
    assert_eq!(v.width, Some(800));
    assert_eq!(v.height, Some(600));
    assert_eq!(v.font_size, DEFAULT_FONT_SIZE);
  }
}

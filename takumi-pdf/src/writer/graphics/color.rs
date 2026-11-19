//! Dealing with colors and color spaces.
//!
//! # Color spaces
//!
//! Paint colors come in two models, RGB and Luma, each with its own color type. CMYK has no
//! color type: it is only the device color space CMYK JPEGs draw in.
//!
//! # Representation of colors
//!
//! When specifying colors in the process color spaces RGB, Luma, and CMYK, it is important
//! to understand the distinction between device-dependent and decide-independent color
//! specification. What follows is only a very brief explanation, if you want to dive into
//! more details, please look for appropriate resources on the web.
//!
//! When specifying colors in a *device-dependent way*, if I instruct the program to draw
//! the RGB color (145, 120, 45), then the program will use these literal values to activate
//! the R/G/B lights to achieve displaying a certain color. The problem is that specifying
//! colors in such a way can lead to slightly different results when actually displaying it,
//! depending on the screen that is used, since each screen is calibrated differently and
//! based on different display technologies. This is especially critical for printers, where
//! different values for CMYK colors might result in different-looking colors when being printed.
//!
//! This is why there is also the option to specify colors in a *device-independent* way,
//! which basically means that the color value (145, 120, 45) is represented in a well-specified
//! color space, and each device can then convert the colors to their native color space
//! so that they match the representation in the given color space as closely as possible.
//! This should lead to a more accurate color representation across different screens.
//!
//! In 90% of the cases, it is totally fine to just use a device-dependent colorspace, and it's
//! what the writer does by default. However, if you do care about that, then you can set the
//! `no_device_cs` property of [`SerializeSettings`] to true, in which case the writer will embed an ICC profile for the
//! sGrey and sRGB color spaces (for Luma and RGB colors, respectively). If a CMYK profile
//! was provided to the serialize settings, this will be used for CMYK colors. Otherwise,
//! it will fall back to device CMYK.
//!
//! [`SerializeSettings`]: crate::writer::SerializeSettings

use std::fmt::Debug;
use std::hash::Hash;

use crate::writer::serialize::SerializeContext;

/// The PDF name for the device RGB color space.
pub(crate) const DEVICE_RGB: &str = "DeviceRGB";
/// The PDF name for the device gray color space.
pub(crate) const DEVICE_GRAY: &str = "DeviceGray";
/// The PDF name for the device CMYK color space.
pub(crate) const DEVICE_CMYK: &str = "DeviceCMYK";

/// An RGB or gray color.
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum Color {
  /// An RGB-based color.
  Rgb(rgb::Color),
  /// A luma-based color.
  Luma(luma::Color),
}

/// An eight-bit colour component as a PDF number.
///
/// Four decimals resolve far finer than the 1/255 the component came from, and
/// shortest round-trip would otherwise spell out `0.047058824` for every one of
/// them.
fn component(value: u8) -> f32 {
  (f32::from(value) / 255.0 * 10_000.0).round() / 10_000.0
}

impl Color {
  pub(crate) fn to_pdf_color(self) -> Vec<f32> {
    match self {
      Self::Rgb(rgb) => rgb.to_pdf_color().to_vec(),
      Self::Luma(l) => vec![l.to_pdf_color()],
    }
  }

  pub(crate) fn color_space(&self, sc: &mut SerializeContext) -> ColorSpace {
    match self {
      Self::Rgb(r) => r.color_space(sc.serialize_settings().no_device_cs),
      Self::Luma(_) => luma::color_space(sc.serialize_settings().no_device_cs),
    }
  }

  /// Return the current color as RGB for use with colored glyphs (SVG and
  /// COLR).
  pub(crate) fn as_rgb(self) -> rgb::Color {
    match self {
      Self::Rgb(r) => r,
      Self::Luma(l) => rgb::Color::new(l.0, l.0, l.0),
    }
  }
}

/// Gray-scale colors.
pub mod luma {
  use crate::writer::color::{CieBasedColorSpace, ColorSpace, DeviceColorSpace};

  /// A luma color.
  #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
  pub struct Color(pub(crate) u8);

  impl Color {
    /// Create a new luma color.
    pub fn new(lightness: u8) -> Color {
      Color(lightness)
    }

    /// Create a black luma color.
    pub fn black() -> Self {
      Self::new(0)
    }

    pub(crate) fn to_pdf_color(self) -> f32 {
      crate::writer::graphics::color::component(self.0)
    }
  }

  impl From<Color> for super::Color {
    fn from(val: Color) -> Self {
      super::Color::Luma(val)
    }
  }

  impl Default for Color {
    fn default() -> Self {
      Color::new(0)
    }
  }

  pub(crate) fn color_space(no_device_cs: bool) -> ColorSpace {
    if no_device_cs {
      CieBasedColorSpace::Luma.into()
    } else {
      DeviceColorSpace::Gray.into()
    }
  }
}

/// RGB colors.
pub mod rgb {
  use crate::writer::color::{CieBasedColorSpace, ColorSpace, DeviceColorSpace};

  /// An RGB color.
  #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
  pub struct Color(pub(crate) u8, pub(crate) u8, pub(crate) u8);

  impl Default for Color {
    fn default() -> Self {
      Color::black()
    }
  }

  impl Color {
    /// Create a new RGB color.
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
      Color(red, green, blue)
    }

    /// Create a black RGB color.
    pub fn black() -> Self {
      Self::new(0, 0, 0)
    }

    pub(crate) fn to_pdf_color(self) -> [f32; 3] {
      [
        crate::writer::graphics::color::component(self.0),
        crate::writer::graphics::color::component(self.1),
        crate::writer::graphics::color::component(self.2),
      ]
    }

    pub(super) fn color_space(&self, no_device_cs: bool) -> ColorSpace {
      color_space(no_device_cs)
    }
  }

  impl From<Color> for super::Color {
    fn from(val: Color) -> Self {
      super::Color::Rgb(val)
    }
  }

  pub(crate) fn color_space(no_device_cs: bool) -> ColorSpace {
    if no_device_cs {
      CieBasedColorSpace::Srgb.into()
    } else {
      DeviceColorSpace::Rgb.into()
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum ColorSpace {
  Device(DeviceColorSpace),
  CieBased(CieBasedColorSpace),
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum DeviceColorSpace {
  Rgb,
  Gray,
  /// The space CMYK JPEGs draw in.
  Cmyk,
}

impl From<DeviceColorSpace> for ColorSpace {
  fn from(value: DeviceColorSpace) -> Self {
    Self::Device(value)
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum CieBasedColorSpace {
  Srgb,
  Luma,
}

impl From<CieBasedColorSpace> for ColorSpace {
  fn from(value: CieBasedColorSpace) -> Self {
    Self::CieBased(value)
  }
}

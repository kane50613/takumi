//! Dealing with colors and color spaces.
//!
//! # Color spaces
//!
//! krilla currently supports four color models:
//! - RGB
//! - Luma
//! - CMYK
//! - Separation (also known as Spot)
//!
//! Each color space is associated with its specific color type, which you can use to create new
//! instances of a specific color in that color space.
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
//! what krilla does by default. However, if you do care about that, then you can set the
//! `no_device_cs` property of [`SerializeSettings`] to true, in which case krilla will embed an ICC profile for the
//! sGrey and sRGB color spaces (for Luma and RGB colors, respectively). If a CMYK profile
//! was provided to the serialize settings, this will be used for CMYK colors. Otherwise,
//! it will fall back to device CMYK.
//!
//! # Separations
//!
//! An alternative way to achieve exact color reproduction in print are Separation colors.
//! Using these colors, you can request a specific colorant through its well-known name
//! (e.g. through a color registry such as PANTONE or RAL). Your production shop can then
//! supply that exact pigment.
//!
//! It is important to recognize that, when viewed on a computer or printed on a home printer,
//! the requested colorant won't be available. For that reason, you will need to supply a fallback
//! process color.
//!
//! Pay attention to remain consistent: Both in your use of colorant names and fallback colors. A
//! given colorant should not be referred to by multiple, slightly different names. Just the same
//! for fallback colors: For a given colorant, there should be only one fallback color. Using multiple
//! Separation color spaces with the same colorant and distinct fallback colors is considered a bad
//! practice and forbidden in PDF/A-2 and later.
//!
//! [`SerializeSettings`]: crate::krilla::SerializeSettings

use std::fmt::Debug;
use std::hash::Hash;

use crate::krilla::configure::ValidationError;
use crate::krilla::graphics::icc::ICCBasedColorSpace;
use crate::krilla::serialize::SerializeContext;

/// The PDF name for the device RGB color space.
pub(crate) const DEVICE_RGB: &str = "DeviceRGB";
/// The PDF name for the device gray color space.
pub(crate) const DEVICE_GRAY: &str = "DeviceGray";
/// The PDF name for the device CMYK color space.
pub(crate) const DEVICE_CMYK: &str = "DeviceCMYK";

/// A wrapper for storing colors from different color spaces.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum Color {
  /// A device or CIE-based color.
  Regular(RegularColor),
  /// A special color space.
  Special(SpecialColor),
}

/// A device or CIE-based color.
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum RegularColor {
  /// An RGB-based color.
  Rgb(rgb::Color),
  /// A luma-based color.
  Luma(luma::Color),
  /// A device CMYK color.
  Cmyk(cmyk::Color),
}

/// A special color space color.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum SpecialColor {
  /// A separation color.
  Separation(separation::Color),
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
  pub(crate) fn to_pdf_color(&self) -> Vec<f32> {
    match self {
      Color::Regular(RegularColor::Rgb(rgb)) => rgb.to_pdf_color().to_vec(),
      Color::Regular(RegularColor::Luma(l)) => vec![l.to_pdf_color()],
      Color::Regular(RegularColor::Cmyk(cmyk)) => cmyk.to_pdf_color().to_vec(),
      Color::Special(SpecialColor::Separation(spot)) => vec![spot.to_pdf_color()],
    }
  }

  pub(crate) fn color_space(&self, sc: &mut SerializeContext) -> ColorSpace {
    match self {
      Color::Regular(c) => c.color_space(sc).into(),
      Color::Special(c) => c.color_space().into(),
    }
  }

  /// Convert a color to a regular color for use with constructs like tags or
  /// annotations that don't support special color spaces
  pub(crate) fn to_regular(&self) -> RegularColor {
    match self {
      Color::Regular(c) => *c,
      Color::Special(SpecialColor::Separation(c)) => c.space.fallback,
    }
  }
}

impl RegularColor {
  pub(crate) fn color_space(&self, sc: &mut SerializeContext) -> RegularColorSpace {
    match self {
      Self::Rgb(r) => r.color_space(sc.serialize_settings().no_device_cs),
      Self::Luma(_) => luma::color_space(sc.serialize_settings().no_device_cs),
      Self::Cmyk(_) => match cmyk::color_space(&sc.serialize_settings()) {
        None => {
          sc.register_validation_error(ValidationError::MissingCMYKProfile);
          DeviceColorSpace::Cmyk.into()
        }
        Some(cs) => cs,
      },
    }
  }

  /// Return the current color as RGB for use with colored glyphs (SVG and
  /// COLR).
  pub(crate) fn as_rgb(self) -> Option<rgb::Color> {
    Some(match self {
      Self::Rgb(r) => r,
      Self::Luma(l) => rgb::Color::new(l.0, l.0, l.0),
      Self::Cmyk(_) => return None,
    })
  }

  /// Returns true if this is a subtractive color space (CMYK), false otherwise (RGB, Luma).
  /// Used for determining the correct tint transform behavior in Separation color spaces.
  pub(crate) fn is_subtractive(self) -> bool {
    matches!(self, Self::Cmyk(_))
  }
}

impl SpecialColor {
  pub(crate) fn color_space(&self) -> SpecialColorSpace {
    match self {
      Self::Separation(spot) => spot.color_space().into(),
    }
  }
}

/// Gray-scale colors.
pub mod luma {
  use crate::krilla::color::{
    CieBasedColorSpace, DeviceColorSpace, RegularColor, RegularColorSpace,
  };

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

    /// Create a white RGB color.
    pub fn white() -> Self {
      Self::new(255)
    }

    pub(crate) fn to_pdf_color(self) -> f32 {
      crate::krilla::graphics::color::component(self.0)
    }
  }

  impl From<Color> for super::RegularColor {
    fn from(val: Color) -> Self {
      super::RegularColor::Luma(val)
    }
  }

  impl From<Color> for super::Color {
    fn from(val: Color) -> Self {
      RegularColor::from(val).into()
    }
  }

  impl Default for Color {
    fn default() -> Self {
      Color::new(0)
    }
  }

  pub(crate) fn color_space(no_device_cs: bool) -> RegularColorSpace {
    if no_device_cs {
      CieBasedColorSpace::Luma.into()
    } else {
      DeviceColorSpace::Gray.into()
    }
  }
}

impl From<RegularColor> for Color {
  fn from(value: RegularColor) -> Self {
    Self::Regular(value)
  }
}

impl From<SpecialColor> for Color {
  fn from(value: SpecialColor) -> Self {
    Self::Special(value)
  }
}

/// CMYK colors.
pub mod cmyk {
  use crate::krilla::SerializeSettings;
  use crate::krilla::color::{CieBasedColorSpace, DeviceColorSpace, RegularColorSpace};
  use crate::krilla::graphics::icc::ICCBasedColorSpace;

  /// A CMYK color.
  #[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
  pub struct Color(pub(crate) u8, pub(crate) u8, pub(crate) u8, pub(crate) u8);

  impl Color {
    /// Create a new CMYK color.
    pub fn new(cyan: u8, magenta: u8, yellow: u8, black: u8) -> Color {
      Color(cyan, magenta, yellow, black)
    }

    pub(crate) fn to_pdf_color(self) -> [f32; 4] {
      [
        crate::krilla::graphics::color::component(self.0),
        crate::krilla::graphics::color::component(self.1),
        crate::krilla::graphics::color::component(self.2),
        crate::krilla::graphics::color::component(self.3),
      ]
    }
  }

  impl From<Color> for super::RegularColor {
    fn from(val: Color) -> Self {
      super::RegularColor::Cmyk(val)
    }
  }

  impl From<Color> for super::Color {
    fn from(val: Color) -> Self {
      super::RegularColor::from(val).into()
    }
  }

  impl Default for Color {
    fn default() -> Self {
      Color::new(0, 0, 0, 255)
    }
  }

  pub(crate) fn color_space(ss: &SerializeSettings) -> Option<RegularColorSpace> {
    if ss.no_device_cs {
      ss.clone()
        .cmyk_profile
        .map(|p| CieBasedColorSpace::Cmyk(ICCBasedColorSpace::<4>(p.clone())).into())
    } else {
      Some(DeviceColorSpace::Cmyk.into())
    }
  }
}

/// RGB colors.
pub mod rgb {
  use crate::krilla::color::{CieBasedColorSpace, DeviceColorSpace, RegularColorSpace};

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

    /// Create a new linear RGB color.
    pub fn new_linear(red: u8, green: u8, blue: u8) -> Self {
      Color(red, green, blue)
    }

    /// Create a black RGB color.
    pub fn black() -> Self {
      Self::new(0, 0, 0)
    }

    /// Create a white RGB color.
    pub fn white() -> Self {
      Self::new(255, 255, 255)
    }

    /// The `red` component of the color.
    pub fn red(&self) -> u8 {
      self.0
    }

    /// The `green` component of the color.
    pub fn green(&self) -> u8 {
      self.1
    }

    /// The `blue` component of the color.
    pub fn blue(&self) -> u8 {
      self.2
    }

    pub(crate) fn to_pdf_color(self) -> [f32; 3] {
      [
        crate::krilla::graphics::color::component(self.0),
        crate::krilla::graphics::color::component(self.1),
        crate::krilla::graphics::color::component(self.2),
      ]
    }

    pub(super) fn color_space(&self, no_device_cs: bool) -> RegularColorSpace {
      color_space(no_device_cs)
    }
  }

  impl From<Color> for super::RegularColor {
    fn from(val: Color) -> Self {
      super::RegularColor::Rgb(val)
    }
  }

  impl From<Color> for super::Color {
    fn from(val: Color) -> Self {
      super::RegularColor::from(val).into()
    }
  }

  pub(crate) fn color_space(no_device_cs: bool) -> RegularColorSpace {
    if no_device_cs {
      CieBasedColorSpace::Srgb.into()
    } else {
      DeviceColorSpace::Rgb.into()
    }
  }
}

/// Separation (spot) colors.
pub mod separation {
  use crate::krilla::color::RegularColor;

  /// A spot color.
  #[derive(Debug, Hash, Eq, PartialEq, Clone)]
  pub struct Color {
    pub(crate) tint: u8,
    pub(crate) space: SeparationSpace,
  }

  impl Color {
    /// Create a new spot color.
    pub fn new(tint: u8, space: SeparationSpace) -> Self {
      Self { tint, space }
    }

    pub(crate) fn to_pdf_color(&self) -> f32 {
      crate::krilla::graphics::color::component(self.tint)
    }

    pub(crate) fn color_space(&self) -> SeparationSpace {
      self.space.clone()
    }
  }

  impl Default for Color {
    fn default() -> Self {
      Color::new(0, SeparationSpace::default())
    }
  }

  impl From<Color> for super::SpecialColor {
    fn from(val: Color) -> Self {
      super::SpecialColor::Separation(val)
    }
  }

  impl From<Color> for super::Color {
    fn from(val: Color) -> Self {
      super::SpecialColor::from(val).into()
    }
  }

  /// A Separation color space (also known as spot colors).
  ///
  /// Separation color spaces use a single, subtractive colorant and allow
  /// achieving exact color reproduction in print.
  ///
  /// Krilla automatically linearly scales the fallback color with the separation
  /// tint.
  #[derive(Debug, Eq, PartialEq, Hash, Clone)]
  pub struct SeparationSpace {
    pub(crate) colorant: SeparationColorant,
    pub(crate) fallback: RegularColor,
  }

  impl SeparationSpace {
    /// Create a new Separation space.
    ///
    /// To export PDF/A-2 and later, make sure that a single Separation Colorant
    /// is always used with the same fallback color.
    pub fn new(colorant: SeparationColorant, fallback: RegularColor) -> Self {
      Self { colorant, fallback }
    }
  }

  impl From<SeparationSpace> for super::SpecialColorSpace {
    fn from(value: SeparationSpace) -> Self {
      Self::Separation(value)
    }
  }

  impl Default for SeparationSpace {
    fn default() -> Self {
      Self {
        colorant: SeparationColorant::default(),
        fallback: super::rgb::Color::default().into(),
      }
    }
  }

  /// What colorant to use for colors in this space.
  #[derive(Debug, Eq, PartialEq, Hash, Clone, Default)]
  pub enum SeparationColorant {
    /// Don't apply colorant at all. Sometimes used to indicate other production
    /// info, such as cuts.
    #[default]
    NoColorant,
    /// Apply the same amount of each available colorants.
    AllColorants,
    /// Specify a colorant name.
    Custom(String),
  }

  impl SeparationColorant {
    pub(crate) fn to_pdf<'a>(&'a self) -> pdf_writer::Name<'a> {
      match self {
        Self::AllColorants => pdf_writer::Name(b"All"),
        Self::NoColorant => pdf_writer::Name(b"None"),
        Self::Custom(s) => pdf_writer::Name(s.as_bytes()),
      }
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum ColorSpace {
  Device(DeviceColorSpace),
  CieBased(CieBasedColorSpace),
  Special(SpecialColorSpace),
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum RegularColorSpace {
  Device(DeviceColorSpace),
  CieBased(CieBasedColorSpace),
}

impl From<RegularColorSpace> for ColorSpace {
  fn from(value: RegularColorSpace) -> Self {
    match value {
      RegularColorSpace::Device(s) => Self::Device(s),
      RegularColorSpace::CieBased(s) => Self::CieBased(s),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum DeviceColorSpace {
  Rgb,
  Gray,
  Cmyk,
}

impl From<DeviceColorSpace> for ColorSpace {
  fn from(value: DeviceColorSpace) -> Self {
    Self::Device(value)
  }
}

impl From<DeviceColorSpace> for RegularColorSpace {
  fn from(value: DeviceColorSpace) -> Self {
    Self::Device(value)
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum CieBasedColorSpace {
  Srgb,
  Luma,
  Cmyk(ICCBasedColorSpace<4>),
}

impl From<CieBasedColorSpace> for ColorSpace {
  fn from(value: CieBasedColorSpace) -> Self {
    Self::CieBased(value)
  }
}

impl From<CieBasedColorSpace> for RegularColorSpace {
  fn from(value: CieBasedColorSpace) -> Self {
    Self::CieBased(value)
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum SpecialColorSpace {
  /// A Separation color space with its colorant and fallback.
  Separation(separation::SeparationSpace),
}

impl From<SpecialColorSpace> for ColorSpace {
  fn from(value: SpecialColorSpace) -> Self {
    Self::Special(value)
  }
}

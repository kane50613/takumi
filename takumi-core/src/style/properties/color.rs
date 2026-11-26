use std::{
  fmt::{self, Display},
  sync::Arc,
};

use color::{AlphaColor, ColorSpaceTag, DynamicColor, HueDirection, Rgba8, Srgb, parse_color};
use cssparser::{
  Parser, Token,
  color::{parse_hash_color, parse_named_color},
  match_ignore_ascii_case,
};
use tiny_skia::{ColorU8, PremultipliedColorU8};

use super::gradient_utils::interpolate_rgba_premultiplied;

use crate::{
  resources::image_buffer::premultiply_pixel,
  style::{
    Animatable, Color as CurrentColor, FromCss, FromCssStr, MakeComputed, ParseResult,
    SizingContext, ToCss, impl_css_enum,
    math::mul_div_255,
    parse_percentage_value, significant,
    tw::TailwindPropertyParser,
    tw::{Namespace, extract_arbitrary_value},
    unexpected_token,
  },
  viewport::ColorScheme,
};

fn is_cylindrical_color_space(color_space: ColorSpaceTag) -> bool {
  matches!(
    color_space,
    ColorSpaceTag::Lch | ColorSpaceTag::Oklch | ColorSpaceTag::Hsl | ColorSpaceTag::Hwb
  )
}

/// Color interpolation configuration used by functions like `color-mix()` and gradients.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ColorInterpolationMethod {
  /// The color space used to interpolate between two colors.
  pub(crate) color_space: ColorSpaceTag,
  /// Optional hue interpolation strategy for cylindrical color spaces.
  pub(crate) hue_direction: HueDirection,
}

impl Default for ColorInterpolationMethod {
  /// <https://developer.mozilla.org/en-US/docs/Web/CSS/color-interpolation-method>: interpolating
  /// `<color>` values defaults to Oklab.
  fn default() -> Self {
    Self::MODERN
  }
}

impl ColorInterpolationMethod {
  /// Gradient default while every stop is written in a legacy form
  /// (css-color-4 §12.2, Blink `Gradient::CreateShaderInternal`).
  pub(crate) const LEGACY: Self = Self {
    color_space: ColorSpaceTag::Srgb,
    hue_direction: HueDirection::Shorter,
  };
  /// Gradient default once any stop uses a modern color function.
  pub(crate) const MODERN: Self = Self {
    color_space: ColorSpaceTag::Oklab,
    hue_direction: HueDirection::Shorter,
  };

  pub(crate) const fn gradient_default(has_modern_stop: bool) -> Self {
    if has_modern_stop {
      Self::MODERN
    } else {
      Self::LEGACY
    }
  }
}

impl<'i> FromCss<'i> for ColorInterpolationMethod {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    input.expect_ident_matching("in")?;

    let token = input.next()?;
    let Token::Ident(color_space_ident) = token else {
      return Err(unexpected_token!(token));
    };

    let color_space = match_ignore_ascii_case! { &color_space_ident,
      "srgb" => ColorSpaceTag::Srgb,
      "srgb-linear" => ColorSpaceTag::LinearSrgb,
      "lab" => ColorSpaceTag::Lab,
      "oklab" => ColorSpaceTag::Oklab,
      "lch" => ColorSpaceTag::Lch,
      "oklch" => ColorSpaceTag::Oklch,
      "hsl" => ColorSpaceTag::Hsl,
      "hwb" => ColorSpaceTag::Hwb,
      "display-p3" => ColorSpaceTag::DisplayP3,
      "a98-rgb" => ColorSpaceTag::A98Rgb,
      "prophoto-rgb" => ColorSpaceTag::ProphotoRgb,
      "rec2020" => ColorSpaceTag::Rec2020,
      "xyz" | "xyz-d65" => ColorSpaceTag::XyzD65,
      "xyz-d50" => ColorSpaceTag::XyzD50,
      _ => return Err(unexpected_token!(token)),
    };

    let hue_direction = input
      .try_parse(|input| -> ParseResult<'i, HueDirection> {
        let token = input.next()?;
        let Token::Ident(ident) = token else {
          return Err(unexpected_token!(token));
        };

        let direction = match_ignore_ascii_case! { &ident,
          "shorter" => HueDirection::Shorter,
          "longer" => HueDirection::Longer,
          "increasing" => HueDirection::Increasing,
          "decreasing" => HueDirection::Decreasing,
          _ => return Err(unexpected_token!(token)),
        };

        input.expect_ident_matching("hue")?;

        Ok(direction)
      })
      .ok();

    if hue_direction.is_some() && !is_cylindrical_color_space(color_space) {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self {
      color_space,
      hue_direction: hue_direction.unwrap_or(HueDirection::Shorter),
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["<in <color-space>>"];
}

impl ToCss for ColorInterpolationMethod {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let space = match self.color_space {
      ColorSpaceTag::Srgb => "srgb",
      ColorSpaceTag::LinearSrgb => "srgb-linear",
      ColorSpaceTag::Lab => "lab",
      ColorSpaceTag::Oklab => "oklab",
      ColorSpaceTag::Lch => "lch",
      ColorSpaceTag::Oklch => "oklch",
      ColorSpaceTag::Hsl => "hsl",
      ColorSpaceTag::Hwb => "hwb",
      ColorSpaceTag::DisplayP3 => "display-p3",
      ColorSpaceTag::A98Rgb => "a98-rgb",
      ColorSpaceTag::ProphotoRgb => "prophoto-rgb",
      ColorSpaceTag::Rec2020 => "rec2020",
      ColorSpaceTag::XyzD65 => "xyz-d65",
      ColorSpaceTag::XyzD50 => "xyz-d50",
      _ => "oklab",
    };
    let hue = match self.hue_direction {
      HueDirection::Shorter => "",
      HueDirection::Longer => " longer hue",
      HueDirection::Increasing => " increasing hue",
      HueDirection::Decreasing => " decreasing hue",
      _ => "",
    };
    write!(dest, "in {space}{hue}")
  }
}

/// Represents a color with 8-bit RGBA components.
#[derive(Debug, Default, Clone, PartialEq, Copy)]
pub struct Color(pub [u8; 4]);

impl From<[u8; 4]> for Color {
  fn from(value: [u8; 4]) -> Self {
    Self(value)
  }
}

impl From<[f32; 4]> for Color {
  fn from(value: [f32; 4]) -> Self {
    Self([
      value[0].clamp(0.0, 255.0).round() as u8,
      value[1].clamp(0.0, 255.0).round() as u8,
      value[2].clamp(0.0, 255.0).round() as u8,
      value[3].clamp(0.0, 255.0).round() as u8,
    ])
  }
}

/// A `<color>` value: as specified, the tree of Blink's colour `CSSValue`s, and once computed,
/// Blink's `StyleColor`.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum ColorInput {
  /// Inherit from the `color` value.
  #[default]
  CurrentColor,
  /// A color value.
  Value(Color),
  /// A system colour keyword, computing to its colour in the used color scheme.
  System(SystemColor),
  /// A `light-dark()` pair, computing to one of its colours by the used color scheme.
  LightDark(Arc<LightDarkValuePair>),
  /// A `color-mix()`, which stays unresolved once computed while an operand depends on
  /// `currentColor`.
  ColorMix(Arc<ColorMixValue>),
  /// A relative colour, which stays unresolved once computed while its origin depends on
  /// `currentColor`.
  RelativeColor(Arc<RelativeColorValue>),
}

impl MakeComputed for ColorInput {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if !matches!(self, ColorInput::CurrentColor | ColorInput::Value(_)) {
      *self = self.resolve_color_value(sizing.color_scheme);
    }
  }
}

impl Animatable for ColorInput {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: CurrentColor,
  ) {
    let from = from.resolve_color_value(sizing.color_scheme);
    let to = to.resolve_color_value(sizing.color_scheme);

    *self = match (&from, &to) {
      (ColorInput::CurrentColor, ColorInput::CurrentColor) => ColorInput::CurrentColor,
      // Animating a color is not `color-mix()`: every colour this engine holds is
      // a legacy sRGB one, and those interpolate in sRGB with premultiplied alpha.
      // Oklab is for colours a legacy space cannot express.
      // https://www.w3.org/TR/css-color-4/#interpolation-space
      _ => ColorInput::Value(from.resolve(current_color).interpolate(
        to.resolve(current_color),
        progress,
        ColorSpaceTag::Srgb,
        HueDirection::Shorter,
      )),
    };
  }
}

impl ColorInput {
  /// A fully transparent color, the initial value for `background-color`.
  pub const fn transparent() -> Self {
    ColorInput::Value(Color::transparent())
  }

  /// Blink's `StyleColor::Resolve`: the used colour, with `currentColor` as `current_color`.
  pub fn resolve(&self, current_color: Color) -> Color {
    match self {
      ColorInput::CurrentColor => current_color,
      ColorInput::Value(color) => *color,
      ColorInput::ColorMix(color_mix) => color_mix.resolve(current_color),
      ColorInput::RelativeColor(relative_color) => relative_color.resolve(current_color),
      // Unlike Blink, which resolves only computed values, a value never computed resolves in the
      // light color scheme.
      ColorInput::System(_) | ColorInput::LightDark(_) => self
        .resolve_color_value(ColorScheme::Light)
        .resolve(current_color),
    }
  }

  /// Blink's `ResolveColorValue`: the computed value in `color_scheme`, a colour unless it depends
  /// on `currentColor`.
  pub(crate) fn resolve_color_value(&self, color_scheme: ColorScheme) -> Self {
    match self {
      ColorInput::CurrentColor | ColorInput::Value(_) => self.clone(),
      ColorInput::System(system_color) => ColorInput::Value(system_color.color(color_scheme)),
      ColorInput::LightDark(pair) => pair.resolve(color_scheme).resolve_color_value(color_scheme),
      ColorInput::ColorMix(color_mix) => color_mix.resolve_color_value(color_scheme),
      ColorInput::RelativeColor(relative_color) => relative_color.resolve_color_value(color_scheme),
    }
  }

  /// Blink's `StyleColor::IsAbsoluteColor`, for a computed value.
  fn is_absolute_color(&self) -> bool {
    matches!(self, ColorInput::Value(_))
  }

  /// Whether resolving this computed value reads the current colour.
  #[cfg(feature = "svg")]
  pub(crate) fn reads_current_color(&self) -> bool {
    *self == ColorInput::CurrentColor || self.is_unresolved_color_function()
  }

  /// Blink's `StyleColor::IsUnresolvedColorFunction`, for a computed value.
  pub(crate) fn is_unresolved_color_function(&self) -> bool {
    matches!(self, ColorInput::ColorMix(_) | ColorInput::RelativeColor(_))
  }
}

impl TailwindPropertyParser for ColorInput {
  const NAMESPACES: &'static [Namespace] = &[Namespace::Color];

  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("current") {
      return Some(ColorInput::CurrentColor);
    }

    Color::parse_tw(token).map(ColorInput::Value)
  }

  /// `Color::parse_tw` already reads the `/50` modifier, so this only carries it across the
  /// `current` keyword that `Color` has no representation for.
  fn parse_tw_with_arbitrary(token: &str) -> Option<Self> {
    if let Some(value) = extract_arbitrary_value(token) {
      return Self::from_css_str(&value).ok();
    }

    let base = token.split_once('/').map_or(token, |(color, _)| color);

    if base.eq_ignore_ascii_case("current") {
      return Some(ColorInput::CurrentColor);
    }

    Color::parse_tw(token).map(ColorInput::Value)
  }
}

/// Tailwind v4 palette (OKLCH→sRGB via CSS Color 4 with gamut mapping).
const SLATE: [u32; 11] = [
  0xf8fafc, 0xf1f5f9, 0xe2e8f0, 0xcad5e2, 0x90a1b9, 0x62748e, 0x45556c, 0x314158, 0x1d293d,
  0x0f172b, 0x020618,
];

const GRAY: [u32; 11] = [
  0xf9fafb, 0xf3f4f6, 0xe5e7eb, 0xd1d5dc, 0x99a1af, 0x6a7282, 0x4a5565, 0x364153, 0x1e2939,
  0x101828, 0x030712,
];

const ZINC: [u32; 11] = [
  0xfafafa, 0xf4f4f5, 0xe4e4e7, 0xd4d4d8, 0x9f9fa9, 0x71717b, 0x52525c, 0x3f3f46, 0x27272a,
  0x18181b, 0x09090b,
];

const NEUTRAL: [u32; 11] = [
  0xfafafa, 0xf5f5f5, 0xe5e5e5, 0xd4d4d4, 0xa1a1a1, 0x737373, 0x525252, 0x404040, 0x262626,
  0x171717, 0x0a0a0a,
];

const STONE: [u32; 11] = [
  0xfafaf9, 0xf5f5f4, 0xe7e5e4, 0xd6d3d1, 0xa6a09b, 0x79716b, 0x57534d, 0x44403b, 0x292524,
  0x1c1917, 0x0c0a09,
];

const TAUPE: [u32; 11] = [
  0xfbfaf9, 0xf3f1f1, 0xe8e4e3, 0xd8d2d0, 0xaba09c, 0x7c6d67, 0x5b4f4b, 0x473c39, 0x2b2422,
  0x1d1816, 0x0c0a09,
];

const MAUVE: [u32; 11] = [
  0xfafafa, 0xf3f1f3, 0xe7e4e7, 0xd7d0d7, 0xa89ea9, 0x79697b, 0x594c5b, 0x463947, 0x2a212c,
  0x1d161e, 0x0c090c,
];

const MIST: [u32; 11] = [
  0xf9fbfb, 0xf1f3f3, 0xe3e7e8, 0xd0d6d8, 0x9ca8ab, 0x67787c, 0x4b585b, 0x394447, 0x22292b,
  0x161b1d, 0x090b0c,
];

const OLIVE: [u32; 11] = [
  0xfbfbf9, 0xf4f4f0, 0xe8e8e3, 0xd8d8d0, 0xabab9c, 0x7c7c67, 0x5b5b4b, 0x474739, 0x2b2b22,
  0x1d1d16, 0x0c0c09,
];

const RED: [u32; 11] = [
  0xfef2f2, 0xffe2e2, 0xffc9c9, 0xffa2a2, 0xff6467, 0xfb2c36, 0xe7000b, 0xc10007, 0x9f0712,
  0x82181a, 0x460809,
];

const ORANGE: [u32; 11] = [
  0xfff7ed, 0xffedd4, 0xffd6a7, 0xffb86a, 0xff8904, 0xff6900, 0xf54900, 0xca3500, 0x9f2d00,
  0x7e2a0c, 0x441306,
];

const AMBER: [u32; 11] = [
  0xfffbeb, 0xfef3c6, 0xfee685, 0xffd230, 0xfcbb00, 0xf99c00, 0xe17100, 0xbb4d00, 0x973c00,
  0x7b3306, 0x461901,
];

const YELLOW: [u32; 11] = [
  0xfefce8, 0xfef9c2, 0xfff085, 0xffdf20, 0xfac800, 0xedb200, 0xd08700, 0xa65f00, 0x894b00,
  0x733e0a, 0x432004,
];

const LIME: [u32; 11] = [
  0xf7fee7, 0xecfcca, 0xd8f999, 0xbbf451, 0x9ae600, 0x7ccf00, 0x5ea500, 0x497d00, 0x3c6300,
  0x35530e, 0x192e03,
];

const GREEN: [u32; 11] = [
  0xf0fdf4, 0xdcfce7, 0xb9f8cf, 0x7bf1a8, 0x05df72, 0x00c950, 0x00a63e, 0x008236, 0x016630,
  0x0d542b, 0x032e15,
];

const EMERALD: [u32; 11] = [
  0xecfdf5, 0xd0fae5, 0xa4f4cf, 0x5ee9b5, 0x00d492, 0x00bc7d, 0x009966, 0x007a55, 0x006045,
  0x004f3b, 0x002c22,
];

const TEAL: [u32; 11] = [
  0xf0fdfa, 0xcbfbf1, 0x96f7e4, 0x46ecd5, 0x00d5be, 0x00bba7, 0x009689, 0x00786f, 0x005f5a,
  0x0b4f4a, 0x022f2e,
];

const CYAN: [u32; 11] = [
  0xecfeff, 0xcefafe, 0xa2f4fd, 0x53eafd, 0x00d3f2, 0x00b8db, 0x0092b8, 0x007595, 0x005f78,
  0x104e64, 0x053345,
];

const SKY: [u32; 11] = [
  0xf0f9ff, 0xdff2fe, 0xb8e6fe, 0x74d4ff, 0x00bcff, 0x00a6f4, 0x0084d1, 0x0069a8, 0x00598a,
  0x024a70, 0x052f4a,
];

const BLUE: [u32; 11] = [
  0xeff6ff, 0xdbeafe, 0xbedbff, 0x8ec5ff, 0x51a2ff, 0x2b7fff, 0x155dfc, 0x1447e6, 0x193cb8,
  0x1c398e, 0x162456,
];

const INDIGO: [u32; 11] = [
  0xeef2ff, 0xe0e7ff, 0xc6d2ff, 0xa3b3ff, 0x7c86ff, 0x615fff, 0x4f39f6, 0x432dd7, 0x372aac,
  0x312c85, 0x1e1a4d,
];

const VIOLET: [u32; 11] = [
  0xf5f3ff, 0xede9fe, 0xddd6ff, 0xc4b4ff, 0xa684ff, 0x8e51ff, 0x7f22fe, 0x7008e7, 0x5d0ec0,
  0x4d179a, 0x2f0d68,
];

const PURPLE: [u32; 11] = [
  0xfaf5ff, 0xf3e8ff, 0xe9d4ff, 0xdab2ff, 0xc27aff, 0xad46ff, 0x9810fa, 0x8200db, 0x6e11b0,
  0x59168b, 0x3c0366,
];

const FUCHSIA: [u32; 11] = [
  0xfdf4ff, 0xfae8ff, 0xf6cfff, 0xf4a8ff, 0xed6aff, 0xe12afb, 0xc800de, 0xa800b7, 0x8a0194,
  0x721378, 0x4b004f,
];

const PINK: [u32; 11] = [
  0xfdf2f8, 0xfce7f3, 0xfccee8, 0xfda5d5, 0xfb64b6, 0xf6339a, 0xe60076, 0xc6005c, 0xa3004c,
  0x861043, 0x510424,
];

const ROSE: [u32; 11] = [
  0xfff1f2, 0xffe4e6, 0xffccd3, 0xffa1ad, 0xff637e, 0xff2056, 0xec003f, 0xc70036, 0xa50036,
  0x8b0836, 0x4d0218,
];

/// Shade values in ascending order for binary search
const SHADES: [u16; 11] = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

const PALETTE: [(&str, &[u32; 11]); 26] = [
  ("slate", &SLATE),
  ("gray", &GRAY),
  ("zinc", &ZINC),
  ("neutral", &NEUTRAL),
  ("stone", &STONE),
  ("taupe", &TAUPE),
  ("mauve", &MAUVE),
  ("mist", &MIST),
  ("olive", &OLIVE),
  ("red", &RED),
  ("orange", &ORANGE),
  ("amber", &AMBER),
  ("yellow", &YELLOW),
  ("lime", &LIME),
  ("green", &GREEN),
  ("emerald", &EMERALD),
  ("teal", &TEAL),
  ("cyan", &CYAN),
  ("sky", &SKY),
  ("blue", &BLUE),
  ("indigo", &INDIGO),
  ("violet", &VIOLET),
  ("purple", &PURPLE),
  ("fuchsia", &FUCHSIA),
  ("pink", &PINK),
  ("rose", &ROSE),
];

/// Returns the RGB value as a u32 (0xRRGGBB format)
fn lookup_tailwind_color(color_name: &str, shade: u16) -> Option<u32> {
  let index = SHADES.binary_search(&shade).ok()?;
  let colors = PALETTE
    .iter()
    .find(|(name, _)| name.eq_ignore_ascii_case(color_name))
    .map(|(_, c)| *c)?;
  colors.get(index).copied()
}

impl TailwindPropertyParser for Color {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Some((color, opacity)) = token.split_once('/') {
      let color = Color::parse_tw(color)?;
      let opacity = (opacity.parse::<f32>().ok()? * 2.55).round() as u8;

      return Some(color.with_opacity(opacity));
    }

    match_ignore_ascii_case! {token,
      "transparent" => return Some(Color::transparent()),
      "black" => return Some(Color::black()),
      "white" => return Some(Color::white()),
      _ => {}
    }

    let (color_name, shade_str) = token.rsplit_once('-')?;
    let shade: u16 = shade_str.parse().ok()?;

    lookup_tailwind_color(color_name, shade).map(Color::from_rgb)
  }
}

impl From<Color> for ColorInput {
  fn from(color: Color) -> Self {
    ColorInput::Value(color)
  }
}

impl Display for Color {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let [r, g, b, a] = self.0;

    if a == 255 {
      return write!(f, "rgb({r}, {g}, {b})");
    }

    write!(
      f,
      "rgba({r}, {g}, {b}, {alpha:.6})",
      alpha = a as f32 / 255.0
    )
  }
}

impl ToCss for Color {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{self}")
  }
}

impl ToCss for ColorInput {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::CurrentColor => dest.write_str("currentColor"),
      Self::Value(color) => color.to_css(dest),
      Self::System(system_color) => system_color.to_css(dest),
      Self::LightDark(pair) => pair.to_css(dest),
      Self::ColorMix(color_mix) => color_mix.to_css(dest),
      Self::RelativeColor(relative_color) => relative_color.to_css(dest),
    }
  }
}

impl Color {
  /// Creates a new transparent color.
  pub const fn transparent() -> Self {
    Color([0, 0, 0, 0])
  }

  /// Creates a new black color.
  pub const fn black() -> Self {
    Color([0, 0, 0, 255])
  }

  /// Creates a new white color.
  pub const fn white() -> Self {
    Color([255, 255, 255, 255])
  }

  fn to_dynamic(self) -> DynamicColor {
    DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(Rgba8::from_u8_array(self.0)))
  }

  fn from_dynamic(color: DynamicColor) -> Self {
    Color(color.to_alpha_color::<Srgb>().to_rgba8().to_u8_array())
  }

  /// Premultiplies the straight-alpha colour into a `tiny_skia`
  /// [`PremultipliedColorU8`].
  pub fn premultiplied(self) -> PremultipliedColorU8 {
    let [red, green, blue, alpha] = premultiply_pixel(self.0);

    PremultipliedColorU8::from_rgba(red, green, blue, alpha)
      .unwrap_or(PremultipliedColorU8::TRANSPARENT)
  }

  /// Interpolates toward `other` in sRGB with premultiplied alpha.
  pub(crate) fn lerp_premultiplied(self, other: Color, t: f32) -> Color {
    if t <= f32::EPSILON {
      return self;
    }

    if t >= 1.0 - f32::EPSILON {
      return other;
    }

    let demul: ColorU8 =
      interpolate_rgba_premultiplied(self.premultiplied(), other.premultiplied(), t).demultiply();

    Color([demul.red(), demul.green(), demul.blue(), demul.alpha()])
  }

  /// Interpolates toward `other` in the given color space and hue direction.
  pub(crate) fn interpolate(
    self,
    other: Color,
    t: f32,
    color_space: ColorSpaceTag,
    hue_direction: HueDirection,
  ) -> Color {
    if self == other {
      return self;
    }

    if color_space == ColorSpaceTag::Srgb && hue_direction == HueDirection::Shorter {
      return self.lerp_premultiplied(other, t);
    }

    if t <= f32::EPSILON {
      return self;
    }

    if t >= 1.0 - f32::EPSILON {
      return other;
    }

    Color::from_dynamic(
      self
        .to_dynamic()
        .interpolate(other.to_dynamic(), color_space, hue_direction)
        .eval(t),
    )
  }

  // `dark`, `light`, and `inset_outset` follow Blink, under the notices in LICENSE-CHROMIUM.

  /// The colour darkened as Blink's `Color::Dark` darkens the shadow edge of a 3D border.
  pub(crate) fn dark(self) -> Self {
    let v = self.max_channel();

    self.scaled_rgb(if v == 0.0 {
      0.0
    } else {
      ((v - 0.33) / v).max(0.0)
    })
  }

  /// The colour lightened as Blink's `Color::Light` lightens the lit edge of a 3D border.
  pub(crate) fn light(self) -> Self {
    let v = self.max_channel();

    if v == 0.0 {
      return Color([84, 84, 84, self.0[3]]);
    }

    self.scaled_rgb((v + 0.33).min(1.0) / v)
  }

  /// The colour of one edge of an `inset`, `outset`, `groove` or `ridge` border, as Blink's
  /// `CalculateInsetOutsetColor` shades it: `darken` picks the shadowed edge.
  pub(crate) fn inset_outset(self, darken: bool) -> Self {
    // The relative luminances of rgb(32, 32, 32) and rgb(235, 235, 235).
    const DARK_LUMINANCE: f32 = 0.014_443_844;
    const LIGHT_LUMINANCE: f32 = 0.830_77;

    let luminance = self.relative_luminance();

    if luminance <= DARK_LUMINANCE {
      return if darken {
        self.light()
      } else {
        self.light().light()
      };
    }
    if darken {
      return self.dark();
    }
    if luminance > LIGHT_LUMINANCE {
      self
    } else {
      self.light()
    }
  }

  /// The [WCAG relative luminance](https://www.w3.org/TR/WCAG21/#dfn-relative-luminance) of the
  /// colour, ignoring alpha.
  fn relative_luminance(self) -> f32 {
    let linear = |value: u8| {
      let value = f32::from(value) / 255.0;

      if value <= 0.040_45 {
        value / 12.92
      } else {
        ((value + 0.055) / 1.055).powf(2.4)
      }
    };

    0.2126 * linear(self.0[0]) + 0.7152 * linear(self.0[1]) + 0.0722 * linear(self.0[2])
  }

  /// The largest of the red, green and blue channels, from 0 to 1.
  fn max_channel(self) -> f32 {
    f32::from(self.0[0].max(self.0[1]).max(self.0[2])) / 255.0
  }

  /// Every colour channel multiplied by `factor` and truncated to 8 bits as Blink quantizes them,
  /// keeping this alpha.
  fn scaled_rgb(self, factor: f32) -> Self {
    // Blink's `QuantizeTo8Bit` scales by the float just below 256.
    const SCALE: f32 = 255.999_98;

    let channel = |value: u8| (f32::from(value) / 255.0 * factor * SCALE) as u8;

    Color([
      channel(self.0[0]),
      channel(self.0[1]),
      channel(self.0[2]),
      self.0[3],
    ])
  }

  /// Apply opacity to alpha channel
  pub(crate) fn with_opacity(mut self, opacity: u8) -> Self {
    self.0[3] = mul_div_255(self.0[3], opacity);

    self
  }

  /// Creates a new color from a 32-bit integer containing RGB values.
  pub const fn from_rgb(rgb: u32) -> Self {
    Color([
      ((rgb >> 16) & 0xFF) as u8,
      ((rgb >> 8) & 0xFF) as u8,
      (rgb & 0xFF) as u8,
      255,
    ])
  }

  /// Blink's `Color::FromColorMix`.
  fn from_color_mix(
    interpolation: ColorInterpolationMethod,
    color1: Color,
    color2: Color,
    percentage: f32,
    alpha_multiplier: f32,
  ) -> Self {
    Color::from_dynamic(
      color1
        .to_dynamic()
        .interpolate(
          color2.to_dynamic(),
          interpolation.color_space,
          interpolation.hue_direction,
        )
        .eval(percentage)
        .multiply_alpha(alpha_multiplier),
    )
  }
}

/// A system colour keyword, one Blink's `StyleColor::IsSystemColorIncludingDeprecated` accepts.
// Blink's `Highlight`, `HighlightText`, `SelectedItem` and `SelectedItemText` follow the platform's
// selection colours, which this engine has none of, so they are not parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SystemColor {
  /// `ActiveBorder`, deprecated for `ButtonBorder`.
  ActiveBorder,
  /// `ActiveCaption`, deprecated for `Canvas`.
  ActiveCaption,
  /// `ActiveText`.
  ActiveText,
  /// `AppWorkspace`, deprecated for `Canvas`.
  AppWorkspace,
  /// `Background`, deprecated for `Canvas`.
  Background,
  /// `ButtonBorder`.
  ButtonBorder,
  /// `ButtonFace`.
  ButtonFace,
  /// `ButtonHighlight`, deprecated for `ButtonFace`.
  ButtonHighlight,
  /// `ButtonShadow`, deprecated for `ButtonFace`.
  ButtonShadow,
  /// `ButtonText`.
  ButtonText,
  /// `Canvas`.
  Canvas,
  /// `CanvasText`.
  CanvasText,
  /// `CaptionText`, deprecated for `CanvasText`.
  CaptionText,
  /// `Field`.
  Field,
  /// `FieldText`.
  FieldText,
  /// `GrayText`.
  GrayText,
  /// `InactiveBorder`, deprecated for `ButtonBorder`.
  InactiveBorder,
  /// `InactiveCaption`, deprecated for `Canvas`.
  InactiveCaption,
  /// `InactiveCaptionText`, deprecated for `GrayText`.
  InactiveCaptionText,
  /// `InfoBackground`, deprecated for `Canvas`.
  InfoBackground,
  /// `InfoText`, deprecated for `CanvasText`.
  InfoText,
  /// `LinkText`.
  LinkText,
  /// `Mark`.
  Mark,
  /// `MarkText`.
  MarkText,
  /// `Menu`, deprecated for `Canvas`.
  Menu,
  /// `MenuText`, deprecated for `CanvasText`.
  MenuText,
  /// `Scrollbar`, deprecated for `Canvas`.
  Scrollbar,
  /// `ThreeDDarkShadow`, deprecated for `ButtonBorder`.
  ThreeDDarkShadow,
  /// `ThreeDFace`, deprecated for `ButtonFace`.
  ThreeDFace,
  /// `ThreeDHighlight`, deprecated for `ButtonBorder`.
  ThreeDHighlight,
  /// `ThreeDLightShadow`, deprecated for `ButtonBorder`.
  ThreeDLightShadow,
  /// `ThreeDShadow`, deprecated for `ButtonBorder`.
  ThreeDShadow,
  /// `VisitedText`.
  VisitedText,
  /// `Window`, deprecated for `Canvas`.
  Window,
  /// `WindowFrame`, deprecated for `ButtonBorder`.
  WindowFrame,
  /// `WindowText`, deprecated for `CanvasText`.
  WindowText,
}

impl_css_enum!(
  ident keyword SystemColor,
  "activeborder" => SystemColor::ActiveBorder,
  "activecaption" => SystemColor::ActiveCaption,
  "activetext" => SystemColor::ActiveText,
  "appworkspace" => SystemColor::AppWorkspace,
  "background" => SystemColor::Background,
  "buttonborder" => SystemColor::ButtonBorder,
  "buttonface" => SystemColor::ButtonFace,
  "buttonhighlight" => SystemColor::ButtonHighlight,
  "buttonshadow" => SystemColor::ButtonShadow,
  "buttontext" => SystemColor::ButtonText,
  "canvas" => SystemColor::Canvas,
  "canvastext" => SystemColor::CanvasText,
  "captiontext" => SystemColor::CaptionText,
  "field" => SystemColor::Field,
  "fieldtext" => SystemColor::FieldText,
  "graytext" => SystemColor::GrayText,
  "inactiveborder" => SystemColor::InactiveBorder,
  "inactivecaption" => SystemColor::InactiveCaption,
  "inactivecaptiontext" => SystemColor::InactiveCaptionText,
  "infobackground" => SystemColor::InfoBackground,
  "infotext" => SystemColor::InfoText,
  "linktext" => SystemColor::LinkText,
  "mark" => SystemColor::Mark,
  "marktext" => SystemColor::MarkText,
  "menu" => SystemColor::Menu,
  "menutext" => SystemColor::MenuText,
  "scrollbar" => SystemColor::Scrollbar,
  "threeddarkshadow" => SystemColor::ThreeDDarkShadow,
  "threedface" => SystemColor::ThreeDFace,
  "threedhighlight" => SystemColor::ThreeDHighlight,
  "threedlightshadow" => SystemColor::ThreeDLightShadow,
  "threedshadow" => SystemColor::ThreeDShadow,
  "visitedtext" => SystemColor::VisitedText,
  "window" => SystemColor::Window,
  "windowframe" => SystemColor::WindowFrame,
  "windowtext" => SystemColor::WindowText,
);

impl SystemColor {
  /// Blink's `LayoutTheme::DefaultSystemColor`.
  fn color(self, color_scheme: ColorScheme) -> Color {
    let by_scheme = |light: u32, dark: u32| {
      Color::from_rgb(match color_scheme {
        ColorScheme::Light => light,
        ColorScheme::Dark => dark,
      })
    };

    match self {
      SystemColor::ActiveText => Color::from_rgb(0xff0000),
      SystemColor::ButtonBorder
      | SystemColor::ActiveBorder
      | SystemColor::InactiveBorder
      | SystemColor::ThreeDDarkShadow
      | SystemColor::ThreeDHighlight
      | SystemColor::ThreeDLightShadow
      | SystemColor::ThreeDShadow
      | SystemColor::WindowFrame => by_scheme(0x767676, 0x6b6b6b),
      SystemColor::ButtonFace
      | SystemColor::ButtonHighlight
      | SystemColor::ButtonShadow
      | SystemColor::ThreeDFace => by_scheme(0xefefef, 0x6b6b6b),
      SystemColor::Canvas
      | SystemColor::ActiveCaption
      | SystemColor::AppWorkspace
      | SystemColor::Background
      | SystemColor::InactiveCaption
      | SystemColor::InfoBackground
      | SystemColor::Menu
      | SystemColor::Scrollbar
      | SystemColor::Window => by_scheme(0xffffff, 0x121212),
      SystemColor::ButtonText
      | SystemColor::CanvasText
      | SystemColor::CaptionText
      | SystemColor::InfoText
      | SystemColor::MenuText
      | SystemColor::WindowText
      | SystemColor::FieldText => by_scheme(0x000000, 0xffffff),
      SystemColor::Field => by_scheme(0xffffff, 0x3b3b3b),
      SystemColor::GrayText | SystemColor::InactiveCaptionText => Color::from_rgb(0x808080),
      SystemColor::LinkText => by_scheme(0x0000ee, 0x9e9eff),
      SystemColor::Mark => Color::from_rgb(0xffff00),
      SystemColor::MarkText => Color::from_rgb(0x000000),
      SystemColor::VisitedText => by_scheme(0x551a8b, 0xd0adf0),
    }
  }
}

/// Blink's `CSSLightDarkValuePair`, of colours.
#[derive(Debug, Clone, PartialEq)]
pub struct LightDarkValuePair {
  first: ColorInput,
  second: ColorInput,
}

impl LightDarkValuePair {
  /// Blink's `ResolveLightDarkPair`: the colour of `color_scheme`.
  fn resolve(&self, color_scheme: ColorScheme) -> &ColorInput {
    match color_scheme {
      ColorScheme::Light => &self.first,
      ColorScheme::Dark => &self.second,
    }
  }
}

impl<'i> FromCss<'i> for LightDarkValuePair {
  /// Blink's `ConsumeLightDark`, inside the function's block.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let first = ColorInput::from_css(input)?;

    input.expect_comma()?;

    Ok(Self {
      first,
      second: ColorInput::from_css(input)?,
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["<light-dark()>"];
}

impl ToCss for LightDarkValuePair {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str("light-dark(")?;
    self.first.to_css(dest)?;
    dest.write_str(", ")?;
    self.second.to_css(dest)?;
    dest.write_str(")")
  }
}

/// Blink's `CSSColorMixValue`, and once computed, its `StyleColor::UnresolvedColorMix`.
// Unlike Blink's `UnresolvedColorMix`, an unresolved mix keeps the percentages as written.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorMixValue {
  color1: ColorInput,
  color2: ColorInput,
  percentage1: Option<f32>,
  percentage2: Option<f32>,
  interpolation: ColorInterpolationMethod,
}

impl ColorMixValue {
  /// Blink's `CSSColorMixValue::NormalizePercentages`: the amount of the second colour and the
  /// alpha multiplier.
  fn normalize_percentages(&self) -> (f32, f32) {
    let p1 = self.percentage1.map(|p1| p1.clamp(0.0, 1.0));
    let p2 = self.percentage2.map(|p2| p2.clamp(0.0, 1.0));
    let (p1, p2) = match (p1, p2) {
      (None, None) => (0.5, 0.5),
      (Some(p1), None) => (p1, 1.0 - p1),
      (None, Some(p2)) => (1.0 - p2, p2),
      (Some(p1), Some(p2)) => (p1, p2),
    };
    let percentage_sum = p1 + p2;

    // https://drafts.csswg.org/css-color-5/#color-mix-result
    if percentage_sum == 0.0 {
      return (0.5, 0.0);
    }

    (p2 / percentage_sum, percentage_sum.min(1.0))
  }

  /// Blink's `ResolveColorValueImpl` for a `CSSColorMixValue`.
  fn resolve_color_value(&self, color_scheme: ColorScheme) -> ColorInput {
    let color_mix = Self {
      color1: self.color1.resolve_color_value(color_scheme),
      color2: self.color2.resolve_color_value(color_scheme),
      percentage1: self.percentage1,
      percentage2: self.percentage2,
      interpolation: self.interpolation,
    };

    // https://drafts.csswg.org/css-color-5/#resolving-mix
    if color_mix.color1.is_absolute_color() && color_mix.color2.is_absolute_color() {
      ColorInput::Value(color_mix.resolve(Color::transparent()))
    } else {
      ColorInput::ColorMix(Arc::new(color_mix))
    }
  }

  /// Blink's `StyleColor::UnresolvedColorMix::Resolve`.
  fn resolve(&self, current_color: Color) -> Color {
    let (mix_amount, alpha_multiplier) = self.normalize_percentages();

    Color::from_color_mix(
      self.interpolation,
      self.color1.resolve(current_color),
      self.color2.resolve(current_color),
      mix_amount,
      alpha_multiplier,
    )
  }
}

/// A `color-mix()` colour and its percentage, which can come before it, as Blink's
/// `ConsumeColorMixFunction` reads them.
fn parse_color_mix_operand<'i>(
  input: &mut Parser<'i>,
) -> ParseResult<'i, (ColorInput, Option<f32>)> {
  if let Ok(color) = input.try_parse(ColorInput::from_css) {
    return Ok((color, input.try_parse(parse_color_mix_percentage).ok()));
  }

  let percentage = parse_color_mix_percentage(input)?;

  Ok((ColorInput::from_css(input)?, Some(percentage)))
}

/// A `color-mix()` percentage as a fraction, as Blink's `ConsumeColorMixFunction` reads it: a
/// literal outside 0% to 100% is invalid, and a math function clamps when the mix resolves.
fn parse_color_mix_percentage<'i>(input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  let start = input.state();

  if let Ok(&Token::Percentage { unit_value, .. }) = input.next() {
    if (0.0..=1.0).contains(&unit_value) {
      return Ok(unit_value);
    }

    return Err(input.new_error_for_next_token());
  }

  input.reset(&start);
  parse_percentage_value(input)
}

impl<'i> FromCss<'i> for ColorMixValue {
  /// Blink's `ConsumeColorMixFunction`, inside the function's block.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    // Only an explicit `in <space>` is followed by a comma.
    let interpolation = match input.try_parse(ColorInterpolationMethod::from_css) {
      Ok(interpolation) => {
        input.expect_comma()?;
        interpolation
      }
      Err(_) => ColorInterpolationMethod::default(),
    };
    let (color1, percentage1) = parse_color_mix_operand(input)?;
    input.expect_comma()?;
    let (color2, percentage2) = parse_color_mix_operand(input)?;
    Ok(Self {
      color1,
      color2,
      percentage1,
      percentage2,
      interpolation,
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["<color-mix()>"];
}

impl ToCss for ColorMixValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let write_operand = |dest: &mut W, color: &ColorInput, percentage: Option<f32>| {
      color.to_css(dest)?;

      match percentage {
        // Only a math function holds a percentage outside 0% to 100%, and it reparses as one.
        Some(percentage) if !(0.0..=1.0).contains(&percentage) => {
          write!(dest, " calc({}%)", significant(percentage * 100.0))
        }
        Some(percentage) => write!(dest, " {}%", significant(percentage * 100.0)),
        None => Ok(()),
      }
    };

    dest.write_str("color-mix(")?;

    if self.interpolation != ColorInterpolationMethod::default() {
      self.interpolation.to_css(dest)?;
      dest.write_str(", ")?;
    }

    write_operand(dest, &self.color1, self.percentage1)?;
    dest.write_str(", ")?;
    write_operand(dest, &self.color2, self.percentage2)?;
    dest.write_str(")")
  }
}

/// Reference: https://www.w3.org/TR/css-color-5/#relative-colors
fn relative_target_cs(name: &str) -> Option<ColorSpaceTag> {
  Some(match_ignore_ascii_case! { name,
    "rgb" | "rgba" => ColorSpaceTag::Srgb,
    "hsl" | "hsla" => ColorSpaceTag::Hsl,
    "hwb" => ColorSpaceTag::Hwb,
    "lab" => ColorSpaceTag::Lab,
    "lch" => ColorSpaceTag::Lch,
    "oklab" => ColorSpaceTag::Oklab,
    "oklch" => ColorSpaceTag::Oklch,
    _ => return None,
  })
}

/// The function a relative colour in `cs` serializes as, the inverse of [`relative_target_cs`].
fn relative_function_name(cs: ColorSpaceTag) -> &'static str {
  match cs {
    ColorSpaceTag::Hsl => "hsl",
    ColorSpaceTag::Hwb => "hwb",
    ColorSpaceTag::Lab => "lab",
    ColorSpaceTag::Lch => "lch",
    ColorSpaceTag::Oklab => "oklab",
    ColorSpaceTag::Oklch => "oklch",
    _ => "rgb",
  }
}

fn channel_names(cs: ColorSpaceTag) -> [&'static str; 3] {
  match cs {
    ColorSpaceTag::Srgb => ["r", "g", "b"],
    ColorSpaceTag::Hsl => ["h", "s", "l"],
    ColorSpaceTag::Hwb => ["h", "w", "b"],
    ColorSpaceTag::Lab | ColorSpaceTag::Oklab => ["l", "a", "b"],
    ColorSpaceTag::Lch | ColorSpaceTag::Oklch => ["l", "c", "h"],
    _ => ["", "", ""],
  }
}

/// sRGB stores 0..1 but `r`/`g`/`b` keywords resolve to 0..255 per spec.
fn channel_keyword_scale(cs: ColorSpaceTag) -> f32 {
  if cs == ColorSpaceTag::Srgb {
    255.0
  } else {
    1.0
  }
}

fn channel_percentage_scale(cs: ColorSpaceTag, index: usize) -> f32 {
  match (cs, index) {
    (_, ALPHA_CHANNEL) => 1.0,
    (ColorSpaceTag::Srgb, _) => 255.0,
    (ColorSpaceTag::Hsl, _) | (ColorSpaceTag::Hwb, _) => 100.0,
    (ColorSpaceTag::Lab, 0) | (ColorSpaceTag::Lch, 0) => 100.0,
    (ColorSpaceTag::Lab, _) => 125.0,
    (ColorSpaceTag::Lch, 1) => 150.0,
    (ColorSpaceTag::Oklab, 0) | (ColorSpaceTag::Oklch, 0) => 1.0,
    (ColorSpaceTag::Oklab, _) | (ColorSpaceTag::Oklch, 1) => 0.4,
    _ => 1.0,
  }
}

fn is_hue_slot(cs: ColorSpaceTag, index: usize) -> bool {
  matches!(
    (cs, index),
    (ColorSpaceTag::Hsl | ColorSpaceTag::Hwb, 0) | (ColorSpaceTag::Lch | ColorSpaceTag::Oklch, 2)
  )
}

/// The index of the alpha channel among a relative colour's channels.
const ALPHA_CHANNEL: usize = 3;

/// A channel of a relative colour as written, one of the channel `CSSValue`s of Blink's
/// `CSSRelativeColorValue`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RelativeColorChannel {
  /// `none`, which resolves to zero. Unlike Blink, whose colours keep a missing channel for an
  /// enclosing `color-mix()` to take from the other colour, the resolved colour has no such channel.
  None,
  /// The origin's channel at this index, [`ALPHA_CHANNEL`] for `alpha`.
  Keyword(usize),
  /// A number.
  Number(f32),
  /// A percentage, from 0 to 1.
  Percentage(f32),
  /// An angle, in degrees.
  Angle(f32),
}

impl RelativeColorChannel {
  /// The channel at `index` of a relative colour in `cs`.
  fn parse<'i>(input: &mut Parser<'i>, cs: ColorSpaceTag, index: usize) -> ParseResult<'i, Self> {
    let token = input.next()?;
    let is_hue = is_hue_slot(cs, index);

    match token {
      Token::Ident(ident) => {
        if ident.eq_ignore_ascii_case("none") {
          return Ok(Self::None);
        }

        if ident.eq_ignore_ascii_case("alpha") {
          return Ok(Self::Keyword(ALPHA_CHANNEL));
        }

        channel_names(cs)
          .iter()
          .position(|name| !name.is_empty() && ident.eq_ignore_ascii_case(name))
          .map(Self::Keyword)
          .ok_or_else(|| unexpected_token!(ColorInput, token))
      }
      Token::Number { value, .. } => Ok(Self::Number(*value)),
      Token::Percentage { unit_value, .. } if !is_hue => Ok(Self::Percentage(*unit_value)),
      Token::Dimension { value, unit, .. } if is_hue => {
        Ok(Self::Angle(match_ignore_ascii_case! { unit.as_ref(),
          "deg" => *value,
          "grad" => *value / 400.0 * 360.0,
          "turn" => *value * 360.0,
          "rad" => value.to_degrees(),
          _ => return Err(unexpected_token!(ColorInput, token)),
        }))
      }
      _ => Err(unexpected_token!(ColorInput, token)),
    }
  }

  /// The channel's value at `index` of a colour in `cs`, with the origin's channels as
  /// `keyword_values`.
  fn value(self, keyword_values: [f32; 4], cs: ColorSpaceTag, index: usize) -> f32 {
    match self {
      Self::None => 0.0,
      Self::Keyword(keyword) => keyword_values[keyword],
      Self::Number(value) | Self::Angle(value) => value,
      Self::Percentage(unit_value) => unit_value * channel_percentage_scale(cs, index),
    }
  }

  fn to_css<W: fmt::Write>(self, dest: &mut W, cs: ColorSpaceTag) -> fmt::Result {
    match self {
      Self::None => dest.write_str("none"),
      Self::Keyword(ALPHA_CHANNEL) => dest.write_str("alpha"),
      Self::Keyword(keyword) => dest.write_str(channel_names(cs)[keyword]),
      Self::Number(value) => value.to_css(dest),
      Self::Percentage(unit_value) => write!(dest, "{}%", significant(unit_value * 100.0)),
      Self::Angle(degrees) => write!(dest, "{degrees}deg"),
    }
  }
}

/// Blink's `CSSRelativeColorValue`, and once computed, its `StyleColor::UnresolvedRelativeColor`.
// `calc()` expressions in channels are not yet supported.
#[derive(Debug, Clone, PartialEq)]
pub struct RelativeColorValue {
  origin_color: ColorInput,
  color_interpolation_space: ColorSpaceTag,
  channels: [RelativeColorChannel; 3],
  alpha: Option<RelativeColorChannel>,
}

impl RelativeColorValue {
  /// The relative colour in `cs` after the `from` keyword of its function's block.
  fn parse<'i>(input: &mut Parser<'i>, cs: ColorSpaceTag) -> ParseResult<'i, Self> {
    let origin_color = ColorInput::from_css(input)?;
    let channels = [
      RelativeColorChannel::parse(input, cs, 0)?,
      RelativeColorChannel::parse(input, cs, 1)?,
      RelativeColorChannel::parse(input, cs, 2)?,
    ];
    let alpha = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
      Some(RelativeColorChannel::parse(input, cs, ALPHA_CHANNEL)?)
    } else {
      None
    };

    Ok(Self {
      origin_color,
      color_interpolation_space: cs,
      channels,
      alpha,
    })
  }

  /// Blink's `ResolveColorValueImpl` for a `CSSRelativeColorValue`.
  fn resolve_color_value(&self, color_scheme: ColorScheme) -> ColorInput {
    let relative_color = Self {
      origin_color: self.origin_color.resolve_color_value(color_scheme),
      color_interpolation_space: self.color_interpolation_space,
      channels: self.channels,
      alpha: self.alpha,
    };

    // https://drafts.csswg.org/css-color-5/#resolving-rcs
    if relative_color.origin_color.is_absolute_color() {
      ColorInput::Value(relative_color.resolve(Color::transparent()))
    } else {
      ColorInput::RelativeColor(Arc::new(relative_color))
    }
  }

  /// Blink's `StyleColor::UnresolvedRelativeColor::Resolve`.
  fn resolve(&self, current_color: Color) -> Color {
    let cs = self.color_interpolation_space;
    let scale = channel_keyword_scale(cs);
    let origin = self
      .origin_color
      .resolve(current_color)
      .to_dynamic()
      .convert(cs);
    let [k0, k1, k2, k_alpha] = origin.components;
    let keyword_values = [k0 * scale, k1 * scale, k2 * scale, k_alpha];
    let [c0, c1, c2] = [0, 1, 2].map(|index| self.channels[index].value(keyword_values, cs, index));
    let alpha = self.alpha.map_or(k_alpha, |alpha| {
      alpha.value(keyword_values, cs, ALPHA_CHANNEL)
    });
    let components = [c0 / scale, c1 / scale, c2 / scale, alpha.clamp(0.0, 1.0)];

    Color::from_dynamic(origin.map(|_, _, _, _| components))
  }
}

impl ToCss for RelativeColorValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let cs = self.color_interpolation_space;

    write!(dest, "{}(from ", relative_function_name(cs))?;
    self.origin_color.to_css(dest)?;

    for channel in self.channels {
      dest.write_char(' ')?;
      channel.to_css(dest, cs)?;
    }

    if let Some(alpha) = self.alpha {
      dest.write_str(" / ")?;
      alpha.to_css(dest, cs)?;
    }

    dest.write_char(')')
  }
}

impl<'i> FromCss<'i> for ColorInput {
  /// Blink's `ConsumeColor`.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let position = input.position();
    let token = input.next()?;

    match *token {
      Token::Ident(ref ident) => {
        if ident.eq_ignore_ascii_case("currentcolor") {
          return Ok(ColorInput::CurrentColor);
        }

        if ident.eq_ignore_ascii_case("transparent") {
          return Ok(ColorInput::transparent());
        }

        if let Ok((r, g, b)) = parse_named_color(ident) {
          return Ok(ColorInput::Value(Color([r, g, b, 255])));
        }

        SystemColor::from_keyword(ident)
          .map(ColorInput::System)
          .ok_or_else(|| unexpected_token!(token))
      }
      Token::Hash(ref value) | Token::IDHash(ref value) => parse_hash_color(value.as_bytes())
        .map(|(r, g, b, a)| ColorInput::Value(Color([r, g, b, (a * 255.0) as u8])))
        .map_err(|_| unexpected_token!(token)),
      Token::Function(ref name) => {
        let name = name.clone();

        match_ignore_ascii_case! { &*name,
          "light-dark" => input
            .parse_nested_block(LightDarkValuePair::from_css)
            .map(|pair| ColorInput::LightDark(Arc::new(pair))),
          "color-mix" => input
            .parse_nested_block(ColorMixValue::from_css)
            .map(|color_mix| ColorInput::ColorMix(Arc::new(color_mix))),
          _ => input.parse_nested_block(|input| {
            if let Some(cs) = relative_target_cs(&name)
              && input.try_parse(|input| input.expect_ident_matching("from")).is_ok()
            {
              return RelativeColorValue::parse(input, cs)
                .map(|relative_color| ColorInput::RelativeColor(Arc::new(relative_color)));
            }

            while input.next().is_ok() {}

            let mut function = input.slice_from(position).to_string();

            function.push(')');

            parse_color(&function)
              .map(|color| ColorInput::Value(Color::from_dynamic(color)))
              .map_err(|_| unexpected_token!(&Token::Function(name.clone())))
          }),
        }
      }
      _ => Err(unexpected_token!(token)),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["currentColor", "<color>"];
}

impl<'i> FromCss<'i> for Color {
  /// A `<color>` that resolves without a current colour, in the light color scheme.
  // Unlike Blink's `ConsumeAbsoluteColor`, which rejects them, `light-dark()` and system colours
  // take their light colour.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let start = input.state();

    match ColorInput::from_css(input)?.resolve_color_value(ColorScheme::Light) {
      ColorInput::Value(color) => Ok(color),
      _ => {
        input.reset(&start);

        Err(unexpected_token!(input.next()?))
      }
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["<color>"];
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::{style::sizing::SizingContext, viewport::Viewport};

  fn sizing() -> SizingContext {
    SizingContext::builder()
      .viewport(Viewport::new((200, 100)))
      .font_size(16.0)
      .line_height(0.0)
      .ch(16.0 / 2.0)
      .ex(16.0 / 2.0)
      .build()
  }

  const RED: Color = Color([255, 0, 0, 255]);
  const BLUE: Color = Color([0, 0, 255, 255]);

  fn in_scheme(color_scheme: ColorScheme) -> SizingContext {
    sizing().with_color_scheme(color_scheme)
  }

  /// `css` parsed and computed in the light color scheme.
  fn computed(css: &str) -> Option<ColorInput> {
    ColorInput::from_css_str(css)
      .ok()
      .map(|color| color.resolve_color_value(ColorScheme::Light))
  }

  /// `css` parsed and computed in `color_scheme`, then resolved against `current_color`.
  fn used(css: &str, color_scheme: ColorScheme, current_color: Color) -> Option<Color> {
    ColorInput::from_css_str(css).ok().map(|mut color| {
      color.make_computed(&in_scheme(color_scheme));
      color.resolve(current_color)
    })
  }

  #[test]
  fn light_dark_keeps_both_colours_until_computed() {
    assert_eq!(Color::from_css_str("light-dark(red, blue)"), Ok(RED));
    assert!(matches!(
      ColorInput::from_css_str("light-dark(#ff0000, rgb(0 0 255))"),
      Ok(ColorInput::LightDark(_))
    ));
    assert!(Color::from_css_str("light-dark(red)").is_err());
    assert!(Color::from_css_str("light-dark(red, 1px)").is_err());
    assert!(ColorInput::from_css_str("light-dark(red, 1px)").is_err());
  }

  #[test]
  fn light_dark_computes_to_the_colour_of_the_used_scheme() {
    for (css, light, dark) in [
      (
        "light-dark(red, blue)",
        ColorInput::Value(RED),
        ColorInput::Value(BLUE),
      ),
      (
        "light-dark(red, currentColor)",
        ColorInput::Value(RED),
        ColorInput::CurrentColor,
      ),
      (
        "light-dark(light-dark(red, lime), light-dark(lime, blue))",
        ColorInput::Value(RED),
        ColorInput::Value(BLUE),
      ),
    ] {
      let Ok(color) = ColorInput::from_css_str(css) else {
        panic!("{css} did not parse");
      };

      assert_eq!(
        color.resolve_color_value(ColorScheme::Light),
        light,
        "{css}"
      );
      assert_eq!(color.resolve_color_value(ColorScheme::Dark), dark, "{css}");
    }
  }

  #[test]
  fn animating_light_dark_takes_the_colours_of_the_used_scheme() {
    let mut result = ColorInput::default();
    let Ok(endpoint) = ColorInput::from_css_str("light-dark(red, blue)") else {
      panic!("light-dark() did not parse");
    };

    result.interpolate(
      &endpoint,
      &endpoint,
      0.5,
      &in_scheme(ColorScheme::Dark),
      RED,
    );

    assert_eq!(result, ColorInput::Value(BLUE));
  }

  #[test]
  fn color_mix_mixes_the_colour_of_light_dark_in_the_used_scheme() {
    for (css, light, dark) in [
      (
        "color-mix(in srgb, light-dark(white, black) 25%, red)",
        "color-mix(in srgb, white 25%, red)",
        "color-mix(in srgb, black 25%, red)",
      ),
      (
        "color-mix(in oklab, color-mix(in srgb, light-dark(white, black), blue), red)",
        "color-mix(in oklab, color-mix(in srgb, white, blue), red)",
        "color-mix(in oklab, color-mix(in srgb, black, blue), red)",
      ),
    ] {
      assert_eq!(
        used(css, ColorScheme::Light, RED),
        used(light, ColorScheme::Light, RED),
        "{css}"
      );
      assert_eq!(
        used(css, ColorScheme::Dark, RED),
        used(dark, ColorScheme::Light, RED),
        "{css}"
      );
    }

    assert_eq!(
      Color::from_css_str("color-mix(in srgb, light-dark(white, black) 25%, red)").ok(),
      used(
        "color-mix(in srgb, white 25%, red)",
        ColorScheme::Light,
        RED
      )
    );
  }

  #[test]
  fn color_mix_of_current_color_resolves_at_use() {
    let css = "color-mix(in srgb, currentColor 25%, white)";
    let Some(computed) = computed(css) else {
      panic!("{css} did not parse");
    };

    assert!(computed.is_unresolved_color_function());
    assert_eq!(
      computed.resolve(RED),
      Color([255, 191, 191, 255]),
      "against red"
    );
    assert_eq!(
      computed.resolve(BLUE),
      Color([191, 191, 255, 255]),
      "against blue"
    );
    assert_eq!(
      used(
        "color-mix(in srgb, light-dark(white, currentColor), black)",
        ColorScheme::Dark,
        BLUE
      ),
      Some(Color([0, 0, 128, 255]))
    );
    assert!(Color::from_css_str(css).is_err());
  }

  #[test]
  fn relative_colour_takes_the_origin_of_the_used_scheme() {
    let css = "rgb(from light-dark(red, blue) r g b / 0.5)";

    assert_eq!(
      used(css, ColorScheme::Light, Color::black()),
      Some(Color([255, 0, 0, 128]))
    );
    assert_eq!(
      used(css, ColorScheme::Dark, Color::black()),
      Some(Color([0, 0, 255, 128]))
    );
  }

  #[test]
  fn relative_colour_of_current_color_resolves_at_use() {
    let Some(computed) = computed("rgb(from currentColor b g r)") else {
      panic!("the relative colour did not parse");
    };

    assert!(computed.is_unresolved_color_function());
    assert_eq!(computed.resolve(RED), BLUE);
    assert_eq!(computed.resolve(BLUE), RED);
  }

  #[test]
  fn system_colours_compute_by_the_used_scheme() {
    for (css, light, dark) in [
      ("Canvas", Color::white(), Color::from_rgb(0x121212)),
      ("CanvasText", Color::black(), Color::white()),
      (
        "ButtonFace",
        Color::from_rgb(0xefefef),
        Color::from_rgb(0x6b6b6b),
      ),
      (
        "threedface",
        Color::from_rgb(0xefefef),
        Color::from_rgb(0x6b6b6b),
      ),
      (
        "LinkText",
        Color::from_rgb(0x0000ee),
        Color::from_rgb(0x9e9eff),
      ),
      (
        "GrayText",
        Color::from_rgb(0x808080),
        Color::from_rgb(0x808080),
      ),
    ] {
      assert_eq!(used(css, ColorScheme::Light, RED), Some(light), "{css}");
      assert_eq!(used(css, ColorScheme::Dark, RED), Some(dark), "{css}");
    }

    assert_eq!(
      used(
        "color-mix(in srgb, Canvas, CanvasText)",
        ColorScheme::Dark,
        RED
      ),
      Some(Color([137, 137, 137, 255]))
    );
    assert_eq!(Color::from_css_str("Canvas"), Ok(Color::white()));
    assert!(ColorInput::from_css_str("Highlight").is_err());
  }

  #[test]
  fn specified_colours_serialize_as_written() {
    for css in [
      "currentColor",
      "light-dark(rgb(255, 0, 0), currentColor)",
      "color-mix(in srgb, currentColor 25%, rgb(255, 255, 255))",
      "color-mix(in lch longer hue, rgb(255, 0, 0), rgb(0, 0, 255) 40%)",
      "color-mix(light-dark(rgb(255, 255, 255), rgb(0, 0, 0)), rgb(255, 0, 0))",
      "rgb(from currentColor b g r / alpha)",
      "hsl(from light-dark(rgb(255, 0, 0), rgb(0, 0, 255)) 120deg s 50%)",
      "oklch(from rgb(255, 0, 0) l c none / 0.5)",
      "color-mix(in srgb, rgb(255, 255, 255), rgb(0, 0, 0) 30%)",
      "color-mix(in srgb, canvas, buttontext 30%)",
    ] {
      let Ok(color) = ColorInput::from_css_str(css) else {
        panic!("{css} did not parse");
      };
      let serialized = color.to_css_string();

      assert_eq!(serialized, css);
      assert_eq!(ColorInput::from_css_str(&serialized), Ok(color), "{css}");
    }
  }
  #[test]
  fn animating_a_colour_interpolates_in_srgb() {
    let mut result = ColorInput::default();

    result.interpolate(
      &ColorInput::Value(Color([0, 0, 255, 255])),
      &ColorInput::Value(Color([255, 255, 0, 255])),
      0.5,
      &sizing(),
      Color([0, 0, 0, 255]),
    );

    // sRGB, as Chromium interpolates a legacy colour.
    assert_eq!(result, ColorInput::Value(Color([128, 128, 128, 255])));
  }

  #[test]
  fn animating_a_transparent_colour_premultiplies() {
    let mut result = ColorInput::default();

    result.interpolate(
      &ColorInput::Value(Color([255, 0, 0, 0])),
      &ColorInput::Value(Color([0, 0, 255, 255])),
      0.5,
      &sizing(),
      Color([0, 0, 0, 255]),
    );

    // Premultiplied, the transparent red contributes nothing to the midpoint's
    // hue: half-opaque blue, not purple.
    assert_eq!(result, ColorInput::Value(Color([0, 0, 255, 128])));
  }

  #[test]
  fn test_color_from_f32_array_clamps_before_rounding() {
    assert_eq!(
      Color::from([-1.0, 127.5, 255.4, 999.0]),
      Color([0, 128, 255, 255])
    );
  }

  #[test]
  fn test_parse_color_opaque() {
    for (css, expected) in [
      ("#f09", Color([255, 0, 153, 255])),
      ("#ff0099", Color([255, 0, 153, 255])),
      ("transparent", Color([0, 0, 0, 0])),
      ("rgb(255, 0, 153)", Color([255, 0, 153, 255])),
      ("rgb(255 0 153)", Color([255, 0, 153, 255])),
      ("grey", Color([128, 128, 128, 255])),
      ("deepskyblue", Color([0, 191, 255, 255])),
    ] {
      assert_eq!(
        ColorInput::from_css_str(css),
        Ok(ColorInput::Value(expected)),
        "failed for {css}",
      );
    }
  }

  #[test]
  fn test_parse_color_with_alpha() {
    for (css, expected) in [
      ("rgba(255, 0, 153, 0.5)", Color([255, 0, 153, 128])),
      ("rgb(255 0 153 / 0.5)", Color([255, 0, 153, 128])),
    ] {
      assert_eq!(
        ColorInput::from_css_str(css),
        Ok(ColorInput::Value(expected)),
        "failed for {css}",
      );
    }
  }

  #[test]
  fn test_parse_color_invalid_function() {
    assert!(ColorInput::from_css_str("invalid(255, 0, 153)").is_err());
  }

  #[test]
  fn test_parse_color_mix_srgb_default_percentages() {
    assert_eq!(
      computed("color-mix(in srgb, red, blue)"),
      Some(ColorInput::Value(Color([128, 0, 128, 255])))
    );
  }

  #[test]
  fn test_parse_color_mix_defaults_to_oklab() {
    assert_eq!(
      computed("color-mix(red, blue)"),
      computed("color-mix(in oklab, red, blue)")
    );
    assert!(computed("color-mix(red, blue)").is_some());
  }

  #[test]
  fn test_parse_color_mix_equivalent_percentage_syntaxes() {
    let canonical = computed("color-mix(in srgb, red 25%, blue 75%)");

    assert_eq!(canonical, computed("color-mix(in srgb, 25% red, 75% blue)"));
    assert_eq!(canonical, computed("color-mix(in srgb, red 25%, 75% blue)"));
    assert_eq!(canonical, computed("color-mix(in srgb, red 25%, blue)"));
    assert_eq!(canonical, Some(ColorInput::Value(Color([64, 0, 191, 255]))));
  }

  #[test]
  fn test_parse_color_mix_lch_missing_percentage_equivalence() {
    let canonical = computed("color-mix(in lch, purple 50%, plum 50%)");

    assert_eq!(canonical, computed("color-mix(in lch, purple 50%, plum)"));
    assert_eq!(canonical, computed("color-mix(in lch, purple, plum 50%)"));
    assert_eq!(canonical, computed("color-mix(in lch, purple, plum)"));
    assert_eq!(canonical, computed("color-mix(in lch, plum, purple)"));
  }

  #[test]
  fn test_parse_color_mix_lch_normalizes_equal_opaque_percentages() {
    let canonical = computed("color-mix(in lch, purple 50%, plum 50%)");

    assert_eq!(
      canonical,
      computed("color-mix(in lch, purple 55%, plum 55%)")
    );
    assert_eq!(
      canonical,
      computed("color-mix(in lch, purple 70%, plum 70%)")
    );
    assert_eq!(
      canonical,
      computed("color-mix(in lch, purple 95%, plum 95%)")
    );
  }

  #[test]
  fn test_parse_color_mix_endpoint_percentages_return_endpoint_colors() {
    assert_eq!(
      computed("color-mix(in srgb, red 100%, blue 0%)"),
      computed("red")
    );
    assert_eq!(
      computed("color-mix(in srgb, red 0%, blue 100%)"),
      computed("blue")
    );
  }

  #[test]
  fn test_parse_color_mix_alpha_multiplier_under_100_percent() {
    assert_eq!(
      computed("color-mix(in srgb, red 30%, blue 30%)"),
      Some(ColorInput::Value(Color([128, 0, 128, 153])))
    );
  }

  #[test]
  fn test_parse_color_mix_hue_directions_change_result() {
    assert_eq!(
      computed("color-mix(in hsl shorter hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",),
      Some(ColorInput::Value(Color([191, 86, 64, 255])))
    );
    assert_eq!(
      computed("color-mix(in hsl decreasing hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",),
      Some(ColorInput::Value(Color([191, 86, 64, 255])))
    );
    assert_eq!(
      computed("color-mix(in hsl longer hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",),
      Some(ColorInput::Value(Color([64, 169, 191, 255])))
    );
    assert_eq!(
      computed("color-mix(in hsl increasing hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",),
      Some(ColorInput::Value(Color([64, 169, 191, 255])))
    );
  }

  #[test]
  fn test_parse_color_mix_rejects_percentages_outside_0_to_100() {
    assert!(computed("color-mix(in srgb, red 120%, blue 80%)").is_none());
    assert!(computed("color-mix(in srgb, currentColor -10%, blue 60%)").is_none());
    assert!(computed("color-mix(in srgb, red, -10% blue)").is_none());
  }

  #[test]
  fn test_parse_color_mix_unknown_color_space() {
    assert!(computed("color-mix(in unknown, red, blue)").is_none());
  }

  #[test]
  fn test_parse_color_mix_hue_method_with_non_cylindrical_space_errors() {
    assert!(computed("color-mix(in srgb longer hue, red, blue)").is_none());
  }

  #[test]
  fn test_parse_color_mix_malformed_missing_comma_errors() {
    assert!(computed("color-mix(in srgb, red blue)").is_none());
  }

  #[test]
  fn test_parse_color_mix_zero_sum_percentages_mix_to_transparent() {
    assert_eq!(
      computed("color-mix(in srgb, red 0%, blue 0%)"),
      Some(ColorInput::Value(Color([128, 0, 128, 0])))
    );
  }

  #[test]
  fn test_parse_color_mix_rejects_a_number_as_percentage() {
    assert!(computed("color-mix(in srgb, red 0.5, blue 0.5)").is_none());
  }

  #[test]
  fn test_color_mix_percentages_survive_serialization() {
    for css in [
      "color-mix(in srgb, red 0.0000119209%, blue 0.0000119209%)",
      "color-mix(in srgb, red 0.00001192094%, blue 0%)",
      "color-mix(in srgb, currentColor 25%, blue)",
      "color-mix(in srgb, red calc(120%), blue)",
      "color-mix(in srgb, red calc(-10%), blue 60%)",
    ] {
      let serialized = ColorInput::from_css_str(css).map(|color| color.to_css_string());

      assert!(serialized.is_ok(), "{css}");

      let reparsed = serialized
        .clone()
        .and_then(|serialized| ColorInput::from_css_str(&serialized))
        .map(|color| color.to_css_string());

      assert_eq!(reparsed, serialized, "{css}");
    }
  }

  #[test]
  fn test_parse_color_mix_nested_color_mix() {
    assert!(computed("color-mix(in srgb, color-mix(in srgb, red, blue), white)").is_some());
  }

  #[test]
  fn test_parse_relative_oklch_keywords_only() {
    let relative = computed("oklch(from oklch(0.7 0.15 30) l c h / 0.5)");
    let direct = computed("oklch(0.7 0.15 30 / 0.5)");
    assert_eq!(relative, direct);
  }

  #[test]
  fn test_parse_relative_oklch_keywords_only_from_named_color() {
    assert!(computed("oklch(from red l c h)").is_some());
  }

  #[test]
  fn test_parse_relative_rgb_keywords_only_scales_to_255() {
    assert_eq!(
      computed("rgb(from #336699 r g b)"),
      computed("rgb(51 102 153)"),
    );
  }

  #[test]
  fn test_parse_relative_rgb_drops_origin_alpha_when_alpha_omitted() {
    assert_eq!(
      computed("rgb(from rgb(10 20 30 / 0.5) r g b / 0.25)"),
      computed("rgba(10, 20, 30, 0.25)"),
    );
  }

  #[test]
  fn test_parse_relative_hsl_with_dimension_hue() {
    assert!(computed("hsl(from red 120deg s l)").is_some());
  }

  #[test]
  fn test_parse_relative_oklch_substitutes_origin_alpha() {
    assert_eq!(
      computed("oklch(from oklch(0.5 0.1 200 / 0.4) l c h / alpha)",),
      computed("oklch(0.5 0.1 200 / 0.4)"),
    );
  }

  #[test]
  fn test_parse_relative_color_with_color_mix_origin() {
    assert!(computed("lch(from color-mix(in srgb, red, blue) l c h)").is_some());
  }

  #[test]
  fn test_parse_relative_color_in_linear_gradient() {
    use crate::style::properties::linear_gradient::LinearGradient;

    assert!(
      LinearGradient::from_css_str(
        "linear-gradient(to right, oklch(from oklch(0.7 0.15 30) l c h / 0.5), white)",
      )
      .is_ok()
    );
  }

  #[test]
  fn test_parse_color_mix_inside_linear_gradient() {
    use crate::style::properties::linear_gradient::LinearGradient;

    assert!(
      LinearGradient::from_css_str(
        "linear-gradient(to right, color-mix(in srgb, red, blue), white)"
      )
      .is_ok()
    );
  }

  #[test]
  fn dark_and_light_follow_blink() {
    assert_eq!(Color::white().dark(), Color([171, 171, 171, 255]));
    assert_eq!(Color::black().light(), Color([84, 84, 84, 255]));
    assert_eq!(Color([100, 116, 139, 255]).dark(), Color([39, 45, 55, 255]));
  }

  #[test]
  fn a_very_dark_3d_border_lightens_both_edges() {
    assert_eq!(Color::black().inset_outset(true), Color([84, 84, 84, 255]));
    assert_eq!(
      Color::black().inset_outset(false),
      Color([168, 168, 168, 255])
    );
  }

  #[test]
  fn a_very_light_3d_border_keeps_its_lit_edge() {
    assert_eq!(Color::white().inset_outset(false), Color::white());
    assert_eq!(
      Color::white().inset_outset(true),
      Color([171, 171, 171, 255])
    );
  }
}

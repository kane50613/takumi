use std::fmt::{self, Display};

use color::{AlphaColor, ColorSpaceTag, DynamicColor, HueDirection, Rgba8, Srgb, parse_color};
use cssparser::{
  Parser, Token,
  color::{parse_hash_color, parse_named_color},
  match_ignore_ascii_case,
};
use tiny_skia::{ColorU8, PremultipliedColorU8};

use crate::style::tw::{Namespace, extract_arbitrary_value};
use crate::style::{
  Animatable, Color as CurrentColor, CssDescriptorKind, CssSyntaxKind, CssToken, FromCss,
  FromCssStr, MakeComputed, ParseResult, PercentageNumber, SizingContext, ToCss,
  math::fast_div_255, tw::TailwindPropertyParser, unexpected_token,
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
  fn default() -> Self {
    Self {
      // Reference: https://developer.mozilla.org/en-US/docs/Web/CSS/color-interpolation-method
      // When interpolating <color> values, the interpolation color space defaults to Oklab.
      color_space: ColorSpaceTag::Oklab,
      hue_direction: HueDirection::Shorter,
    }
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
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    input.expect_ident_matching("in")?;

    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(color_space_ident) = token else {
      return Err(unexpected_token!(location, token));
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
      _ => return Err(unexpected_token!(location, token)),
    };

    let mut hue_direction = HueDirection::Shorter;
    let mut has_hue_direction = false;

    if let Ok(direction) = input.try_parse(|input| -> ParseResult<'i, HueDirection> {
      let location = input.current_source_location();
      let token = input.next()?;
      let Token::Ident(ident) = token else {
        return Err(unexpected_token!(location, token));
      };

      let direction = match_ignore_ascii_case! { &ident,
        "shorter" => HueDirection::Shorter,
        "longer" => HueDirection::Longer,
        "increasing" => HueDirection::Increasing,
        "decreasing" => HueDirection::Decreasing,
        _ => return Err(unexpected_token!(location, token)),
      };

      input.expect_ident_matching("hue")?;

      Ok(direction)
    }) {
      hue_direction = direction;
      has_hue_direction = true;
    }

    if has_hue_direction && !is_cylindrical_color_space(color_space) {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self {
      color_space,
      hue_direction,
    })
  }

  const VALID_TOKENS: &'static [CssToken] =
    &[CssToken::Descriptor(CssDescriptorKind::InColorSpace)];
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
    write!(dest, "in {}{}", space, hue)
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

/// Represents a color input value.
#[derive(Debug, Clone, PartialEq, Copy, Default)]
#[non_exhaustive]
pub enum ColorInput {
  /// Inherit from the `color` value.
  #[default]
  CurrentColor,
  /// A color value.
  Value(Color),
}

impl MakeComputed for ColorInput {}

impl Animatable for ColorInput {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    current_color: CurrentColor,
  ) {
    *self = match (from, to) {
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

  /// Resolves the color input to a color.
  pub fn resolve(self, current_color: Color) -> Color {
    match self {
      ColorInput::Value(color) => color,
      ColorInput::CurrentColor => current_color,
    }
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

  /// `Color::parse_tw` already reads the `/50` modifier, so this only carries it
  /// across the `current` keyword that `Color` has no representation for.
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
/// Shades: 50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950.
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
    // handle opacity text like `text-red-50/30`
    if let Some((color, opacity)) = token.split_once('/') {
      let color = Color::parse_tw(color)?;
      let opacity = (opacity.parse::<f32>().ok()? * 2.55).round() as u8;

      return Some(color.with_opacity(opacity));
    }

    // Handle basic colors first
    match_ignore_ascii_case! {token,
      "transparent" => return Some(Color::transparent()),
      "black" => return Some(Color::black()),
      "white" => return Some(Color::white()),
      _ => {}
    }

    // Parse color-shade format (e.g., "red-500")
    let (color_name, shade_str) = token.rsplit_once('-')?;
    let shade: u16 = shade_str.parse().ok()?;

    // Lookup in color table
    lookup_tailwind_color(color_name, shade).map(Color::from_rgb)
  }
}

impl From<Color> for ColorInput {
  fn from(color: Color) -> Self {
    ColorInput::Value(color)
  }
}

impl Display for Color {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if self.0[3] == 255 {
      return write!(f, "rgb({}, {}, {})", self.0[0], self.0[1], self.0[2]);
    }

    write!(
      f,
      "rgba({}, {}, {}, {:.6})",
      self.0[0],
      self.0[1],
      self.0[2],
      self.0[3] as f32 / 255.0
    )
  }
}

impl ToCss for Color {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self)
  }
}

impl ToCss for ColorInput {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::CurrentColor => dest.write_str("currentColor"),
      Self::Value(c) => c.to_css(dest),
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

  /// Premultiplies the straight-alpha colour into a `tiny_skia`
  /// [`PremultipliedColorU8`].
  pub(crate) fn premultiplied(self) -> PremultipliedColorU8 {
    let [r, g, b, a] = self.0;
    let premul_r = fast_div_255(r as u32 * a as u32);
    let premul_g = fast_div_255(g as u32 * a as u32);
    let premul_b = fast_div_255(b as u32 * a as u32);

    PremultipliedColorU8::from_rgba(premul_r, premul_g, premul_b, a)
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

    let [r1, g1, b1, a1] = self.0;
    let [r2, g2, b2, a2] = other.0;
    let premul_1 = [
      fast_div_255(r1 as u32 * a1 as u32),
      fast_div_255(g1 as u32 * a1 as u32),
      fast_div_255(b1 as u32 * a1 as u32),
      a1,
    ];
    let premul_2 = [
      fast_div_255(r2 as u32 * a2 as u32),
      fast_div_255(g2 as u32 * a2 as u32),
      fast_div_255(b2 as u32 * a2 as u32),
      a2,
    ];

    let mut result = [0u8; 4];
    for i in 0..4 {
      result[i] = (premul_1[i] as f32 * (1.0 - t) + premul_2[i] as f32 * t)
        .round()
        .clamp(0.0, 255.0) as u8;
    }

    let premul = PremultipliedColorU8::from_rgba(
      result[0].min(result[3]),
      result[1].min(result[3]),
      result[2].min(result[3]),
      result[3],
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT);
    let demul: ColorU8 = premul.demultiply();
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

    let dynamic_1 =
      DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(Rgba8::from_u8_array(self.0)));
    let dynamic_2 =
      DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(Rgba8::from_u8_array(other.0)));

    let mixed = dynamic_1
      .interpolate(dynamic_2, color_space, hue_direction)
      .eval(t);
    let rgba = mixed.to_alpha_color::<Srgb>().to_rgba8().to_u8_array();

    Color(rgba)
  }

  /// Mixes `amount` of `target` into the colour, keeping this alpha.
  pub(crate) fn mix(self, target: Color, amount: f32) -> Self {
    let amount = amount.clamp(0.0, 1.0);
    let inverse = 1.0 - amount;

    Color([
      (self.0[0] as f32 * inverse + target.0[0] as f32 * amount).round() as u8,
      (self.0[1] as f32 * inverse + target.0[1] as f32 * amount).round() as u8,
      (self.0[2] as f32 * inverse + target.0[2] as f32 * amount).round() as u8,
      self.0[3],
    ])
  }

  /// Apply opacity to alpha channel
  pub(crate) fn with_opacity(mut self, opacity: u8) -> Self {
    self.0[3] = fast_div_255(self.0[3] as u32 * opacity as u32);

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
}

#[derive(Debug, Clone, Copy)]
struct ColorMixItem {
  color: Color,
  percentage: Option<PercentageNumber>,
}

impl<'i> FromCss<'i> for ColorMixItem {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(item) = input.try_parse(|input| -> ParseResult<'i, Self> {
      let color = Color::from_css(input)?;
      let percentage = input.try_parse(PercentageNumber::from_css).ok();

      Ok(Self { color, percentage })
    }) {
      return Ok(item);
    }

    input.try_parse(|input| -> ParseResult<'i, Self> {
      let percentage = PercentageNumber::from_css(input)?;
      let color = Color::from_css(input)?;

      Ok(Self {
        color,
        percentage: Some(percentage),
      })
    })
  }

  const VALID_TOKENS: &'static [CssToken] =
    &[CssToken::Descriptor(CssDescriptorKind::ColorAndPercentage)];
}

#[derive(Debug, Clone, Copy)]
struct ColorMix {
  interpolation: ColorInterpolationMethod,
  first: ColorMixItem,
  second: ColorMixItem,
}

impl ColorMix {
  fn evaluate(self) -> Option<Color> {
    let mut p1 = self.first.percentage;
    let mut p2 = self.second.percentage;

    match (p1, p2) {
      (None, None) => {
        p1 = Some(PercentageNumber(0.5));
        p2 = Some(PercentageNumber(0.5));
      }
      (Some(p1_value), None) => {
        p2 = Some(PercentageNumber((1.0 - p1_value.0).max(0.0)));
      }
      (None, Some(p2_value)) => {
        p1 = Some(PercentageNumber((1.0 - p2_value.0).max(0.0)));
      }
      _ => {}
    }

    let p1 = p1.unwrap_or(PercentageNumber(0.5)).0;
    let p2 = p2.unwrap_or(PercentageNumber(0.5)).0;
    let sum = p1 + p2;

    if sum <= f32::EPSILON {
      return None;
    }

    let weight_2 = p2 / sum;
    let alpha_multiplier = sum.min(1.0);

    let dynamic_1 = DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(
      color::Rgba8::from_u8_array(self.first.color.0),
    ));
    let dynamic_2 = DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from(
      color::Rgba8::from_u8_array(self.second.color.0),
    ));

    let mixed = dynamic_1
      .interpolate(
        dynamic_2,
        self.interpolation.color_space,
        self.interpolation.hue_direction,
      )
      .eval(weight_2)
      .multiply_alpha(alpha_multiplier);

    Some(Color(
      mixed.to_alpha_color::<Srgb>().to_rgba8().to_u8_array(),
    ))
  }
}

impl<'i> FromCss<'i> for ColorMix {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let interpolation = ColorInterpolationMethod::from_css(input)?;

    input.expect_comma()?;
    let first = ColorMixItem::from_css(input)?;
    input.expect_comma()?;
    let second = ColorMixItem::from_css(input)?;

    if !input.is_exhausted() {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self {
      interpolation,
      first,
      second,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Descriptor(CssDescriptorKind::ColorMixFn)];
}

impl<'i> FromCss<'i> for ColorInput {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("currentcolor"))
      .is_ok()
    {
      return Ok(ColorInput::CurrentColor);
    }

    Ok(ColorInput::Value(Color::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("currentColor"),
    CssToken::Syntax(CssSyntaxKind::Color),
  ];
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
    (_, 3) => 1.0,
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

fn parse_relative_slot<'i>(
  input: &mut Parser<'i, '_>,
  target_cs: ColorSpaceTag,
  index: usize,
  keyword_values: [f32; 4],
) -> ParseResult<'i, f32> {
  let location = input.current_source_location();
  let token = input.next()?.clone();
  let is_hue = is_hue_slot(target_cs, index);
  match &token {
    Token::Ident(ident) => {
      if ident.eq_ignore_ascii_case("none") {
        return Ok(0.0);
      }
      if ident.eq_ignore_ascii_case("alpha") {
        return Ok(keyword_values[3]);
      }
      for (i, name) in channel_names(target_cs).iter().enumerate() {
        if !name.is_empty() && ident.eq_ignore_ascii_case(name) {
          return Ok(keyword_values[i]);
        }
      }
      Err(unexpected_token!(Color, location, &token))
    }
    Token::Number { value, .. } => Ok(*value),
    Token::Percentage { unit_value, .. } if !is_hue => {
      Ok(*unit_value * channel_percentage_scale(target_cs, index))
    }
    Token::Dimension { value, unit, .. } if is_hue => Ok(match_ignore_ascii_case! { unit.as_ref(),
      "deg" => *value,
      "grad" => *value / 400.0 * 360.0,
      "turn" => *value * 360.0,
      "rad" => value.to_degrees(),
      _ => return Err(unexpected_token!(Color, location, &token)),
    }),
    _ => Err(unexpected_token!(Color, location, &token)),
  }
}

/// `calc()` expressions in component slots are not yet supported.
fn parse_relative_color<'i>(
  input: &mut Parser<'i, '_>,
  target_cs: ColorSpaceTag,
) -> ParseResult<'i, Color> {
  let origin_start = input.position();
  let origin_location = input.current_source_location();
  let origin_token = input.next()?.clone();
  match &origin_token {
    Token::Hash(_) | Token::IDHash(_) | Token::Ident(_) => {}
    Token::Function(_) => {
      input.parse_nested_block(|inner| -> ParseResult<'i, ()> {
        while inner.next().is_ok() {}
        Ok(())
      })?;
    }
    _ => return Err(unexpected_token!(Color, origin_location, &origin_token)),
  }

  let scale = channel_keyword_scale(target_cs);
  let origin_color = Color::from_css_str(input.slice_from(origin_start).trim())
    .map_err(|_| unexpected_token!(Color, origin_location, &origin_token))?;
  let [r, g, b, a] = origin_color.0;
  let converted =
    DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from_rgba8(r, g, b, a)).convert(target_cs);
  let [k0, k1, k2, k_alpha] = converted.components;
  let keyword_values = [k0 * scale, k1 * scale, k2 * scale, k_alpha];

  let s0 = parse_relative_slot(input, target_cs, 0, keyword_values)?;
  let s1 = parse_relative_slot(input, target_cs, 1, keyword_values)?;
  let s2 = parse_relative_slot(input, target_cs, 2, keyword_values)?;
  let alpha = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
    parse_relative_slot(input, target_cs, 3, keyword_values)?
  } else {
    k_alpha
  };

  let new_components = [s0 / scale, s1 / scale, s2 / scale, alpha.clamp(0.0, 1.0)];
  let result = converted.map(|_, _, _, _| new_components);
  Ok(Color(
    result.to_alpha_color::<Srgb>().to_rgba8().to_u8_array(),
  ))
}

impl<'i> FromCss<'i> for Color {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let position = input.position();
    let token = input.next()?;

    match *token {
      Token::Hash(ref value) | Token::IDHash(ref value) => parse_hash_color(value.as_bytes())
        .map(|(r, g, b, a)| Color([r, g, b, (a * 255.0) as u8]))
        .map_err(|_| unexpected_token!(location, token)),
      Token::Ident(ref ident) => {
        if ident.eq_ignore_ascii_case("transparent") {
          return Ok(Color::transparent());
        }

        parse_named_color(ident)
          .map(|(r, g, b)| Color([r, g, b, 255]))
          .map_err(|_| unexpected_token!(location, token))
      }
      Token::Function(_) => {
        // Have to clone to persist token, and allow input to be borrowed
        let token = token.clone();

        if let Token::Function(function) = &token
          && function.eq_ignore_ascii_case("color-mix")
        {
          return input.parse_nested_block(|input| {
            let color_mix = ColorMix::from_css(input)?;
            color_mix
              .evaluate()
              .ok_or_else(|| input.new_error_for_next_token())
          });
        }

        let target_cs = if let Token::Function(name) = &token {
          relative_target_cs(name)
        } else {
          None
        };

        input.parse_nested_block(|input| {
          if let Some(cs) = target_cs
            && input.try_parse(|i| i.expect_ident_matching("from")).is_ok()
          {
            return parse_relative_color(input, cs);
          }

          while input.next().is_ok() {}

          // Slice from the function name till before the closing parenthesis
          let body = input.slice_from(position);

          let mut function = body.to_string();

          // Add closing parenthesis
          function.push(')');

          parse_color(&function)
            .map(|color| Color(color.to_alpha_color::<Srgb>().to_rgba8().to_u8_array()))
            .map_err(|_| unexpected_token!(location, &token))
        })
      }
      _ => Err(unexpected_token!(location, token)),
    }
  }
  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Color)];
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{
    geometry::Size,
    style::{CalcArena, sizing::SizingContext},
    viewport::Viewport,
  };

  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport::new((200, 100)),
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
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

    // Oklab would swing the midpoint through a lighter, pinker path; sRGB keeps
    // each channel on its own straight line, which is what Chromium does for a
    // legacy colour.
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
      ColorInput::from_css_str("color-mix(in srgb, red, blue)"),
      Ok(ColorInput::Value(Color([128, 0, 128, 255])))
    );
  }

  #[test]
  fn test_parse_color_mix_equivalent_percentage_syntaxes() {
    let canonical = ColorInput::from_css_str("color-mix(in srgb, red 25%, blue 75%)");

    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in srgb, 25% red, 75% blue)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in srgb, red 25%, 75% blue)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in srgb, red 25%, blue)")
    );
    assert_eq!(canonical, Ok(ColorInput::Value(Color([64, 0, 191, 255]))));
  }

  #[test]
  fn test_parse_color_mix_lch_missing_percentage_equivalence() {
    let canonical = ColorInput::from_css_str("color-mix(in lch, purple 50%, plum 50%)");

    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple 50%, plum)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple, plum 50%)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple, plum)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, plum, purple)")
    );
  }

  #[test]
  fn test_parse_color_mix_lch_normalizes_equal_opaque_percentages() {
    let canonical = ColorInput::from_css_str("color-mix(in lch, purple 50%, plum 50%)");

    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple 55%, plum 55%)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple 70%, plum 70%)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple 95%, plum 95%)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple 125%, plum 125%)")
    );
    assert_eq!(
      canonical,
      ColorInput::from_css_str("color-mix(in lch, purple 9999%, plum 9999%)")
    );
  }

  #[test]
  fn test_parse_color_mix_endpoint_percentages_return_endpoint_colors() {
    assert_eq!(
      ColorInput::from_css_str("color-mix(in srgb, red 100%, blue 0%)"),
      ColorInput::from_css_str("red")
    );
    assert_eq!(
      ColorInput::from_css_str("color-mix(in srgb, red 0%, blue 100%)"),
      ColorInput::from_css_str("blue")
    );
  }

  #[test]
  fn test_parse_color_mix_alpha_multiplier_under_100_percent() {
    assert_eq!(
      ColorInput::from_css_str("color-mix(in srgb, red 30%, blue 30%)"),
      Ok(ColorInput::Value(Color([128, 0, 128, 153])))
    );
  }

  #[test]
  fn test_parse_color_mix_hue_directions_change_result() {
    assert_eq!(
      ColorInput::from_css_str(
        "color-mix(in hsl shorter hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",
      ),
      Ok(ColorInput::Value(Color([191, 86, 64, 255])))
    );
    assert_eq!(
      ColorInput::from_css_str(
        "color-mix(in hsl decreasing hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",
      ),
      Ok(ColorInput::Value(Color([191, 86, 64, 255])))
    );
    assert_eq!(
      ColorInput::from_css_str(
        "color-mix(in hsl longer hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",
      ),
      Ok(ColorInput::Value(Color([64, 169, 191, 255])))
    );
    assert_eq!(
      ColorInput::from_css_str(
        "color-mix(in hsl increasing hue, hsl(50deg 50% 50%), hsl(330deg 50% 50%))",
      ),
      Ok(ColorInput::Value(Color([64, 169, 191, 255])))
    );
  }

  #[test]
  fn test_parse_color_mix_over_100_percent_normalizes_weights() {
    assert_eq!(
      ColorInput::from_css_str("color-mix(in srgb, red 120%, blue 80%)"),
      Ok(ColorInput::Value(Color([153, 0, 102, 255])))
    );
  }

  #[test]
  fn test_parse_color_mix_unknown_color_space() {
    assert!(ColorInput::from_css_str("color-mix(in unknown, red, blue)").is_err());
  }

  #[test]
  fn test_parse_color_mix_hue_method_with_non_cylindrical_space_errors() {
    assert!(ColorInput::from_css_str("color-mix(in srgb longer hue, red, blue)").is_err());
  }

  #[test]
  fn test_parse_color_mix_malformed_missing_comma_errors() {
    assert!(ColorInput::from_css_str("color-mix(in srgb, red blue)").is_err());
  }

  #[test]
  fn test_parse_color_mix_zero_sum_percentages_errors() {
    assert!(ColorInput::from_css_str("color-mix(in srgb, red 0%, blue 0%)").is_err());
  }

  #[test]
  fn test_parse_color_mix_accepts_number_as_percentage() {
    assert_eq!(
      ColorInput::from_css_str("color-mix(in srgb, red 0.5, blue 0.5)"),
      Ok(ColorInput::Value(Color([128, 0, 128, 255])))
    );
  }

  #[test]
  fn test_parse_color_mix_nested_color_mix() {
    assert!(
      ColorInput::from_css_str("color-mix(in srgb, color-mix(in srgb, red, blue), white)").is_ok()
    );
  }

  #[test]
  fn test_parse_relative_oklch_keywords_only() {
    let relative = ColorInput::from_css_str("oklch(from oklch(0.7 0.15 30) l c h / 0.5)");
    let direct = ColorInput::from_css_str("oklch(0.7 0.15 30 / 0.5)");
    assert_eq!(relative, direct);
  }

  #[test]
  fn test_parse_relative_oklch_keywords_only_from_named_color() {
    assert!(ColorInput::from_css_str("oklch(from red l c h)").is_ok());
  }

  #[test]
  fn test_parse_relative_rgb_keywords_only_scales_to_255() {
    assert_eq!(
      ColorInput::from_css_str("rgb(from #336699 r g b)"),
      ColorInput::from_css_str("rgb(51 102 153)"),
    );
  }

  #[test]
  fn test_parse_relative_rgb_drops_origin_alpha_when_alpha_omitted() {
    assert_eq!(
      ColorInput::from_css_str("rgb(from rgb(10 20 30 / 0.5) r g b / 0.25)"),
      ColorInput::from_css_str("rgba(10, 20, 30, 0.25)"),
    );
  }

  #[test]
  fn test_parse_relative_hsl_with_dimension_hue() {
    assert!(ColorInput::from_css_str("hsl(from red 120deg s l)").is_ok());
  }

  #[test]
  fn test_parse_relative_oklch_substitutes_origin_alpha() {
    assert_eq!(
      ColorInput::from_css_str("oklch(from oklch(0.5 0.1 200 / 0.4) l c h / alpha)",),
      ColorInput::from_css_str("oklch(0.5 0.1 200 / 0.4)"),
    );
  }

  #[test]
  fn test_parse_relative_color_with_color_mix_origin() {
    assert!(ColorInput::from_css_str("lch(from color-mix(in srgb, red, blue) l c h)").is_ok());
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
}

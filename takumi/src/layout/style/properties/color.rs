use std::fmt::Display;

use color::{Srgb, parse_color};
use cssparser::{
  Parser, Token,
  color::{parse_hash_color, parse_named_color},
  match_ignore_ascii_case,
};
use image::Rgba;

use crate::{
  layout::style::{CssToken, FromCss, ParseResult, tw::TailwindPropertyParser},
  rendering::fast_div_255,
};

/// Represents a color with 8-bit RGBA components.
#[derive(Debug, Default, Clone, PartialEq, Copy)]
pub struct Color(pub [u8; 4]);

/// Represents a color input value.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ColorInput<const DEFAULT_CURRENT_COLOR: bool = true> {
  /// Inherit from the `color` value.
  CurrentColor,
  /// A color value.
  Value(Color),
}

impl<const DEFAULT_CURRENT_COLOR: bool> Default for ColorInput<DEFAULT_CURRENT_COLOR> {
  fn default() -> Self {
    if DEFAULT_CURRENT_COLOR {
      ColorInput::CurrentColor
    } else {
      ColorInput::Value(Color::transparent())
    }
  }
}

impl<const DEFAULT_CURRENT_COLOR: bool> ColorInput<DEFAULT_CURRENT_COLOR> {
  /// Resolves the color input to a color.
  pub fn resolve(self, current_color: Color) -> Color {
    match self {
      ColorInput::Value(color) => color,
      ColorInput::CurrentColor => current_color,
    }
  }
}

impl<const DEFAULT_CURRENT_COLOR: bool> TailwindPropertyParser
  for ColorInput<DEFAULT_CURRENT_COLOR>
{
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("current") {
      return Some(ColorInput::CurrentColor);
    }

    Color::parse_tw(token).map(ColorInput::Value)
  }
}

/// Tailwind color shades and their corresponding RGB values
/// Each color has 11 shades: 50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950
const SLATE: [u32; 11] = [
  0xf8fafc, 0xf1f5f9, 0xe2e8f0, 0xcbd5e1, 0x94a3b8, 0x64748b, 0x475569, 0x334155, 0x1e293b,
  0x0f172a, 0x020617,
];

const GRAY: [u32; 11] = [
  0xf9fafb, 0xf3f4f6, 0xe5e7eb, 0xd1d5db, 0x9ca3af, 0x6b7280, 0x4b5563, 0x374151, 0x1f2937,
  0x111827, 0x030712,
];

const ZINC: [u32; 11] = [
  0xfafafa, 0xf4f4f5, 0xe4e4e7, 0xd4d4d8, 0xa1a1aa, 0x71717a, 0x52525b, 0x3f3f46, 0x27272a,
  0x18181b, 0x09090b,
];

const NEUTRAL: [u32; 11] = [
  0xfafafa, 0xf5f5f5, 0xe5e5e5, 0xd4d4d4, 0xa3a3a3, 0x737373, 0x525252, 0x404040, 0x262626,
  0x171717, 0x0a0a0a,
];

const STONE: [u32; 11] = [
  0xfafaf9, 0xf5f5f4, 0xe7e5e4, 0xd6d3d1, 0xa8a29e, 0x78716c, 0x57534e, 0x44403c, 0x292524,
  0x1c1917, 0x0c0a09,
];

const RED: [u32; 11] = [
  0xfef2f2, 0xfee2e2, 0xfecaca, 0xfca5a5, 0xf87171, 0xef4444, 0xdc2626, 0xb91c1c, 0x991b1b,
  0x7f1d1d, 0x450a0a,
];

const ORANGE: [u32; 11] = [
  0xfff7ed, 0xffedd5, 0xfed7aa, 0xfdba74, 0xfb923c, 0xf97316, 0xea580c, 0xc2410c, 0x9a3412,
  0x7c2d12, 0x431407,
];

const AMBER: [u32; 11] = [
  0xfffbeb, 0xfef3c7, 0xfde68a, 0xfcd34d, 0xfbbf24, 0xf59e0b, 0xd97706, 0xb45309, 0x92400e,
  0x78350f, 0x451a03,
];

const YELLOW: [u32; 11] = [
  0xfefce8, 0xfef9c3, 0xfef08a, 0xfde047, 0xfacc15, 0xeab308, 0xca8a04, 0xa16207, 0x854d0e,
  0x713f12, 0x422006,
];

const LIME: [u32; 11] = [
  0xf7fee7, 0xecfccb, 0xd9f99d, 0xbef264, 0xa3e635, 0x84cc16, 0x65a30d, 0x4d7c0f, 0x3f6212,
  0x365314, 0x1a2e05,
];

const GREEN: [u32; 11] = [
  0xf0fdf4, 0xdcfce7, 0xbbf7d0, 0x86efac, 0x4ade80, 0x22c55e, 0x16a34a, 0x15803d, 0x166534,
  0x14532d, 0x052e16,
];

const EMERALD: [u32; 11] = [
  0xecfdf5, 0xd1fae5, 0xa7f3d0, 0x6ee7b7, 0x34d399, 0x10b981, 0x059669, 0x047857, 0x065f46,
  0x064e3b, 0x052c22,
];

const TEAL: [u32; 11] = [
  0xf0fdfa, 0xccfbf1, 0x99f6e4, 0x5eead4, 0x2dd4bf, 0x14b8a6, 0x0d9488, 0x0f766e, 0x115e59,
  0x134e4a, 0x042f2e,
];

const CYAN: [u32; 11] = [
  0xecfeff, 0xcffafe, 0xa5f3fc, 0x67e8f9, 0x22d3ee, 0x06b6d4, 0x0891b2, 0x0e7490, 0x155e75,
  0x164e63, 0x083344,
];

const SKY: [u32; 11] = [
  0xf0f9ff, 0xe0f2fe, 0xbae6fd, 0x7dd3fc, 0x38bdf8, 0x0ea5e9, 0x0284c7, 0x0369a1, 0x075985,
  0x0c4a6e, 0x082f49,
];

const BLUE: [u32; 11] = [
  0xeff6ff, 0xdbeafe, 0xbfdbfe, 0x93c5fd, 0x60a5fa, 0x3b82f6, 0x2563eb, 0x1d4ed8, 0x1e40af,
  0x1e3a8a, 0x172554,
];

const INDIGO: [u32; 11] = [
  0xeef2ff, 0xe0e7ff, 0xc7d2fe, 0xa5b4fc, 0x818cf8, 0x6366f1, 0x4f46e5, 0x4338ca, 0x3730a3,
  0x312e81, 0x1e1b4b,
];

const VIOLET: [u32; 11] = [
  0xf5f3ff, 0xede9fe, 0xddd6fe, 0xc4b5fd, 0xa78bfa, 0x8b5cf6, 0x7c3aed, 0x6d28d9, 0x5b21b6,
  0x4c1d95, 0x2e1065,
];

const PURPLE: [u32; 11] = [
  0xfaf5ff, 0xf3e8ff, 0xe9d5ff, 0xd8b4fe, 0xc084fc, 0xa855f7, 0x9333ea, 0x7e22ce, 0x6b21a8,
  0x581c87, 0x3b0764,
];

const FUCHSIA: [u32; 11] = [
  0xfdf4ff, 0xfae8ff, 0xf5d0fe, 0xf0abfc, 0xe879f9, 0xd946ef, 0xc026d3, 0xa21caf, 0x86198f,
  0x701a75, 0x4a044e,
];

const PINK: [u32; 11] = [
  0xfdf2f8, 0xfce7f3, 0xfbcfe8, 0xf9a8d4, 0xf472b6, 0xec4899, 0xdb2777, 0xbe185d, 0x9d174d,
  0x831843, 0x500724,
];

const ROSE: [u32; 11] = [
  0xfff1f2, 0xffe4e6, 0xfecdd3, 0xfda4af, 0xfb7185, 0xf43f5e, 0xe11d48, 0xbe123c, 0x9f1239,
  0x881337, 0x4c0519,
];

/// Shade values in ascending order for binary search
const SHADES: [u16; 11] = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

/// Map shade number to array index using binary search
#[inline]
fn shade_to_index(shade: u16) -> Option<usize> {
  SHADES.binary_search(&shade).ok()
}

/// Lookup Tailwind color by name and shade
///
/// Returns the RGB value as a u32 (0xRRGGBB format)
fn lookup_tailwind_color(color_name: &str, shade: u16) -> Option<u32> {
  let index = shade_to_index(shade)?;

  let colors = match_ignore_ascii_case! {color_name,
      "slate" => &SLATE,
      "gray" => &GRAY,
      "zinc" => &ZINC,
      "neutral" => &NEUTRAL,
      "stone" => &STONE,
      "red" => &RED,
      "orange" => &ORANGE,
      "amber" => &AMBER,
      "yellow" => &YELLOW,
      "lime" => &LIME,
      "green" => &GREEN,
      "emerald" => &EMERALD,
      "teal" => &TEAL,
      "cyan" => &CYAN,
      "sky" => &SKY,
      "blue" => &BLUE,
      "indigo" => &INDIGO,
      "violet" => &VIOLET,
      "purple" => &PURPLE,
      "fuchsia" => &FUCHSIA,
      "pink" => &PINK,
      "rose" => &ROSE,
      _ => return None,
  };

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

impl<const DEFAULT_CURRENT_COLOR: bool> From<Color> for ColorInput<DEFAULT_CURRENT_COLOR> {
  fn from(color: Color) -> Self {
    ColorInput::Value(color)
  }
}

impl From<Color> for Rgba<u8> {
  fn from(color: Color) -> Self {
    Rgba(color.0)
  }
}

impl Display for Color {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "rgb({} {} {} / {})",
      self.0[0],
      self.0[1],
      self.0[2],
      self.0[3] as f32 / 255.0
    )
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

  /// Apply opacity to alpha channel
  pub fn with_opacity(mut self, opacity: u8) -> Self {
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

impl<'i, const DEFAULT_CURRENT_COLOR: bool> FromCss<'i> for ColorInput<DEFAULT_CURRENT_COLOR> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("currentcolor"))
      .is_ok()
    {
      return Ok(ColorInput::CurrentColor);
    }

    Ok(ColorInput::Value(Color::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("currentColor"), CssToken::Token("color")]
  }
}

impl<'i> FromCss<'i> for Color {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let position = input.position();
    let token = input.next()?;

    match *token {
      Token::Hash(ref value) | Token::IDHash(ref value) => parse_hash_color(value.as_bytes())
        .map(|(r, g, b, a)| Color([r, g, b, (a * 255.0) as u8]))
        .map_err(|_| Self::unexpected_token_error(location, token)),
      Token::Ident(ref ident) => {
        if ident.eq_ignore_ascii_case("transparent") {
          return Ok(Color::transparent());
        }

        parse_named_color(ident)
          .map(|(r, g, b)| Color([r, g, b, 255]))
          .map_err(|_| Self::unexpected_token_error(location, token))
      }
      Token::Function(_) => {
        // Have to clone to persist token, and allow input to be borrowed
        let token = token.clone();

        input.parse_nested_block(|input| {
          while input.next().is_ok() {}

          // Slice from the function name till before the closing parenthesis
          let body = input.slice_from(position);

          let mut function = body.to_string();

          // Add closing parenthesis
          function.push(')');

          parse_color(&function)
            .map(|color| Color(color.to_alpha_color::<Srgb>().to_rgba8().to_u8_array()))
            .map_err(|_| Self::unexpected_token_error(location, &token))
        })
      }
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }
  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("color")]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_hex_color_3_digits() {
    // Test 3-digit hex color
    assert_eq!(
      ColorInput::from_str("#f09"),
      Ok(ColorInput::<true>::Value(Color([255, 0, 153, 255])))
    );
  }

  #[test]
  fn test_parse_hex_color_6_digits() {
    // Test 6-digit hex color
    assert_eq!(
      ColorInput::from_str("#ff0099"),
      Ok(ColorInput::<true>::Value(Color([255, 0, 153, 255])))
    );
  }

  #[test]
  fn test_parse_color_transparent() {
    // Test parsing transparent keyword
    assert_eq!(
      ColorInput::from_str("transparent"),
      Ok(ColorInput::<true>::Value(Color([0, 0, 0, 0])))
    );
  }

  #[test]
  fn test_parse_color_rgb_function() {
    // Test parsing rgb() function through main parse function
    assert_eq!(
      ColorInput::from_str("rgb(255, 0, 153)"),
      Ok(ColorInput::<true>::Value(Color([255, 0, 153, 255])))
    );
  }

  #[test]
  fn test_parse_color_rgba_function() {
    // Test parsing rgba() function through main parse function
    assert_eq!(
      ColorInput::from_str("rgba(255, 0, 153, 0.5)"),
      Ok(ColorInput::<true>::Value(Color([255, 0, 153, 128])))
    );
  }

  #[test]
  fn test_parse_color_rgb_space_separated() {
    // Test parsing rgb() function with space-separated values
    assert_eq!(
      ColorInput::from_str("rgb(255 0 153)"),
      Ok(ColorInput::<true>::Value(Color([255, 0, 153, 255])))
    );
  }

  #[test]
  fn test_parse_color_rgb_with_alpha_slash() {
    // Test parsing rgb() function with alpha value using slash
    assert_eq!(
      ColorInput::from_str("rgb(255 0 153 / 0.5)"),
      Ok(ColorInput::<true>::Value(Color([255, 0, 153, 128])))
    );
  }

  #[test]
  fn test_parse_named_color_grey() {
    assert_eq!(
      ColorInput::from_str("grey"),
      Ok(ColorInput::<true>::Value(Color([128, 128, 128, 255])))
    );
  }

  #[test]
  fn test_parse_color_invalid_function() {
    // Test parsing invalid function
    assert!(ColorInput::<true>::from_str("invalid(255, 0, 153)").is_err());
  }

  #[test]
  fn test_parse_arbitrary_color_from_str() {
    // Test that ColorInput::from_str can parse arbitrary color names like deepskyblue
    assert_eq!(
      ColorInput::from_str("deepskyblue"),
      Ok(ColorInput::<true>::Value(Color([0, 191, 255, 255])))
    );
  }
}

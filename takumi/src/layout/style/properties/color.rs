use std::fmt::Display;

use csscolorparser::{NAMED_COLORS, ParseColorError};
use cssparser::{Parser, ToCss, Token, match_ignore_ascii_case};
use image::Rgba;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

/// `Color` proxy type for deserializing CSS color values.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(untagged)]
pub(crate) enum ColorInputValue {
  /// RGB color with 8-bit components
  Rgb(u8, u8, u8),
  /// RGBA color with 8-bit RGB components and 32-bit float alpha (alpha is between 0.0 and 1.0)
  Rgba(u8, u8, u8, f32),
  /// Single 32-bit integer containing RGB values
  RgbInt(u32),
  /// CSS color string
  #[ts(type = "\"currentColor\" | string")]
  Css(String),
}

/// Represents a color with 8-bit RGBA components.
#[derive(Debug, Clone, PartialEq, Serialize, TS, Copy)]
pub struct Color(pub [u8; 4]);

/// Represents a color input value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Copy)]
#[serde(try_from = "ColorInputValue")]
#[ts(as = "ColorInputValue")]
pub enum ColorInput {
  #[serde(rename = "currentColor")]
  /// Inherit from the `color` value.
  CurrentColor,
  /// A color value.
  #[serde(untagged)]
  Value(Color),
}

impl ColorInput {
  /// Resolves the color input to a color.
  pub fn resolve(self, current_color: Color, opacity: f32) -> Color {
    match self {
      ColorInput::Value(color) => color.with_opacity(opacity),
      ColorInput::CurrentColor => current_color.with_opacity(opacity),
    }
  }
}

impl TailwindPropertyParser for ColorInput {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("current") {
      return Some(ColorInput::CurrentColor);
    }

    Color::parse_tw(token).map(ColorInput::Value)
  }
}

impl TailwindPropertyParser for Color {
  fn parse_tw(token: &str) -> Option<Self> {
    // handle opacity text like `text-red-50/30`
    if let Some((color, opacity)) = token.split_once('/') {
      let color = Color::parse_tw(color)?;
      let opacity = opacity.parse::<f32>().ok()? / 100.0;

      return Some(color.with_opacity(opacity));
    }

    match_ignore_ascii_case! {token,
      "transparent" => Some(Color::transparent()),
      "black" => Some(Color::black()),
      "white" => Some(Color::white()),
      "slate-50" => Some(Color::from_rgb(0xf8fafc)),
      "slate-100" => Some(Color::from_rgb(0xf1f5f9)),
      "slate-200" => Some(Color::from_rgb(0xe2e8f0)),
      "slate-300" => Some(Color::from_rgb(0xcbd5e1)),
      "slate-400" => Some(Color::from_rgb(0x94a3b8)),
      "slate-500" => Some(Color::from_rgb(0x64748b)),
      "slate-600" => Some(Color::from_rgb(0x475569)),
      "slate-700" => Some(Color::from_rgb(0x334155)),
      "slate-800" => Some(Color::from_rgb(0x1e293b)),
      "slate-900" => Some(Color::from_rgb(0x0f172a)),
      "slate-950" => Some(Color::from_rgb(0x020617)),
      "gray-50" => Some(Color::from_rgb(0xf9fafb)),
      "gray-100" => Some(Color::from_rgb(0xf3f4f6)),
      "gray-200" => Some(Color::from_rgb(0xe5e7eb)),
      "gray-300" => Some(Color::from_rgb(0xd1d5db)),
      "gray-400" => Some(Color::from_rgb(0x9ca3af)),
      "gray-500" => Some(Color::from_rgb(0x6b7280)),
      "gray-600" => Some(Color::from_rgb(0x4b5563)),
      "gray-700" => Some(Color::from_rgb(0x374151)),
      "gray-800" => Some(Color::from_rgb(0x1f2937)),
      "gray-900" => Some(Color::from_rgb(0x111827)),
      "gray-950" => Some(Color::from_rgb(0x030712)),
      "zinc-50" => Some(Color::from_rgb(0xfafafa)),
      "zinc-100" => Some(Color::from_rgb(0xf4f4f5)),
      "zinc-200" => Some(Color::from_rgb(0xe4e4e7)),
      "zinc-300" => Some(Color::from_rgb(0xd4d4d8)),
      "zinc-400" => Some(Color::from_rgb(0xa1a1aa)),
      "zinc-500" => Some(Color::from_rgb(0x71717a)),
      "zinc-600" => Some(Color::from_rgb(0x52525b)),
      "zinc-700" => Some(Color::from_rgb(0x3f3f46)),
      "zinc-800" => Some(Color::from_rgb(0x27272a)),
      "zinc-900" => Some(Color::from_rgb(0x18181b)),
      "zinc-950" => Some(Color::from_rgb(0x09090b)),
      "neutral-50" => Some(Color::from_rgb(0xfafafa)),
      "neutral-100" => Some(Color::from_rgb(0xf5f5f5)),
      "neutral-200" => Some(Color::from_rgb(0xe5e5e5)),
      "neutral-300" => Some(Color::from_rgb(0xd4d4d4)),
      "neutral-400" => Some(Color::from_rgb(0xa3a3a3)),
      "neutral-500" => Some(Color::from_rgb(0x737373)),
      "neutral-600" => Some(Color::from_rgb(0x525252)),
      "neutral-700" => Some(Color::from_rgb(0x404040)),
      "neutral-800" => Some(Color::from_rgb(0x262626)),
      "neutral-900" => Some(Color::from_rgb(0x171717)),
      "neutral-950" => Some(Color::from_rgb(0x0a0a0a)),
      "stone-50" => Some(Color::from_rgb(0xfafaf9)),
      "stone-100" => Some(Color::from_rgb(0xf5f5f4)),
      "stone-200" => Some(Color::from_rgb(0xe7e5e4)),
      "stone-300" => Some(Color::from_rgb(0xd6d3d1)),
      "stone-400" => Some(Color::from_rgb(0xa8a29e)),
      "stone-500" => Some(Color::from_rgb(0x78716c)),
      "stone-600" => Some(Color::from_rgb(0x57534e)),
      "stone-700" => Some(Color::from_rgb(0x44403c)),
      "stone-800" => Some(Color::from_rgb(0x292524)),
      "stone-900" => Some(Color::from_rgb(0x1c1917)),
      "stone-950" => Some(Color::from_rgb(0x0c0a09)),
      "red-50" => Some(Color::from_rgb(0xfef2f2)),
      "red-100" => Some(Color::from_rgb(0xfee2e2)),
      "red-200" => Some(Color::from_rgb(0xfecaca)),
      "red-300" => Some(Color::from_rgb(0xfca5a5)),
      "red-400" => Some(Color::from_rgb(0xf87171)),
      "red-500" => Some(Color::from_rgb(0xef4444)),
      "red-600" => Some(Color::from_rgb(0xdc2626)),
      "red-700" => Some(Color::from_rgb(0xb91c1c)),
      "red-800" => Some(Color::from_rgb(0x991b1b)),
      "red-900" => Some(Color::from_rgb(0x7f1d1d)),
      "red-950" => Some(Color::from_rgb(0x450a0a)),
      "orange-50" => Some(Color::from_rgb(0xfff7ed)),
      "orange-100" => Some(Color::from_rgb(0xffedd5)),
      "orange-200" => Some(Color::from_rgb(0xfed7aa)),
      "orange-300" => Some(Color::from_rgb(0xfdba74)),
      "orange-400" => Some(Color::from_rgb(0xfb923c)),
      "orange-500" => Some(Color::from_rgb(0xf97316)),
      "orange-600" => Some(Color::from_rgb(0xea580c)),
      "orange-700" => Some(Color::from_rgb(0xc2410c)),
      "orange-800" => Some(Color::from_rgb(0x9a3412)),
      "orange-900" => Some(Color::from_rgb(0x7c2d12)),
      "orange-950" => Some(Color::from_rgb(0x431407)),
      "amber-50" => Some(Color::from_rgb(0xfffbeb)),
      "amber-100" => Some(Color::from_rgb(0xfef3c7)),
      "amber-200" => Some(Color::from_rgb(0xfde68a)),
      "amber-300" => Some(Color::from_rgb(0xfcd34d)),
      "amber-400" => Some(Color::from_rgb(0xfbbf24)),
      "amber-500" => Some(Color::from_rgb(0xf59e0b)),
      "amber-600" => Some(Color::from_rgb(0xd97706)),
      "amber-700" => Some(Color::from_rgb(0xb45309)),
      "amber-800" => Some(Color::from_rgb(0x92400e)),
      "amber-900" => Some(Color::from_rgb(0x78350f)),
      "amber-950" => Some(Color::from_rgb(0x451a03)),
      "yellow-50" => Some(Color::from_rgb(0xfefce8)),
      "yellow-100" => Some(Color::from_rgb(0xfef9c3)),
      "yellow-200" => Some(Color::from_rgb(0xfef08a)),
      "yellow-300" => Some(Color::from_rgb(0xfde047)),
      "yellow-400" => Some(Color::from_rgb(0xfacc15)),
      "yellow-500" => Some(Color::from_rgb(0xeab308)),
      "yellow-600" => Some(Color::from_rgb(0xca8a04)),
      "yellow-700" => Some(Color::from_rgb(0xa16207)),
      "yellow-800" => Some(Color::from_rgb(0x854d0e)),
      "yellow-900" => Some(Color::from_rgb(0x713f12)),
      "yellow-950" => Some(Color::from_rgb(0x422006)),
      "lime-50" => Some(Color::from_rgb(0xf7fee7)),
      "lime-100" => Some(Color::from_rgb(0xecfccb)),
      "lime-200" => Some(Color::from_rgb(0xd9f99d)),
      "lime-300" => Some(Color::from_rgb(0xbef264)),
      "lime-400" => Some(Color::from_rgb(0xa3e635)),
      "lime-500" => Some(Color::from_rgb(0x84cc16)),
      "lime-600" => Some(Color::from_rgb(0x65a30d)),
      "lime-700" => Some(Color::from_rgb(0x4d7c0f)),
      "lime-800" => Some(Color::from_rgb(0x3f6212)),
      "lime-900" => Some(Color::from_rgb(0x365314)),
      "lime-950" => Some(Color::from_rgb(0x1a2e05)),
      "green-50" => Some(Color::from_rgb(0xf0fdf4)),
      "green-100" => Some(Color::from_rgb(0xdcfce7)),
      "green-200" => Some(Color::from_rgb(0xbbf7d0)),
      "green-300" => Some(Color::from_rgb(0x86efac)),
      "green-400" => Some(Color::from_rgb(0x4ade80)),
      "green-500" => Some(Color::from_rgb(0x22c55e)),
      "green-600" => Some(Color::from_rgb(0x16a34a)),
      "green-700" => Some(Color::from_rgb(0x15803d)),
      "green-800" => Some(Color::from_rgb(0x166534)),
      "green-900" => Some(Color::from_rgb(0x14532d)),
      "green-950" => Some(Color::from_rgb(0x052e16)),
      "emerald-50" => Some(Color::from_rgb(0xecfdf5)),
      "emerald-100" => Some(Color::from_rgb(0xd1fae5)),
      "emerald-200" => Some(Color::from_rgb(0xa7f3d0)),
      "emerald-300" => Some(Color::from_rgb(0x6ee7b7)),
      "emerald-400" => Some(Color::from_rgb(0x34d399)),
      "emerald-500" => Some(Color::from_rgb(0x10b981)),
      "emerald-600" => Some(Color::from_rgb(0x059669)),
      "emerald-700" => Some(Color::from_rgb(0x047857)),
      "emerald-800" => Some(Color::from_rgb(0x065f46)),
      "emerald-900" => Some(Color::from_rgb(0x064e3b)),
      "emerald-950" => Some(Color::from_rgb(0x052c22)),
      "teal-50" => Some(Color::from_rgb(0xf0fdfa)),
      "teal-100" => Some(Color::from_rgb(0xccfbf1)),
      "teal-200" => Some(Color::from_rgb(0x99f6e4)),
      "teal-300" => Some(Color::from_rgb(0x5eead4)),
      "teal-400" => Some(Color::from_rgb(0x2dd4bf)),
      "teal-500" => Some(Color::from_rgb(0x14b8a6)),
      "teal-600" => Some(Color::from_rgb(0x0d9488)),
      "teal-700" => Some(Color::from_rgb(0x0f766e)),
      "teal-800" => Some(Color::from_rgb(0x115e59)),
      "teal-900" => Some(Color::from_rgb(0x134e4a)),
      "teal-950" => Some(Color::from_rgb(0x042f2e)),
      "cyan-50" => Some(Color::from_rgb(0xecfeff)),
      "cyan-100" => Some(Color::from_rgb(0xcffafe)),
      "cyan-200" => Some(Color::from_rgb(0xa5f3fc)),
      "cyan-300" => Some(Color::from_rgb(0x67e8f9)),
      "cyan-400" => Some(Color::from_rgb(0x22d3ee)),
      "cyan-500" => Some(Color::from_rgb(0x06b6d4)),
      "cyan-600" => Some(Color::from_rgb(0x0891b2)),
      "cyan-700" => Some(Color::from_rgb(0x0e7490)),
      "cyan-800" => Some(Color::from_rgb(0x155e75)),
      "cyan-900" => Some(Color::from_rgb(0x164e63)),
      "cyan-950" => Some(Color::from_rgb(0x083344)),
      "sky-50" => Some(Color::from_rgb(0xf0f9ff)),
      "sky-100" => Some(Color::from_rgb(0xe0f2fe)),
      "sky-200" => Some(Color::from_rgb(0xbae6fd)),
      "sky-300" => Some(Color::from_rgb(0x7dd3fc)),
      "sky-400" => Some(Color::from_rgb(0x38bdf8)),
      "sky-500" => Some(Color::from_rgb(0x0ea5e9)),
      "sky-600" => Some(Color::from_rgb(0x0284c7)),
      "sky-700" => Some(Color::from_rgb(0x0369a1)),
      "sky-800" => Some(Color::from_rgb(0x075985)),
      "sky-900" => Some(Color::from_rgb(0x0c4a6e)),
      "sky-950" => Some(Color::from_rgb(0x082f49)),
      "blue-50" => Some(Color::from_rgb(0xeff6ff)),
      "blue-100" => Some(Color::from_rgb(0xdbeafe)),
      "blue-200" => Some(Color::from_rgb(0xbfdbfe)),
      "blue-300" => Some(Color::from_rgb(0x93c5fd)),
      "blue-400" => Some(Color::from_rgb(0x60a5fa)),
      "blue-500" => Some(Color::from_rgb(0x3b82f6)),
      "blue-600" => Some(Color::from_rgb(0x2563eb)),
      "blue-700" => Some(Color::from_rgb(0x1d4ed8)),
      "blue-800" => Some(Color::from_rgb(0x1e40af)),
      "blue-900" => Some(Color::from_rgb(0x1e3a8a)),
      "blue-950" => Some(Color::from_rgb(0x172554)),
      "indigo-50" => Some(Color::from_rgb(0xeef2ff)),
      "indigo-100" => Some(Color::from_rgb(0xe0e7ff)),
      "indigo-200" => Some(Color::from_rgb(0xc7d2fe)),
      "indigo-300" => Some(Color::from_rgb(0xa5b4fc)),
      "indigo-400" => Some(Color::from_rgb(0x818cf8)),
      "indigo-500" => Some(Color::from_rgb(0x6366f1)),
      "indigo-600" => Some(Color::from_rgb(0x4f46e5)),
      "indigo-700" => Some(Color::from_rgb(0x4338ca)),
      "indigo-800" => Some(Color::from_rgb(0x3730a3)),
      "indigo-900" => Some(Color::from_rgb(0x312e81)),
      "indigo-950" => Some(Color::from_rgb(0x1e1b4b)),
      "violet-50" => Some(Color::from_rgb(0xf5f3ff)),
      "violet-100" => Some(Color::from_rgb(0xede9fe)),
      "violet-200" => Some(Color::from_rgb(0xddd6fe)),
      "violet-300" => Some(Color::from_rgb(0xc4b5fd)),
      "violet-400" => Some(Color::from_rgb(0xa78bfa)),
      "violet-500" => Some(Color::from_rgb(0x8b5cf6)),
      "violet-600" => Some(Color::from_rgb(0x7c3aed)),
      "violet-700" => Some(Color::from_rgb(0x6d28d9)),
      "violet-800" => Some(Color::from_rgb(0x5b21b6)),
      "violet-900" => Some(Color::from_rgb(0x4c1d95)),
      "violet-950" => Some(Color::from_rgb(0x2e1065)),
      "purple-50" => Some(Color::from_rgb(0xfaf5ff)),
      "purple-100" => Some(Color::from_rgb(0xf3e8ff)),
      "purple-200" => Some(Color::from_rgb(0xe9d5ff)),
      "purple-300" => Some(Color::from_rgb(0xd8b4fe)),
      "purple-400" => Some(Color::from_rgb(0xc084fc)),
      "purple-500" => Some(Color::from_rgb(0xa855f7)),
      "purple-600" => Some(Color::from_rgb(0x9333ea)),
      "purple-700" => Some(Color::from_rgb(0x7e22ce)),
      "purple-800" => Some(Color::from_rgb(0x6b21a8)),
      "purple-900" => Some(Color::from_rgb(0x581c87)),
      "purple-950" => Some(Color::from_rgb(0x3b0764)),
      "fuchsia-50" => Some(Color::from_rgb(0xfdf4ff)),
      "fuchsia-100" => Some(Color::from_rgb(0xfae8ff)),
      "fuchsia-200" => Some(Color::from_rgb(0xf5d0fe)),
      "fuchsia-300" => Some(Color::from_rgb(0xf0abfc)),
      "fuchsia-400" => Some(Color::from_rgb(0xe879f9)),
      "fuchsia-500" => Some(Color::from_rgb(0xd946ef)),
      "fuchsia-600" => Some(Color::from_rgb(0xc026d3)),
      "fuchsia-700" => Some(Color::from_rgb(0xa21caf)),
      "fuchsia-800" => Some(Color::from_rgb(0x86198f)),
      "fuchsia-900" => Some(Color::from_rgb(0x701a75)),
      "fuchsia-950" => Some(Color::from_rgb(0x4a044e)),
      "pink-50" => Some(Color::from_rgb(0xfdf2f8)),
      "pink-100" => Some(Color::from_rgb(0xfce7f3)),
      "pink-200" => Some(Color::from_rgb(0xfbcfe8)),
      "pink-300" => Some(Color::from_rgb(0xf9a8d4)),
      "pink-400" => Some(Color::from_rgb(0xf472b6)),
      "pink-500" => Some(Color::from_rgb(0xec4899)),
      "pink-600" => Some(Color::from_rgb(0xdb2777)),
      "pink-700" => Some(Color::from_rgb(0xbe185d)),
      "pink-800" => Some(Color::from_rgb(0x9d174d)),
      "pink-900" => Some(Color::from_rgb(0x831843)),
      "pink-950" => Some(Color::from_rgb(0x500724)),
      "rose-50" => Some(Color::from_rgb(0xfff1f2)),
      "rose-100" => Some(Color::from_rgb(0xffe4e6)),
      "rose-200" => Some(Color::from_rgb(0xfecdd3)),
      "rose-300" => Some(Color::from_rgb(0xfda4af)),
      "rose-400" => Some(Color::from_rgb(0xfb7185)),
      "rose-500" => Some(Color::from_rgb(0xf43f5e)),
      "rose-600" => Some(Color::from_rgb(0xe11d48)),
      "rose-700" => Some(Color::from_rgb(0xbe123c)),
      "rose-800" => Some(Color::from_rgb(0x9f1239)),
      "rose-900" => Some(Color::from_rgb(0x881337)),
      "rose-950" => Some(Color::from_rgb(0x4c0519)),
      _ => None,
    }
  }
}

impl From<Color> for ColorInput {
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

impl Default for Color {
  fn default() -> Self {
    Self::transparent()
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
  pub fn with_opacity(mut self, opacity: f32) -> Self {
    self.0[3] = ((self.0[3] as f32) * opacity).round() as u8;

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

impl TryFrom<ColorInputValue> for ColorInput {
  type Error = String;

  fn try_from(value: ColorInputValue) -> Result<Self, Self::Error> {
    match value {
      ColorInputValue::Rgb(r, g, b) => Ok(ColorInput::Value(Color([r, g, b, 255]))),
      ColorInputValue::Rgba(r, g, b, a) => {
        Ok(ColorInput::Value(Color([r, g, b, (a * 255.0) as u8])))
      }
      ColorInputValue::RgbInt(rgb) => {
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let b = (rgb & 0xFF) as u8;

        Ok(ColorInput::Value(Color([r, g, b, 255])))
      }
      ColorInputValue::Css(css) => ColorInput::from_str(&css).map_err(|e| e.to_string()),
    }
  }
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
}

impl<'i> FromCss<'i> for Color {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let position = input.position();
    let token = input.next()?;

    match *token {
      Token::Hash(_) | Token::IDHash(_) => {
        parse_color_string(&token.to_css_string()).map_err(|_| {
          location
            .new_basic_unexpected_token_error(token.clone())
            .into()
        })
      }
      Token::Ident(ref ident) => {
        if ident.eq_ignore_ascii_case("transparent") {
          return Ok(Color::transparent());
        }

        let Some([r, g, b]) = NAMED_COLORS.get(ident) else {
          return Err(
            location
              .new_basic_unexpected_token_error(token.clone())
              .into(),
          );
        };

        Ok(Color([*r, *g, *b, 255]))
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

          parse_color_string(&function)
            .map_err(|_| location.new_basic_unexpected_token_error(token).into())
        })
      }
      _ => Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      ),
    }
  }
}

fn parse_color_string(string: &str) -> Result<Color, ParseColorError> {
  csscolorparser::parse(string).map(|color| Color(color.to_rgba8()))
}

#[cfg(test)]
mod tests {
  use super::*;
  use cssparser::{Parser, ParserInput};

  fn parse_color_str(input: &str) -> ParseResult<'_, ColorInput> {
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);

    ColorInput::from_css(&mut parser)
  }

  #[test]
  fn test_parse_hex_color_3_digits() {
    // Test 3-digit hex color
    let result = parse_color_str("#f09").unwrap();
    assert_eq!(result, ColorInput::Value(Color([255, 0, 153, 255])));
  }

  #[test]
  fn test_parse_hex_color_6_digits() {
    // Test 6-digit hex color
    let result = parse_color_str("#ff0099").unwrap();
    assert_eq!(result, ColorInput::Value(Color([255, 0, 153, 255])));
  }

  #[test]
  fn test_parse_color_transparent() {
    // Test parsing transparent keyword
    let result = parse_color_str("transparent").unwrap();
    assert_eq!(result, ColorInput::Value(Color([0, 0, 0, 0])));
  }

  #[test]
  fn test_parse_color_rgb_function() {
    // Test parsing rgb() function through main parse function
    let result = parse_color_str("rgb(255, 0, 153)").unwrap();
    assert_eq!(result, ColorInput::Value(Color([255, 0, 153, 255])));
  }

  #[test]
  fn test_parse_color_rgba_function() {
    // Test parsing rgba() function through main parse function
    let result = parse_color_str("rgba(255, 0, 153, 0.5)").unwrap();
    assert_eq!(result, ColorInput::Value(Color([255, 0, 153, 128])));
  }

  #[test]
  fn test_parse_color_rgb_space_separated() {
    // Test parsing rgb() function with space-separated values
    let result = parse_color_str("rgb(255 0 153)").unwrap();
    assert_eq!(result, ColorInput::Value(Color([255, 0, 153, 255])));
  }

  #[test]
  fn test_parse_color_rgb_with_alpha_slash() {
    // Test parsing rgb() function with alpha value using slash
    let result = parse_color_str("rgb(255 0 153 / 0.5)").unwrap();
    assert_eq!(result, ColorInput::Value(Color([255, 0, 153, 128])));
  }

  #[test]
  fn test_parse_named_color_grey() {
    let result = parse_color_str("grey").unwrap();
    assert_eq!(result, ColorInput::Value(Color([128, 128, 128, 255])));
  }

  #[test]
  fn test_parse_color_invalid_function() {
    // Test parsing invalid function
    let result = parse_color_str("invalid(255, 0, 153)");
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_arbitrary_color_from_str() {
    // Test that ColorInput::from_str can parse arbitrary color names like deepskyblue
    let result = ColorInput::from_str("deepskyblue").unwrap();
    match result {
      ColorInput::Value(color) => {
        // deepskyblue is rgb(0, 191, 255)
        assert_eq!(color.0[0], 0); // red
        assert_eq!(color.0[1], 191); // green
        assert_eq!(color.0[2], 255); // blue
        assert_eq!(color.0[3], 255); // alpha
      }
      _ => panic!("Expected ColorInput::Value"),
    }
  }
}

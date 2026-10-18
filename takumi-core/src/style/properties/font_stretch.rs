use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, Color, FromCss, MakeComputed, ParseResult, SizingContext, ToCss, lerp,
  tw::TailwindPropertyParser, unexpected_token,
};

/// Controls the width/stretch of text rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontStretch(f32);

impl Default for FontStretch {
  fn default() -> Self {
    Self::NORMAL
  }
}

impl MakeComputed for FontStretch {}

impl Animatable for FontStretch {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = FontStretch::from_percentage(lerp(from.percentage(), to.percentage(), progress));
  }
}

impl<'i> FromCss<'i> for FontStretch {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();

    if let Ok(value) = input.try_parse(Parser::expect_percentage) {
      return Ok(Self::from_percentage(value));
    }

    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { ident,
      "normal" => Ok(Self::NORMAL),
      "ultra-condensed" => Ok(Self::ULTRA_CONDENSED),
      "extra-condensed" => Ok(Self::EXTRA_CONDENSED),
      "condensed" => Ok(Self::CONDENSED),
      "semi-condensed" => Ok(Self::SEMI_CONDENSED),
      "semi-expanded" => Ok(Self::SEMI_EXPANDED),
      "expanded" => Ok(Self::EXPANDED),
      "extra-expanded" => Ok(Self::EXTRA_EXPANDED),
      "ultra-expanded" => Ok(Self::ULTRA_EXPANDED),
      _ => Err(unexpected_token!(location, &Token::Ident(ident.to_owned()))),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &[
    "normal",
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
    "<percentage>",
  ];
}

impl TailwindPropertyParser for FontStretch {}

impl FontStretch {
  const ULTRA_CONDENSED: Self = Self(0.5);
  const EXTRA_CONDENSED: Self = Self(0.625);
  const CONDENSED: Self = Self(0.75);
  const SEMI_CONDENSED: Self = Self(0.875);
  const NORMAL: Self = Self(1.0);
  const SEMI_EXPANDED: Self = Self(1.125);
  const EXPANDED: Self = Self(1.25);
  const EXTRA_EXPANDED: Self = Self(1.5);
  const ULTRA_EXPANDED: Self = Self(2.0);

  /// The stretch as a fraction where `1.0` is normal.
  pub fn percentage(self) -> f32 {
    self.0
  }

  /// Builds from a fraction where `1.0` is normal.
  pub(crate) fn from_percentage(value: f32) -> Self {
    Self(value.max(0.0))
  }
}

impl ToCss for FontStretch {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match *self {
      Self::ULTRA_CONDENSED => dest.write_str("ultra-condensed"),
      Self::EXTRA_CONDENSED => dest.write_str("extra-condensed"),
      Self::CONDENSED => dest.write_str("condensed"),
      Self::SEMI_CONDENSED => dest.write_str("semi-condensed"),
      Self::NORMAL => dest.write_str("normal"),
      Self::SEMI_EXPANDED => dest.write_str("semi-expanded"),
      Self::EXPANDED => dest.write_str("expanded"),
      Self::EXTRA_EXPANDED => dest.write_str("extra-expanded"),
      Self::ULTRA_EXPANDED => dest.write_str("ultra-expanded"),
      _ => write!(dest, "{percentage}%", percentage = self.0 * 100.0),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_font_stretch_keywords() {
    assert_eq!(
      FontStretch::from_css_str("condensed"),
      Ok(FontStretch::CONDENSED)
    );
    assert_eq!(
      FontStretch::from_css_str("expanded"),
      Ok(FontStretch::EXPANDED)
    );
    assert_eq!(FontStretch::from_css_str("normal"), Ok(FontStretch::NORMAL));
  }

  #[test]
  fn test_parse_font_stretch_percentage() {
    assert_eq!(FontStretch::from_css_str("75%"), Ok(FontStretch::CONDENSED));
  }

  #[test]
  fn test_tailwind_parser() {
    assert_eq!(
      FontStretch::parse_tw("condensed"),
      Some(FontStretch::CONDENSED)
    );
    assert_eq!(
      FontStretch::parse_tw("ultra-expanded"),
      Some(FontStretch::ULTRA_EXPANDED)
    );
    assert_eq!(FontStretch::parse_tw("invalid"), None);
  }

  #[test]
  fn test_tailwind_parser_percentage() {
    assert_eq!(FontStretch::parse_tw("75%"), Some(FontStretch::CONDENSED));
    assert_eq!(
      FontStretch::parse_tw("150%"),
      Some(FontStretch::EXTRA_EXPANDED)
    );
  }
}

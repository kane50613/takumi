use std::fmt;

use crate::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token, match_ignore_ascii_case};
use parley::FontWidth;

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, FromCssStr, MakeComputed, ParseResult,
  SizingContext, lerp, tw::TailwindPropertyParser,
};

/// Controls the width/stretch of text rendering.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct FontStretch(FontWidth);

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
      return Ok(Self(FontWidth::from_percentage(value.max(0.0) * 100.0)));
    }

    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { ident,
      "normal" => Ok(Self(FontWidth::NORMAL)),
      "ultra-condensed" => Ok(Self(FontWidth::ULTRA_CONDENSED)),
      "extra-condensed" => Ok(Self(FontWidth::EXTRA_CONDENSED)),
      "condensed" => Ok(Self(FontWidth::CONDENSED)),
      "semi-condensed" => Ok(Self(FontWidth::SEMI_CONDENSED)),
      "semi-expanded" => Ok(Self(FontWidth::SEMI_EXPANDED)),
      "expanded" => Ok(Self(FontWidth::EXPANDED)),
      "extra-expanded" => Ok(Self(FontWidth::EXTRA_EXPANDED)),
      "ultra-expanded" => Ok(Self(FontWidth::ULTRA_EXPANDED)),
      _ => Err(unexpected_token!(location, &Token::Ident(ident.to_owned()))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("ultra-condensed"),
    CssToken::Keyword("extra-condensed"),
    CssToken::Keyword("condensed"),
    CssToken::Keyword("semi-condensed"),
    CssToken::Keyword("semi-expanded"),
    CssToken::Keyword("expanded"),
    CssToken::Keyword("extra-expanded"),
    CssToken::Keyword("ultra-expanded"),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

impl TailwindPropertyParser for FontStretch {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_css_str(token).ok()
  }
}

impl FontStretch {
  /// The stretch as a fraction where `1.0` is normal.
  pub fn percentage(self) -> f32 {
    self.0.percentage() / 100.0
  }

  /// Builds from a fraction where `1.0` is normal.
  pub fn from_percentage(value: f32) -> Self {
    Self(FontWidth::from_percentage(value.max(0.0) * 100.0))
  }
}

impl From<FontStretch> for FontWidth {
  fn from(value: FontStretch) -> Self {
    value.0
  }
}

impl ToCss for FontStretch {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self.0 {
      FontWidth::ULTRA_CONDENSED => dest.write_str("ultra-condensed"),
      FontWidth::EXTRA_CONDENSED => dest.write_str("extra-condensed"),
      FontWidth::CONDENSED => dest.write_str("condensed"),
      FontWidth::SEMI_CONDENSED => dest.write_str("semi-condensed"),
      FontWidth::NORMAL => dest.write_str("normal"),
      FontWidth::SEMI_EXPANDED => dest.write_str("semi-expanded"),
      FontWidth::EXPANDED => dest.write_str("expanded"),
      FontWidth::EXTRA_EXPANDED => dest.write_str("extra-expanded"),
      FontWidth::ULTRA_EXPANDED => dest.write_str("ultra-expanded"),
      _ => write!(dest, "{}%", self.0.percentage()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_font_stretch_keywords() {
    assert_eq!(
      FontStretch::from_css_str("condensed"),
      Ok(FontStretch(FontWidth::CONDENSED))
    );
    assert_eq!(
      FontStretch::from_css_str("expanded"),
      Ok(FontStretch(FontWidth::EXPANDED))
    );
    assert_eq!(
      FontStretch::from_css_str("normal"),
      Ok(FontStretch(FontWidth::NORMAL))
    );
  }

  #[test]
  fn test_parse_font_stretch_percentage() {
    assert_eq!(
      FontStretch::from_css_str("75%"),
      Ok(FontStretch(FontWidth::CONDENSED))
    );
  }

  #[test]
  fn test_tailwind_parser() {
    assert_eq!(
      FontStretch::parse_tw("condensed"),
      Some(FontStretch(FontWidth::CONDENSED))
    );
    assert_eq!(
      FontStretch::parse_tw("ultra-expanded"),
      Some(FontStretch(FontWidth::ULTRA_EXPANDED))
    );
    assert_eq!(FontStretch::parse_tw("invalid"), None);
  }

  #[test]
  fn test_tailwind_parser_percentage() {
    assert_eq!(
      FontStretch::parse_tw("75%"),
      Some(FontStretch(FontWidth::CONDENSED))
    );
    assert_eq!(
      FontStretch::parse_tw("150%"),
      Some(FontStretch(FontWidth::EXTRA_EXPANDED))
    );
  }
}

use std::fmt;

use cssparser::{ParseError as CssParseError, Parser, Token, match_ignore_ascii_case};

use crate::style::{Angle, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token};

/// Controls the slant (italic/oblique) of text rendering.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct FontStyle(Slant);

#[derive(Default, Debug, Clone, Copy, PartialEq)]
enum Slant {
  #[default]
  Normal,
  Italic,
  /// An angle in degrees, `None` for the keyword alone.
  Oblique(Option<f32>),
}

/// The angle `oblique` and `italic` request, as CSS Fonts 4 defines it.
const DEFAULT_OBLIQUE_ANGLE: f32 = 14.0;

/// The steepest angle `oblique` takes. <https://drafts.csswg.org/css-fonts-4/#valdef-font-style-oblique-angle--90deg-90deg>
const MAX_OBLIQUE_ANGLE: f32 = 90.0;

impl MakeComputed for FontStyle {}

/// An `oblique` angle in degrees, signed where `Angle` keeps a turn in [0, 360), and within
/// ±90deg.
pub(crate) fn oblique_angle<'i>(input: &mut Parser<'i>) -> ParseResult<'i, f32> {
  let start = input.state();
  let angle = *Angle::from_css(input)?;
  let angle = if angle > 180.0 { angle - 360.0 } else { angle };

  if (-MAX_OBLIQUE_ANGLE..=MAX_OBLIQUE_ANGLE).contains(&angle) {
    return Ok(angle);
  }

  input.reset(&start);

  input.next()?;

  Err(CssParseError::unexpected_token())
}

impl<'i> FromCss<'i> for FontStyle {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "normal" => Ok(Self(Slant::Normal)),
      "italic" => Ok(Self(Slant::Italic)),
      "oblique" => Ok(Self(Slant::Oblique(input.try_parse(oblique_angle).ok()))),
      _ => Err(unexpected_token!(&Token::Ident(ident.to_owned()))),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["normal", "italic", "oblique"];
}

impl FontStyle {
  /// The normal font style.
  pub const fn normal() -> Self {
    Self(Slant::Normal)
  }

  /// The italic font style.
  pub const fn italic() -> Self {
    Self(Slant::Italic)
  }

  /// The oblique font style with a given angle.
  pub const fn oblique(angle: f32) -> Self {
    Self(Slant::Oblique(Some(angle)))
  }

  /// Whether this is the `italic` keyword, Blink's `StyleSyntax::kItalicKeyword`.
  pub(crate) fn is_italic(self) -> bool {
    self.0 == Slant::Italic
  }

  /// The slope font matching requests, in degrees clockwise.
  pub(crate) fn slope(self) -> f32 {
    match self.0 {
      Slant::Normal => 0.0,
      Slant::Italic | Slant::Oblique(None) => DEFAULT_OBLIQUE_ANGLE,
      Slant::Oblique(Some(angle)) => angle,
    }
  }
}

impl ToCss for FontStyle {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match &self.0 {
      Slant::Normal => dest.write_str("normal"),
      Slant::Italic => dest.write_str("italic"),
      Slant::Oblique(angle) => match angle {
        Some(a) => write!(dest, "oblique {a}deg"),
        None => dest.write_str("oblique"),
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_font_style() {
    for (css, expected) in [
      ("normal", FontStyle::normal()),
      ("italic", FontStyle::italic()),
      ("oblique", FontStyle(Slant::Oblique(None))),
      ("oblique 10deg", FontStyle::oblique(10.0)),
      ("oblique -14deg", FontStyle::oblique(-14.0)),
    ] {
      assert_eq!(
        FontStyle::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_font_style_round_trip() {
    for css in [
      "normal",
      "italic",
      "oblique",
      "oblique 10deg",
      "oblique -14deg",
    ] {
      let parsed = FontStyle::from_css_str(css).unwrap();
      let reparsed = FontStyle::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_font_style_invalid() {
    assert!(FontStyle::from_css_str("bogus").is_err());
    assert_ne!(
      FontStyle::from_css_str("oblique 100deg"),
      Ok(FontStyle::oblique(100.0))
    );
    assert!(FontStyle::from_css_str("123").is_err());
  }
}

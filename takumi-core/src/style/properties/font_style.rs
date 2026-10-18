use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

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

impl MakeComputed for FontStyle {}

impl<'i> FromCss<'i> for FontStyle {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "normal" => Ok(Self(Slant::Normal)),
      "italic" => Ok(Self(Slant::Italic)),
      "oblique" => {
        let angle = input.try_parse(Angle::from_css).ok().map(|angle| *angle);
        Ok(Self(Slant::Oblique(angle)))
      },
      _ => Err(unexpected_token!(location, &Token::Ident(ident.to_owned()))),
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
    for css in ["normal", "italic", "oblique", "oblique 10deg"] {
      let parsed = FontStyle::from_css_str(css).unwrap();
      let reparsed = FontStyle::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_font_style_invalid() {
    assert!(FontStyle::from_css_str("bogus").is_err());
    assert!(FontStyle::from_css_str("123").is_err());
  }
}

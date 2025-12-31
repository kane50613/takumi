use cssparser::{Parser, Token, match_ignore_ascii_case};
use parley::style::FontStyle as ParleyFontStyle;

use crate::layout::style::{Angle, CssToken, FromCss, ParseResult};

/// Controls the slant (italic/oblique) of text rendering.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct FontStyle(ParleyFontStyle);

impl<'i> FromCss<'i> for FontStyle {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "normal" => Ok(Self(ParleyFontStyle::Normal)),
      "italic" => Ok(Self(ParleyFontStyle::Italic)),
      "oblique" => {
        let angle = input.try_parse(Angle::from_css).ok().map(|angle| *angle);
        Ok(Self(ParleyFontStyle::Oblique(angle)))
      },
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident.to_owned()))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("normal"),
      CssToken::Keyword("italic"),
      CssToken::Keyword("oblique"),
    ]
  }
}

impl FontStyle {
  /// The normal font style.
  pub const fn normal() -> Self {
    Self(ParleyFontStyle::Normal)
  }

  /// The italic font style.
  pub const fn italic() -> Self {
    Self(ParleyFontStyle::Italic)
  }

  /// The oblique font style with a given angle.
  pub const fn oblique(angle: f32) -> Self {
    Self(ParleyFontStyle::Oblique(Some(angle)))
  }
}

impl From<FontStyle> for ParleyFontStyle {
  fn from(value: FontStyle) -> Self {
    value.0
  }
}

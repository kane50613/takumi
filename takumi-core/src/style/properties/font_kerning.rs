use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, CssToken, FontFeature, FromCss, MakeComputed, ParseResult, Tag, ToCss,
  unexpected_token,
};

/// `font-kerning`. The shaper kerns by default, so only `normal`/`none` emit a `kern` tag.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontKerning {
  /// Shaper default (kerning enabled).
  #[default]
  Auto,
  /// Kerning explicitly enabled (`kern` 1).
  Normal,
  /// Kerning disabled (`kern` 0), including the shaper's fallback kerning.
  None,
}

impl FontKerning {
  fn from_keyword(ident: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { ident,
      "auto" => Self::Auto,
      "normal" => Self::Normal,
      "none" => Self::None,
      _ => return None,
    })
  }

  pub(crate) fn append_features(&self, out: &mut Vec<FontFeature>) {
    match self {
      Self::Auto => {}
      Self::Normal => out.push(FontFeature::new(Tag::new(b"kern"), 1)),
      Self::None => out.push(FontFeature::new(Tag::new(b"kern"), 0)),
    }
  }

  fn keyword(&self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::Normal => "normal",
      Self::None => "none",
    }
  }
}

impl MakeComputed for FontKerning {}
impl Animatable for FontKerning {}

impl<'i> FromCss<'i> for FontKerning {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    Self::from_keyword(ident)
      .ok_or_else(|| unexpected_token!(location, &Token::Ident(ident.to_owned())))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Keyword("normal"),
    CssToken::Keyword("none"),
  ];
}

impl ToCss for FontKerning {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(self.keyword())
  }
}

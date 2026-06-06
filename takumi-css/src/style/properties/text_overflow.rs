use std::fmt;

use cssparser::{Parser, match_ignore_ascii_case};

use crate::style::{
  CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss, properties::write_css_string,
};

/// Defines how text should be overflowed.
///
/// This enum determines how text should be handled when it exceeds the container width.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TextOverflow {
  /// Text is simply clipped at the overflow edge with no visual indication
  #[default]
  Clip,
  /// Text is truncated with an ellipsis (…) at the end when it overflows
  Ellipsis,
  /// Text is truncated with a custom string at the end when it overflows
  Custom(String),
}

impl MakeComputed for TextOverflow {}

impl<'i> FromCss<'i> for TextOverflow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let string = input.expect_ident_or_string()?;

    match_ignore_ascii_case! {string,
      "clip" => Ok(TextOverflow::Clip),
      "ellipsis" => Ok(TextOverflow::Ellipsis),
      _ => Ok(TextOverflow::Custom(string.to_string())),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("clip"),
    CssToken::Keyword("ellipsis"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for TextOverflow {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Clip => dest.write_str("clip"),
      Self::Ellipsis => dest.write_str("ellipsis"),
      Self::Custom(s) => write_css_string(dest, s),
    }
  }
}

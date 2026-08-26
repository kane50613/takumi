use std::fmt;

use cssparser::{Parser, match_ignore_ascii_case, serialize_string};

use crate::style::{CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss};

/// Defines how text should be overflowed.
///
/// This enum determines how text should be handled when it exceeds the container width.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
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
      Self::Custom(s) => serialize_string(s, dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_text_overflow() {
    for (css, expected) in [
      ("clip", TextOverflow::Clip),
      ("ellipsis", TextOverflow::Ellipsis),
      ("\"foo\"", TextOverflow::Custom("foo".to_string())),
      ("bar", TextOverflow::Custom("bar".to_string())),
    ] {
      assert_eq!(
        TextOverflow::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_text_overflow_round_trip() {
    for css in ["clip", "ellipsis", "\"foo\"", "bar"] {
      let parsed = TextOverflow::from_css_str(css).unwrap();
      let reparsed = TextOverflow::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_text_overflow_invalid() {
    assert!(TextOverflow::from_css_str("123").is_err());
  }
}

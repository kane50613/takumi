use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  CssToken, FromCss, FromCssStr, MakeComputed, ParseResult, ToCss, tw::TailwindPropertyParser,
  unexpected_token,
};

/// Controls how text should be overflowed.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct OverflowWrap(parley::OverflowWrap);

impl TailwindPropertyParser for OverflowWrap {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_css_str(token).ok()
  }
}

impl<'i> FromCss<'i> for OverflowWrap {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    let Token::Ident(ident) = token else {
      return Err(unexpected_token!(location, token));
    };

    match_ignore_ascii_case! {&ident,
      "normal" => Ok(Self(parley::OverflowWrap::Normal)),
      "anywhere" => Ok(Self(parley::OverflowWrap::Anywhere)),
      "break-word" => Ok(Self(parley::OverflowWrap::BreakWord)),
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("anywhere"),
    CssToken::Keyword("break-word"),
  ];
}

impl MakeComputed for OverflowWrap {}

impl ToCss for OverflowWrap {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self.0 {
      parley::OverflowWrap::Normal => dest.write_str("normal"),
      parley::OverflowWrap::Anywhere => dest.write_str("anywhere"),
      parley::OverflowWrap::BreakWord => dest.write_str("break-word"),
    }
  }
}

impl OverflowWrap {
  pub(crate) fn into_parley(self) -> parley::OverflowWrap {
    self.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_overflow_wrap() {
    for (css, expected) in [
      ("normal", OverflowWrap(parley::OverflowWrap::Normal)),
      ("anywhere", OverflowWrap(parley::OverflowWrap::Anywhere)),
      ("break-word", OverflowWrap(parley::OverflowWrap::BreakWord)),
    ] {
      assert_eq!(
        OverflowWrap::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_overflow_wrap_round_trip() {
    for css in ["normal", "anywhere", "break-word"] {
      let parsed = OverflowWrap::from_css_str(css).unwrap();
      let reparsed = OverflowWrap::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_overflow_wrap_invalid() {
    assert!(OverflowWrap::from_css_str("bogus").is_err());
    assert!(OverflowWrap::from_css_str("123").is_err());
  }
}

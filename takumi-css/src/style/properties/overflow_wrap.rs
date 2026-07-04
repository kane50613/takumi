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

impl From<OverflowWrap> for parley::OverflowWrap {
  fn from(value: OverflowWrap) -> Self {
    value.0
  }
}

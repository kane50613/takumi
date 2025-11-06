use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

/// Controls how text should be overflowed.
#[derive(Debug, Default, Copy, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(from = "OverflowWrapValue", into = "OverflowWrapValue")]
#[ts(as = "OverflowWrapValue")]
pub struct OverflowWrap(parley::OverflowWrap);

impl TailwindPropertyParser for OverflowWrap {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl<'i> FromCss<'i> for OverflowWrap {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "normal" => Ok(OverflowWrap(parley::OverflowWrap::Normal)),
      "anywhere" => Ok(OverflowWrap(parley::OverflowWrap::Anywhere)),
      "break-word" => Ok(OverflowWrap(parley::OverflowWrap::BreakWord)),
      _ => Err(location.new_unexpected_token_error(
        Token::Ident(ident.clone())
      )),
    }
  }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum OverflowWrapValue {
  Normal,
  Anywhere,
  BreakWord,
}

impl From<OverflowWrap> for OverflowWrapValue {
  fn from(value: OverflowWrap) -> Self {
    match value.0 {
      parley::OverflowWrap::Normal => OverflowWrapValue::Normal,
      parley::OverflowWrap::Anywhere => OverflowWrapValue::Anywhere,
      parley::OverflowWrap::BreakWord => OverflowWrapValue::BreakWord,
    }
  }
}

impl From<OverflowWrapValue> for OverflowWrap {
  fn from(value: OverflowWrapValue) -> Self {
    match value {
      OverflowWrapValue::Normal => OverflowWrap(parley::OverflowWrap::Normal),
      OverflowWrapValue::Anywhere => OverflowWrap(parley::OverflowWrap::Anywhere),
      OverflowWrapValue::BreakWord => OverflowWrap(parley::OverflowWrap::BreakWord),
    }
  }
}

impl From<OverflowWrap> for parley::OverflowWrap {
  fn from(value: OverflowWrap) -> Self {
    value.0
  }
}

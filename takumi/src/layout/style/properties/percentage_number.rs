use std::ops::Neg;

use cssparser::{Parser, ParserInput, Token};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{
  properties::{FromCss, ParseResult},
  tw::TailwindPropertyParser,
};

/// Represents a percentage value (0.0-1.0) in CSS parsing.
///
/// This struct wraps an f32 value that represents a percentage
/// where 0.0 corresponds to 0% and 1.0 corresponds to 100%.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(try_from = "PercentageNumberValue")]
#[ts(as = "PercentageNumberValue")]
pub struct PercentageNumber(pub f32);

impl Default for PercentageNumber {
  fn default() -> Self {
    Self(1.0)
  }
}

impl Neg for PercentageNumber {
  type Output = Self;

  fn neg(self) -> Self::Output {
    Self(-self.0)
  }
}

impl TailwindPropertyParser for PercentageNumber {
  fn parse_tw(token: &str) -> Option<Self> {
    let value = token.parse::<f32>().ok()?;

    Some(PercentageNumber(value / 100.0))
  }
}

impl<'i> FromCss<'i> for PercentageNumber {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Number { value, .. } => Ok(PercentageNumber(value.max(0.0))),
      Token::Percentage { unit_value, .. } => Ok(PercentageNumber(unit_value.max(0.0))),
      _ => Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      ),
    }
  }
}

/// Represents a percentage value that can be used in stylesheets.
#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(untagged)]
pub(crate) enum PercentageNumberValue {
  /// A CSS string value (e.g., "50%", "inherit")
  Css(String),
  /// A numeric value (0.0-1.0)
  Number(f32),
}

impl TryFrom<PercentageNumberValue> for PercentageNumber {
  type Error = String;

  fn try_from(value: PercentageNumberValue) -> Result<Self, Self::Error> {
    match value {
      PercentageNumberValue::Css(str) => {
        let mut input = ParserInput::new(&str);
        let mut parser = Parser::new(&mut input);
        PercentageNumber::from_css(&mut parser).map_err(|e| e.to_string())
      }
      PercentageNumberValue::Number(value) => Ok(PercentageNumber(value)),
    }
  }
}

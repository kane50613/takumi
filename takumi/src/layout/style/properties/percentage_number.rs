use std::ops::{Deref, Neg};

use cssparser::{Parser, Token};

use crate::layout::style::{
  Animatable, Color, MakeComputed,
  properties::{FromCss, ParseResult},
  tw::TailwindPropertyParser,
};
use crate::rendering::Sizing;

use super::CssToken;

/// Represents a percentage value (0.0-1.0) in CSS parsing.
///
/// This struct wraps an f32 value that represents a percentage
/// where 0.0 corresponds to 0% and 1.0 corresponds to 100%.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PercentageNumber(pub f32);

impl MakeComputed for PercentageNumber {}

impl Animatable for PercentageNumber {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &Sizing,
    _current_color: Color,
  ) {
    *self = Self(from.0 + (to.0 - from.0) * progress);
  }
}

impl Default for PercentageNumber {
  fn default() -> Self {
    Self(1.0)
  }
}

impl Deref for PercentageNumber {
  type Target = f32;

  fn deref(&self) -> &Self::Target {
    &self.0
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
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("number"), CssToken::Token("percentage")]
  }
}

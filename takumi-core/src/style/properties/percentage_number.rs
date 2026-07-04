use crate::style::{ToCss, unexpected_token};
use std::{
  fmt,
  ops::{Deref, Neg},
};

use cssparser::{Parser, Token};

use crate::style::{
  Animatable, Color, MakeComputed, SizingContext, lerp,
  properties::{FromCss, ParseResult, flex_grow::parse_numeric_tw},
  tw::TailwindPropertyParser,
};

use crate::style::{CssSyntaxKind, CssToken};

/// Represents a percentage value (0.0-1.0) in CSS parsing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PercentageNumber(pub f32);

impl MakeComputed for PercentageNumber {}

impl Animatable for PercentageNumber {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = Self(lerp(from.0, to.0, progress));
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
    parse_numeric_tw(token, |v| PercentageNumber(v / 100.0))
  }
}

impl<'i> FromCss<'i> for PercentageNumber {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Number { value, .. } => Ok(PercentageNumber(*value)),
      Token::Percentage { unit_value, .. } => Ok(PercentageNumber(*unit_value)),
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

impl ToCss for PercentageNumber {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self.0)
  }
}

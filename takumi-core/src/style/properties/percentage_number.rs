use std::{
  fmt,
  ops::{Deref, Neg},
};

use cssparser::{Parser, Token};

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, MakeComputed, SizingContext, ToCss, lerp,
  properties::{FromCss, ParseResult, flex_grow::parse_numeric_tw},
  tw::TailwindPropertyParser,
  unexpected_token,
};

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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_percentage_number() {
    for (css, expected) in [
      ("0.5", PercentageNumber(0.5)),
      ("50%", PercentageNumber(0.5)),
      ("1", PercentageNumber(1.0)),
      ("100%", PercentageNumber(1.0)),
    ] {
      assert_eq!(
        PercentageNumber::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_percentage_number_round_trip() {
    for css in ["0.5", "50%", "1"] {
      let parsed = PercentageNumber::from_css_str(css).unwrap();
      let reparsed = PercentageNumber::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_percentage_number_invalid() {
    assert!(PercentageNumber::from_css_str("auto").is_err());
    assert!(PercentageNumber::from_css_str("\"foo\"").is_err());
  }
}

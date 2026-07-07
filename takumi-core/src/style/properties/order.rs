use std::fmt;

use cssparser::Parser;

use crate::style::{
  Animatable, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Represents the CSS `order` value used for flex/grid item ordering.
pub struct Order(pub i32);

impl MakeComputed for Order {}
impl Animatable for Order {}

impl<'i> FromCss<'i> for Order {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Self(input.expect_integer()?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Integer)];
}

impl ToCss for Order {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_order() {
    for (css, expected) in [("0", Order(0)), ("5", Order(5)), ("-3", Order(-3))] {
      assert_eq!(Order::from_css_str(css), Ok(expected), "failed for {css}");
    }
  }

  #[test]
  fn test_order_round_trip() {
    for css in ["0", "5", "-3"] {
      let parsed = Order::from_css_str(css).unwrap();
      let reparsed = Order::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_order_invalid() {
    assert!(Order::from_css_str("1.5").is_err());
    assert!(Order::from_css_str("auto").is_err());
  }
}

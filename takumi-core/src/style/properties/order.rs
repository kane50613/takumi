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

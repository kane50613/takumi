use cssparser::Parser;
use std::fmt;

use crate::layout::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, SizingContext,
  ToCss, lerp, tw::TailwindPropertyParser,
};

#[derive(Debug, Clone, Copy, PartialEq)]
/// Represents a flex grow value.
pub struct FlexGrow(pub f32);

impl MakeComputed for FlexGrow {}

impl Animatable for FlexGrow {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    self.0 = lerp(from.0, to.0, progress);
  }
}

impl<'i> FromCss<'i> for FlexGrow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FlexGrow(input.expect_number()?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Number)];
}

impl TailwindPropertyParser for FlexGrow {
  fn parse_tw(token: &str) -> Option<Self> {
    let value = token.parse::<f32>().ok()?;

    Some(FlexGrow(value))
  }
}

impl ToCss for FlexGrow {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self.0)
  }
}

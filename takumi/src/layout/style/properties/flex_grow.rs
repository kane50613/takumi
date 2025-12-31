use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, ParseResult, tw::TailwindPropertyParser};

#[derive(Debug, Clone, Copy, PartialEq)]
/// Represents a flex grow value.
pub struct FlexGrow(pub f32);

impl<'i> FromCss<'i> for FlexGrow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FlexGrow(input.expect_number()?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("number")]
  }
}

impl TailwindPropertyParser for FlexGrow {
  fn parse_tw(token: &str) -> Option<Self> {
    let value = token.parse::<f32>().ok()?;

    Some(FlexGrow(value))
  }
}

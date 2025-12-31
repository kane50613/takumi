use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, ParseResult, tw::TailwindPropertyParser};

#[derive(Debug, Clone, PartialEq)]
/// Represents a line clamp value.
pub struct LineClamp {
  /// The number of lines to clamp.
  pub count: u32,
  /// The ellipsis character to use when the text is clamped.
  pub ellipsis: Option<String>,
}

impl TailwindPropertyParser for LineClamp {
  fn parse_tw(token: &str) -> Option<Self> {
    let count = token.parse::<u32>().ok()?;
    Some(LineClamp {
      count,
      ellipsis: None,
    })
  }
}

impl From<u32> for LineClamp {
  fn from(count: u32) -> Self {
    Self {
      count,
      ellipsis: None,
    }
  }
}

impl<'i> FromCss<'i> for LineClamp {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let count = input.try_parse(Parser::expect_integer)?;

    let ellipsis = input.try_parse(Parser::expect_string_cloned).ok();

    Ok(LineClamp {
      count: count as u32,
      ellipsis: ellipsis.map(|s| s.to_string()),
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("integer"), CssToken::Token("string")]
  }
}

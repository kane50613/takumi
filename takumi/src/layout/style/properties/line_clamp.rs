use cssparser::Parser;
use std::fmt;

use crate::layout::style::{
  CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss, properties::write_css_string,
  tw::TailwindPropertyParser,
};

#[derive(Debug, Clone, PartialEq)]
/// Represents a line clamp value.
#[non_exhaustive]
pub struct LineClamp {
  /// The number of lines to clamp.
  pub count: u32,
  /// The ellipsis character to use when the text is clamped.
  pub ellipsis: Option<String>,
}

impl MakeComputed for LineClamp {}

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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Integer),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for LineClamp {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match &self.ellipsis {
      Some(e) => {
        write!(dest, "{} ", self.count)?;
        write_css_string(dest, e)
      }
      None => write!(dest, "{}", self.count),
    }
  }
}

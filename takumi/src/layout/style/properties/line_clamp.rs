use cssparser::Parser;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(try_from = "LineClampValue")]
#[ts(as = "LineClampValue")]
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

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub(crate) enum LineClampValue {
  Structured {
    count: u32,
    ellipsis: Option<String>,
  },
  Number(u32),
  Css(String),
}

impl TryFrom<LineClampValue> for LineClamp {
  type Error = String;

  fn try_from(value: LineClampValue) -> Result<Self, Self::Error> {
    match value {
      LineClampValue::Structured { count, ellipsis } => Ok(LineClamp { count, ellipsis }),
      LineClampValue::Number(count) => Ok(LineClamp {
        count,
        ellipsis: None,
      }),
      LineClampValue::Css(css) => LineClamp::from_str(&css).map_err(|e| e.to_string()),
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
}

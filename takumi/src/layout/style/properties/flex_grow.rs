use cssparser::Parser;
use serde::{
  Deserialize, Deserializer, Serialize,
  de::{Error, Unexpected},
};
use serde_untagged::UntaggedEnumVisitor;
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

#[derive(Debug, Clone, Serialize, Copy, TS, PartialEq)]
#[ts(type = "number | string")]
#[serde(transparent)]
/// Represents a flex grow value.
pub struct FlexGrow(pub f32);

impl<'i> FromCss<'i> for FlexGrow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FlexGrow(input.expect_number()?))
  }
}

impl TailwindPropertyParser for FlexGrow {
  fn parse_tw(token: &str) -> Option<Self> {
    let value = token.parse::<f32>().ok()?;

    Some(FlexGrow(value))
  }
}

impl<'de> Deserialize<'de> for FlexGrow {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    UntaggedEnumVisitor::new()
      .i32(|num| Ok(FlexGrow(num as f32)))
      .f32(|num| Ok(FlexGrow(num)))
      .string(|str| {
        Ok(FlexGrow(str.parse::<f32>().map_err(|_| {
          serde_untagged::de::Error::invalid_value(Unexpected::Str(str), &"a number")
        })?))
      })
      .deserialize(deserializer)
  }
}

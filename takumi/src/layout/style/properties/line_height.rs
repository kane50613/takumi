use cssparser::Parser;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
  layout::{
    DEFAULT_LINE_HEIGHT_SCALER,
    style::{
      FromCss, LengthUnit, ParseResult,
      tw::{TW_VAR_SPACING, TailwindPropertyParser},
    },
  },
  rendering::RenderContext,
};

/// Represents a line height value, number value is parsed as em.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS, Copy)]
#[serde(try_from = "LineHeightValue")]
#[ts(as = "LineHeightValue")]
pub struct LineHeight(pub LengthUnit);

impl Default for LineHeight {
  fn default() -> Self {
    Self(LengthUnit::Em(DEFAULT_LINE_HEIGHT_SCALER)) // Default line height
  }
}

impl TailwindPropertyParser for LineHeight {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Self(LengthUnit::Em(1.0)));
    }

    let Ok(value) = token.parse::<f32>() else {
      return None;
    };

    Some(Self(LengthUnit::Em(value * TW_VAR_SPACING)))
  }
}

/// Proxy type for `LineHeight` Css deserialization.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub(crate) enum LineHeightValue {
  /// A number value.
  Number(f32),
  /// A CSS string value.
  Css(String),
  /// A length value.
  Length(LengthUnit),
}

impl TryFrom<LineHeightValue> for LineHeight {
  type Error = String;

  fn try_from(value: LineHeightValue) -> Result<Self, Self::Error> {
    match value {
      LineHeightValue::Number(number) => Ok(LineHeight(LengthUnit::Em(number))),
      LineHeightValue::Css(css) => LineHeight::from_str(&css).map_err(|e| e.to_string()),
      LineHeightValue::Length(length) => Ok(LineHeight(length)),
    }
  }
}

impl<'i> FromCss<'i> for LineHeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let Ok(number) = input.try_parse(Parser::expect_number) else {
      return LengthUnit::from_css(input).map(LineHeight);
    };

    Ok(LineHeight(LengthUnit::Em(number)))
  }
}

impl LineHeight {
  /// Converts the line height to a parley line height.
  pub(crate) fn into_parley(self, context: &RenderContext) -> parley::LineHeight {
    match self.0 {
      LengthUnit::Px(value) => parley::LineHeight::Absolute(value),
      LengthUnit::Em(value) => parley::LineHeight::FontSizeRelative(value),
      LengthUnit::Percentage(value) => parley::LineHeight::FontSizeRelative(value / 100.0),
      unit => parley::LineHeight::Absolute(unit.resolve_to_px(context, context.font_size)),
    }
  }
}

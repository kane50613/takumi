use cssparser::Parser;

use crate::{
  layout::{
    DEFAULT_LINE_HEIGHT_SCALER,
    style::{
      FromCss, LengthUnit, ParseResult,
      tw::{TW_VAR_SPACING, TailwindPropertyParser},
    },
  },
  rendering::Sizing,
};

/// Represents a line height value, number value is parsed as em.
#[derive(Debug, Clone, PartialEq, Copy)]
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
  pub(crate) fn into_parley(self, sizing: &Sizing) -> parley::LineHeight {
    match self.0 {
      LengthUnit::Px(value) => parley::LineHeight::Absolute(value),
      LengthUnit::Em(value) => parley::LineHeight::FontSizeRelative(value),
      LengthUnit::Percentage(value) => parley::LineHeight::FontSizeRelative(value / 100.0),
      unit => parley::LineHeight::Absolute(unit.to_px(sizing, sizing.font_size)),
    }
  }
}

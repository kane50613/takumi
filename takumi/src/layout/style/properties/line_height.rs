use cssparser::Parser;

use crate::{
  layout::{
    DEFAULT_LINE_HEIGHT_SCALER,
    style::{
      CssToken, FromCss, Length, ParseResult,
      tw::{TW_VAR_SPACING, TailwindPropertyParser},
    },
  },
  rendering::Sizing,
};

/// Represents a line height value, number value is parsed as em.
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct LineHeight(pub Length);

impl Default for LineHeight {
  fn default() -> Self {
    Self(Length::Em(DEFAULT_LINE_HEIGHT_SCALER)) // Default line height
  }
}

impl TailwindPropertyParser for LineHeight {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Self(Length::Em(1.0)));
    }

    let Ok(value) = token.parse::<f32>() else {
      return None;
    };

    Some(Self(Length::Em(value * TW_VAR_SPACING)))
  }
}

impl<'i> FromCss<'i> for LineHeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let Ok(number) = input.try_parse(Parser::expect_number) else {
      return Length::from_css(input).map(LineHeight);
    };

    Ok(LineHeight(Length::Em(number)))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("number"), CssToken::Token("length")]
  }
}

impl LineHeight {
  /// Converts the line height to a parley line height.
  pub(crate) fn into_parley(self, sizing: &Sizing) -> parley::LineHeight {
    match self.0 {
      Length::Px(value) => parley::LineHeight::Absolute(value),
      Length::Em(value) => parley::LineHeight::FontSizeRelative(value),
      Length::Percentage(value) => parley::LineHeight::FontSizeRelative(value / 100.0),
      unit => parley::LineHeight::Absolute(unit.to_px(sizing, sizing.font_size)),
    }
  }
}

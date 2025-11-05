use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{
  FromCss, LengthUnit, ParseResult, SpacePair, tw::TailwindPropertyParser,
};

/// Horizontal keywords for `background-position`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PositionKeywordX {
  /// Align to the left edge.
  Left,
  /// Align to the horizontal center.
  Center,
  /// Align to the right edge.
  Right,
}

/// Vertical keywords for `background-position`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PositionKeywordY {
  /// Align to the top edge.
  Top,
  /// Align to the vertical center.
  Center,
  /// Align to the bottom edge.
  Bottom,
}

/// A single `background-position` component for an axis.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(untagged)]
pub enum PositionComponent {
  /// A horizontal keyword.
  KeywordX(PositionKeywordX),
  /// A vertical keyword.
  KeywordY(PositionKeywordY),
  /// An absolute length value.
  Length(LengthUnit),
}

impl PositionComponent {
  pub(crate) fn to_length_unit(self) -> LengthUnit {
    match self {
      PositionComponent::KeywordX(keyword) => match keyword {
        PositionKeywordX::Center => LengthUnit::Percentage(50.0),
        PositionKeywordX::Left => LengthUnit::Percentage(0.0),
        PositionKeywordX::Right => LengthUnit::Percentage(100.0),
      },
      PositionComponent::KeywordY(keyword) => match keyword {
        PositionKeywordY::Center => LengthUnit::Percentage(50.0),
        PositionKeywordY::Top => LengthUnit::Percentage(0.0),
        PositionKeywordY::Bottom => LengthUnit::Percentage(100.0),
      },
      PositionComponent::Length(length) => length,
    }
  }
}

/// Parsed `background-position` value for one layer.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
pub struct BackgroundPosition(pub SpacePair<PositionComponent>);

impl TailwindPropertyParser for BackgroundPosition {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "top-left" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Left),
        PositionComponent::KeywordY(PositionKeywordY::Top),
      ))),
      "top" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Center),
        PositionComponent::KeywordY(PositionKeywordY::Top),
      ))),
      "top-right" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Right),
        PositionComponent::KeywordY(PositionKeywordY::Top),
      ))),
      "left" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Left),
        PositionComponent::KeywordY(PositionKeywordY::Center),
      ))),
      "center" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Center),
        PositionComponent::KeywordY(PositionKeywordY::Center),
      ))),
      "right" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Right),
        PositionComponent::KeywordY(PositionKeywordY::Center),
      ))),
      "bottom-left" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Left),
        PositionComponent::KeywordY(PositionKeywordY::Bottom),
      ))),
      "bottom" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Center),
        PositionComponent::KeywordY(PositionKeywordY::Bottom),
      ))),
      "bottom-right" => Some(Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Right),
        PositionComponent::KeywordY(PositionKeywordY::Bottom),
      ))),
      _ => None,
    }
  }
}

impl Default for BackgroundPosition {
  fn default() -> Self {
    Self(SpacePair::from_pair(
      PositionComponent::KeywordX(PositionKeywordX::Center),
      PositionComponent::KeywordY(PositionKeywordY::Center),
    ))
  }
}

impl<'i> FromCss<'i> for BackgroundPosition {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = PositionComponent::from_css(input)?;
    // If a second exists, parse it; otherwise, 1-value syntax means y=center
    let second = input.try_parse(PositionComponent::from_css).ok();

    let (x, y) = match (first, second) {
      (PositionComponent::KeywordY(_), None) => {
        (PositionComponent::KeywordX(PositionKeywordX::Center), first)
      }
      (PositionComponent::KeywordY(_), Some(second)) => (second, first),
      (x, None) => (x, PositionComponent::KeywordY(PositionKeywordY::Center)),
      (x, Some(y)) => (x, y),
    };

    Ok(BackgroundPosition(SpacePair::from_pair(x, y)))
  }
}

impl<'i> FromCss<'i> for PositionComponent {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(v) = input.try_parse(LengthUnit::from_css) {
      return Ok(PositionComponent::Length(v));
    }

    let location = input.current_source_location();
    let token = input.expect_ident()?;

    match_ignore_ascii_case! {
      &token,
      "left" => Ok(PositionComponent::KeywordX(PositionKeywordX::Left)),
      "center" => Ok(PositionComponent::KeywordX(PositionKeywordX::Center)),
      "right" => Ok(PositionComponent::KeywordX(PositionKeywordX::Right)),
      "top" => Ok(PositionComponent::KeywordY(PositionKeywordY::Top)),
      "bottom" => Ok(PositionComponent::KeywordY(PositionKeywordY::Bottom)),
      _ => Err(location.new_basic_unexpected_token_error(Token::Ident(token.clone())).into()),
    }
  }
}

/// A value representing either a list of parsed positions or a raw CSS string.
#[derive(Debug, Clone, PartialEq, TS, Deserialize)]
#[serde(untagged)]
pub(crate) enum BackgroundPositionsValue {
  /// Parsed positions for one or more layers.
  Positions(Vec<BackgroundPosition>),
  /// Raw CSS to be parsed at runtime.
  Css(String),
}

/// A list of `background-position` values (one per layer).
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, TS)]
#[ts(as = "BackgroundPositionsValue")]
#[serde(try_from = "BackgroundPositionsValue")]
pub struct BackgroundPositions(pub Vec<BackgroundPosition>);

impl TryFrom<BackgroundPositionsValue> for BackgroundPositions {
  type Error = String;

  fn try_from(value: BackgroundPositionsValue) -> Result<Self, Self::Error> {
    match value {
      BackgroundPositionsValue::Positions(v) => Ok(Self(v)),
      BackgroundPositionsValue::Css(css) => Self::from_str(&css).map_err(|e| e.to_string()),
    }
  }
}

impl<'i> FromCss<'i> for BackgroundPositions {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = Vec::new();
    values.push(BackgroundPosition::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundPosition::from_css(input)?);
    }

    Ok(Self(values))
  }
}

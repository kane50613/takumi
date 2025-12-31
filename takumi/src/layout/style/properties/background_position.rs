use cssparser::{Parser, Token, match_ignore_ascii_case};
use smallvec::SmallVec;
use taffy::{Point, Size};

use crate::{
  layout::style::{CssToken, FromCss, Length, ParseResult, SpacePair, tw::TailwindPropertyParser},
  rendering::Sizing,
};

/// Horizontal keywords for `background-position`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionKeywordX {
  /// Align to the left edge.
  Left,
  /// Align to the horizontal center.
  Center,
  /// Align to the right edge.
  Right,
}

/// Vertical keywords for `background-position`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionKeywordY {
  /// Align to the top edge.
  Top,
  /// Align to the vertical center.
  Center,
  /// Align to the bottom edge.
  Bottom,
}

/// A single `background-position` component for an axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionComponent {
  /// A horizontal keyword.
  KeywordX(PositionKeywordX),
  /// A vertical keyword.
  KeywordY(PositionKeywordY),
  /// An absolute length value.
  Length(Length),
}

impl From<PositionComponent> for Length {
  fn from(component: PositionComponent) -> Self {
    match component {
      PositionComponent::KeywordX(keyword) => match keyword {
        PositionKeywordX::Center => Self::Percentage(50.0),
        PositionKeywordX::Left => Self::Percentage(0.0),
        PositionKeywordX::Right => Self::Percentage(100.0),
      },
      PositionComponent::KeywordY(keyword) => match keyword {
        PositionKeywordY::Center => Self::Percentage(50.0),
        PositionKeywordY::Top => Self::Percentage(0.0),
        PositionKeywordY::Bottom => Self::Percentage(100.0),
      },
      PositionComponent::Length(length) => length,
    }
  }
}

/// Parsed `background-position` value for one layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundPosition(pub SpacePair<PositionComponent>);

impl BackgroundPosition {
  pub(crate) fn to_point(self, sizing: &Sizing, border_box: Size<f32>) -> Point<f32> {
    Point {
      x: Length::from(self.0.x).to_px(sizing, border_box.width),
      y: Length::from(self.0.y).to_px(sizing, border_box.height),
    }
  }
}

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

  fn valid_tokens() -> &'static [CssToken] {
    PositionComponent::valid_tokens()
  }
}

impl<'i> FromCss<'i> for PositionComponent {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(v) = input.try_parse(Length::from_css) {
      return Ok(PositionComponent::Length(v));
    }

    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(ident) = token else {
      return Err(Self::unexpected_token_error(location, token));
    };

    match_ignore_ascii_case! {
      &ident,
      "left" => Ok(PositionComponent::KeywordX(PositionKeywordX::Left)),
      "center" => Ok(PositionComponent::KeywordX(PositionKeywordX::Center)),
      "right" => Ok(PositionComponent::KeywordX(PositionKeywordX::Right)),
      "top" => Ok(PositionComponent::KeywordY(PositionKeywordY::Top)),
      "bottom" => Ok(PositionComponent::KeywordY(PositionKeywordY::Bottom)),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("left"),
      CssToken::Keyword("center"),
      CssToken::Keyword("right"),
      CssToken::Keyword("top"),
      CssToken::Keyword("bottom"),
      CssToken::Token("length"),
    ]
  }
}

/// A list of `background-position` values (one per layer).
pub type BackgroundPositions = SmallVec<[BackgroundPosition; 4]>;

impl<'i> FromCss<'i> for BackgroundPositions {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = SmallVec::new();
    values.push(BackgroundPosition::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundPosition::from_css(input)?);
    }

    Ok(values)
  }

  fn valid_tokens() -> &'static [CssToken] {
    BackgroundPosition::valid_tokens()
  }
}

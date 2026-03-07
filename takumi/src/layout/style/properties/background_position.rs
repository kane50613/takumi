use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::{Point, Size};

use crate::{
  layout::style::{
    Animatable, Color, CssToken, FromCss, Length, MakeComputed, ParseResult, SpacePair,
    tw::TailwindPropertyParser,
  },
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

impl MakeComputed for PositionComponent {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for PositionComponent {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &Sizing,
    current_color: Color,
  ) {
    let mut length = Length::from(*from);
    length.interpolate(
      &Length::from(*from),
      &Length::from(*to),
      progress,
      sizing,
      current_color,
    );
    *self = PositionComponent::Length(length);
  }
}

impl From<Length> for PositionComponent {
  fn from(value: Length) -> Self {
    PositionComponent::Length(value)
  }
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

/// Parsed position value for one layer-like CSS property.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundPosition<const DEFAULT_TOP_LEFT: bool = true>(
  pub SpacePair<PositionComponent>,
);

/// `object-position` value with a CSS initial value of `center center`.
pub type ObjectPosition = BackgroundPosition<false>;
/// `transform-origin` value with a CSS initial value of `center center`.
pub type TransformOrigin = BackgroundPosition<false>;

impl<const DEFAULT_TOP_LEFT: bool> MakeComputed for BackgroundPosition<DEFAULT_TOP_LEFT> {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.0.make_computed(sizing);
  }
}

impl<const DEFAULT_TOP_LEFT: bool> Animatable for BackgroundPosition<DEFAULT_TOP_LEFT> {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &Sizing,
    current_color: Color,
  ) {
    let mut value = from.0;
    value.interpolate(&from.0, &to.0, progress, sizing, current_color);
    self.0 = value;
  }
}

impl<const DEFAULT_TOP_LEFT: bool> BackgroundPosition<DEFAULT_TOP_LEFT> {
  pub(crate) fn to_point(self, sizing: &Sizing, border_box: Size<f32>) -> Point<f32> {
    Point {
      x: Length::from(self.0.x).to_px(sizing, border_box.width),
      y: Length::from(self.0.y).to_px(sizing, border_box.height),
    }
  }
}

impl<const DEFAULT_TOP_LEFT: bool> TailwindPropertyParser for BackgroundPosition<DEFAULT_TOP_LEFT> {
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

impl<const DEFAULT_TOP_LEFT: bool> Default for BackgroundPosition<DEFAULT_TOP_LEFT> {
  fn default() -> Self {
    if DEFAULT_TOP_LEFT {
      Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Left),
        PositionComponent::KeywordY(PositionKeywordY::Top),
      ))
    } else {
      Self(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Center),
        PositionComponent::KeywordY(PositionKeywordY::Center),
      ))
    }
  }
}

impl<'i, const DEFAULT_TOP_LEFT: bool> FromCss<'i> for BackgroundPosition<DEFAULT_TOP_LEFT> {
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
      return Ok(v.into());
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
pub type BackgroundPositions = Box<[BackgroundPosition]>;

impl<'i> FromCss<'i> for BackgroundPositions {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = Vec::new();
    values.push(BackgroundPosition::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundPosition::from_css(input)?);
    }

    Ok(values.into_boxed_slice())
  }

  fn valid_tokens() -> &'static [CssToken] {
    BackgroundPosition::<true>::valid_tokens()
  }
}

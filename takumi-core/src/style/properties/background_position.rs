use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use super::background_image::parse_comma_list;
use crate::context::RenderContext;
use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, ListInterpolationStrategy,
  MakeComputed, ParseResult, SizingContext, SpacePair, ToCss, tw::TailwindPropertyParser,
  unexpected_token,
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

impl PositionComponent {
  /// Where this component lands in `available` space. A keyword is a share of
  /// that space, so `center` sits halfway; `auto` behaves the same, since a
  /// position with nothing to say centres.
  pub fn resolve(self, context: &RenderContext, available: f32) -> f32 {
    match Length::from(self) {
      Length::Auto => available * 0.5,
      length => length.to_px(&context.sizing, available),
    }
  }
}

impl MakeComputed for PositionComponent {
  fn make_computed(&mut self, sizing: &SizingContext) {
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
    sizing: &SizingContext,
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
pub struct PositionValue(pub SpacePair<PositionComponent>);

impl MakeComputed for PositionValue {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

impl Animatable for PositionValue {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::RepeatToLcm
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    let mut value = from.0;
    value.interpolate(&from.0, &to.0, progress, sizing, current_color);
    self.0 = value;
  }
}

impl PositionValue {
  /// Resolves the position to a pixel point within the border box.
  pub(crate) fn to_point(self, sizing: &SizingContext, width: f32, height: f32) -> (f32, f32) {
    (
      Length::from(self.0.x).to_px(sizing, width),
      Length::from(self.0.y).to_px(sizing, height),
    )
  }
}

impl TailwindPropertyParser for PositionValue {
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

impl Default for PositionValue {
  fn default() -> Self {
    Self(SpacePair::from_pair(
      PositionComponent::KeywordX(PositionKeywordX::Left),
      PositionComponent::KeywordY(PositionKeywordY::Top),
    ))
  }
}

impl PositionValue {
  /// Center position (`center center`), the initial value for `object-position` and
  /// `transform-origin`.
  pub const fn center() -> Self {
    Self(SpacePair::from_pair(
      PositionComponent::KeywordX(PositionKeywordX::Center),
      PositionComponent::KeywordY(PositionKeywordY::Center),
    ))
  }
}

impl<'i> FromCss<'i> for PositionValue {
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

    Ok(PositionValue(SpacePair::from_pair(x, y)))
  }

  const VALID_TOKENS: &'static [CssToken] = PositionComponent::VALID_TOKENS;
}

impl<'i> FromCss<'i> for PositionComponent {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(v) = input.try_parse(Length::from_css) {
      return Ok(v.into());
    }

    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(ident) = token else {
      return Err(unexpected_token!(location, token));
    };

    match_ignore_ascii_case! {
      &ident,
      "left" => Ok(PositionComponent::KeywordX(PositionKeywordX::Left)),
      "center" => Ok(PositionComponent::KeywordX(PositionKeywordX::Center)),
      "right" => Ok(PositionComponent::KeywordX(PositionKeywordX::Right)),
      "top" => Ok(PositionComponent::KeywordY(PositionKeywordY::Top)),
      "bottom" => Ok(PositionComponent::KeywordY(PositionKeywordY::Bottom)),
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("left"),
    CssToken::Keyword("center"),
    CssToken::Keyword("right"),
    CssToken::Keyword("top"),
    CssToken::Keyword("bottom"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

/// An ordered list of [`PositionValue`] values.
pub type PositionValues = Box<[PositionValue]>;

impl<'i> FromCss<'i> for PositionValues {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_list(input, PositionValue::from_css)
  }

  const VALID_TOKENS: &'static [CssToken] = PositionValue::VALID_TOKENS;
}

impl ToCss for PositionKeywordX {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Left => dest.write_str("left"),
      Self::Center => dest.write_str("center"),
      Self::Right => dest.write_str("right"),
    }
  }
}

impl ToCss for PositionKeywordY {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Top => dest.write_str("top"),
      Self::Center => dest.write_str("center"),
      Self::Bottom => dest.write_str("bottom"),
    }
  }
}

impl ToCss for PositionComponent {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::KeywordX(k) => k.to_css(dest),
      Self::KeywordY(k) => k.to_css(dest),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

impl ToCss for PositionValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_position_value() {
    for (css, expected) in [
      (
        "10px 20px",
        PositionValue(SpacePair::from_pair(
          PositionComponent::Length(Length::Px(10.0)),
          PositionComponent::Length(Length::Px(20.0)),
        )),
      ),
      (
        "left top",
        PositionValue(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Left),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        )),
      ),
      (
        "top",
        PositionValue(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Top),
        )),
      ),
      ("center", PositionValue::center()),
    ] {
      assert_eq!(
        PositionValue::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_position_value_round_trip() {
    for css in ["10px 20px", "left top", "top"] {
      let parsed = PositionValue::from_css_str(css).unwrap();
      let reparsed = PositionValue::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_position_value_center_does_not_round_trip() {
    let parsed = PositionValue::from_css_str("center").unwrap();
    assert_eq!(parsed, PositionValue::center());
    let reparsed = PositionValue::from_css_str(&parsed.to_css_string()).unwrap();
    // TODO: looks wrong: "center" serializes to "center center", and reparsing always reads
    // the ambiguous second "center" as KeywordX (PositionComponent::from_css keyword order),
    // so the y-axis silently becomes KeywordX instead of KeywordY.
    assert_eq!(
      reparsed,
      PositionValue(SpacePair::from_pair(
        PositionComponent::KeywordX(PositionKeywordX::Center),
        PositionComponent::KeywordX(PositionKeywordX::Center),
      ))
    );
  }

  #[test]
  fn test_parse_position_value_invalid() {
    assert!(PositionValue::from_css_str("diagonal").is_err());
    assert!(PositionValue::from_css_str("\"foo\"").is_err());
  }
}

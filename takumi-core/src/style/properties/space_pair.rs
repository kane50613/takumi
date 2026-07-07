use std::fmt;

use cssparser::Parser;
use taffy::Point;

use crate::style::{
  CssExpectedMessage, CssToken, FromCss, Length, MakeComputed, Overflow, ParseResult,
  SizingContext, ToCss,
};

/// A pair of values for horizontal and vertical axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacePair<T: Copy> {
  /// The horizontal value.
  pub x: T,
  /// The vertical value.
  pub y: T,
}

impl<T: Copy + Default> Default for SpacePair<T> {
  fn default() -> Self {
    Self::from_single(T::default())
  }
}

impl<'i, T: Copy + FromCss<'i>> FromCss<'i> for SpacePair<T> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = T::from_css(input)?;
    if let Ok(second) = T::from_css(input) {
      Ok(Self::from_pair(first, second))
    } else {
      Ok(Self::from_single(first))
    }
  }

  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::OneOrTwoValues;

  const VALID_TOKENS: &'static [CssToken] = T::VALID_TOKENS;
}

impl<T: Copy> SpacePair<T> {
  /// Create a new [`SpacePair`] from a single value.
  #[inline]
  pub const fn from_single(value: T) -> Self {
    Self::from_pair(value, value)
  }

  /// Create a new [`SpacePair`] from a pair of values.
  #[inline]
  pub const fn from_pair(x: T, y: T) -> Self {
    Self { x, y }
  }
}

impl<T: Copy + MakeComputed> MakeComputed for SpacePair<T> {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.x.make_computed(sizing);
    self.y.make_computed(sizing);
  }
}

impl<T: Copy> SpacePair<T> {
  pub(crate) fn into_taffy(self) -> Point<T> {
    Point {
      x: self.x,
      y: self.y,
    }
  }
}

impl SpacePair<Overflow> {
  /// Whether either axis clips content (not `visible`).
  pub fn should_clip_content(&self) -> bool {
    self.x != Overflow::Visible || self.y != Overflow::Visible
  }
}

/// A pair of values for horizontal and vertical border radii.
pub(crate) type BorderRadiusPair = SpacePair<Length>;

impl BorderRadiusPair {
  /// Resolves both radii to non-negative pixels against the border box.
  pub fn to_px(self, sizing: &SizingContext, width: f32, height: f32) -> SpacePair<f32> {
    SpacePair::from_pair(
      self.x.to_px(sizing, width).max(0.0),
      self.y.to_px(sizing, height).max(0.0),
    )
  }
}

impl<T: Copy + ToCss> ToCss for SpacePair<T> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.x.to_css(dest)?;
    dest.write_str(" ")?;
    self.y.to_css(dest)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_space_pair_length() {
    for (css, expected) in [
      ("10px", SpacePair::from_single(Length::Px(10.0))),
      (
        "10px 20px",
        SpacePair::from_pair(Length::Px(10.0), Length::Px(20.0)),
      ),
      ("auto", SpacePair::from_single(Length::Auto)),
    ] {
      assert_eq!(
        SpacePair::<Length>::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_space_pair_length_round_trip() {
    for css in ["10px", "10px 20px", "auto"] {
      let parsed = SpacePair::<Length>::from_css_str(css).unwrap();
      let reparsed = SpacePair::<Length>::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_space_pair_length_invalid() {
    assert!(SpacePair::<Length>::from_css_str("10xyz").is_err());
    assert!(SpacePair::<Length>::from_css_str("\"foo\"").is_err());
  }
}

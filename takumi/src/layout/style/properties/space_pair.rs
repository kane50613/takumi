use cssparser::Parser;
use std::fmt;
use taffy::{Point, Size};

use crate::{
  layout::style::SizingContext,
  layout::style::{
    CssExpectedMessage, CssToken, FromCss, LengthDefaultsToZero, MakeComputed, Overflow,
    ParseResult, ToCss,
  },
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

impl<T: Copy> From<SpacePair<T>> for Point<T> {
  fn from(value: SpacePair<T>) -> Self {
    Point {
      x: value.x,
      y: value.y,
    }
  }
}

impl SpacePair<Overflow> {
  pub(crate) fn should_clip_content(&self) -> bool {
    self.x != Overflow::Visible || self.y != Overflow::Visible
  }
}

/// A pair of values for horizontal and vertical border radii.
pub type BorderRadiusPair = SpacePair<LengthDefaultsToZero>;

impl BorderRadiusPair {
  pub(crate) fn to_px(self, sizing: &SizingContext, border_box: Size<f32>) -> SpacePair<f32> {
    SpacePair::from_pair(
      self.x.to_px(sizing, border_box.width).max(0.0),
      self.y.to_px(sizing, border_box.height).max(0.0),
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

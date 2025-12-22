use cssparser::Parser;
use taffy::{LengthPercentage, Point, Size};

use crate::{
  layout::style::{FromCss, LengthUnit, Overflow, ParseResult},
  rendering::Sizing,
};

/// A pair of values for horizontal and vertical axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacePair<T: Copy, const Y_FIRST: bool = false> {
  /// The horizontal value.
  pub x: T,
  /// The vertical value.
  pub y: T,
}

/// A pair of gap values which has the vertical value first.
pub type Gap = SpacePair<LengthUnit<false>, true>;

impl<T: Copy + Default, const Y_FIRST: bool> Default for SpacePair<T, Y_FIRST> {
  fn default() -> Self {
    Self::from_single(T::default())
  }
}

impl<'i, T: Copy + FromCss<'i>, const Y_FIRST: bool> FromCss<'i> for SpacePair<T, Y_FIRST> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = T::from_css(input)?;
    if let Ok(second) = T::from_css(input) {
      Ok(Self::from_pair(first, second))
    } else {
      Ok(Self::from_single(first))
    }
  }
}

impl<T: Copy, const Y_FIRST: bool> SpacePair<T, Y_FIRST> {
  /// Create a new [`SpacePair`] from a single value.
  #[inline]
  pub const fn from_single(value: T) -> Self {
    Self::from_pair(value, value)
  }

  /// Create a new [`SpacePair`] from a pair of values.
  ///
  /// When `Y_FIRST` is true, the first value is the vertical value and the second value is the horizontal value.
  /// Otherwise, the first value is the horizontal value and the second value is the vertical value.
  #[inline]
  pub const fn from_pair(first: T, second: T) -> Self {
    if Y_FIRST {
      Self {
        x: second,
        y: first,
      }
    } else {
      Self {
        x: first,
        y: second,
      }
    }
  }
}

impl<const DEFAULT_AUTO: bool, const Y_FIRST: bool> SpacePair<LengthUnit<DEFAULT_AUTO>, Y_FIRST> {
  pub(crate) fn resolve_to_size(self, sizing: &Sizing) -> Size<LengthPercentage> {
    Size {
      width: self.x.resolve_to_length_percentage(sizing),
      height: self.y.resolve_to_length_percentage(sizing),
    }
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
pub type BorderRadiusPair = SpacePair<LengthUnit<false>>;

impl BorderRadiusPair {
  pub(crate) fn to_px(self, sizing: &Sizing, border_box: Size<f32>) -> SpacePair<f32> {
    SpacePair::from_pair(
      self.x.to_px(sizing, border_box.width).max(0.0),
      self.y.to_px(sizing, border_box.height).max(0.0),
    )
  }
}

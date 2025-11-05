use cssparser::Parser;
use serde::{Deserialize, Deserializer, Serialize, de::Error as DeError};
use taffy::{LengthPercentage, Point, Size};
use ts_rs::TS;

use crate::{
  layout::style::{FromCss, LengthUnit, ParseResult},
  rendering::RenderContext,
};

/// A pair of values for horizontal and vertical axes.
#[derive(Debug, Clone, Copy, Serialize, TS, PartialEq)]
#[serde(try_from = "SpacePairValue<T>")]
#[ts(as = "SpacePairValue<T>")]
pub struct SpacePair<T: TS + Copy, const Y_FIRST: bool = false> {
  /// The horizontal value.
  pub x: T,
  /// The vertical value.
  pub y: T,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(untagged)]
pub(crate) enum SpacePairValue<T: TS + Copy> {
  SingleValue(T),
  Structured { x: T, y: T },
  Css(String),
}

impl<'de, T, const Y_FIRST: bool> Deserialize<'de> for SpacePair<T, Y_FIRST>
where
  T: TS + Copy + Deserialize<'de> + for<'i> FromCss<'i>,
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let proxy = SpacePairValue::<T>::deserialize(deserializer)?;
    SpacePair::try_from(proxy).map_err(D::Error::custom)
  }
}

impl<T: TS + Copy + for<'i> FromCss<'i>, const Y_FIRST: bool> TryFrom<SpacePairValue<T>>
  for SpacePair<T, Y_FIRST>
{
  type Error = String;
  fn try_from(value: SpacePairValue<T>) -> Result<Self, Self::Error> {
    match value {
      SpacePairValue::SingleValue(value) => Ok(Self::from_single(value)),
      SpacePairValue::Structured { x, y } => Ok(Self { x, y }),
      SpacePairValue::Css(css) => Self::from_str(&css).map_err(|e| e.to_string()),
    }
  }
}

impl<'i, T: TS + Copy + FromCss<'i>, const Y_FIRST: bool> FromCss<'i> for SpacePair<T, Y_FIRST> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = T::from_css(input)?;
    if let Ok(second) = T::from_css(input) {
      Ok(Self::from_pair(first, second))
    } else {
      Ok(Self::from_single(first))
    }
  }
}

impl<T: TS + Copy, const Y_FIRST: bool> SpacePair<T, Y_FIRST> {
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

impl<const Y_FIRST: bool> SpacePair<LengthUnit, Y_FIRST> {
  pub(crate) fn resolve_to_size(self, context: &RenderContext) -> Size<LengthPercentage> {
    Size {
      width: self.x.resolve_to_length_percentage(context),
      height: self.y.resolve_to_length_percentage(context),
    }
  }
}

impl<T: TS + Copy> From<SpacePair<T>> for Point<T> {
  fn from(value: SpacePair<T>) -> Self {
    Point {
      x: value.x,
      y: value.y,
    }
  }
}

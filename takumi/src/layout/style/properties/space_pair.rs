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
pub struct SpacePair<T: TS + Copy>(pub T, pub T);

#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(untagged)]
pub(crate) enum SpacePairValue<T: TS + Copy> {
  SingleValue(T),
  Array(T, T),
  Css(String),
}

impl<'de, T> Deserialize<'de> for SpacePair<T>
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

impl<T: TS + Copy + for<'i> FromCss<'i>> TryFrom<SpacePairValue<T>> for SpacePair<T> {
  type Error = String;
  fn try_from(value: SpacePairValue<T>) -> Result<Self, Self::Error> {
    match value {
      SpacePairValue::SingleValue(value) => Ok(Self(value, value)),
      SpacePairValue::Array(horizontal, vertical) => Ok(Self(horizontal, vertical)),
      SpacePairValue::Css(css) => Self::from_str(&css).map_err(|e| e.to_string()),
    }
  }
}

impl<'i, T: TS + Copy + FromCss<'i>> FromCss<'i> for SpacePair<T> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let first = T::from_css(input)?;
    if let Ok(second) = T::from_css(input) {
      Ok(Self(first, second))
    } else {
      Ok(Self(first, first))
    }
  }
}

impl<T: TS + Copy> SpacePair<T> {
  /// Create a new [`SpacePair`] from a single value.
  pub const fn from_single(value: T) -> Self {
    Self(value, value)
  }
}

impl SpacePair<LengthUnit> {
  pub(crate) fn resolve_to_size(self, context: &RenderContext) -> Size<LengthPercentage> {
    Size {
      width: self.0.resolve_to_length_percentage(context),
      height: self.1.resolve_to_length_percentage(context),
    }
  }
}

impl<T: TS + Copy> From<SpacePair<T>> for Point<T> {
  fn from(value: SpacePair<T>) -> Self {
    Point {
      x: value.0,
      y: value.1,
    }
  }
}

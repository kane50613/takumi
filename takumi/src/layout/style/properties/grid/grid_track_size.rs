use cssparser::{Parser, match_ignore_ascii_case};
use taffy::{MaxTrackSizingFunction, MinTrackSizingFunction, TrackSizingFunction};

use crate::{
  layout::style::{
    FromCss, GridLengthUnit, GridMinMaxSize, LengthUnit, ParseResult, tw::TailwindPropertyParser,
  },
  rendering::RenderContext,
};

/// A list of `GridTrackSize`
pub type GridTrackSizes = Vec<GridTrackSize>;

impl<'i> FromCss<'i> for GridTrackSizes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut components: Vec<GridTrackSize> = Vec::new();
    while let Ok(size) = GridTrackSize::from_css(input) {
      components.push(size);
    }

    Ok(components)
  }
}

/// Represents a grid track size
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrackSize {
  /// A minmax() track size
  MinMax(GridMinMaxSize),
  /// A fixed track size
  Fixed(GridLengthUnit),
}

impl From<GridLengthUnit> for GridTrackSize {
  fn from(length: GridLengthUnit) -> Self {
    Self::Fixed(length)
  }
}

impl GridTrackSize {
  /// Converts the grid track size to a non-repeated track sizing function.
  pub fn to_min_max(&self, context: &RenderContext) -> TrackSizingFunction {
    match self {
      // SAFETY: The compact length is a valid track sizing function.
      Self::Fixed(size) => unsafe {
        TrackSizingFunction {
          min: MinTrackSizingFunction::from_raw(size.to_compact_length(context)),
          max: MaxTrackSizingFunction::from_raw(size.to_compact_length(context)),
        }
      },
      Self::MinMax(min_max) => unsafe {
        TrackSizingFunction {
          min: MinTrackSizingFunction::from_raw(min_max.min.to_compact_length(context)),
          max: MaxTrackSizingFunction::from_raw(min_max.max.to_compact_length(context)),
        }
      },
    }
  }
}

impl TailwindPropertyParser for GridTrackSize {
  fn parse_tw(token: &str) -> Option<Self> {
    let track_size = match_ignore_ascii_case! {token,
      "auto" => GridTrackSize::Fixed(GridLengthUnit::Unit(LengthUnit::Auto)),
      "fr" => GridTrackSize::Fixed(GridLengthUnit::Fr(1.0)),
      _ => return None,
    };
    Some(track_size)
  }
}

impl<'i> FromCss<'i> for GridTrackSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_function_matching("minmax"))
      .is_ok()
    {
      return input.parse_nested_block(|input| {
        let min = GridLengthUnit::from_css(input)?;
        input.expect_comma()?;
        let max = GridLengthUnit::from_css(input)?;
        Ok(GridTrackSize::MinMax(GridMinMaxSize { min, max }))
      });
    }

    let length = GridLengthUnit::from_css(input)?;
    Ok(GridTrackSize::Fixed(length))
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::LengthUnit;

  use super::*;

  #[test]
  fn test_parse_minmax_and_track_size() {
    assert_eq!(
      GridTrackSize::from_str("minmax(10px, 1fr)"),
      Ok(GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLengthUnit::Unit(LengthUnit::Px(10.0)),
        max: GridLengthUnit::Fr(1.0)
      }))
    );

    assert_eq!(
      GridTrackSize::from_str("2fr"),
      Ok(GridTrackSize::Fixed(GridLengthUnit::Fr(2.0)))
    );
  }
}

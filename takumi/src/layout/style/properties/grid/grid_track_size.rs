use cssparser::{Parser, match_ignore_ascii_case};
use taffy::{MaxTrackSizingFunction, MinTrackSizingFunction, TrackSizingFunction};

use crate::{
  layout::style::{
    CssToken, FromCss, GridLength, GridMinMaxSize, Length, ParseResult, tw::TailwindPropertyParser,
  },
  rendering::Sizing,
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

  fn valid_tokens() -> &'static [CssToken] {
    GridTrackSize::valid_tokens()
  }
}

/// Represents a grid track size
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrackSize {
  /// A minmax() track size
  MinMax(GridMinMaxSize),
  /// A fixed track size
  Fixed(GridLength),
}

impl From<GridLength> for GridTrackSize {
  fn from(length: GridLength) -> Self {
    Self::Fixed(length)
  }
}

impl GridTrackSize {
  /// Converts the grid track size to a non-repeated track sizing function.
  pub(crate) fn to_min_max(self, sizing: &Sizing) -> TrackSizingFunction {
    match self {
      // SAFETY: The compact length is a valid track sizing function.
      Self::Fixed(size) => unsafe {
        TrackSizingFunction {
          min: MinTrackSizingFunction::from_raw(size.to_compact_length(sizing)),
          max: MaxTrackSizingFunction::from_raw(size.to_compact_length(sizing)),
        }
      },
      Self::MinMax(min_max) => unsafe {
        TrackSizingFunction {
          min: MinTrackSizingFunction::from_raw(min_max.min.to_compact_length(sizing)),
          max: MaxTrackSizingFunction::from_raw(min_max.max.to_compact_length(sizing)),
        }
      },
    }
  }
}

impl TailwindPropertyParser for GridTrackSize {
  fn parse_tw(token: &str) -> Option<Self> {
    let track_size = match_ignore_ascii_case! {token,
      "auto" => GridTrackSize::Fixed(GridLength::Unit(Length::Auto)),
      "fr" => GridTrackSize::Fixed(GridLength::Fr(1.0)),
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
        let min = GridLength::from_css(input)?;
        input.expect_comma()?;
        let max = GridLength::from_css(input)?;
        Ok(GridTrackSize::MinMax(GridMinMaxSize { min, max }))
      });
    }

    let length = GridLength::from_css(input)?;
    Ok(GridTrackSize::Fixed(length))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("minmax()"), CssToken::Token("length")]
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::Length;

  use super::*;

  #[test]
  fn test_parse_minmax_and_track_size() {
    assert_eq!(
      GridTrackSize::from_str("minmax(10px, 1fr)"),
      Ok(GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLength::Unit(Length::Px(10.0)),
        max: GridLength::Fr(1.0)
      }))
    );

    assert_eq!(
      GridTrackSize::from_str("2fr"),
      Ok(GridTrackSize::Fixed(GridLength::Fr(2.0)))
    );
  }
}

use std::fmt::Write;

use cssparser::{Parser, match_ignore_ascii_case};
use taffy::{MaxTrackSizingFunction, MinTrackSizingFunction, TrackSizingFunction};

use crate::style::{
  CssDescriptorKind, CssSyntaxKind, CssToken, FromCss, GridLength, GridMinMaxSize, Length,
  MakeComputed, ParseResult, SizingContext, ToCss, tw::TailwindPropertyParser,
};

/// A list of `GridTrackSize`
pub(crate) type GridTrackSizes = Vec<GridTrackSize>;

impl<'i> FromCss<'i> for GridTrackSizes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut components: Vec<GridTrackSize> = Vec::new();
    while let Ok(size) = GridTrackSize::from_css(input) {
      components.push(size);
    }

    Ok(components)
  }

  const VALID_TOKENS: &'static [CssToken] = GridTrackSize::VALID_TOKENS;
}

/// Represents a grid track size
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GridTrackSize {
  /// A minmax() track size
  MinMax(GridMinMaxSize),
  /// A fixed track size
  Fixed(GridLength),
}

impl GridTrackSize {
  pub(crate) fn to_min_max(self, sizing: &SizingContext) -> TrackSizingFunction {
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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Descriptor(CssDescriptorKind::MinmaxFn),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl MakeComputed for GridTrackSize {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      GridTrackSize::MinMax(min_max) => min_max.make_computed(sizing),
      GridTrackSize::Fixed(length) => length.make_computed(sizing),
    }
  }
}

impl ToCss for GridTrackSize {
  fn to_css<W: Write>(&self, dest: &mut W) -> std::fmt::Result {
    match self {
      Self::MinMax(mm) => mm.to_css(dest),
      Self::Fixed(gl) => gl.to_css(dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{FromCssStr, Length};

  #[test]
  fn test_parse_minmax_and_track_size() {
    assert_eq!(
      GridTrackSize::from_css_str("minmax(10px, 1fr)"),
      Ok(GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLength::Unit(Length::Px(10.0)),
        max: GridLength::Fr(1.0)
      }))
    );

    assert_eq!(
      GridTrackSize::from_css_str("2fr"),
      Ok(GridTrackSize::Fixed(GridLength::Fr(2.0)))
    );
  }
}

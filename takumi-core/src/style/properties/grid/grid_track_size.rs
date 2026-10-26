use std::fmt;

use crate::style::{
  FromCss, GridLength, GridMinMaxSize, Length, MakeComputed, ParseResult, SizingContext, ToCss,
  tw::TailwindPropertyParser,
};
use cssparser::{Parser, match_ignore_ascii_case};

/// A list of `GridTrackSize`
pub(crate) type GridTrackSizes = Vec<GridTrackSize>;

impl<'i> FromCss<'i> for GridTrackSizes {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let mut components: Vec<GridTrackSize> = Vec::new();
    while let Ok(size) = input.try_parse(GridTrackSize::from_css) {
      components.push(size);
    }

    Ok(components)
  }

  const VALID_TOKENS: &'static [&'static str] = GridTrackSize::VALID_TOKENS;
}

/// Represents a grid track size
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GridTrackSize {
  /// A minmax() track size
  MinMax(GridMinMaxSize),
  /// A fixed track size
  Fixed(GridLength),
  /// A `fit-content()` track size: its content's size, at most the length.
  FitContent(Length),
}

impl GridTrackSize {}

impl TailwindPropertyParser for GridTrackSize {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "auto" => Some(GridTrackSize::Fixed(GridLength::Unit(Length::Auto))),
      "fr" => Some(GridTrackSize::Fixed(GridLength::Fr(1.0))),
      _ => None,
    }
  }
}

impl<'i> FromCss<'i> for GridTrackSize {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_function_matching("minmax"))
      .is_ok()
    {
      return input.parse_nested_block(|input| {
        let min = GridLength::from_css(input)?;

        if matches!(min, GridLength::Fr(_)) {
          return Err(input.new_error_for_next_token());
        }
        input.expect_comma()?;
        let max = GridLength::from_css(input)?;
        Ok(GridTrackSize::MinMax(GridMinMaxSize { min, max }))
      });
    }
    if input
      .try_parse(|input| input.expect_function_matching("fit-content"))
      .is_ok()
    {
      return input
        .parse_nested_block(|input| Ok(GridTrackSize::FitContent(Length::from_css(input)?)));
    }

    let length = GridLength::from_css(input)?;
    Ok(GridTrackSize::Fixed(length))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<minmax()>", "<fit-content()>", "<length>"];
}

impl MakeComputed for GridTrackSize {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      GridTrackSize::MinMax(min_max) => min_max.make_computed(sizing),
      GridTrackSize::Fixed(length) => length.make_computed(sizing),
      GridTrackSize::FitContent(length) => length.make_computed(sizing),
    }
  }
}

impl ToCss for GridTrackSize {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::MinMax(mm) => mm.to_css(dest),
      Self::Fixed(gl) => gl.to_css(dest),
      Self::FitContent(length) => {
        dest.write_str("fit-content(")?;
        length.to_css(dest)?;
        dest.write_str(")")
      }
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

  #[test]
  fn parses_intrinsic_track_sizes() {
    assert_eq!(
      GridTrackSize::from_css_str("minmax(0px, min-content)"),
      Ok(GridTrackSize::MinMax(GridMinMaxSize {
        min: GridLength::Unit(Length::Px(0.0)),
        max: GridLength::MinContent
      }))
    );
    assert_eq!(
      GridTrackSize::from_css_str("fit-content(100px)"),
      Ok(GridTrackSize::FitContent(Length::Px(100.0)))
    );
    assert!(GridTrackSize::from_css_str("minmax(1fr, 10px)").is_err());
  }
}

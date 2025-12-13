use cssparser::Parser;

use crate::layout::style::{FromCss, GridLengthUnit, ParseResult};

/// Represents a grid minmax()
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMinMaxSize {
  /// The minimum size of the grid item
  pub min: GridLengthUnit,
  /// The maximum size of the grid item
  pub max: GridLengthUnit,
}

impl<'i> FromCss<'i> for GridMinMaxSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    input.expect_function_matching("minmax")?;
    input.parse_nested_block(|input| {
      let min = GridLengthUnit::from_css(input)?;
      input.expect_comma()?;
      let max = GridLengthUnit::from_css(input)?;
      Ok(GridMinMaxSize { min, max })
    })
  }
}

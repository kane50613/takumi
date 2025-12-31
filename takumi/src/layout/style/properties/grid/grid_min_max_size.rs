use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, GridLength, ParseResult};

/// Represents a grid minmax()
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMinMaxSize {
  /// The minimum size of the grid item
  pub min: GridLength,
  /// The maximum size of the grid item
  pub max: GridLength,
}

impl<'i> FromCss<'i> for GridMinMaxSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    input.expect_function_matching("minmax")?;
    input.parse_nested_block(|input| {
      let min = GridLength::from_css(input)?;
      input.expect_comma()?;
      let max = GridLength::from_css(input)?;
      Ok(GridMinMaxSize { min, max })
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("minmax()")]
  }
}

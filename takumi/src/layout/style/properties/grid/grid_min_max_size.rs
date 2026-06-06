use cssparser::Parser;

use crate::{
  layout::style::SizingContext,
  layout::style::{
    CssDescriptorKind, CssToken, FromCss, GridLength, MakeComputed, ParseResult, ToCss,
  },
};

/// Represents a grid minmax()
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
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

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Descriptor(CssDescriptorKind::MinmaxFn)];
}

impl MakeComputed for GridMinMaxSize {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.min.make_computed(sizing);
    self.max.make_computed(sizing);
  }
}

impl ToCss for GridMinMaxSize {
  fn to_css<W: std::fmt::Write>(&self, dest: &mut W) -> std::fmt::Result {
    dest.write_str("minmax(")?;
    self.min.to_css(dest)?;
    dest.write_str(", ")?;
    self.max.to_css(dest)?;
    dest.write_str(")")
  }
}

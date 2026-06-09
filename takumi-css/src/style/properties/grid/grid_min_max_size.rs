use crate::style::{GridLength, MakeComputed, SizingContext, ToCss};

/// Represents a grid minmax()
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct GridMinMaxSize {
  /// The minimum size of the grid item
  pub min: GridLength,
  /// The maximum size of the grid item
  pub max: GridLength,
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

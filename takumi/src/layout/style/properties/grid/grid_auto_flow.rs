use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, ParseResult};

/// Represents the direction of the grid auto flow.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridDirection {
  /// The grid auto flow is in the row direction.
  #[default]
  Row,
  /// The grid auto flow is in the column direction.
  Column,
}

/// Represents the flow of the grid auto placement.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GridAutoFlow {
  /// The direction of the grid auto flow.
  pub direction: GridDirection,
  /// Whether the grid auto flow is dense.
  pub dense: bool,
}

impl From<GridAutoFlow> for taffy::GridAutoFlow {
  fn from(value: GridAutoFlow) -> Self {
    match (value.direction, value.dense) {
      (GridDirection::Row, false) => taffy::GridAutoFlow::Row,
      (GridDirection::Column, false) => taffy::GridAutoFlow::Column,
      (GridDirection::Row, true) => taffy::GridAutoFlow::RowDense,
      (GridDirection::Column, true) => taffy::GridAutoFlow::ColumnDense,
    }
  }
}

impl GridAutoFlow {
  /// The grid auto flow is in the row direction.
  pub const fn row() -> Self {
    Self {
      direction: GridDirection::Row,
      dense: false,
    }
  }

  /// The grid auto flow is in the column direction.
  pub const fn column() -> Self {
    Self {
      direction: GridDirection::Column,
      dense: false,
    }
  }

  /// The grid auto flow is dense.
  pub const fn dense(self) -> Self {
    Self {
      dense: true,
      ..self
    }
  }
}

impl<'i> FromCss<'i> for GridAutoFlow {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut direction = GridDirection::default();
    let mut dense = false;

    loop {
      if input.is_exhausted() {
        break;
      }

      if input
        .try_parse(|input| input.expect_ident_matching("dense"))
        .is_ok()
      {
        dense = true;
        continue;
      }

      if input
        .try_parse(|input| input.expect_ident_matching("row"))
        .is_ok()
      {
        direction = GridDirection::Row;
        continue;
      }

      if input
        .try_parse(|input| input.expect_ident_matching("column"))
        .is_ok()
      {
        direction = GridDirection::Column;
        continue;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(GridAutoFlow { direction, dense })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("row"),
      CssToken::Keyword("column"),
      CssToken::Keyword("dense"),
    ]
  }
}

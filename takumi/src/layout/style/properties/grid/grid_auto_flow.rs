use cssparser::Parser;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult};

/// Represents the direction of the grid auto flow.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GridDirection {
  /// The grid auto flow is in the row direction.
  #[default]
  Row,
  /// The grid auto flow is in the column direction.
  Column,
}

/// Represents the flow of the grid auto placement.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[ts(as = "GridAutoFlowValue")]
#[serde(try_from = "GridAutoFlowValue")]
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

#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(untagged)]
pub(crate) enum GridAutoFlowValue {
  Structured {
    direction: GridDirection,
    dense: bool,
  },
  Css(String),
}

impl TryFrom<GridAutoFlowValue> for GridAutoFlow {
  type Error = String;

  fn try_from(value: GridAutoFlowValue) -> Result<Self, Self::Error> {
    match value {
      GridAutoFlowValue::Structured { direction, dense } => Ok(GridAutoFlow { direction, dense }),
      GridAutoFlowValue::Css(css) => GridAutoFlow::from_str(&css).map_err(|e| e.to_string()),
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
}

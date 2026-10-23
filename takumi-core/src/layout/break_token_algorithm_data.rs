//! Blink's [`BreakTokenAlgorithmData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/break_token_algorithm_data.h):
//! what a layout algorithm keeps in a break token to resume with, under the notice in
//! LICENSE-CHROMIUM.

use std::{fmt, rc::Rc};

use crate::{
  layout::{
    flex::flex_layout_algorithm::FlexBreakTokenData,
    grid::grid_layout_algorithm::GridBreakTokenData, table::TableBreakTokenData,
  },
  layout_unit::LayoutUnit,
};

/// The data an algorithm resumes a box from.
#[derive(Clone)]
pub(crate) enum BreakTokenAlgorithmData {
  Flex(Rc<FlexBreakTokenData>),
  Grid(Rc<GridBreakTokenData>),
  Table(Rc<TableBreakTokenData>),
  /// Blink's `TableRowBreakTokenData`: the block size the row's earlier fragments took.
  TableRow {
    previous_consumed_row_block_size: LayoutUnit,
  },
}

impl PartialEq for BreakTokenAlgorithmData {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Flex(data), Self::Flex(other)) => data == other,
      (Self::Grid(data), Self::Grid(other)) => Rc::ptr_eq(data, other),
      (Self::Table(data), Self::Table(other)) => Rc::ptr_eq(data, other),
      (
        Self::TableRow {
          previous_consumed_row_block_size,
        },
        Self::TableRow {
          previous_consumed_row_block_size: other,
        },
      ) => previous_consumed_row_block_size == other,
      _ => false,
    }
  }
}

impl fmt::Debug for BreakTokenAlgorithmData {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Flex(_) => formatter.write_str("Flex"),
      Self::Grid(_) => formatter.write_str("Grid"),
      Self::Table(_) => formatter.write_str("Table"),
      Self::TableRow {
        previous_consumed_row_block_size,
      } => formatter
        .debug_struct("TableRow")
        .field(
          "previous_consumed_row_block_size",
          previous_consumed_row_block_size,
        )
        .finish(),
    }
  }
}

//! Blink's [`TableRowLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_row_layout_algorithm.cc),
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::Point,
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    content_alignment::BlockContentAlignment,
    layout_result::LayoutResult,
    logical_rect::LogicalSize,
    table::{
      columns::span_inline_size, constraint_space_data::TableConstraintSpaceData,
      rows::cell_block_size,
    },
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
};

/// Lays a row's cells out at the block size of the rows each spans, its content aligned in it.
pub(super) struct TableRowLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  data: &'t Rc<TableConstraintSpaceData>,
  section: usize,
  row: usize,
}

impl<'t, 'r> TableRowLayoutAlgorithm<'t, 'r> {
  pub(super) fn new(
    tree: &'t mut LayoutTree<'r>,
    data: &'t Rc<TableConstraintSpaceData>,
    section: usize,
    row: usize,
  ) -> Self {
    Self {
      tree,
      data,
      section,
      row,
    }
  }

  /// Blink's `Layout`.
  pub(super) fn layout(self) -> LayoutResult {
    let data = self.data;
    let rows = &data.rows;
    let row_index = rows.sections[self.section].start_row + self.row;
    let row = &data.structure.sections[self.section].rows[self.row];
    let row_data = &rows.rows[row_index];
    let mut out_of_flow = Vec::new();

    for (index, cell) in row.cells.iter().enumerate() {
      let cell_index = row_data.start_cell_index + index;
      let constraint = &rows.cells[cell_index];
      let measure = rows.measures[cell_index];
      let (rowspan_block_size, is_initial_block_size_indefinite) = cell_block_size(
        constraint,
        &rows.rows,
        row_index,
        data.spacing.1,
        data.is_table_block_size_specified,
      );
      let block_size = if constraint.effective_rowspan > 1 {
        rowspan_block_size
      } else {
        row_data.block_size
      };
      let inline_size = span_inline_size(&data.column_locations, cell.column, cell.colspan);
      let render_node = self.tree.render_node(cell.node);
      let alignment = BlockContentAlignment::for_table_cell(&render_node.context.style);
      let shift = match (alignment, row_data.baseline, measure.first_baseline) {
        (BlockContentAlignment::Baseline, Some(row), Some(cell)) => row - cell,
        _ => alignment.offset(block_size - measure.block_size),
      };
      // The content lays out moved down, in a content box the size it measured at unless the
      // cell's block size is definite.
      let below = if is_initial_block_size_indefinite {
        (block_size - measure.block_size - shift).clamp_negative_to_zero()
      } else {
        LayoutUnit::ZERO
      };
      let padding = self.tree.padding(cell.node, Some(data.inline_size));

      self.tree.set_table_cell_aligned_padding(
        cell.node,
        Some((
          (padding.top + shift).to_float(),
          (padding.bottom + below).to_float(),
        )),
      );

      let space = ConstraintSpace::for_table_cell(
        self.tree.layout_style(cell.node).direction,
        inline_size,
        Some(block_size),
        is_initial_block_size_indefinite,
        data.inline_size,
        CacheSlot::Layout,
      );
      let result = self.tree.layout(cell.node, &space);
      let start = data
        .column_locations
        .get(cell.column)
        .map_or(LayoutUnit::ZERO, |location| location.offset)
        - data.spacing.0;
      let x = if data.rtl {
        data.inline_size - start - inline_size
      } else {
        start
      };
      let location = Point::new(x, LayoutUnit::ZERO);

      self.tree.place_child(cell.node, 0, location, &result);
      out_of_flow.extend(
        result
          .out_of_flow
          .iter()
          .map(|candidate| candidate.moved_by(location)),
      );
    }

    let mut result = LayoutResult::new(LogicalSize::new(data.inline_size, row_data.block_size));

    result.out_of_flow = out_of_flow;
    result
  }
}

//! Blink's [`TableSectionLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_section_layout_algorithm.cc),
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::Point,
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    layout_result::LayoutResult,
    logical_rect::LogicalSize,
    table::constraint_space_data::{TableConstraintSpaceData, TablePart},
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
};

/// Stacks a section's rows at the block sizes its table gave them.
pub(super) struct TableSectionLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  data: &'t Rc<TableConstraintSpaceData>,
  section: usize,
  space: &'t ConstraintSpace,
}

impl<'t, 'r> TableSectionLayoutAlgorithm<'t, 'r> {
  pub(super) fn new(
    tree: &'t mut LayoutTree<'r>,
    data: &'t Rc<TableConstraintSpaceData>,
    section: usize,
    space: &'t ConstraintSpace,
  ) -> Self {
    Self {
      tree,
      data,
      section,
      space,
    }
  }

  /// Blink's `Layout`, which lays the rows out only when the layout places them.
  pub(super) fn layout(self) -> LayoutResult {
    let data = self.data;
    let section = &data.structure.sections[self.section];
    let start_row = data.rows.sections[self.section].start_row;
    let extents = data.row_extents(self.section);
    let mut out_of_flow = Vec::new();

    if self.space.cache_slot == CacheSlot::Layout {
      for (index, (row, &(start, _))) in section.rows.iter().zip(&extents).enumerate() {
        let part = TablePart::Row {
          section: self.section,
          row: index,
        };
        let space = data.space(part, self.space.direction, CacheSlot::Layout);
        let result = self.tree.layout(row.node, &space);
        let location = Point::new(LayoutUnit::ZERO, start);

        self.tree.place_child(row.node, 0, location, &result);
        out_of_flow.extend(
          result
            .out_of_flow
            .iter()
            .map(|candidate| candidate.moved_by(location)),
        );
      }
    }

    let block_size = extents
      .last()
      .map_or(data.rows.sections[self.section].block_size, |&(_, end)| end);
    let mut result = LayoutResult::new(LogicalSize::new(data.inline_size, block_size));

    result.first_baseline = extents.first().map(|&(start, _)| {
      start
        + data.rows.rows[start_row]
          .baseline
          .unwrap_or(LayoutUnit::ZERO)
    });
    result.out_of_flow = out_of_flow;
    result
  }
}

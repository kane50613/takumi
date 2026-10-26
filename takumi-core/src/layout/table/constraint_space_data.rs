//! Blink's [`TableConstraintSpaceData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_constraint_space_data.h):
//! what a table computed once for the sections and rows it lays out, under the notice in
//! LICENSE-CHROMIUM.

use std::{
  fmt,
  rc::Rc,
  sync::atomic::{AtomicU64, Ordering},
};

use crate::{
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    fragmentation_layout_input::FragmentationLayoutInput,
    layout_result::LayoutResult,
    table::{
      columns::ColumnLocation, layout::TableStructure,
      row_layout_algorithm::TableRowLayoutAlgorithm, rows::Rows,
      section_layout_algorithm::TableSectionLayoutAlgorithm,
    },
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
  style::Direction,
};

static NEXT_LAYOUT_ID: AtomicU64 = AtomicU64::new(0);

/// The grid a table laid out, which its sections and rows place their parts in.
pub(crate) struct TableConstraintSpaceData {
  /// The table layout that made it, so a section or row reuses a layout only within that one.
  layout_id: u64,
  pub(super) structure: Rc<TableStructure>,
  pub(super) rows: Rc<Rows>,
  pub(super) column_locations: Vec<ColumnLocation>,
  /// The inline size of every section and row.
  pub(super) inline_size: LayoutUnit,
  /// The inline and block `border-spacing`, none when borders collapse or the grid is empty.
  pub(super) spacing: (LayoutUnit, LayoutUnit),
  pub(super) is_table_block_size_specified: bool,
  pub(super) rtl: bool,
}

impl TableConstraintSpaceData {
  pub(super) fn new(
    structure: Rc<TableStructure>,
    rows: Rc<Rows>,
    column_locations: Vec<ColumnLocation>,
    inline_size: LayoutUnit,
    spacing: (LayoutUnit, LayoutUnit),
    is_table_block_size_specified: bool,
    rtl: bool,
  ) -> Self {
    Self {
      layout_id: NEXT_LAYOUT_ID.fetch_add(1, Ordering::Relaxed),
      structure,
      rows,
      column_locations,
      inline_size,
      spacing,
      is_table_block_size_specified,
      rtl,
    }
  }

  /// Where each row of section `section_index` starts and ends in it.
  pub(super) fn row_extents(&self, section_index: usize) -> Vec<(LayoutUnit, LayoutUnit)> {
    let section = &self.rows.sections[section_index];
    let mut offset = LayoutUnit::ZERO;
    let mut is_first_non_collapsed_row = true;

    self.rows.rows[section.start_row..section.start_row + section.row_count]
      .iter()
      .map(|row| {
        if !is_first_non_collapsed_row && !row.is_collapsed {
          offset += self.spacing.1;
        }

        let start = offset;

        offset += row.block_size;
        is_first_non_collapsed_row &= row.is_collapsed;
        (start, offset)
      })
      .collect()
  }

  /// The space `part` lays out in, in the table's direction as Blink's
  /// `table_writing_direction`.
  pub(super) fn space(self: &Rc<Self>, part: TablePart, cache_slot: CacheSlot) -> ConstraintSpace {
    let direction = if self.rtl {
      Direction::Rtl
    } else {
      Direction::Ltr
    };
    let mut space = ConstraintSpace::new(Some(self.inline_size), direction);

    space.is_fixed_inline_size = true;
    space.cache_slot = cache_slot;
    space.table_part = Some(TablePartSpace {
      data: Rc::clone(self),
      part,
    });
    space
  }
}

/// A part of a table's grid that lays out on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TablePart {
  Section(usize),
  /// The row `row` of the section `section`.
  Row {
    section: usize,
    row: usize,
  },
}

/// The table part a space lays out, with the grid its table computed: Blink's `TableData` with
/// its section or row index.
#[derive(Clone)]
pub(crate) struct TablePartSpace {
  data: Rc<TableConstraintSpaceData>,
  part: TablePart,
}

impl TablePartSpace {
  /// Blink's `DetermineAlgorithmAndRun` for a table section or row.
  pub(crate) fn lay_out(
    &self,
    tree: &mut LayoutTree<'_>,
    space: &ConstraintSpace,
    input: &FragmentationLayoutInput,
  ) -> LayoutResult {
    match self.part {
      TablePart::Section(section) => {
        TableSectionLayoutAlgorithm::new(tree, &self.data, section, space, input).layout()
      }
      TablePart::Row { section, row } => {
        TableRowLayoutAlgorithm::new(tree, &self.data, section, row, space, input).layout()
      }
    }
  }
}

impl PartialEq for TablePartSpace {
  fn eq(&self, other: &Self) -> bool {
    self.data.layout_id == other.data.layout_id && self.part == other.part
  }
}

impl fmt::Debug for TablePartSpace {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("TablePartSpace")
      .field("layout_id", &self.data.layout_id)
      .field("part", &self.part)
      .finish()
  }
}

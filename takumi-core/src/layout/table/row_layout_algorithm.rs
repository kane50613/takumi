//! Blink's [`TableRowLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_row_layout_algorithm.cc),
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::{NodeId, Point},
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    content_alignment::BlockContentAlignment,
    fragmentation_layout_input::{ChildBreakToken, FragmentationLayoutInput},
    layout_result::LayoutResult,
    logical_rect::LogicalSize,
    table::{
      columns::span_inline_size,
      constraint_space_data::TableConstraintSpaceData,
      rows::{CellMeasure, RowBaselineTabulator, cell_block_size},
    },
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
};
#[cfg(feature = "fragmentation")]
use crate::{
  layout::{
    box_fragment_builder::BoxFragmentBuilder,
    break_token_algorithm_data::BreakTokenAlgorithmData,
    fragmentation_utils::{
      finish_fragmentation, fragment_index, fragmentation_layout_input_for_child,
      indexed_block_child_entries, is_break_inside, join_fragmentainer_break_values,
      setup_fragment_builder_for_fragmentation, setup_space_builder_for_fragmentation,
    },
    logical_rect::LogicalBoxStrut,
  },
  style::BreakBetween,
};

/// A cell laid out for the row to place.
struct PlacedCell {
  node: NodeId,
  #[cfg_attr(not(feature = "fragmentation"), allow(dead_code))]
  token: ChildBreakToken,
  location: Point<LayoutUnit>,
  result: Rc<LayoutResult>,
}

/// What laying a row's cells out found.
#[derive(Default)]
struct PlacedCells {
  cells: Vec<PlacedCell>,
  /// The tallest cell that grows the row past the block size its table gave it.
  max_cell_block_size: LayoutUnit,
  /// Whether a cell spanning only this row broke inside.
  has_inflow_break_inside: bool,
  /// The cells' baselines, which give the row's.
  baselines: RowBaselineTabulator,
  #[cfg(feature = "fragmentation")]
  break_before: BreakBetween,
  #[cfg(feature = "fragmentation")]
  break_after: BreakBetween,
}

/// Lays a row's cells out at the block size of the rows each spans, its content aligned in it.
pub(super) struct TableRowLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  data: &'t Rc<TableConstraintSpaceData>,
  section: usize,
  row: usize,
  space: &'t ConstraintSpace,
  #[cfg_attr(not(feature = "fragmentation"), allow(dead_code))]
  input: &'t FragmentationLayoutInput,
  #[cfg(feature = "fragmentation")]
  builder: BoxFragmentBuilder,
}

impl<'t, 'r> TableRowLayoutAlgorithm<'t, 'r> {
  pub(super) fn new(
    tree: &'t mut LayoutTree<'r>,
    data: &'t Rc<TableConstraintSpaceData>,
    section: usize,
    row: usize,
    space: &'t ConstraintSpace,
    input: &'t FragmentationLayoutInput,
  ) -> Self {
    #[cfg(feature = "fragmentation")]
    let builder = {
      let node = data.structure.sections[section].rows[row].node;
      let style = Rc::clone(tree.layout_style_rc(node));
      let mut builder = BoxFragmentBuilder::new(
        space,
        &style,
        LogicalBoxStrut::default(),
        input.break_token.clone(),
      );

      setup_fragment_builder_for_fragmentation(space, &style, data.inline_size, &mut builder);
      builder
    };

    Self {
      tree,
      data,
      section,
      row,
      space,
      input,
      #[cfg(feature = "fragmentation")]
      builder,
    }
  }

  fn row_index(&self) -> usize {
    self.data.rows.sections[self.section].start_row + self.row
  }

  /// Blink's `Layout`.
  pub(super) fn layout(mut self) -> LayoutResult {
    let row = &self.data.rows.rows[self.row_index()];
    let given_block_size = row.block_size;
    // A baseline a percentage block size moves comes from the cells laid out at the row's size.
    // Blink aligns no baselines under block fragmentation, as the alignment baseline may end up
    // on another page.
    let row_baseline = match row.baseline {
      _ if self.space.has_block_fragmentation() => None,
      Some(baseline) => Some(baseline),
      None => Some(
        self
          .place_cells(given_block_size, None)
          .baselines
          .baseline(given_block_size),
      ),
    };
    let mut placed = self.place_cells(given_block_size, row_baseline);
    let previously_consumed_row_block_size = self.previously_consumed_row_block_size();
    let row_block_size =
      (placed.max_cell_block_size + previously_consumed_row_block_size).max(given_block_size);

    // A cell that a break pushed down grows the row, and its other cells with it.
    if self.space.has_block_fragmentation() && row_block_size != given_block_size {
      placed = self.place_cells(row_block_size, None);
    }

    let mut out_of_flow = Vec::new();

    for cell in &placed.cells {
      self.place_cell(cell);
      out_of_flow.extend(
        cell
          .result
          .out_of_flow
          .iter()
          .map(|candidate| candidate.moved_by(cell.location)),
      );
    }

    let block_size =
      self.finish_fragmentation(&placed, row_block_size, previously_consumed_row_block_size);
    let mut result = LayoutResult::new(LogicalSize::new(self.data.inline_size, block_size));

    result.out_of_flow = out_of_flow;
    self.finish_break_info(&mut result);
    result
  }

  /// Blink's `PlaceCells`: each cell laid out at the block size of the rows it spans, the row
  /// `row_block_size` tall, a baseline-aligned one aligned to `row_baseline`.
  fn place_cells(
    &mut self,
    row_block_size: LayoutUnit,
    row_baseline: Option<LayoutUnit>,
  ) -> PlacedCells {
    let data = Rc::clone(self.data);
    let rows = &data.rows;
    let row_index = self.row_index();
    let row = &data.structure.sections[self.section].rows[self.row];
    let row_data = &rows.rows[row_index];
    let mut placed = PlacedCells::default();

    for (index, token) in self.cell_entries() {
      let cell = &row.cells[index];
      let cell_index = row_data.start_cell_index + index;
      let constraint = &rows.cells[cell_index];
      let (rowspan_block_size, is_initial_block_size_indefinite) = cell_block_size(
        constraint,
        &rows.rows,
        row_index,
        data.spacing.1,
        data.is_table_block_size_specified,
      );
      let has_rowspan = constraint.effective_rowspan > 1;
      let block_size = if has_rowspan {
        rowspan_block_size
      } else {
        self.resumed_cell_block_size(row_block_size, &token)
      };
      let encompasses_intrinsic_size = self.encompasses_intrinsic_size(
        cell.node,
        !has_rowspan || rowspan_block_size == row_data.block_size,
      );
      let inline_size = span_inline_size(&data.column_locations, cell.column, cell.colspan);
      let mut space = ConstraintSpace::for_table_cell(
        self.tree.layout_style(cell.node).direction,
        inline_size,
        Some(block_size),
        is_initial_block_size_indefinite,
        data.inline_size,
        CacheSlot::Layout,
      );

      space.table_cell_alignment_baseline = row_baseline;
      self.setup_cell_space_for_fragmentation(&mut space, cell.node, encompasses_intrinsic_size);

      let result = self.lay_out_cell(cell.node, &space, &token);
      let render_node = self.tree.render_node(cell.node);
      let padding = self.tree.padding(cell.node, Some(data.inline_size));

      placed.baselines.process_cell(
        CellMeasure {
          block_size: result.size.block_size,
          first_baseline: result.first_baseline,
        },
        cell.borders.bottom + padding.bottom,
        BlockContentAlignment::for_table_cell(&render_node.context.style),
        render_node.has_table_cell_content(),
        has_rowspan,
        constraint.has_descendant_that_depends_on_percentage_block_size,
      );

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

      self.join_cell_break_values(&mut placed, cell.node, &result);
      if encompasses_intrinsic_size {
        placed.max_cell_block_size = placed.max_cell_block_size.max(result.size.block_size);
      }
      if !has_rowspan && !placed.has_inflow_break_inside {
        placed.has_inflow_break_inside = Self::breaks_inside(&result);
      }
      placed.cells.push(PlacedCell {
        node: cell.node,
        token,
        location: Point::new(x, LayoutUnit::ZERO),
        result,
      });
    }
    placed
  }
}

#[cfg(feature = "fragmentation")]
impl TableRowLayoutAlgorithm<'_, '_> {
  /// Blink's `BlockChildIterator` over the row's cells, by index in the row.
  fn cell_entries(&self) -> Vec<(usize, ChildBreakToken)> {
    let cells: Vec<NodeId> = self.data.structure.sections[self.section].rows[self.row]
      .cells
      .iter()
      .map(|cell| cell.node)
      .collect();

    indexed_block_child_entries(&cells, self.input.break_token.as_deref())
  }

  /// A cell's block size in this fragment: what the row gives it, less what its earlier fragments
  /// took past the row's.
  fn resumed_cell_block_size(
    &self,
    row_block_size: LayoutUnit,
    token: &ChildBreakToken,
  ) -> LayoutUnit {
    match (self.input.break_token.as_deref(), token.as_deref()) {
      (Some(row_token), Some(cell_token))
        if is_break_inside(Some(row_token)) && is_break_inside(Some(cell_token)) =>
      {
        row_block_size - (row_token.consumed_block_size - cell_token.consumed_block_size)
      }
      _ => row_block_size,
    }
  }

  /// Blink's `MinBlockSizeShouldEncompassIntrinsicSize` for a breakable `cell` that spans only
  /// the rows of this row's block size when `spans_this_row`.
  fn encompasses_intrinsic_size(&self, cell: NodeId, spans_this_row: bool) -> bool {
    self.space.has_block_fragmentation() && !self.tree.is_monolithic(cell) && spans_this_row
  }

  fn setup_cell_space_for_fragmentation(
    &self,
    space: &mut ConstraintSpace,
    cell: NodeId,
    encompasses_intrinsic_size: bool,
  ) {
    setup_space_builder_for_fragmentation(
      self.space,
      &self.builder,
      self.tree.is_monolithic(cell),
      LayoutUnit::ZERO,
      space,
    );
    if encompasses_intrinsic_size && let Some(fragmentation) = space.block_fragmentation.as_mut() {
      fragmentation.min_block_size_should_encompass_intrinsic_size = true;
    }
  }

  fn lay_out_cell(
    &mut self,
    cell: NodeId,
    space: &ConstraintSpace,
    token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    let input =
      fragmentation_layout_input_for_child(cell, token.clone(), self.input.early_break.as_deref());

    self.tree.layout_fragment(cell, space, &input)
  }

  fn join_cell_break_values(&self, placed: &mut PlacedCells, cell: NodeId, result: &LayoutResult) {
    if !self.space.should_propagate_child_break_values() {
      return;
    }

    let style = self.tree.layout_style(cell);
    let cell_break_before = join_fragmentainer_break_values(
      style.breaks.break_before,
      result.break_info.initial_break_before.unwrap_or_default(),
    );
    let cell_break_after = join_fragmentainer_break_values(
      style.breaks.break_after,
      result.break_info.final_break_after.unwrap_or_default(),
    );

    placed.break_before = join_fragmentainer_break_values(placed.break_before, cell_break_before);
    placed.break_after = join_fragmentainer_break_values(placed.break_after, cell_break_after);
  }

  fn breaks_inside(result: &LayoutResult) -> bool {
    result
      .break_info
      .break_token
      .as_deref()
      .is_some_and(|token| !token.is_at_block_end)
  }

  fn place_cell(&mut self, cell: &PlacedCell) {
    self.tree.place_child(
      cell.node,
      fragment_index(cell.token.as_deref()),
      fragment_index(self.input.break_token.as_deref()),
      cell.location,
      &cell.result,
    );
    if self.space.has_block_fragmentation() {
      let style = Rc::clone(self.tree.layout_style_rc(cell.node));

      self
        .builder
        .add_result(self.space, &style, &cell.result, LayoutUnit::ZERO);
    }
  }

  fn previously_consumed_row_block_size(&self) -> LayoutUnit {
    match self
      .input
      .break_token
      .as_deref()
      .filter(|token| is_break_inside(Some(token)))
      .and_then(|token| token.algorithm_data.as_ref())
    {
      Some(BreakTokenAlgorithmData::TableRow {
        previous_consumed_row_block_size,
      }) => *previous_consumed_row_block_size,
      _ => LayoutUnit::ZERO,
    }
  }

  /// The row's block size in this fragment, the row `row_block_size` tall across its fragments.
  fn finish_fragmentation(
    &mut self,
    placed: &PlacedCells,
    row_block_size: LayoutUnit,
    previously_consumed_row_block_size: LayoutUnit,
  ) -> LayoutUnit {
    self.builder.has_seen_all_children = true;
    self.builder.is_known_to_fit_in_fragmentainer = !placed.has_inflow_break_inside;
    self.builder.intrinsic_block_size = placed.max_cell_block_size;
    self.builder.fragments_total_block_size = row_block_size;
    if self.space.should_propagate_child_break_values() {
      self.builder.initial_break_before = Some(placed.break_before);
      self.builder.previous_break_after = placed.break_after;
    }
    if !self.builder.involved_in_block_fragmentation() {
      return row_block_size;
    }

    let status = finish_fragmentation(&mut self.builder);

    debug_assert!(
      status.is_ok(),
      "a row breaks only inside its cells, never earlier"
    );
    self.builder.break_token_data = Some(BreakTokenAlgorithmData::TableRow {
      previous_consumed_row_block_size: previously_consumed_row_block_size
        + self.builder.fragment_block_size,
    });
    self.builder.fragment_block_size
  }

  fn finish_break_info(&mut self, result: &mut LayoutResult) {
    let node = self.data.structure.sections[self.section].rows[self.row].node;

    result.break_info = self.builder.finish_break_info(node);
  }
}

/// The fragmentation hooks without the `fragmentation` feature, where nothing breaks.
#[cfg(not(feature = "fragmentation"))]
impl TableRowLayoutAlgorithm<'_, '_> {
  fn cell_entries(&self) -> Vec<(usize, ChildBreakToken)> {
    (0..self.data.structure.sections[self.section].rows[self.row]
      .cells
      .len())
      .map(|index| (index, ()))
      .collect()
  }

  fn resumed_cell_block_size(
    &self,
    row_block_size: LayoutUnit,
    _token: &ChildBreakToken,
  ) -> LayoutUnit {
    row_block_size
  }

  fn encompasses_intrinsic_size(&self, _cell: NodeId, _spans_this_row: bool) -> bool {
    false
  }

  fn setup_cell_space_for_fragmentation(
    &self,
    _space: &mut ConstraintSpace,
    _cell: NodeId,
    _encompasses_intrinsic_size: bool,
  ) {
  }

  fn lay_out_cell(
    &mut self,
    cell: NodeId,
    space: &ConstraintSpace,
    _token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    self.tree.layout(cell, space)
  }

  fn join_cell_break_values(
    &self,
    _placed: &mut PlacedCells,
    _cell: NodeId,
    _result: &LayoutResult,
  ) {
  }

  fn breaks_inside(_result: &LayoutResult) -> bool {
    false
  }

  fn place_cell(&mut self, cell: &PlacedCell) {
    self
      .tree
      .place_child(cell.node, 0, 0, cell.location, &cell.result);
  }

  fn previously_consumed_row_block_size(&self) -> LayoutUnit {
    LayoutUnit::ZERO
  }

  fn finish_fragmentation(
    &mut self,
    _placed: &PlacedCells,
    row_block_size: LayoutUnit,
    _previously_consumed_row_block_size: LayoutUnit,
  ) -> LayoutUnit {
    row_block_size
  }

  fn finish_break_info(&mut self, _result: &mut LayoutResult) {}
}

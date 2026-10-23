//! Blink's block fragmentation in
//! [`GridLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_layout_algorithm.cc)
//! and its [`GridBreakTokenData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_break_token_data.h),
//! for pages alone, without gap decorations.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::take, rc::Rc};

use super::{ForRows, GridBaselineAccumulator, GridLayoutAlgorithm, GridSizingTree, PlacedChild};
use crate::{
  geometry::{NodeId, Point},
  layout::{
    break_token::BreakToken,
    break_token_algorithm_data::BreakTokenAlgorithmData,
    fragmentation_layout_input::FragmentationLayoutInput,
    fragmentation_utils::{
      calculate_break_appeal_before, finish_fragmentation, fragment_index,
      fragmentation_layout_input_for_child, is_avoid_break_value, is_break_inside,
      is_forced_break_value, join_fragmentainer_break_values, move_past_breakpoint_without_builder,
      setup_space_builder_for_fragmentation,
    },
    grid::{grid_item::GridItemData, grid_track_collection::TrackSpanProperties},
    layout_result::{LayoutResult, LayoutStatus, OutOfFlowCandidate},
    logical_rect::{LogicalBoxStrut, LogicalOffset},
    out_of_flow::{ContainingBlock, ContainingBlockClaims},
  },
  layout_unit::LayoutUnit,
  style::{BoxDecorationBreak, BreakBetween, Overflow},
};

/// Blink's `GridItemPlacementData`: where an item sits in the grid laid out in one piece.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GridItemPlacementData {
  /// The border box's offset after alignment.
  offset: LogicalOffset,
  relative_offset: Point<LayoutUnit>,
  has_descendant_that_depends_on_percentage_block_size: bool,
}

/// Blink's `GridBreakTokenData`: the grid laid out in one piece, which each page lays its items out
/// from.
#[derive(Clone)]
pub(crate) struct GridBreakTokenData {
  sizing_tree: GridSizingTree,
  out_of_flow_children: Vec<NodeId>,
  intrinsic_block_size: LayoutUnit,
  /// Blink's `offset_in_stitched_container`: where the next page starts in the grid.
  offset_in_stitched_container: LayoutUnit,
  placement_data: Vec<GridItemPlacementData>,
  /// How far breaks moved each row and those after it.
  row_offset_adjustments: Vec<LayoutUnit>,
  row_break_between: Vec<BreakBetween>,
}

/// The state of Blink's `GridLayoutAlgorithm` that block fragmentation reads and writes.
#[derive(Default)]
pub(super) struct GridFragmentation {
  input: FragmentationLayoutInput,
  row_break_between: Vec<BreakBetween>,
  placement_data: Vec<GridItemPlacementData>,
  row_offset_adjustments: Vec<LayoutUnit>,
  offset_in_stitched_container: LayoutUnit,
  previous_offset_in_stitched_container: LayoutUnit,
}

impl GridFragmentation {
  pub(super) fn new(input: &FragmentationLayoutInput) -> Self {
    Self {
      input: input.clone(),
      ..Self::default()
    }
  }
}

/// What laying the items out on a page found: Blink's `PlaceItems` lambda.
#[derive(Default)]
struct GridPage {
  placed: Vec<(NodeId, Rc<LayoutResult>, LogicalOffset, Point<LayoutUnit>)>,
  baselines: GridBaselineAccumulator,
  max_row_expansion: LayoutUnit,
  max_item_block_end: LayoutUnit,
  expansion_row_set_index: Option<usize>,
  breakpoint_row_set_index: Option<usize>,
  has_subsequent_children: bool,
}

impl GridPage {
  /// Blink's `UpdateBreakpointRowSetIndex`: the earliest row to break before.
  fn update_breakpoint_row_set_index(&mut self, row_set_index: usize) {
    if self
      .breakpoint_row_set_index
      .is_none_or(|breakpoint| row_set_index < breakpoint)
    {
      self.breakpoint_row_set_index = Some(row_set_index);
    }
  }
}

impl GridLayoutAlgorithm<'_, '_> {
  pub(super) fn is_involved_in_block_fragmentation(&self) -> bool {
    self.builder.involved_in_block_fragmentation()
  }

  pub(super) fn is_resuming(&self) -> bool {
    is_break_inside(self.fragmentation.input.break_token.as_deref())
  }

  pub(super) fn fragment_index(&self) -> usize {
    fragment_index(self.fragmentation.input.break_token.as_deref())
  }

  pub(super) fn border_padding_for_size(&self, border_padding: LogicalBoxStrut) -> LogicalBoxStrut {
    self.builder.border_padding_for_size(border_padding)
  }

  pub(super) fn previously_consumed_block_size(&self) -> LayoutUnit {
    self.builder.previously_consumed_block_size()
  }

  pub(super) fn fragmentainer_capacity_for_children(&self) -> LayoutUnit {
    self
      .builder
      .fragmentainer_capacity(true)
      .unwrap_or(LayoutUnit::MAX)
  }

  /// Whether this fragment places the out-of-flow children past the page, which it does unless
  /// the grid goes on.
  pub(super) fn should_process_out_of_flow_block_end(&self) -> bool {
    !self.is_involved_in_block_fragmentation() || !self.builder.should_break()
  }

  /// The grid an earlier page laid out, which this one resumes from: its sizing tree, the
  /// out-of-flow children it left, and its intrinsic block size.
  pub(super) fn resumed_sizing_tree(
    &mut self,
  ) -> Option<(GridSizingTree, Vec<NodeId>, LayoutUnit)> {
    if !self.is_resuming() {
      return None;
    }

    let token = self.fragmentation.input.break_token.as_deref()?;
    let Some(BreakTokenAlgorithmData::Grid(data)) = &token.algorithm_data else {
      return None;
    };
    let mut intrinsic_block_size = data.intrinsic_block_size;

    // A cloned fragment repeats the border and padding the grid laid out in one piece once.
    if self.style.breaks.box_decoration_break == BoxDecorationBreak::Clone && !token.is_at_block_end
    {
      intrinsic_block_size += self.builder.border_padding.block_sum();
    }
    self.fragmentation.offset_in_stitched_container = data.offset_in_stitched_container;
    self.fragmentation.previous_offset_in_stitched_container = data.offset_in_stitched_container;
    self.fragmentation.placement_data = data.placement_data.clone();
    self.fragmentation.row_offset_adjustments = data.row_offset_adjustments.clone();
    self.fragmentation.row_break_between = data.row_break_between.clone();
    Some((
      data.sizing_tree.clone(),
      data.out_of_flow_children.clone(),
      intrinsic_block_size,
    ))
  }

  /// Blink's `row_break_between` before the items join theirs in: one value between each row.
  pub(super) fn reset_row_break_between(&mut self, set_count: usize) {
    if self.space.should_propagate_child_break_values() {
      self.fragmentation.row_break_between = vec![BreakBetween::Auto; set_count + 1];
    }
  }

  /// Joins the break values of `item` laid out as `result` into those before and after its rows.
  pub(super) fn join_row_break_between(&mut self, item: &GridItemData, result: &LayoutResult) {
    if !self.space.should_propagate_child_break_values() {
      return;
    }

    let break_before = join_fragmentainer_break_values(
      item.style.breaks.break_before,
      result.break_info.initial_break_before.unwrap_or_default(),
    );
    let break_after = join_fragmentainer_break_values(
      item.style.breaks.break_after,
      result.break_info.final_break_after.unwrap_or_default(),
    );
    let (begin, end) = item.set_indices(ForRows);
    let row_break_between = &mut self.fragmentation.row_break_between;

    row_break_between[begin] =
      join_fragmentainer_break_values(row_break_between[begin], break_before);
    row_break_between[end] = join_fragmentainer_break_values(row_break_between[end], break_after);
  }

  /// Keeps where an item sits, at `offset` moved by `relative_offset`, for each page to lay it
  /// out from: whether the grid breaks, and so does not place it now.
  pub(super) fn keeps_placement(
    &mut self,
    item: &GridItemData,
    offset: LogicalOffset,
    relative_offset: Point<LayoutUnit>,
  ) -> bool {
    if !self.is_involved_in_block_fragmentation() {
      return false;
    }

    let has_descendant_that_depends_on_percentage_block_size = self
      .tree
      .render_node(item.node)
      .has_descendant_that_depends_on_percentage_block_size();

    self
      .fragmentation
      .placement_data
      .push(GridItemPlacementData {
        offset,
        relative_offset,
        has_descendant_that_depends_on_percentage_block_size,
      });
    true
  }

  /// Blink's `PlaceGridItemsForFragmentation`: the items laid out on this page, rows grown to fit
  /// them and moved past a break, or the `unfragmented` ones when the grid does not break.
  pub(super) fn place_grid_items_for_fragmentation(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    intrinsic_block_size: &mut LayoutUnit,
    unfragmented: (Vec<PlacedChild>, GridBaselineAccumulator),
  ) -> (Vec<PlacedChild>, GridBaselineAccumulator) {
    if !self.is_involved_in_block_fragmentation() {
      return unfragmented;
    }
    if !self.is_resuming() {
      self.fragmentation.row_offset_adjustments =
        vec![LayoutUnit::ZERO; sizing_tree.layout_data.rows.set_count() + 1];
    }

    let mut fragmentainer_space = self
      .space
      .has_block_fragmentation()
      .then(|| self.builder.fragmentainer_space_left(true));
    let cloned_block_start_decoration = if fragmentainer_space.is_some() {
      self.builder.cloned_block_start_decoration()
    } else {
      LayoutUnit::ZERO
    };

    if let Some(space) = &mut fragmentainer_space {
      *space -= cloned_block_start_decoration;
    }

    let mut previous_expansion_row_set_index = None;
    let mut page = self.place_items_on_page(
      sizing_tree,
      fragmentainer_space,
      cloned_block_start_decoration,
      previous_expansion_row_set_index,
    );

    // Blink's `ExpandRow`: a row grows to fit an item that ends lower, and the items lay out
    // again.
    while page.max_row_expansion > LayoutUnit::ZERO
      && let Some(expansion_row_set_index) = page.expansion_row_set_index
    {
      *intrinsic_block_size += page.max_row_expansion;
      self.adjust_item_offsets(
        sizing_tree,
        expansion_row_set_index + 1,
        page.max_row_expansion,
      );
      sizing_tree
        .layout_data
        .rows
        .adjust_set_offsets(expansion_row_set_index + 1, page.max_row_expansion);
      previous_expansion_row_set_index = Some(expansion_row_set_index);
      page = self.place_items_on_page(
        sizing_tree,
        fragmentainer_space,
        cloned_block_start_decoration,
        previous_expansion_row_set_index,
      );
    }

    if self.shift_breakpoint_into_next_fragmentainer(
      sizing_tree,
      &page,
      &mut fragmentainer_space,
      intrinsic_block_size,
    ) {
      page = self.place_items_on_page(
        sizing_tree,
        fragmentainer_space,
        cloned_block_start_decoration,
        previous_expansion_row_set_index,
      );
    } else if let Some(space) = &mut fragmentainer_space {
      *space = (*space).max(page.max_item_block_end - cloned_block_start_decoration);
    }

    if page.has_subsequent_children {
      self.builder.has_subsequent_children = true;
    }

    let mut children = Vec::with_capacity(page.placed.len());

    for (node, result, offset, relative_offset) in take(&mut page.placed) {
      let style = Rc::clone(self.tree.layout_style_rc(node));

      self
        .builder
        .add_result(self.space, &style, &result, offset.block_offset);
      children.push(PlacedChild {
        node,
        location: self.physical_offset(offset, result.size.inline_size) + relative_offset,
        result,
      });
    }
    if let Some(space) = fragmentainer_space {
      self.fragmentation.offset_in_stitched_container += space;
    }
    (children, page.baselines)
  }

  /// Blink's `PlaceItems` in `PlaceGridItemsForFragmentation`: each item that starts on this page
  /// laid out at its place, the row to break before, and how much a row grows to fit its items.
  fn place_items_on_page(
    &mut self,
    sizing_tree: &GridSizingTree,
    fragmentainer_space: Option<LayoutUnit>,
    cloned_block_start_decoration: LayoutUnit,
    previous_expansion_row_set_index: Option<usize>,
  ) -> GridPage {
    let mut page = GridPage::default();
    let layout_data = &sizing_tree.layout_data;
    let offset_in_stitched_container = self.fragmentation.offset_in_stitched_container;
    let capacity = self.fragmentainer_capacity_for_children();
    let break_token = self.fragmentation.input.break_token.clone();
    let mut child_break_tokens = break_token
      .as_deref()
      .map_or(&[][..], |token| &token.child_break_tokens[..])
      .iter()
      .peekable();

    for (index, item) in sizing_tree.grid_items.iter().enumerate() {
      let placement = self.fragmentation.placement_data[index];
      let token = match child_break_tokens.peek() {
        Some(BreakToken::Block(token)) if token.node == item.node => {
          let token = Rc::clone(token);

          child_break_tokens.next();
          Some(token)
        }
        _ => None,
      };
      let child_block_offset = if is_break_inside(token.as_deref()) {
        self.builder.border_padding.block_start
      } else {
        placement.offset.block_offset - offset_in_stitched_container + cloned_block_start_decoration
      };
      let fragmentainer_block_offset = self.builder.fragmentainer_offset(true) + child_block_offset;
      let min_block_size_should_encompass_intrinsic_size = !placement
        .has_descendant_that_depends_on_percentage_block_size
        && self.min_block_size_should_encompass_intrinsic_size(item);
      let unavailable_block_size = match token.as_deref() {
        Some(token) if self.is_resuming() && is_break_inside(Some(token)) => {
          self.fragmentation.previous_offset_in_stitched_container
            - (placement.offset.block_offset + token.consumed_block_size)
        }
        _ => LayoutUnit::ZERO,
      };
      let (mut space, mut grid_area) = self.space_for_layout(item, layout_data);

      if unavailable_block_size != LayoutUnit::ZERO {
        let shrink = |size: LayoutUnit| size - unavailable_block_size;

        space.available_block_size = space.available_block_size.map(shrink);
        space.percentage_resolution_block_size = space.percentage_resolution_block_size.map(shrink);
      }
      if self.space.has_block_fragmentation() {
        setup_space_builder_for_fragmentation(
          self.space,
          &self.builder,
          self.tree.is_monolithic(item.node),
          child_block_offset,
          &mut space,
        );
        if min_block_size_should_encompass_intrinsic_size
          && let Some(fragmentation) = &mut space.block_fragmentation
        {
          fragmentation.min_block_size_should_encompass_intrinsic_size = true;
        }
      }

      let item_row_set_index = item.set_indices(ForRows).0;

      grid_area.offset.block_offset += self.fragmentation.row_offset_adjustments
        [item_row_set_index]
        - offset_in_stitched_container;

      // An item that starts on a later page waits for it, and one an earlier page laid out whole
      // is done.
      if fragmentainer_space.is_some_and(|space| grid_area.offset.block_offset >= space) {
        page.has_subsequent_children = true;
        continue;
      }
      if grid_area.offset.block_offset < LayoutUnit::ZERO && token.is_none() && self.is_resuming() {
        continue;
      }

      let input = fragmentation_layout_input_for_child(
        item.node,
        token,
        self.fragmentation.input.early_break.as_deref(),
      );
      let result = self.tree.layout_fragment(item.node, &space, &input);

      page.baselines.accumulate(item, &result, child_block_offset);
      page.placed.push((
        item.node,
        Rc::clone(&result),
        LogicalOffset::new(placement.offset.inline_offset, child_block_offset),
        placement.relative_offset,
      ));

      let row_has_container_separation = grid_area.offset.block_offset > LayoutUnit::ZERO;

      if row_has_container_separation
        && page
          .breakpoint_row_set_index
          .is_none_or(|breakpoint| item_row_set_index < breakpoint)
      {
        let break_between = self
          .fragmentation
          .row_break_between
          .get(item_row_set_index)
          .copied()
          .unwrap_or_default();

        if is_forced_break_value(self.space, break_between) {
          self.builder.has_forced_break = true;
          page.update_breakpoint_row_set_index(item_row_set_index);
          continue;
        }

        self.builder.previous_break_after = break_between;

        let appeal_before = calculate_break_appeal_before(
          self.space,
          Some(&item.style),
          &result,
          &self.builder,
          row_has_container_separation,
        );

        if !move_past_breakpoint_without_builder(
          (capacity, self.space.min_break_appeal()),
          (item.node, &item.style),
          &result,
          fragmentainer_block_offset,
          appeal_before,
        ) {
          page.update_breakpoint_row_set_index(item_row_set_index);

          // A break before a row that avoids one goes before an earlier row that does not.
          if is_avoid_break_value(self.space, break_between) {
            for index in (0..item_row_set_index).rev() {
              let offset = layout_data.rows.set_offset(index)
                + self.fragmentation.row_offset_adjustments[index]
                - offset_in_stitched_container;

              if offset <= LayoutUnit::ZERO {
                break;
              }
              if self.fragmentation.row_break_between.get(index) == Some(&BreakBetween::Auto) {
                page.update_breakpoint_row_set_index(index);
                break;
              }
            }
          }
          continue;
        }
      }

      if min_block_size_should_encompass_intrinsic_size
        && page
          .expansion_row_set_index
          .is_none_or(|expansion| item_row_set_index <= expansion)
        && previous_expansion_row_set_index.is_none_or(|previous| item_row_set_index > previous)
        && let Some(space) = fragmentainer_space
        && grid_area.offset.block_offset + grid_area.size.block_size <= space
      {
        if page.expansion_row_set_index != Some(item_row_set_index) {
          page.expansion_row_set_index = Some(item_row_set_index);
          page.max_row_expansion = LayoutUnit::ZERO;
        }

        let grid_area_block_end = grid_area.offset.block_offset + grid_area.size.block_size;
        let item_expansion = if result.break_info.break_token.is_some() {
          (space - grid_area_block_end).add_epsilon()
        } else {
          result.size.block_size - grid_area_block_end
        };

        page.max_row_expansion = page.max_row_expansion.max(item_expansion);
      }

      page.max_item_block_end = page
        .max_item_block_end
        .max(child_block_offset + result.size.block_size);
    }
    page
  }

  /// Blink's `AdjustItemOffsets`: the items from row `row_index` on move by `delta`.
  fn adjust_item_offsets(
    &mut self,
    sizing_tree: &GridSizingTree,
    row_index: usize,
    delta: LayoutUnit,
  ) {
    for (placement, item) in self
      .fragmentation
      .placement_data
      .iter_mut()
      .zip(&sizing_tree.grid_items)
    {
      if item.set_indices(ForRows).0 >= row_index {
        placement.offset.block_offset += delta;
      }
    }
  }

  /// Blink's `ShiftBreakpointIntoNextFragmentainer`: the row the page breaks before moves to the
  /// next page's start, whether it moved.
  fn shift_breakpoint_into_next_fragmentainer(
    &mut self,
    sizing_tree: &GridSizingTree,
    page: &GridPage,
    fragmentainer_space: &mut Option<LayoutUnit>,
    intrinsic_block_size: &mut LayoutUnit,
  ) -> bool {
    let Some(breakpoint) = page.breakpoint_row_set_index else {
      return false;
    };
    let row_offset = sizing_tree.layout_data.rows.set_offset(breakpoint)
      + self.fragmentation.row_offset_adjustments[breakpoint];
    let fragment_relative_row_offset = row_offset - self.fragmentation.offset_in_stitched_container;
    let Some(space) = *fragmentainer_space else {
      *fragmentainer_space = Some(fragment_relative_row_offset);
      return true;
    };
    let row_offset_delta = space - fragment_relative_row_offset;

    if row_offset_delta <= LayoutUnit::ZERO {
      return false;
    }

    *intrinsic_block_size += row_offset_delta;
    self.adjust_item_offsets(sizing_tree, breakpoint, row_offset_delta);
    for adjustment in &mut self.fragmentation.row_offset_adjustments[breakpoint..] {
      *adjustment += row_offset_delta;
    }
    true
  }

  /// Blink's `MinBlockSizeShouldEncompassIntrinsicSize` for a grid item: whether it grows past its
  /// block size when a break pushes its content down.
  fn min_block_size_should_encompass_intrinsic_size(&self, item: &GridItemData) -> bool {
    if self.tree.is_monolithic(item.node) {
      return false;
    }

    let height = item.style.size.height;

    if !(height.is_auto() || height.is_content_or_intrinsic())
      && item.style.breaks.box_decoration_break != BoxDecorationBreak::Clone
    {
      return false;
    }
    if item.span_size(ForRows) > 1 {
      return false;
    }

    let properties = item.row_span_properties;

    if properties.contains(TrackSpanProperties::HAS_FIXED_MAXIMUM_TRACK)
      && !properties.contains(TrackSpanProperties::HAS_INTRINSIC_TRACK)
    {
      return false;
    }

    let container_height = self.style.size.height;

    !properties.contains(TrackSpanProperties::HAS_FIXED_MINIMUM_TRACK)
      || container_height.is_auto()
      || container_height.is_content_or_intrinsic()
  }

  /// The fragmentation part of Blink's `LayoutInternal` after the items: this fragment's block size,
  /// the grid `block_size` tall in one piece.
  pub(super) fn finish_fragmentation(
    &mut self,
    block_size: LayoutUnit,
  ) -> Result<LayoutUnit, LayoutStatus> {
    if self.space.should_propagate_child_break_values()
      && let (Some(&first), Some(&last)) = (
        self.fragmentation.row_break_between.first(),
        self.fragmentation.row_break_between.last(),
      )
    {
      self.builder.initial_break_before = Some(first);
      self.builder.previous_break_after = last;
    }
    if !self.is_involved_in_block_fragmentation() {
      return Ok(block_size);
    }

    self.builder.intrinsic_block_size = self.fragmentation.offset_in_stitched_container
      - self.fragmentation.previous_offset_in_stitched_container
      + self.builder.border_padding.block_end;
    self.builder.fragments_total_block_size = block_size;
    finish_fragmentation(&mut self.builder)?;
    Ok(self.builder.fragment_block_size)
  }

  /// The intrinsic block size of this fragment, or of the grid in one piece.
  pub(super) fn fragment_intrinsic_block_size(
    &self,
    intrinsic_block_size: LayoutUnit,
  ) -> LayoutUnit {
    if self.is_involved_in_block_fragmentation() {
      self.builder.intrinsic_block_size
    } else {
      intrinsic_block_size
    }
  }

  /// Keeps the grid in the break token for the next page to resume from.
  pub(super) fn keep_break_token_data(
    &mut self,
    sizing_tree: GridSizingTree,
    intrinsic_block_size: LayoutUnit,
    out_of_flow_children: Vec<NodeId>,
  ) {
    if !self.space.has_block_fragmentation() {
      return;
    }

    self.builder.break_token_data =
      Some(BreakTokenAlgorithmData::Grid(Rc::new(GridBreakTokenData {
        sizing_tree,
        out_of_flow_children,
        intrinsic_block_size,
        offset_in_stitched_container: self.fragmentation.offset_in_stitched_container,
        placement_data: take(&mut self.fragmentation.placement_data),
        row_offset_adjustments: take(&mut self.fragmentation.row_offset_adjustments),
        row_break_between: take(&mut self.fragmentation.row_break_between),
      })));
  }

  /// Whether the out-of-flow boxes go to the fragmentation root, which lays them out across pages.
  pub(super) fn defers_out_of_flow(&self) -> bool {
    self.is_involved_in_block_fragmentation()
  }

  /// Leaves the out-of-flow `candidates` the grid holds, in its padding box `containing_block`, to
  /// the fragmentation root.
  pub(super) fn defer_out_of_flow(
    &self,
    claims: ContainingBlockClaims,
    candidates: Vec<OutOfFlowCandidate>,
    containing_block: ContainingBlock,
  ) -> Vec<OutOfFlowCandidate> {
    let fragmented_containing_block =
      self
        .builder
        .fragmented_containing_block(self.node, self.fragment_index(), containing_block);

    self.tree.defer_out_of_flow(
      claims,
      candidates,
      fragmented_containing_block,
      self.style.overflow.y != Overflow::Visible,
    )
  }

  /// What the grid's layout result tells its container about breaking.
  pub(super) fn finish_break_info(&mut self, result: &mut LayoutResult) {
    result.break_info = self.builder.finish_break_info(self.node);
    self.builder.exclude_broken_sides(result);
  }
}

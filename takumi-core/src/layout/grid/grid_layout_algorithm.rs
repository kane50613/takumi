//! Blink's [`GridLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_layout_algorithm.cc)
//! and the [`grid_layout_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_layout_utils.cc)
//! it runs, in a horizontal writing mode, without `grid-lanes`, gap decorations, `reading-flow`,
//! scrollbars, size containment, or `last baseline`. Every item lies parallel to its grid, and a
//! root grid sizes the tracks of the subgrids in it, which lay their items out in them.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

#[cfg(feature = "fragmentation")]
mod fragmentation;

use std::{
  mem::{swap, take},
  rc::Rc,
};

#[cfg(feature = "fragmentation")]
pub(crate) use fragmentation::GridBreakTokenData;
#[cfg(feature = "fragmentation")]
use fragmentation::GridFragmentation;

#[cfg(not(feature = "fragmentation"))]
use crate::layout::layout_result::LayoutStatus;
#[cfg(feature = "fragmentation")]
use crate::layout::{
  box_fragment_builder::BoxFragmentBuilder,
  fragmentation_utils::setup_fragment_builder_for_fragmentation,
};

use crate::{
  geometry::{NodeId, Point, Size},
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    fragmentation_layout_input::FragmentationLayoutInput,
    grid::{
      grid_area::{GridArea, GridSpan},
      grid_data::GridLayoutData,
      grid_item::{AxisEdge, BaselineGroup, GridItemData, GridItemIndices, GridItems},
      grid_line_resolver::GridLineResolver,
      grid_placement::{GridPlacement, GridPlacementData},
      grid_sizing_tree::{
        GridLayoutSubtree, GridLayoutTree, GridSizingTree, GridTreeNode, SubgriddedItemData,
      },
      grid_style::{GridStyle, GridTrackList, GridTrackSizingDirection, RepeatType},
      grid_track_collection::{GridRangeBuilder, GridTrackCollection},
      grid_track_sizing_algorithm::{
        GridItemContributionType, GridTrackSizingAlgorithm, SizingConstraint, calculate_gutter_size,
      },
    },
    layout_result::{
      BlockEdge, InlineEdge, LayoutResult, LogicalStaticPosition, OutOfFlowCandidate,
    },
    layout_style::{LayoutLength, LayoutStyle},
    length_utils::{FitContentMode, LengthResolver, child_available_size, margins_for},
    logical_rect::{
      AvailableLogicalSize, LogicalBoxStrut, LogicalOffset, LogicalRect, LogicalSize,
    },
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
    out_of_flow::ContainingBlock,
    relative_utils::compute_relative_offset,
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
  style::{Direction, Display, Position},
};

use GridTrackSizingDirection::{ForColumns, ForRows};

/// What a grid keeps of its last layout to place its out-of-flow boxes in their grid areas.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridGeometry {
  layout_data: GridLayoutData,
  direction: Direction,
  inline_size: LayoutUnit,
}

/// What Blink's `ComputeGridGeometry` hands `PlaceGridItems`: the grid's items, its tracks, which
/// fragmentation moves, and the finalized layout subtree its subgrids lay out in.
#[derive(Clone, PartialEq)]
struct GridLayoutState {
  grid_items: GridItems,
  layout_data: GridLayoutData,
  layout_subtree: GridLayoutSubtree,
  placement_data: Rc<GridPlacementData>,
}

impl GridLayoutState {
  fn geometry(&self, direction: Direction, inline_size: LayoutUnit) -> GridGeometry {
    GridGeometry {
      direction,
      inline_size,
      layout_data: self.layout_data.clone(),
    }
  }

  /// The layout subtree of each item that is a subgrid, in the items' order.
  fn item_layout_subtrees(&self) -> Vec<Option<GridLayoutSubtree>> {
    let mut subgrid_subtrees = self.layout_subtree.children().into_iter();

    self
      .grid_items
      .iter()
      .map(|item| {
        if item.is_subgrid() {
          subgrid_subtrees.next()
        } else {
          None
        }
      })
      .collect()
  }
}

/// Blink's `SubgriddedAreaInParent`: the area of `subgrid` in its parent's lines in the axes it
/// subgrids, the other axes indefinite.
fn subgridded_area_in_parent(subgrid: Option<&GridItemData>) -> GridArea {
  let Some(subgrid) = subgrid.filter(|item| item.is_subgrid()) else {
    return GridArea::default();
  };
  let mut area = subgrid.resolved_position;

  if !subgrid.has_subgridded_columns {
    area.columns = GridSpan::indefinite(1);
  }
  if !subgrid.has_subgridded_rows {
    area.rows = GridSpan::indefinite(1);
  }
  area
}

/// Blink's `ValidateMinMaxSizesCache`: drops the intrinsic sizes a subgrid in the subtree at `index`
/// kept when the tracks it takes from its parent changed, whether any did.
fn validate_min_max_sizes_cache(
  tree: &mut LayoutTree<'_>,
  sizing_tree: &GridSizingTree,
  index: usize,
  direction: GridTrackSizingDirection,
) -> bool {
  let mut should_invalidate_min_max_sizes_cache = false;

  for subtree in sizing_tree.children(index) {
    should_invalidate_min_max_sizes_cache |=
      validate_min_max_sizes_cache(tree, sizing_tree, subtree, direction);
  }

  let tree_node = sizing_tree.node(index);

  if tree_node
    .layout_data
    .is_subgrid_with_standalone_axis(direction)
  {
    let layout_grid = &mut tree.node_state_mut(tree_node.node).layout_grid;

    if !should_invalidate_min_max_sizes_cache {
      should_invalidate_min_max_sizes_cache =
        layout_grid.should_invalidate_subgrid_min_max_sizes_cache_for(&tree_node.layout_data);
    }
    if should_invalidate_min_max_sizes_cache {
      layout_grid.invalidate_subgrid_min_max_sizes_cache();
    }
  }
  should_invalidate_min_max_sizes_cache
}

/// Blink's `BaselineCollectionPhase`: which items a baseline pass measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaselineCollectionPhase {
  BaselinesForTrackSizing,
  FinalBaselines,
  BaselinesForStandaloneAxes,
}

/// Blink's `BlockSizeDependentGridItem`.
struct BlockSizeDependentGridItem {
  row_set_indices: (usize, usize),
  cached_block_size: Option<LayoutUnit>,
}

/// What measuring an item's contribution to the tracks of `direction` reads: its measure space, and
/// of the subgrid that holds it, where it lies in that subgrid's tracks and what the subgrid adds
/// around them.
struct ItemMeasure {
  space: ConstraintSpace,
  margins: LogicalBoxStrut,
  direction: GridTrackSizingDirection,
  /// Blink's `GetTrackBaseline` of the item in the measuring grid.
  track_baseline: Option<LayoutUnit>,
  /// Blink's `GetExtraMarginForBaseline`.
  extra_margin_for_baseline: LayoutUnit,
  /// Blink's `StartExtraMargin` and `EndExtraMargin` of the sets the item spans.
  start_extra_margin: LayoutUnit,
  end_extra_margin: LayoutUnit,
  spanned_tracks_definite_max_size: Option<LayoutUnit>,
  /// The item in its own grid, Blink's `SubgriddedItemData`, and its measure grid area there.
  subgridded_item: GridItemData,
  containing_grid_area_size: AvailableLogicalSize,
}

impl ItemMeasure {
  /// Blink's `CalculateBaselineShim`: the space before an item's `baseline` that lines it up with
  /// its track's, less `GetExtraMarginForBaseline`.
  fn baseline_shim(&self, baseline: LayoutUnit) -> LayoutUnit {
    self
      .track_baseline
      .map_or(LayoutUnit::ZERO, |track_baseline| {
        track_baseline - baseline - self.extra_margin_for_baseline
      })
  }
}

/// A laid-out item waiting to be placed.
struct PlacedChild {
  node: NodeId,
  location: Point<LayoutUnit>,
  result: Rc<LayoutResult>,
}

/// Blink's `GridBaselineAccumulator`.
#[derive(Default)]
struct GridBaselineAccumulator {
  first_fallback_baseline: Option<(usize, usize, LayoutUnit)>,
  last_fallback_baseline: Option<(usize, usize, LayoutUnit)>,
  first_major_baseline: Option<(usize, LayoutUnit)>,
  last_major_baseline: Option<(usize, LayoutUnit)>,
  first_minor_baseline: Option<(usize, LayoutUnit)>,
  last_minor_baseline: Option<(usize, LayoutUnit)>,
  first_set_index: Option<usize>,
  last_set_index: Option<usize>,
}

impl GridBaselineAccumulator {
  /// Blink's `Accumulate` for an item at `block_offset` laid out as `result`. Every in-flow item
  /// has a definite area, so the fallbacks compare start and end lines.
  fn accumulate(&mut self, item: &GridItemData, result: &LayoutResult, block_offset: LayoutUnit) {
    let area = &item.resolved_position;
    let (row_start, column_start) = (area.rows.start_line(), area.columns.start_line());
    let (row_end, column_end) = (area.rows.end_line(), area.columns.end_line());

    if self
      .first_fallback_baseline
      .is_none_or(|(row, column, _)| (row_start, column_start) < (row, column))
    {
      self.first_fallback_baseline = Some((
        row_start,
        column_start,
        block_offset + first_baseline_or_synthesize(item, result),
      ));
    }
    if self
      .last_fallback_baseline
      .is_none_or(|(row, column, _)| row_end > row || (row_end == row && column_end >= column))
    {
      self.last_fallback_baseline = Some((
        row_end,
        column_end,
        block_offset + last_baseline_or_synthesize(item, result),
      ));
    }

    let (begin, end) = item.set_indices(ForRows);

    if self.first_set_index.is_none_or(|index| begin < index) {
      self.first_set_index = Some(begin);
    }
    if self.last_set_index.is_none_or(|index| end - 1 > index) {
      self.last_set_index = Some(end - 1);
    }
  }

  /// Blink's `AccumulateRows`.
  fn accumulate_rows(&mut self, layout_data: &GridLayoutData) {
    let rows = &layout_data.rows;

    for index in 0..rows.set_count() {
      let set_offset = rows.set_offset(index);

      if let Some(baseline) = layout_data.major_baseline(ForRows, index) {
        let baseline = set_offset + baseline;

        self.first_major_baseline.get_or_insert((index, baseline));
        self.last_major_baseline = Some((index, baseline));
      }
      if let Some(baseline) = layout_data.minor_baseline(ForRows, index) {
        let baseline = set_offset
          + rows
            .calculate_set_span_size(index, index + 1)
            .unwrap_or_default()
          - baseline;

        self.first_minor_baseline.get_or_insert((index, baseline));
        self.last_minor_baseline = Some((index, baseline));
      }
    }
  }

  fn first_baseline(&self) -> Option<LayoutUnit> {
    [self.first_major_baseline, self.first_minor_baseline]
      .into_iter()
      .flatten()
      .find(|&(index, _)| Some(index) == self.first_set_index)
      .map(|(_, baseline)| baseline)
      .or(
        self
          .first_fallback_baseline
          .map(|(_, _, baseline)| baseline),
      )
  }

  fn last_baseline(&self) -> Option<LayoutUnit> {
    [self.last_minor_baseline, self.last_major_baseline]
      .into_iter()
      .flatten()
      .find(|&(index, _)| Some(index) == self.last_set_index)
      .map(|(_, baseline)| baseline)
      .or(self.last_fallback_baseline.map(|(_, _, baseline)| baseline))
  }
}

/// A scroll container's baseline clamped to its border box, as Blink's `LogicalBoxFragment` gives
/// it.
fn clamped_baseline(
  item: &GridItemData,
  baseline: Option<LayoutUnit>,
  block_size: LayoutUnit,
) -> Option<LayoutUnit> {
  baseline.map(|baseline| {
    if item.style.is_scroll_container() {
      baseline.min(block_size).max(LayoutUnit::ZERO)
    } else {
      baseline
    }
  })
}

/// Blink's `FirstBaselineOrSynthesize` for an alphabetic baseline: the block-end edge when the
/// item has none.
fn first_baseline_or_synthesize(item: &GridItemData, result: &LayoutResult) -> LayoutUnit {
  clamped_baseline(item, result.first_baseline, result.size.block_size)
    .unwrap_or(result.size.block_size)
}

fn last_baseline_or_synthesize(item: &GridItemData, result: &LayoutResult) -> LayoutUnit {
  clamped_baseline(item, result.last_baseline, result.size.block_size)
    .unwrap_or(result.size.block_size)
}

/// Blink's `GetLogicalBaseline` in `direction`'s baseline writing mode. A column baseline lies in
/// `vertical-lr` or `vertical-rl`, which a horizontal item has no baseline in.
fn item_baseline(
  item: &GridItemData,
  direction: GridTrackSizingDirection,
  grid_direction: Direction,
  result: &LayoutResult,
) -> LayoutUnit {
  match direction {
    ForColumns => synthesized_baseline(direction, grid_direction, result.size.inline_size),
    ForRows => first_baseline_or_synthesize(item, result),
  }
}

/// Blink's `GetSynthesizedLogicalBaseline`.
fn synthesized_baseline(
  direction: GridTrackSizingDirection,
  grid_direction: Direction,
  block_size: LayoutUnit,
) -> LayoutUnit {
  match (direction, grid_direction) {
    (ForColumns, Direction::Ltr) => LayoutUnit::ZERO,
    _ => block_size,
  }
}

/// The margin before an item's baseline in `direction`'s baseline writing mode.
fn baseline_margin(margins: LogicalBoxStrut, direction: GridTrackSizingDirection) -> LayoutUnit {
  match direction {
    ForColumns => margins.inline_start,
    ForRows => margins.block_start,
  }
}

/// Blink's `GetTrackBaseline`: the baseline of the set `item` starts in for its major group, or
/// ends in for its minor one.
fn track_baseline(
  item: &GridItemData,
  layout_data: &GridLayoutData,
  direction: GridTrackSizingDirection,
) -> Option<LayoutUnit> {
  let (begin_set_index, end_set_index) = item.set_indices(direction);

  match item.baseline_group(direction) {
    BaselineGroup::Major => layout_data.major_baseline(direction, begin_set_index),
    BaselineGroup::Minor => layout_data.minor_baseline(direction, end_set_index - 1),
  }
}

/// Blink's `SetTrackBaseline`.
fn set_track_baseline(
  item: &GridItemData,
  direction: GridTrackSizingDirection,
  baseline: LayoutUnit,
  layout_data: &mut GridLayoutData,
) {
  let (begin_set_index, end_set_index) = item.set_indices(direction);

  match item.baseline_group(direction) {
    BaselineGroup::Major => layout_data.set_major_baseline(direction, begin_set_index, baseline),
    BaselineGroup::Minor => layout_data.set_minor_baseline(direction, end_set_index - 1, baseline),
  }
}

/// Blink's `GetExtraMarginForBaseline`: what the subgrids `subgridded_item` lies in add before its
/// baseline, its own margin, from the edge of its baseline-sharing group.
fn extra_margin_for_baseline(
  margins: LogicalBoxStrut,
  subgridded_item: &GridItemData,
  track_collection: &GridTrackCollection,
) -> LayoutUnit {
  let direction = track_collection.direction;
  let (begin_set_index, end_set_index) = subgridded_item.set_indices(direction);
  let subgrid_extra_margin = match subgridded_item.baseline_group(direction) {
    BaselineGroup::Major => track_collection.start_extra_margin(begin_set_index),
    BaselineGroup::Minor => track_collection.end_extra_margin(end_set_index),
  };

  subgrid_extra_margin + baseline_margin(margins, direction)
}

/// Blink's `AlignmentOffset`.
fn alignment_offset(
  container_size: LayoutUnit,
  size: LayoutUnit,
  margin_start: LayoutUnit,
  margin_end: LayoutUnit,
  baseline_offset: LayoutUnit,
  axis_edge: AxisEdge,
  is_overflow_safe: bool,
) -> LayoutUnit {
  let mut free_space = container_size - size - margin_start - margin_end;

  if is_overflow_safe {
    free_space = free_space.clamp_negative_to_zero();
  }

  match axis_edge {
    AxisEdge::Start => margin_start,
    AxisEdge::Center => margin_start + free_space / 2,
    AxisEdge::End => margin_start + free_space,
    AxisEdge::FirstBaseline => baseline_offset,
  }
}

/// Blink's `ClampIntrinsicMinSize`.
fn clamp_intrinsic_min_size(
  min_content_contribution: LayoutUnit,
  min_clamp_size: LayoutUnit,
  spanned_tracks_definite_max_size: LayoutUnit,
) -> LayoutUnit {
  min_content_contribution.min(spanned_tracks_definite_max_size.max(min_clamp_size))
}

/// Blink's `CalculateAutomaticRepetitions`, with no intrinsic sizes for an intrinsic repeater.
fn calculate_automatic_repetitions(
  track_list: &GridTrackList,
  gutter_size: LayoutUnit,
  available_size: Option<LayoutUnit>,
  min_available_size: LayoutUnit,
  max_available_size: LayoutUnit,
) -> usize {
  let (available_size, max_available_size) = match available_size {
    Some(size) => (size, size),
    None => (min_available_size, max_available_size),
  };
  let mut auto_repeater_size = LayoutUnit::ZERO;
  let mut non_auto_specified_size = LayoutUnit::ZERO;

  for repeater_index in 0..track_list.repeater_count() {
    let is_auto_repeater = matches!(
      track_list.repeat_type(repeater_index),
      RepeatType::AutoFill | RepeatType::AutoFit
    );
    let mut repeater_size = LayoutUnit::ZERO;

    for index in 0..track_list.repeat_size(repeater_index) {
      let track_size = track_list.repeat_track_size(repeater_index, index);

      if track_size.is_track_definition_intrinsic() {
        return 0;
      }

      let resolve = |length: LayoutLength| {
        length
          .resolve(Some(available_size))
          .unwrap_or(LayoutUnit::ZERO)
      };
      let fixed_min = track_size
        .has_fixed_min_track_breadth()
        .then(|| resolve(track_size.min_track_breadth.length()));
      let fixed_max = track_size
        .has_fixed_max_track_breadth()
        .then(|| resolve(track_size.max_track_breadth.length()));
      let mut contribution = match (fixed_min, fixed_max) {
        (Some(min), Some(max)) => max.max(min),
        (None, Some(max)) => max,
        (Some(min), None) => min,
        (None, None) => LayoutUnit::ZERO,
      };

      if is_auto_repeater {
        contribution = contribution.max(LayoutUnit::from_int(1));
      }
      repeater_size += contribution + gutter_size;
    }

    if is_auto_repeater {
      auto_repeater_size = repeater_size;
    } else {
      non_auto_specified_size += repeater_size * track_list.repeat_count(repeater_index, 0) as i32;
    }
  }

  non_auto_specified_size -= gutter_size;

  let count = if max_available_size == LayoutUnit::MAX {
    ((available_size - non_auto_specified_size) / auto_repeater_size).ceil()
  } else {
    ((max_available_size - non_auto_specified_size) / auto_repeater_size).floor()
  };

  usize::try_from(count).unwrap_or(0).max(1)
}

/// Blink's `ComputeTrackSizesInRange`: each set's size per track, as a quotient and remainder of
/// raw values.
fn track_sizes_in_range(
  track_collection: &GridTrackCollection,
  range_begin_set_index: usize,
  range_set_count: usize,
) -> Vec<(i32, i32)> {
  (range_begin_set_index..range_begin_set_index + range_set_count)
    .map(|index| {
      let set_track_count = track_collection.set_track_count(index) as i32;
      let set_size = (track_collection.set_offset(index + 1)
        - track_collection.set_offset(index)
        - track_collection.gutter_size * set_track_count)
        .clamp_negative_to_zero();

      (
        set_size.raw_value() / set_track_count,
        set_size.raw_value() % set_track_count,
      )
    })
    .collect()
}

/// Blink's `ComputeTrackOffsetInRange`.
fn track_offset_in_range(
  track_collection: &GridTrackCollection,
  range_begin_set_index: usize,
  range_set_count: usize,
  offset_in_range: usize,
) -> LayoutUnit {
  if range_set_count == 0 || offset_in_range == 0 {
    return LayoutUnit::ZERO;
  }

  let floor_set_track_count = offset_in_range / range_set_count;
  let remaining_track_count = offset_in_range % range_set_count;
  let mut index_offset = track_collection.gutter_size * offset_in_range as i32;

  for (index, (quotient, remainder)) in
    track_sizes_in_range(track_collection, range_begin_set_index, range_set_count)
      .into_iter()
      .enumerate()
  {
    let set_count = (floor_set_track_count + usize::from(remaining_track_count > index)) as i32;

    index_offset += LayoutUnit::from_raw_value(set_count.min(remainder) + set_count * quotient);
  }

  index_offset
}

/// Blink's `TrackOffset`.
fn track_offset(
  track_collection: &GridTrackCollection,
  range_index: usize,
  offset_in_range: usize,
  snap_to_end_of_track: bool,
) -> LayoutUnit {
  let range = &track_collection.ranges[range_index];
  let mut offset = if offset_in_range == range.track_count {
    track_collection.set_offset(range.begin_set_index + range.set_count)
  } else {
    track_collection.set_offset(range.begin_set_index)
      + track_offset_in_range(
        track_collection,
        range.begin_set_index,
        range.set_count,
        offset_in_range,
      )
  };

  if snap_to_end_of_track && (range.set_count > 0 || range_index > 0) {
    offset -= track_collection.gutter_size;
  }
  offset
}

/// Blink's `TrackStartOffset`.
fn track_start_offset(
  track_collection: &GridTrackCollection,
  range_index: usize,
  offset_in_range: usize,
) -> LayoutUnit {
  if track_collection.range_count() == 0 {
    return track_collection.set_offset(0);
  }

  let is_end_of_last_range = offset_in_range == track_collection.ranges[range_index].track_count
    && range_index == track_collection.range_count() - 1;

  track_offset(
    track_collection,
    range_index,
    offset_in_range,
    is_end_of_last_range,
  )
}

/// Blink's `TrackEndOffset`.
fn track_end_offset(
  track_collection: &GridTrackCollection,
  range_index: usize,
  offset_in_range: usize,
) -> LayoutUnit {
  if track_collection.range_count() == 0 {
    return track_collection.set_offset(0);
  }

  track_offset(
    track_collection,
    range_index,
    offset_in_range,
    offset_in_range != 0 || range_index != 0,
  )
}

/// Blink's `ComputeOutOfFlowItemContainingRect`: the grid area `item` resolves its insets against,
/// within `padding_box_rect`.
fn compute_out_of_flow_item_containing_rect(
  placement_data: &GridPlacementData,
  layout_data: &GridLayoutData,
  padding_box_rect: LogicalRect,
  item: &mut GridItemData,
) -> LogicalRect {
  item.compute_out_of_flow_item_placement(&layout_data.columns, placement_data);
  item.compute_out_of_flow_item_placement(&layout_data.rows, placement_data);

  let mut rect = padding_box_rect;
  let edges = |placement: GridItemIndices,
               offsets: GridItemIndices,
               track_collection: &GridTrackCollection| {
    (
      placement
        .begin
        .zip(offsets.begin)
        .map(|(range, offset)| track_start_offset(track_collection, range, offset)),
      placement
        .end
        .zip(offsets.end)
        .map(|(range, offset)| track_end_offset(track_collection, range, offset)),
    )
  };
  let (inline_start, inline_end) = edges(
    item.column_placement.range_index,
    item.column_placement.offset_in_range,
    &layout_data.columns,
  );
  let (block_start, block_end) = edges(
    item.row_placement.range_index,
    item.row_placement.offset_in_range,
    &layout_data.rows,
  );

  // Blink's `ShiftInlineStartEdgeTo` and the rest.
  if let Some(edge) = inline_start {
    rect.size.inline_size =
      (rect.offset.inline_offset + rect.size.inline_size - edge).clamp_negative_to_zero();
    rect.offset.inline_offset = edge;
  }
  if let Some(edge) = inline_end {
    rect.size.inline_size = (edge - rect.offset.inline_offset).clamp_negative_to_zero();
  }
  if let Some(edge) = block_start {
    rect.size.block_size =
      (rect.offset.block_offset + rect.size.block_size - edge).clamp_negative_to_zero();
    rect.offset.block_offset = edge;
  }
  if let Some(edge) = block_end {
    rect.size.block_size = (edge - rect.offset.block_offset).clamp_negative_to_zero();
  }
  rect
}

/// Blink's `AlignmentOffsetForOutOfFlow`.
fn alignment_offset_for_out_of_flow(
  inline_axis_edge: AxisEdge,
  block_axis_edge: AxisEdge,
  container_size: LogicalSize,
  static_position: &mut LogicalStaticPosition,
) {
  match inline_axis_edge {
    AxisEdge::Start | AxisEdge::FirstBaseline => static_position.inline_edge = InlineEdge::Start,
    AxisEdge::Center => {
      static_position.inline_edge = InlineEdge::Center;
      static_position.offset.inline_offset += container_size.inline_size / 2;
    }
    AxisEdge::End => {
      static_position.inline_edge = InlineEdge::End;
      static_position.offset.inline_offset += container_size.inline_size;
    }
  }

  match block_axis_edge {
    AxisEdge::Start | AxisEdge::FirstBaseline => static_position.block_edge = BlockEdge::Start,
    AxisEdge::Center => {
      static_position.block_edge = BlockEdge::Center;
      static_position.offset.block_offset += container_size.block_size / 2;
    }
    AxisEdge::End => {
      static_position.block_edge = BlockEdge::End;
      static_position.offset.block_offset += container_size.block_size;
    }
  }
}

/// A physical rectangle in a box `inline_size` wide as a logical one in `direction`.
fn logical_rect(
  origin: Point<LayoutUnit>,
  size: Size<LayoutUnit>,
  direction: Direction,
  inline_size: LayoutUnit,
) -> LogicalRect {
  let inline_offset = match direction {
    Direction::Ltr => origin.x,
    Direction::Rtl => inline_size - origin.x - size.width,
  };

  LogicalRect::new(inline_offset, origin.y, size.width, size.height)
}

impl<'r> LayoutTree<'r> {
  /// The grid area an out-of-flow `node` resolves its insets against, else `containing_block`.
  pub(crate) fn grid_area(
    &self,
    container: NodeId,
    node: NodeId,
    containing_block: ContainingBlock,
    previously_consumed_block_size: LayoutUnit,
  ) -> ContainingBlock {
    let container_state = self.node_state(container);
    let (Some(geometry), Some(placement_data)) = (
      container_state.grid_geometry.as_deref(),
      container_state.layout_grid.last_placement_data(),
    ) else {
      return containing_block;
    };
    let container_style = self.layout_style(container);
    let (direction, inline_size) = (geometry.direction, geometry.inline_size);
    let mut item = GridItemData::new(
      node,
      Rc::clone(self.layout_style_rc(node)),
      container_style,
      self.is_replaced(node),
    );
    let rect = compute_out_of_flow_item_containing_rect(
      placement_data,
      &geometry.layout_data,
      logical_rect(
        containing_block.origin + Point::new(LayoutUnit::ZERO, previously_consumed_block_size),
        containing_block.size,
        direction,
        inline_size,
      ),
      &mut item,
    );
    let x = match direction {
      Direction::Ltr => rect.offset.inline_offset,
      Direction::Rtl => inline_size - rect.offset.inline_offset - rect.size.inline_size,
    };

    ContainingBlock {
      origin: Point::new(x, rect.offset.block_offset - previously_consumed_block_size),
      size: Size {
        width: rect.size.inline_size,
        height: rect.size.block_size,
      },
    }
  }
}

/// Blink's `GridLayoutAlgorithm`.
pub(crate) struct GridLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  node: NodeId,
  space: &'t ConstraintSpace,
  style: Rc<LayoutStyle>,
  grid_style: Rc<GridStyle>,
  border_padding: LogicalBoxStrut,
  /// The border-box inline size, none while sizing intrinsically.
  inline_size: Option<LayoutUnit>,
  grid_available_size: AvailableLogicalSize,
  grid_min_available_size: LogicalSize,
  grid_max_available_size: LogicalSize,
  #[cfg(feature = "fragmentation")]
  builder: BoxFragmentBuilder,
  #[cfg(feature = "fragmentation")]
  fragmentation: GridFragmentation,
}

/// The intrinsic sizes a box's inline size reads, by size type.
type MinMaxSizesFn<'f, 'r> = dyn FnMut(&mut LayoutTree<'r>, SizeType) -> MinMaxSizesResult + 'f;

/// The callback Blink's `ForEachSubgrid` runs with each subgrid's algorithm, the index of its
/// subtree, and its data in the grid.
type SubgridCallback<'f, 'r> =
  dyn FnMut(&mut GridLayoutAlgorithm<'_, 'r>, &mut GridSizingTree, usize, SubgriddedItemData) + 'f;

impl<'t, 'r> GridLayoutAlgorithm<'t, 'r> {
  /// The algorithm for `node` in `space`, resumed as `input` says, with an indefinite inline size
  /// when `is_intrinsic`.
  pub(crate) fn new(
    tree: &'t mut LayoutTree<'r>,
    node: NodeId,
    space: &'t ConstraintSpace,
    is_intrinsic: bool,
    input: &FragmentationLayoutInput,
  ) -> Self {
    Self::with_min_max_sizes(
      tree,
      node,
      space,
      is_intrinsic,
      input,
      &mut |tree, size_type| tree.min_max_sizes(node, space, size_type),
    )
  }

  /// [`Self::new`] with the intrinsic sizes `min_max_sizes` gives, Blink's `CalculateInitialFragmentGeometry`
  /// callback.
  fn with_min_max_sizes(
    tree: &'t mut LayoutTree<'r>,
    node: NodeId,
    space: &'t ConstraintSpace,
    is_intrinsic: bool,
    #[cfg_attr(not(feature = "fragmentation"), expect(unused_variables))]
    input: &FragmentationLayoutInput,
    min_max_sizes: &mut MinMaxSizesFn<'_, 'r>,
  ) -> Self {
    let style = Rc::clone(tree.layout_style_rc(node));
    let grid_style = style.grid.clone().unwrap_or_default();
    let resolver = LengthResolver::new(space, &style);
    let border_padding = resolver.border_padding;
    let inline_size = (!is_intrinsic)
      .then(|| resolver.inline_size_for_fragment(&mut |size_type| min_max_sizes(tree, size_type)));
    let grid_available_size = child_available_size(
      AvailableLogicalSize {
        inline_size,
        block_size: resolver.initial_block_size_for_fragment(None, inline_size),
      },
      border_padding,
    );
    // Blink's `ComputeAvailableSizes`.
    let available_or_zero = LogicalSize::new(
      grid_available_size.inline_size.unwrap_or_default(),
      grid_available_size.block_size.unwrap_or_default(),
    );
    let mut grid_min_available_size = available_or_zero;
    let mut grid_max_available_size = available_or_zero;
    let content_size = |sizes: MinMaxSizes, border_padding_sum: LayoutUnit| {
      (
        (sizes.min_size - border_padding_sum).clamp_negative_to_zero(),
        if sizes.max_size == LayoutUnit::MAX {
          sizes.max_size
        } else {
          (sizes.max_size - border_padding_sum).clamp_negative_to_zero()
        },
      )
    };

    if grid_available_size.inline_size.is_none() {
      let sizes = resolver.min_max_inline_sizes(
        None,
        &mut |_| MinMaxSizesResult::default(),
        true,
        FitContentMode::Normal,
        None,
      );

      (
        grid_min_available_size.inline_size,
        grid_max_available_size.inline_size,
      ) = content_size(sizes, border_padding.inline_sum());
    }
    if grid_available_size.block_size.is_none() {
      (
        grid_min_available_size.block_size,
        grid_max_available_size.block_size,
      ) = content_size(
        resolver.initial_min_max_block_sizes(None),
        border_padding.block_sum(),
      );
    }

    #[cfg(feature = "fragmentation")]
    let builder = {
      let mut builder =
        BoxFragmentBuilder::new(space, &style, border_padding, input.break_token.clone());

      builder.has_non_visible_block_overflow = tree.has_non_visible_block_overflow(node);
      setup_fragment_builder_for_fragmentation(
        space,
        &style,
        inline_size.unwrap_or_default(),
        &mut builder,
      );
      builder
    };

    Self {
      tree,
      node,
      space,
      style,
      grid_style,
      border_padding,
      inline_size,
      grid_available_size,
      grid_min_available_size,
      grid_max_available_size,
      #[cfg(feature = "fragmentation")]
      builder,
      #[cfg(feature = "fragmentation")]
      fragmentation: GridFragmentation::new(input),
    }
  }

  /// Blink's `CalculateInitialFragmentGeometryForSubgrid` and the algorithm for `subgrid` in `space`: its
  /// intrinsic sizes from `sizing_subtree` when it sizes its columns itself and one is given, else
  /// an intrinsic inline size when its inline size reads them.
  fn for_subgrid(
    tree: &'t mut LayoutTree<'r>,
    subgrid: &GridItemData,
    space: &'t ConstraintSpace,
    sizing_subtree: Option<(&mut GridSizingTree, usize)>,
  ) -> Self {
    let node = subgrid.node;
    let input = FragmentationLayoutInput::default();
    let mut needs_to_compute_min_max_sizes = false;

    LengthResolver::new(space, &subgrid.style).inline_size_for_fragment(&mut |_| {
      needs_to_compute_min_max_sizes = true;
      MinMaxSizesResult::default()
    });

    if !needs_to_compute_min_max_sizes {
      return Self::with_min_max_sizes(tree, node, space, false, &input, &mut |_, _| {
        MinMaxSizesResult::default()
      });
    }
    if !subgrid.has_subgridded_columns
      && let Some((sizing_tree, index)) = sizing_subtree
    {
      let sizes = GridLayoutAlgorithm::subgrid_min_max_sizes(tree, sizing_tree, index, space);

      return Self::with_min_max_sizes(tree, node, space, false, &input, &mut |_, _| {
        MinMaxSizesResult::new(sizes, false)
      });
    }

    Self::new(tree, node, space, true, &input)
  }

  /// Blink's `GridNode::ComputeSubgridMinMaxSizes`: the intrinsic inline sizes of the subgrid at
  /// `index` measured in `space`, kept until its subgridded tracks change.
  fn subgrid_min_max_sizes(
    tree: &mut LayoutTree<'r>,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    space: &ConstraintSpace,
  ) -> MinMaxSizes {
    let node = sizing_tree.node(index).node;

    if let Some(sizes) = tree
      .node_state(node)
      .layout_grid
      .cached_subgrid_min_max_sizes()
    {
      return sizes;
    }

    let sizes = GridLayoutAlgorithm::new(
      tree,
      node,
      space,
      true,
      &FragmentationLayoutInput::default(),
    )
    .compute_subgrid_min_max_sizes(sizing_tree, index);

    tree
      .node_state_mut(node)
      .layout_grid
      .set_subgrid_min_max_sizes_cache(sizes, &sizing_tree.node(index).layout_data);
    sizes
  }

  /// Blink's `GridNode::ComputeSubgridIntrinsicBlockSize`: the block size of the subgrid at `index`
  /// measured in `space`, both its intrinsic sizes, kept until its subgridded tracks change.
  fn subgrid_intrinsic_block_size(
    tree: &mut LayoutTree<'r>,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    space: &ConstraintSpace,
  ) -> LayoutUnit {
    let node = sizing_tree.node(index).node;

    if let Some(sizes) = tree
      .node_state(node)
      .layout_grid
      .cached_subgrid_min_max_sizes()
    {
      return sizes.max_size;
    }

    let block_size = GridLayoutAlgorithm::new(
      tree,
      node,
      space,
      true,
      &FragmentationLayoutInput::default(),
    )
    .compute_subgrid_intrinsic_block_size(sizing_tree, index);

    // The min-content and max-content block sizes are both the box's size after layout.
    tree
      .node_state_mut(node)
      .layout_grid
      .set_subgrid_min_max_sizes_cache(
        MinMaxSizes::both(block_size),
        &sizing_tree.node(index).layout_data,
      );
    block_size
  }

  fn direction(&self) -> Direction {
    self.style.direction
  }

  fn available_size(&self, direction: GridTrackSizingDirection) -> Option<LayoutUnit> {
    match direction {
      ForColumns => self.grid_available_size.inline_size,
      ForRows => self.grid_available_size.block_size,
    }
  }

  fn border_padding_start(&self, direction: GridTrackSizingDirection) -> LayoutUnit {
    match direction {
      ForColumns => self.border_padding.inline_start,
      ForRows => self.border_padding.block_start,
    }
  }

  /// Blink's `CalculateGutterSize` for this grid, a `normal` gap `parent_gutter_size`.
  fn gutter_size(
    &self,
    direction: GridTrackSizingDirection,
    parent_gutter_size: LayoutUnit,
  ) -> LayoutUnit {
    calculate_gutter_size(
      &self.style,
      &self.grid_style,
      self.grid_available_size,
      direction,
      parent_gutter_size,
    )
  }

  /// Blink's `ComputeAutomaticRepetitions`, a subgridded axis's in `subgrid_span` of its parent.
  /// <https://drafts.csswg.org/css-grid-2/#auto-repeat>
  fn compute_automatic_repetitions(
    &self,
    subgrid_span: GridSpan,
    direction: GridTrackSizingDirection,
  ) -> usize {
    let track_list = &self.grid_style.template_tracks(direction).track_list;

    if !track_list.has_auto_repeater() {
      return 0;
    }
    if track_list.is_subgridded_axis() {
      // A subgridded axis with no parent grid has the used value `none`.
      // <https://drafts.csswg.org/css-grid-2/#subgrid-listing>
      if subgrid_span.is_indefinite() {
        return 0;
      }
      return self
        .compute_automatic_repetitions_for_subgrid(subgrid_span.integer_span(), direction);
    }

    let (min_available_size, max_available_size) = match direction {
      ForColumns => (
        self.grid_min_available_size.inline_size,
        self.grid_max_available_size.inline_size,
      ),
      ForRows => (
        self.grid_min_available_size.block_size,
        self.grid_max_available_size.block_size,
      ),
    };

    calculate_automatic_repetitions(
      track_list,
      self.gutter_size(direction, LayoutUnit::ZERO),
      self.available_size(direction),
      min_available_size,
      max_available_size,
    )
  }

  /// Blink's `ComputeAutomaticRepetitionsForSubgrid`: an `auto-fill` name list repeats enough to fill
  /// the subgrid's span, none when the other line names already do.
  fn compute_automatic_repetitions_for_subgrid(
    &self,
    subgrid_span_size: usize,
    direction: GridTrackSizingDirection,
  ) -> usize {
    let track_list = &self.grid_style.template_tracks(direction).track_list;
    let non_auto_repeat_line_count = track_list.non_auto_repeat_line_count();
    let tracks_per_repeat = track_list.auto_repeat_track_count();

    if non_auto_repeat_line_count > subgrid_span_size || tracks_per_repeat > subgrid_span_size {
      return 0;
    }

    (subgrid_span_size - non_auto_repeat_line_count + 1) / tracks_per_repeat
  }

  /// Blink's `BuildGridLineResolver`: this grid's, a subgrid's in its area of the grid `parent` resolves.
  fn build_grid_line_resolver(
    &self,
    parent: Option<(&GridLineResolver, GridArea)>,
  ) -> GridLineResolver {
    let subgrid_area = parent
      .as_ref()
      .map_or_else(GridArea::default, |(_, area)| *area);
    let column_auto_repetitions =
      self.compute_automatic_repetitions(subgrid_area.columns, ForColumns);
    let row_auto_repetitions = self.compute_automatic_repetitions(subgrid_area.rows, ForRows);

    match parent {
      Some((parent_line_resolver, subgrid_area)) => GridLineResolver::for_subgrid(
        Rc::clone(&self.grid_style),
        self.direction(),
        parent_line_resolver,
        subgrid_area,
        column_auto_repetitions,
        row_auto_repetitions,
      ),
      None => GridLineResolver::new(
        Rc::clone(&self.grid_style),
        self.direction(),
        column_auto_repetitions,
        row_auto_repetitions,
      ),
    }
  }

  /// Blink's `GridNode::ConstructGridItems`: the in-flow children as items placed as `line_resolver`
  /// resolves them, the placement the grid keeps unless it or an ancestor must place anew, the
  /// out-of-flow children, and whether an item is a subgrid.
  fn construct_grid_items(
    &mut self,
    line_resolver: &GridLineResolver,
    must_invalidate_placement_cache: &mut bool,
    mut out_of_flow_children: Option<&mut Vec<NodeId>>,
  ) -> (GridItems, Rc<GridPlacementData>, bool) {
    let mut grid_items = GridItems::default();
    let mut has_nested_subgrid = false;

    for &child in self.tree.child_nodes(self.node).iter() {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));

      if child_style.display == Display::None {
        if self.space.cache_slot == CacheSlot::Layout {
          self.tree.hide(child);
        }
        continue;
      }
      if child_style.is_out_of_flow() {
        if let Some(out_of_flow_children) = out_of_flow_children.as_deref_mut() {
          out_of_flow_children.push(child);
        }
        continue;
      }

      let item = GridItemData::new_in_grid(
        child,
        child_style,
        &self.style,
        &self.style,
        self.tree.is_replaced(child),
        line_resolver.has_standalone_axis(ForColumns),
        line_resolver.has_standalone_axis(ForRows),
      );

      has_nested_subgrid |= item.is_subgrid();
      grid_items.append(item);
    }

    let placement_styles: Vec<Rc<LayoutStyle>> = [Rc::clone(&self.style)]
      .into_iter()
      .chain(grid_items.iter().map(|item| Rc::clone(&item.style)))
      .collect();
    let cached_placement_data = self
      .tree
      .node_state(self.node)
      .layout_grid
      .cached_placement_data(&placement_styles)
      .filter(|placement_data| {
        !*must_invalidate_placement_cache && placement_data.line_resolver == *line_resolver
      })
      .cloned();

    // A subgrid places its items anew when an ancestor did.
    *must_invalidate_placement_cache |= cached_placement_data.is_none();

    let placement_data = cached_placement_data.unwrap_or_else(|| {
      let item_styles: Vec<&LayoutStyle> = grid_items.iter().map(|item| &*item.style).collect();
      let placement_data = Rc::new(
        GridPlacement::new(&self.grid_style, line_resolver.clone())
          .run_auto_placement_algorithm(&item_styles),
      );

      self
        .tree
        .node_state_mut(self.node)
        .layout_grid
        .set_cached_placement_data(Rc::clone(&placement_data), placement_styles);
      placement_data
    });

    for (item, position) in grid_items
      .iter_mut()
      .zip(&placement_data.grid_item_positions)
    {
      item.resolved_position = *position;
    }
    (grid_items, placement_data, has_nested_subgrid)
  }

  /// Blink's `AppendSubgriddedItems`: the items of the subgrids among `grid_items` that this grid sizes,
  /// in its lines, theirs after them.
  fn append_subgridded_items(&self, sizing_tree: &GridSizingTree, grid_items: &mut GridItems) {
    let mut index = 0;

    while index < grid_items.include_subgridded_items().len() {
      let current_item = grid_items.include_subgridded_items()[index].clone();

      index += 1;
      if !current_item.must_consider_grid_items_for_column_sizing
        && !current_item.must_consider_grid_items_for_row_sizing
      {
        continue;
      }

      let subgrid_node = sizing_tree.node(sizing_tree.lookup_subgrid_index(current_item.node));

      for subgrid_item in subgrid_node.grid_items.iter() {
        let mut subgridded_item = GridItemData::new_in_grid(
          subgrid_item.node,
          Rc::clone(&subgrid_item.style),
          &current_item.style,
          &self.style,
          self.tree.is_replaced(subgrid_item.node),
          current_item.must_consider_grid_items_for_column_sizing,
          current_item.must_consider_grid_items_for_row_sizing,
        );

        subgridded_item.resolved_position = subgrid_item.resolved_position;
        subgridded_item.is_subgridded_to_parent_grid = true;
        subgridded_item.adjust_subgridded_item_span(&current_item);
        grid_items.append(subgridded_item);
      }
    }
  }

  /// Blink's `BuildSizingCollection`: the ranges of `direction`'s tracks the items placed as
  /// `placement_data` cover, and the items' first and last ranges.
  fn build_sizing_collection(
    &self,
    direction: GridTrackSizingDirection,
    placement_data: &GridPlacementData,
    grid_items: &mut GridItems,
    layout_data: &mut GridLayoutData,
  ) {
    let mut range_builder = GridRangeBuilder::new(
      &self.grid_style,
      direction,
      placement_data.line_resolver.auto_repetitions(direction),
      placement_data.start_offset(direction),
    );
    let items = grid_items.include_subgridded_items_mut();
    let mut must_create_baselines = false;
    let mut covered_items = Vec::new();

    for (index, item) in items.iter().enumerate() {
      if item.is_considered_for_sizing(direction) {
        must_create_baselines |= item.is_baseline_specified(direction);
      }
      if item.must_cache_placement_indices(direction) {
        range_builder.ensure_track_coverage(
          item.start_line(direction),
          item.span_size(direction),
          index,
        );
        covered_items.push(index);
      }
    }

    let (ranges, item_ranges) = range_builder.finalize_ranges(items.len());

    for index in covered_items {
      let (begin, end) = item_ranges[index];

      *items[index].range_indices_mut(direction) = GridItemIndices {
        begin: Some(begin),
        end: Some(end),
      };
    }
    layout_data.set_track_collection(GridTrackCollection::new(ranges, direction));
    if must_create_baselines {
      layout_data.create_baselines(direction);
    }
  }

  /// Blink's `BuildGridSizingTree`: this grid and its nested subgrids, their items placed, and its
  /// out-of-flow children.
  fn build_grid_sizing_tree(&mut self) -> (GridSizingTree, Vec<NodeId>, Rc<GridPlacementData>) {
    let line_resolver = self.build_grid_line_resolver(None);
    let mut sizing_tree = GridSizingTree::default();
    let mut out_of_flow_children = Vec::new();
    let placement_data = self.build_grid_sizing_subtree(
      &mut sizing_tree,
      &line_resolver,
      Some(&mut out_of_flow_children),
      None,
      false,
      false,
    );

    (sizing_tree, out_of_flow_children, placement_data)
  }

  /// Blink's `BuildGridSizingTreeIgnoringChildren`: the explicit grid alone, which a size-contained
  /// grid sizes its tracks from.
  fn build_grid_sizing_tree_ignoring_children(&mut self) -> GridSizingTree {
    let line_resolver = self.build_grid_line_resolver(None);
    let mut sizing_tree = GridSizingTree::default();

    self.build_grid_sizing_subtree(&mut sizing_tree, &line_resolver, None, None, false, true);
    sizing_tree
  }

  /// Blink's `BuildGridSizingSubtree`: this grid's node, the subtrees of its subgrids after it, and the
  /// items of those subgrids it sizes, and the placement of its own items.
  fn build_grid_sizing_subtree(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    line_resolver: &GridLineResolver,
    out_of_flow_children: Option<&mut Vec<NodeId>>,
    subgrid_data: Option<SubgriddedItemData>,
    mut must_invalidate_placement_cache: bool,
    must_ignore_children: bool,
  ) -> Rc<GridPlacementData> {
    let (mut grid_items, placement_data, has_nested_subgrid) = if must_ignore_children {
      (
        GridItems::default(),
        Rc::new(
          GridPlacement::new(&self.grid_style, line_resolver.clone())
            .run_auto_placement_algorithm(&[]),
        ),
        false,
      )
    } else {
      self.construct_grid_items(
        line_resolver,
        &mut must_invalidate_placement_cache,
        out_of_flow_children,
      )
    };
    let subgrid_area = subgridded_area_in_parent(subgrid_data.map(|data| data.item(sizing_tree)));
    let has_standalone_columns = subgrid_area.columns.is_indefinite();
    let has_standalone_rows = subgrid_area.rows.is_indefinite();
    let mut layout_data = GridLayoutData::default();

    if has_standalone_columns {
      self.build_sizing_collection(
        ForColumns,
        &placement_data,
        &mut grid_items,
        &mut layout_data,
      );
    }
    if has_standalone_rows {
      self.build_sizing_collection(ForRows, &placement_data, &mut grid_items, &mut layout_data);
    }

    let index = sizing_tree.add_to_preorder_traversal(self.node);

    if !has_nested_subgrid {
      sizing_tree.set_sizing_node_data(index, grid_items, layout_data);
      return placement_data;
    }

    for direction in [ForColumns, ForRows] {
      self.initialize_track_collection(sizing_tree, subgrid_data, direction, &mut layout_data);
    }
    if has_standalone_columns {
      layout_data.columns.cache_definite_sets_geometry();
    }
    if has_standalone_rows {
      layout_data.rows.cache_definite_sets_geometry();
    }

    // The subgrids read this grid's tracks and their own data in it through the tree.
    {
      let tree_node = sizing_tree.node_mut(index);

      tree_node.grid_items = grid_items;
      tree_node.layout_data = layout_data;
    }

    let subgrid_item_indices: Vec<usize> = sizing_tree
      .node(index)
      .grid_items
      .iter()
      .enumerate()
      .filter(|(_, item)| item.is_subgrid())
      .map(|(item_index, _)| item_index)
      .collect();

    for item_index in subgrid_item_indices {
      let GridTreeNode {
        grid_items,
        layout_data,
        ..
      } = sizing_tree.node_mut(index);

      // Blink's `ComputeSetIndicesForSubgrid`.
      grid_items[item_index].compute_set_indices(&layout_data.columns);
      grid_items[item_index].compute_set_indices(&layout_data.rows);

      let subgrid_data = SubgriddedItemData {
        item_index_in_parent: item_index,
        parent_grid_index: index,
      };
      let subgrid_item = subgrid_data.item(sizing_tree).clone();
      let (space, _) = self.create_constraint_space_for_layout(
        &subgrid_item,
        subgrid_data.parent_layout_data(sizing_tree),
        None,
      );
      let mut subgrid_algorithm =
        GridLayoutAlgorithm::for_subgrid(&mut *self.tree, &subgrid_item, &space, None);
      let subgrid_line_resolver = subgrid_algorithm.build_grid_line_resolver(Some((
        line_resolver,
        subgridded_area_in_parent(Some(&subgrid_item)),
      )));

      subgrid_algorithm.build_grid_sizing_subtree(
        sizing_tree,
        &subgrid_line_resolver,
        None,
        Some(subgrid_data),
        must_invalidate_placement_cache,
        false,
      );
      // Sizing the subgridded items moves the ranges, so the item finds its own again.
      sizing_tree.node_mut(index).grid_items[item_index].reset_placement_indices();
    }

    let mut grid_items = take(&mut sizing_tree.node_mut(index).grid_items);
    let mut layout_data = take(&mut sizing_tree.node_mut(index).layout_data);

    self.append_subgridded_items(sizing_tree, &mut grid_items);
    // The ranges cover the subgridded items now, which a subgrid's automatic repetitions needed
    // the ranges for.
    if has_standalone_columns {
      self.build_sizing_collection(
        ForColumns,
        &placement_data,
        &mut grid_items,
        &mut layout_data,
      );
    }
    if has_standalone_rows {
      self.build_sizing_collection(ForRows, &placement_data, &mut grid_items, &mut layout_data);
    }
    sizing_tree.set_sizing_node_data(index, grid_items, layout_data);
    placement_data
  }

  /// Blink's `CreateSubgridTrackCollection`: the tracks of this subgrid's `direction`, its parent's
  /// tracks it spans.
  fn create_subgrid_track_collection(
    &self,
    sizing_tree: &GridSizingTree,
    subgrid_data: SubgriddedItemData,
    direction: GridTrackSizingDirection,
  ) -> GridTrackCollection {
    let subgrid_item = subgrid_data.item(sizing_tree);
    let parent_track_collection = subgrid_data
      .parent_layout_data(sizing_tree)
      .collection(direction);
    let range_indices = subgrid_item.range_indices(direction);

    parent_track_collection.create_subgrid_track_collection(
      range_indices.begin.unwrap_or(0),
      range_indices.end.unwrap_or(0),
      self.gutter_size(direction, parent_track_collection.gutter_size),
      margins_for(
        &self.style,
        self.space.percentage_resolution_inline_size,
        self.direction(),
      ),
      self.border_padding,
      direction,
      subgrid_item.is_opposite_direction_in_root_grid(direction),
    )
  }

  /// Blink's `InitializeTrackCollection`: a subgridded axis's tracks from the parent, a standalone
  /// one's sets.
  fn initialize_track_collection(
    &self,
    sizing_tree: &GridSizingTree,
    subgrid_data: Option<SubgriddedItemData>,
    direction: GridTrackSizingDirection,
    layout_data: &mut GridLayoutData,
  ) {
    if layout_data.has_subgridded_axis(direction) {
      if let Some(subgrid_data) = subgrid_data {
        layout_data.set_track_collection(self.create_subgrid_track_collection(
          sizing_tree,
          subgrid_data,
          direction,
        ));
      }
      return;
    }

    layout_data.collection_mut(direction).build_sets(
      &self.grid_style,
      self.available_size(direction),
      self.gutter_size(direction, LayoutUnit::ZERO),
    );
  }

  /// Blink's `ForEachSubgrid`: `callback` with each subgrid of the grid at `index`, its algorithm's
  /// intrinsic sizes from its subtree when `should_compute_min_max_sizes`.
  fn for_each_subgrid(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    should_compute_min_max_sizes: bool,
    callback: &mut SubgridCallback<'_, 'r>,
  ) {
    let subgrid_subtrees = sizing_tree.children(index);

    if subgrid_subtrees.is_empty() {
      return;
    }

    let subgrid_item_indices = sizing_tree
      .node(index)
      .grid_items
      .iter()
      .enumerate()
      .filter(|(_, item)| item.is_subgrid())
      .map(|(item_index, _)| item_index);

    for (item_index, subtree) in subgrid_item_indices
      .collect::<Vec<_>>()
      .into_iter()
      .zip(subgrid_subtrees)
    {
      let subgrid_data = SubgriddedItemData {
        item_index_in_parent: item_index,
        parent_grid_index: index,
      };
      let subgrid_item = subgrid_data.item(sizing_tree).clone();
      let (space, _) = self.create_constraint_space_for_layout(
        &subgrid_item,
        subgrid_data.parent_layout_data(sizing_tree),
        None,
      );
      let mut subgrid_algorithm = GridLayoutAlgorithm::for_subgrid(
        &mut *self.tree,
        &subgrid_item,
        &space,
        should_compute_min_max_sizes.then_some((&mut *sizing_tree, subtree)),
      );

      callback(&mut subgrid_algorithm, sizing_tree, subtree, subgrid_data);
    }
  }

  /// Blink's `InitializeTrackSizes` for the subtree at `index`, the subgrid `subgrid_data` or the root,
  /// in `direction` or both.
  fn initialize_track_sizes(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    subgrid_data: Option<SubgriddedItemData>,
    direction: Option<GridTrackSizingDirection>,
  ) {
    let directions = match direction {
      Some(direction) => vec![direction],
      None => vec![ForColumns, ForRows],
    };

    for direction in directions {
      let mut layout_data = take(&mut sizing_tree.node_mut(index).layout_data);

      self.initialize_track_collection(sizing_tree, subgrid_data, direction, &mut layout_data);

      let grid_items = &mut sizing_tree.node_mut(index).grid_items;
      let mut has_indefinite_standalone_axis = false;

      if layout_data.has_subgridded_axis(direction) {
        for item in grid_items.iter_mut() {
          item.compute_set_indices(layout_data.collection(direction));
        }
      } else {
        let track_collection = layout_data.collection_mut(direction);

        GridTrackSizingAlgorithm::cache_grid_items_properties(
          track_collection,
          grid_items.include_subgridded_items_mut(),
        );

        if track_collection.has_non_definite_track() {
          has_indefinite_standalone_axis = subgrid_data.is_some();
          track_collection.cache_initialized_sets_geometry(self.border_padding_start(direction));
        } else {
          self.finalize_sets_geometry(track_collection);
        }

        let set_count = track_collection.set_count();

        if layout_data.has_baselines(direction) {
          layout_data.reset_baselines(direction, set_count);
        }
      }
      sizing_tree.node_mut(index).layout_data = layout_data;
      if has_indefinite_standalone_axis {
        sizing_tree.set_subgrid_has_indefinite_standalone_axis();
      }
    }

    // Blink's `InitializeTrackSizesForEachSubgrid`.
    self.for_each_subgrid(
      sizing_tree,
      index,
      false,
      &mut |subgrid_algorithm, sizing_tree, subtree, subgrid_data| {
        subgrid_algorithm.initialize_track_sizes(
          sizing_tree,
          subtree,
          Some(subgrid_data),
          direction,
        );
      },
    );
  }

  /// Blink's `ComputeFirstSetGeometry` then `FinalizeSetsGeometry`.
  fn finalize_sets_geometry(&self, track_collection: &mut GridTrackCollection) {
    let direction = track_collection.direction;
    let geometry = GridTrackSizingAlgorithm::compute_first_set_geometry(
      track_collection,
      &self.style,
      self.available_size(direction),
      self.border_padding_start(direction),
    );

    track_collection.finalize_sets_geometry(geometry.start_offset, geometry.gutter_size);
  }

  /// Blink's `CreateConstraintSpace`: a space in the item's grid area `containing_grid_area_size`,
  /// its sizes fixed to those of `fixed_available_size` that are definite, a subgrid's with the
  /// tracks it lays out in.
  fn create_constraint_space(
    &self,
    cache_slot: CacheSlot,
    item: &GridItemData,
    containing_grid_area_size: AvailableLogicalSize,
    fixed_available_size: AvailableLogicalSize,
    layout_subtree: Option<GridLayoutSubtree>,
  ) -> ConstraintSpace {
    let mut space = ConstraintSpace::for_child(
      self.space,
      containing_grid_area_size.inline_size,
      item.style.direction,
    );

    space.is_new_formatting_context = true;
    space.cache_slot = cache_slot;
    if let Some(inline_size) = fixed_available_size.inline_size {
      space.available_inline_size = Some(inline_size);
      space.is_fixed_inline_size = true;
    }
    space.available_block_size = containing_grid_area_size.block_size;
    if let Some(block_size) = fixed_available_size.block_size {
      space.available_block_size = Some(block_size);
      space.is_fixed_block_size = true;
    }
    space.percentage_resolution_inline_size = containing_grid_area_size.inline_size;
    space.percentage_resolution_block_size = containing_grid_area_size.block_size;
    space.inline_auto_behavior = item.column_auto_behavior;
    space.block_auto_behavior = item.row_auto_behavior;
    space.grid_layout_subtree = layout_subtree;
    space
  }

  /// A subgrid's grid area less its margins, Blink's `ShrinkLogicalSize`, which a subgrid fills.
  fn subgrid_fixed_available_size(
    &self,
    subgrid: &GridItemData,
    containing_grid_area_size: AvailableLogicalSize,
  ) -> AvailableLogicalSize {
    let margins = margins_for(
      &subgrid.style,
      containing_grid_area_size.inline_size,
      self.direction(),
    );

    AvailableLogicalSize {
      inline_size: containing_grid_area_size
        .inline_size
        .map(|size| (size - margins.inline_sum()).clamp_negative_to_zero()),
      block_size: containing_grid_area_size
        .block_size
        .map(|size| (size - margins.block_sum()).clamp_negative_to_zero()),
    }
  }

  /// Blink's `CreateConstraintSpaceForLayout`: the space and the grid area of `subgridded_item` in
  /// its parent's tracks `layout_data`, a subgrid's sized to the area in the axes it subgrids and
  /// laid out in `layout_subtree`.
  fn create_constraint_space_for_layout(
    &self,
    subgridded_item: &GridItemData,
    layout_data: &GridLayoutData,
    layout_subtree: Option<GridLayoutSubtree>,
  ) -> (ConstraintSpace, LogicalRect) {
    let (inline_size, inline_offset) =
      subgridded_item.calculate_available_size(&layout_data.columns);
    let (block_size, block_offset) = subgridded_item.calculate_available_size(&layout_data.rows);
    let containing_grid_area = LogicalRect::new(
      inline_offset,
      block_offset,
      inline_size.unwrap_or_default(),
      block_size.unwrap_or_default(),
    );
    let containing_grid_area_size = AvailableLogicalSize {
      inline_size,
      block_size,
    };
    let fixed_available_size = if subgridded_item.is_subgrid() {
      let fixed = self.subgrid_fixed_available_size(subgridded_item, containing_grid_area_size);

      AvailableLogicalSize {
        inline_size: fixed
          .inline_size
          .filter(|_| subgridded_item.has_subgridded_columns),
        block_size: fixed
          .block_size
          .filter(|_| subgridded_item.has_subgridded_rows),
      }
    } else {
      AvailableLogicalSize::default()
    };
    let space = self.create_constraint_space(
      self.space.cache_slot,
      subgridded_item,
      containing_grid_area_size,
      fixed_available_size,
      layout_subtree,
    );

    (space, containing_grid_area)
  }

  /// The grid area Blink's `CreateConstraintSpaceForMeasure` measures `subgridded_item` for
  /// `direction`'s tracks in: the size of the tracks it spans in the other axis.
  fn measure_grid_area_size(
    subgridded_item: &GridItemData,
    layout_data: &GridLayoutData,
    direction: GridTrackSizingDirection,
  ) -> AvailableLogicalSize {
    match direction {
      ForColumns => AvailableLogicalSize {
        inline_size: None,
        block_size: subgridded_item
          .calculate_available_size(&layout_data.rows)
          .0,
      },
      ForRows => AvailableLogicalSize {
        inline_size: subgridded_item
          .calculate_available_size(&layout_data.columns)
          .0,
        block_size: None,
      },
    }
  }

  /// Blink's `CreateConstraintSpaceForMeasure` in `containing_grid_area_size`, from
  /// [`Self::measure_grid_area_size`], a subgrid's sized to the area.
  fn create_constraint_space_for_measure(
    &self,
    subgridded_item: &GridItemData,
    containing_grid_area_size: AvailableLogicalSize,
    fixed_inline_size: Option<LayoutUnit>,
  ) -> ConstraintSpace {
    let mut fixed_available_size = if subgridded_item.is_subgrid() {
      self.subgrid_fixed_available_size(subgridded_item, containing_grid_area_size)
    } else {
      AvailableLogicalSize::default()
    };

    if fixed_inline_size.is_some() {
      fixed_available_size.inline_size = fixed_inline_size;
    }
    self.create_constraint_space(
      CacheSlot::Measure,
      subgridded_item,
      containing_grid_area_size,
      fixed_available_size,
      None,
    )
  }

  /// Blink's `ContributionSizeForGridItem` for an item of the subtree at `index`, a subgridded one
  /// measured in the tracks of the subgrid that holds it.
  fn contribution_size_for_grid_item(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    contribution_type: GridItemContributionType,
    direction: GridTrackSizingDirection,
    item: &mut GridItemData,
  ) -> LayoutUnit {
    let item_style = Rc::clone(&item.style);
    let measure = {
      let (subgridded_item, parent_layout_data) = if item.is_subgridded_to_parent_grid {
        let subgridded_item_data = sizing_tree.lookup_subgridded_item_data(item);

        (
          subgridded_item_data.item(sizing_tree),
          subgridded_item_data.parent_layout_data(sizing_tree),
        )
      } else {
        (&*item, &sizing_tree.node(index).layout_data)
      };
      let containing_grid_area_size =
        Self::measure_grid_area_size(subgridded_item, parent_layout_data, direction);
      let space =
        self.create_constraint_space_for_measure(subgridded_item, containing_grid_area_size, None);
      let track_collection = parent_layout_data.collection(direction);
      let (begin_set_index, end_set_index) = subgridded_item.set_indices(direction);
      let margins = margins_for(
        &item_style,
        space.percentage_resolution_inline_size,
        self.direction(),
      );

      ItemMeasure {
        track_baseline: track_baseline(item, &sizing_tree.node(index).layout_data, direction),
        extra_margin_for_baseline: extra_margin_for_baseline(
          margins,
          subgridded_item,
          track_collection,
        ),
        start_extra_margin: track_collection.start_extra_margin(begin_set_index),
        end_extra_margin: track_collection.end_extra_margin(end_set_index),
        spanned_tracks_definite_max_size: track_collection
          .calculate_set_span_size(begin_set_index, end_set_index),
        subgridded_item: subgridded_item.clone(),
        containing_grid_area_size,
        space,
        margins,
        direction,
      }
    };
    let margin_sum = match direction {
      ForColumns => measure.margins.inline_sum(),
      ForRows => measure.margins.block_sum(),
    } + measure.start_extra_margin
      + measure.end_extra_margin;
    let contribution = match contribution_type {
      GridItemContributionType::ContentBasedMinimums
      | GridItemContributionType::IntrinsicMaximums => {
        self
          .content_contribution(sizing_tree, &measure, item, true)
          .0
      }
      GridItemContributionType::MaxContentMinimums
      | GridItemContributionType::MaxContentMaximums => {
        self
          .content_contribution(sizing_tree, &measure, item, false)
          .0
      }
      GridItemContributionType::IntrinsicMinimums => {
        self.intrinsic_minimum_contribution(sizing_tree, &measure, item, margin_sum)
      }
      GridItemContributionType::FreeSpace => LayoutUnit::ZERO,
    };

    (contribution + margin_sum).clamp_negative_to_zero()
  }

  /// Blink's `CalculateIntrinsicMinimumContribution`, clamped by the definite tracks it spans as
  /// `ContributionSizeForGridItem` does.
  fn intrinsic_minimum_contribution(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    measure: &ItemMeasure,
    item: &mut GridItemData,
    margin_sum: LayoutUnit,
  ) -> LayoutUnit {
    let direction = measure.direction;
    let is_for_columns = direction == ForColumns;
    let item_style = Rc::clone(&item.style);
    let special_spanning_criteria = !item.is_spanning_auto_minimum_track(direction)
      || (item.is_spanning_flexible_track(direction) && item.span_size(direction) > 1);
    let (main_length, min_length, overflow) = if is_for_columns {
      (
        item_style.size.width,
        item_style.min_size.width,
        item_style.overflow.x,
      )
    } else {
      (
        item_style.size.height,
        item_style.min_size.height,
        item_style.overflow.y,
      )
    };

    match main_length {
      LayoutLength::MaxContent => {
        return self
          .content_contribution(sizing_tree, measure, item, false)
          .0;
      }
      LayoutLength::MinContent | LayoutLength::Fixed(_) | LayoutLength::MinIntrinsic => {
        return self
          .content_contribution(sizing_tree, measure, item, true)
          .0;
      }
      LayoutLength::Auto
      | LayoutLength::FitContent
      | LayoutLength::Stretch
      | LayoutLength::Percent(_)
      | LayoutLength::Calc { .. }
      | LayoutLength::Math(_) => {}
    }

    let resolver = LengthResolver::new(&measure.space, &item_style);

    if !min_length.is_auto() || !overflow.is_clip_or_visible() || special_spanning_criteria {
      if !is_for_columns {
        return resolver.resolve_initial_min_block_length(min_length, None);
      }

      let subgrid_min_max_sizes = if item.is_subgrid() {
        let subgrid_index = sizing_tree.lookup_subgrid_index(item.node);
        let sizes = if sizing_tree
          .node(subgrid_index)
          .layout_data
          .is_subgrid_with_standalone_axis(ForColumns)
        {
          GridLayoutAlgorithm::subgrid_min_max_sizes(
            self.tree,
            sizing_tree,
            subgrid_index,
            &measure.space,
          )
        } else {
          MinMaxSizes::default()
        };

        Some(MinMaxSizesResult::new(sizes, false))
      } else {
        None
      };
      let tree = &mut *self.tree;

      return resolver.resolve_min_inline_length(
        &mut |size_type| {
          subgrid_min_max_sizes
            .unwrap_or_else(|| tree.min_max_sizes(item.node, &measure.space, size_type))
        },
        min_length,
        None,
        None,
        FitContentMode::Normal,
      );
    }

    let (contribution, baseline_shim) = self.content_contribution(sizing_tree, measure, item, true);
    let Some(spanned_tracks_definite_max_size) = measure.spanned_tracks_definite_max_size else {
      return contribution;
    };
    let border_padding_sum = if is_for_columns {
      resolver.border_padding.inline_sum()
    } else {
      resolver.border_padding.block_sum()
    };

    clamp_intrinsic_min_size(
      contribution + margin_sum,
      margin_sum + baseline_shim + border_padding_sum,
      spanned_tracks_definite_max_size,
    ) - margin_sum
  }

  /// Blink's `MinContentSize` or `MaxContentSize` for a column and `BlockContributionSize` for a
  /// row: the contribution and the baseline shim in it. A subgrid sizes its standalone axis from
  /// its subtree.
  fn content_contribution(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    measure: &ItemMeasure,
    item: &mut GridItemData,
    is_min_content: bool,
  ) -> (LayoutUnit, LayoutUnit) {
    let direction = measure.direction;

    if direction == ForColumns {
      return self.min_or_max_content_size(sizing_tree, measure, item, is_min_content);
    }
    if item.is_subgrid() {
      // Blink's `ComputeBlockSizeForSubgrid`.
      let subgrid_index = sizing_tree.lookup_subgrid_index(item.node);
      let intrinsic_block_size = GridLayoutAlgorithm::subgrid_intrinsic_block_size(
        self.tree,
        sizing_tree,
        subgrid_index,
        &measure.space,
      );
      let block_size = LengthResolver::new(&measure.space, &item.style)
        .block_size_for_fragment(
          Some(intrinsic_block_size),
          measure.space.available_inline_size,
          None,
        )
        .unwrap_or(intrinsic_block_size);

      return (block_size, LayoutUnit::ZERO);
    }

    let result = if measure.space.available_inline_size.is_none() {
      let (max_content_size, _) = self.min_or_max_content_size(sizing_tree, measure, item, false);
      let fallback_space = self.create_constraint_space_for_measure(
        &measure.subgridded_item,
        measure.containing_grid_area_size,
        Some(max_content_size),
      );

      self.tree.layout(item.node, &fallback_space)
    } else {
      self.tree.layout(item.node, &measure.space)
    };
    let baseline_shim = if item.is_baseline_aligned(direction) {
      measure.baseline_shim(first_baseline_or_synthesize(item, &result))
    } else {
      LayoutUnit::ZERO
    };

    (result.size.block_size + baseline_shim, baseline_shim)
  }

  /// Blink's `MinOrMaxContentSize`.
  fn min_or_max_content_size(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    measure: &ItemMeasure,
    item: &mut GridItemData,
    is_min_content: bool,
  ) -> (LayoutUnit, LayoutUnit) {
    let result = if item.is_subgrid() {
      let subgrid_index = sizing_tree.lookup_subgrid_index(item.node);
      let sizes = GridLayoutAlgorithm::subgrid_min_max_sizes(
        self.tree,
        sizing_tree,
        subgrid_index,
        &measure.space,
      );

      LengthResolver::new(&measure.space, &item.style)
        .min_and_max_content_contribution(&mut |_| MinMaxSizesResult::new(sizes, false))
    } else {
      self
        .tree
        .min_and_max_content_contribution(item.node, &measure.space)
    };

    if result.depends_on_block_constraints {
      item.is_sizing_dependent_on_block_size = true;
      sizing_tree.set_has_block_size_dependent_grid_item();
    }

    let content_size = if is_min_content {
      result.sizes.min_size
    } else {
      result.sizes.max_size
    };
    let baseline_shim = if item.is_baseline_aligned(measure.direction) {
      measure.baseline_shim(synthesized_baseline(
        measure.direction,
        self.direction(),
        content_size,
      ))
    } else {
      LayoutUnit::ZERO
    };

    (content_size + baseline_shim, baseline_shim)
  }

  /// Blink's `AccommodateSubgridExtraMargins`: the edge sets of a subgrid's span in `track_collection`
  /// grow to its margin, border, and padding, which no item on its edge may carry.
  fn accommodate_subgrid_extra_margins(
    sizing_tree: &GridSizingTree,
    index: usize,
    track_collection: &mut GridTrackCollection,
    direction: GridTrackSizingDirection,
  ) {
    let mut accommodate_extra_margin = |extra_margin: LayoutUnit, set_index: usize| {
      let set = &mut track_collection.sets[set_index];

      if set.track_size.has_intrinsic_min_track_breadth() && set.base_size < extra_margin {
        set.increase_base_size(extra_margin);
      }
    };

    for item in sizing_tree
      .node(index)
      .grid_items
      .include_subgridded_items()
    {
      if !item.must_consider_grid_items_for_sizing(direction)
        || !item.is_spanning_intrinsic_track(direction)
      {
        continue;
      }

      let subgrid_track_collection = sizing_tree
        .node(sizing_tree.lookup_subgrid_index(item.node))
        .layout_data
        .collection(direction);
      let mut start_extra_margin = subgrid_track_collection.start_extra_margin(0);
      let mut end_extra_margin = subgrid_track_collection.end_extra_margin(usize::MAX);

      if item.is_opposite_direction_in_root_grid(direction) {
        swap(&mut start_extra_margin, &mut end_extra_margin);
      }

      let (begin_set_index, end_set_index) = item.set_indices(direction);
      let end_position = end_set_index - 1;

      if begin_set_index < end_position {
        accommodate_extra_margin(start_extra_margin, begin_set_index);
        accommodate_extra_margin(end_extra_margin, end_position);
      } else {
        accommodate_extra_margin(start_extra_margin + end_extra_margin, begin_set_index);
      }
    }
  }

  /// Blink's `ComputeUsedTrackSizes` for the subtree at `index`.
  fn compute_used_track_sizes(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    direction: GridTrackSizingDirection,
    sizing_constraint: SizingConstraint,
  ) {
    let available_size = self.available_size(direction);
    let gutter_size = self.gutter_size(direction, LayoutUnit::ZERO);

    sizing_tree
      .node_mut(index)
      .layout_data
      .collection_mut(direction)
      .build_sets(&self.grid_style, available_size, gutter_size);

    // The contributions read the geometry the tracks had before sizing.
    let mut track_collection = sizing_tree
      .node(index)
      .layout_data
      .collection(direction)
      .clone();

    Self::accommodate_subgrid_extra_margins(sizing_tree, index, &mut track_collection, direction);

    let mut grid_items = take(&mut sizing_tree.node_mut(index).grid_items);
    let algorithm = GridTrackSizingAlgorithm::new(
      &self.style,
      self.grid_available_size,
      self.grid_min_available_size,
      sizing_constraint,
    );

    algorithm.compute_used_track_sizes(
      &mut |contribution_type, item| {
        self.contribution_size_for_grid_item(sizing_tree, index, contribution_type, direction, item)
      },
      &mut track_collection,
      grid_items.include_subgridded_items_mut(),
    );

    let tree_node = sizing_tree.node_mut(index);

    tree_node.grid_items = grid_items;
    *tree_node.layout_data.collection_mut(direction) = track_collection;
  }

  /// Blink's `CompleteTrackSizingAlgorithm` for the subtree at `index`: a subgridded axis's tracks
  /// from its parent's sized ones, a standalone one's sized.
  fn complete_track_sizing_algorithm(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    subgrid_data: Option<SubgriddedItemData>,
    direction: GridTrackSizingDirection,
    sizing_constraint: SizingConstraint,
    mut needs_additional_pass: Option<&mut bool>,
  ) {
    let layout_data = &sizing_tree.node(index).layout_data;

    if layout_data.collection(direction).has_non_definite_track() {
      if layout_data.has_subgridded_axis(direction) {
        if let Some(subgrid_data) = subgrid_data {
          let track_collection =
            self.create_subgrid_track_collection(sizing_tree, subgrid_data, direction);

          sizing_tree
            .node_mut(index)
            .layout_data
            .set_track_collection(track_collection);
        }
      } else {
        self.compute_used_track_sizes(sizing_tree, index, direction, sizing_constraint);

        let needs_to_check_block_size_dependent_grid_items = direction == ForRows
          && needs_additional_pass
            .as_deref()
            .is_some_and(|needs_additional_pass| !needs_additional_pass);
        let tree_node = sizing_tree.node_mut(index);
        let block_size_dependent_items: Vec<BlockSizeDependentGridItem> =
          if needs_to_check_block_size_dependent_grid_items {
            tree_node
              .grid_items
              .iter()
              .filter(|item| item.is_sizing_dependent_on_block_size)
              .map(|item| {
                let row_set_indices = item.set_indices(ForRows);

                BlockSizeDependentGridItem {
                  row_set_indices,
                  cached_block_size: tree_node
                    .layout_data
                    .rows
                    .calculate_set_span_size(row_set_indices.0, row_set_indices.1),
                }
              })
              .collect()
          } else {
            Vec::new()
          };

        self.finalize_sets_geometry(tree_node.layout_data.collection_mut(direction));

        // Blink's `MayChangeBlockSizeDependentGridItemContributions`.
        if needs_to_check_block_size_dependent_grid_items
          && let Some(needs_additional_pass) = needs_additional_pass.as_deref_mut()
        {
          *needs_additional_pass = block_size_dependent_items.iter().any(|item| {
            tree_node
              .layout_data
              .rows
              .calculate_set_span_size(item.row_set_indices.0, item.row_set_indices.1)
              != item.cached_block_size
          });
        }
      }
    }

    // Blink's `CompleteTrackSizingAlgorithmForEachSubgrid`.
    self.for_each_subgrid(
      sizing_tree,
      index,
      true,
      &mut |subgrid_algorithm, sizing_tree, subtree, subgrid_data| {
        subgrid_algorithm.complete_track_sizing_algorithm(
          sizing_tree,
          subtree,
          Some(subgrid_data),
          direction,
          sizing_constraint,
          needs_additional_pass.as_deref_mut(),
        );
      },
    );
  }

  /// Blink's `CompleteTrackSizingAlgorithm` for the whole sizing tree.
  fn complete_track_sizing_algorithm_for_tree(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    direction: GridTrackSizingDirection,
    sizing_constraint: SizingConstraint,
    needs_additional_pass: Option<&mut bool>,
  ) {
    validate_min_max_sizes_cache(self.tree, sizing_tree, 0, direction);
    if sizing_tree.has_baselines() {
      let layout_tree = sizing_tree.finalize_tree();

      self.compute_baseline_alignment(&layout_tree, sizing_tree, 0, None, Some(direction));
    }
    self.complete_track_sizing_algorithm(
      sizing_tree,
      0,
      None,
      direction,
      sizing_constraint,
      needs_additional_pass,
    );
  }

  /// Blink's `ComputeSubgridMinMaxSizes`: the subgrid at `index`'s columns sized at min-content and
  /// max-content.
  fn compute_subgrid_min_max_sizes(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
  ) -> MinMaxSizes {
    MinMaxSizes {
      min_size: self.compute_subgrid_intrinsic_size(
        sizing_tree,
        index,
        ForColumns,
        SizingConstraint::MinContent,
      ),
      max_size: self.compute_subgrid_intrinsic_size(
        sizing_tree,
        index,
        ForColumns,
        SizingConstraint::MaxContent,
      ),
    }
  }

  /// Blink's `ComputeSubgridIntrinsicBlockSize`.
  fn compute_subgrid_intrinsic_block_size(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
  ) -> LayoutUnit {
    self.compute_subgrid_intrinsic_size(sizing_tree, index, ForRows, SizingConstraint::MaxContent)
  }

  /// Blink's `ComputeSubgridIntrinsicSize`: the border box of the subgrid at `index` around its
  /// `direction` tracks sized for `sizing_constraint`.
  fn compute_subgrid_intrinsic_size(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    direction: GridTrackSizingDirection,
    sizing_constraint: SizingConstraint,
  ) -> LayoutUnit {
    self.compute_used_track_sizes(sizing_tree, index, direction, sizing_constraint);

    let border_padding = match direction {
      ForColumns => self.border_padding.inline_sum(),
      ForRows => self.border_padding.block_sum(),
    };

    border_padding
      + sizing_tree
        .node(index)
        .layout_data
        .collection(direction)
        .total_track_size()
  }

  /// Blink's `ComputeBaselineAlignment` for the subtree at `index`, the subgrid `subgrid_data` or the
  /// root, in `direction` or both, its subgrids measured from `layout_tree`: a subgridded axis's
  /// baselines its parent's.
  fn compute_baseline_alignment(
    &mut self,
    layout_tree: &Rc<GridLayoutTree>,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    subgrid_data: Option<SubgriddedItemData>,
    direction: Option<GridTrackSizingDirection>,
  ) {
    let (directions, phase) = match direction {
      Some(direction) => (
        vec![direction],
        BaselineCollectionPhase::BaselinesForTrackSizing,
      ),
      None => (
        vec![ForColumns, ForRows],
        BaselineCollectionPhase::FinalBaselines,
      ),
    };

    for direction in directions {
      if !sizing_tree
        .node(index)
        .layout_data
        .has_subgridded_axis(direction)
      {
        self.compute_grid_item_baselines(layout_tree, 0, sizing_tree, index, direction, phase);
        continue;
      }

      let Some(subgrid_data) = subgrid_data else {
        continue;
      };
      let subgrid_item = subgrid_data.item(sizing_tree);
      let parent_layout_data = subgrid_data.parent_layout_data(sizing_tree);
      let Some(parent_baselines) = parent_layout_data.baselines(direction) else {
        continue;
      };
      let parent_track_collection = parent_layout_data.collection(direction);
      let range_indices = subgrid_item.range_indices(direction);
      let baselines = parent_track_collection.create_subgrid_baselines(
        range_indices.begin.unwrap_or(0),
        range_indices.end.unwrap_or(0),
        self.gutter_size(direction, parent_track_collection.gutter_size),
        margins_for(
          &self.style,
          self.space.percentage_resolution_inline_size,
          self.direction(),
        ),
        self.border_padding,
        direction,
        subgrid_item.is_opposite_direction_in_root_grid(direction),
        parent_baselines,
      );

      sizing_tree
        .node_mut(index)
        .layout_data
        .set_baselines(direction, baselines);
    }

    // Blink's `ComputeBaselineAlignmentForEachSubgrid`.
    self.for_each_subgrid(
      sizing_tree,
      index,
      true,
      &mut |subgrid_algorithm, sizing_tree, subtree, subgrid_data| {
        subgrid_algorithm.compute_baseline_alignment(
          layout_tree,
          sizing_tree,
          subtree,
          Some(subgrid_data),
          direction,
        );
      },
    );
  }

  /// Blink's `ResolveBaselinesInStandaloneAxes`: the baselines of the subgrids in the subtree at `index`
  /// whose geometry track sizing had yet to resolve, the innermost first, so each grid measures
  /// its subgrids finalized.
  fn resolve_baselines_in_standalone_axes(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    index: usize,
  ) {
    self.for_each_subgrid(
      sizing_tree,
      index,
      true,
      &mut |subgrid_algorithm, sizing_tree, subtree, _| {
        subgrid_algorithm.resolve_baselines_in_standalone_axes(sizing_tree, subtree);
      },
    );

    let layout_data = &sizing_tree.node(index).layout_data;

    if layout_data.has_subgridded_axis(ForColumns) && layout_data.has_subgridded_axis(ForRows) {
      return;
    }

    let layout_tree = Rc::new(sizing_tree.finalize_subtree_at(index));

    for direction in [ForColumns, ForRows] {
      if !sizing_tree
        .node(index)
        .layout_data
        .has_subgridded_axis(direction)
      {
        self.compute_grid_item_baselines(
          &layout_tree,
          index,
          sizing_tree,
          index,
          direction,
          BaselineCollectionPhase::BaselinesForStandaloneAxes,
        );
      }
    }
  }

  /// Blink's `ComputeGridItemBaselines`: each set's baselines in `direction` for `phase`, a subgrid
  /// measured in its subtree of `layout_tree`, which is rooted at sizing tree index
  /// `layout_tree_root`.
  fn compute_grid_item_baselines(
    &mut self,
    layout_tree: &Rc<GridLayoutTree>,
    layout_tree_root: usize,
    sizing_tree: &mut GridSizingTree,
    index: usize,
    direction: GridTrackSizingDirection,
    phase: BaselineCollectionPhase,
  ) {
    if !sizing_tree.node(index).layout_data.has_baselines(direction) {
      return;
    }

    let tree_node = sizing_tree.node_mut(index);
    let set_count = tree_node.layout_data.collection(direction).set_count();

    // The standalone-axes phase adds the subgrids' baselines to those track sizing found.
    if phase != BaselineCollectionPhase::BaselinesForStandaloneAxes {
      tree_node.layout_data.reset_baselines(direction, set_count);
    }

    let item_count = tree_node.grid_items.include_subgridded_items().len();

    for item_index in 0..item_count {
      let item = &sizing_tree
        .node(index)
        .grid_items
        .include_subgridded_items()[item_index];

      if !item.is_baseline_specified(direction)
        || !item.is_considered_for_sizing(direction)
        || (phase == BaselineCollectionPhase::BaselinesForStandaloneAxes && !item.is_subgrid())
      {
        continue;
      }

      let subgrid_layout_subtree = if item.is_subgrid() {
        let layout_subtree = GridLayoutSubtree::at(
          Rc::clone(layout_tree),
          sizing_tree.lookup_subgrid_index(item.node) - layout_tree_root,
        );

        // A nested subgrid lays out in resolved geometry alone, so the standalone-axes pass
        // measures it once track sizing finalized its subtree.
        if layout_subtree.has_unresolved_geometry() {
          sizing_tree.set_has_deferred_subgrid_baseline();
          continue;
        }
        Some(layout_subtree)
      } else {
        None
      };
      let node = item.node;
      let (space, extra_margin) = {
        let (subgridded_item, parent_layout_data) = if item.is_subgridded_to_parent_grid {
          let subgridded_item_data = sizing_tree.lookup_subgridded_item_data(item);

          (
            subgridded_item_data.item(sizing_tree),
            subgridded_item_data.parent_layout_data(sizing_tree),
          )
        } else {
          (item, &sizing_tree.node(index).layout_data)
        };
        let space = if phase == BaselineCollectionPhase::BaselinesForTrackSizing {
          let containing_grid_area_size =
            Self::measure_grid_area_size(subgridded_item, parent_layout_data, direction);

          self.create_constraint_space_for_measure(subgridded_item, containing_grid_area_size, None)
        } else {
          self
            .create_constraint_space_for_layout(
              subgridded_item,
              parent_layout_data,
              subgrid_layout_subtree,
            )
            .0
        };
        let margins = margins_for(
          &item.style,
          space.percentage_resolution_inline_size,
          self.direction(),
        );
        let extra_margin = extra_margin_for_baseline(
          margins,
          subgridded_item,
          parent_layout_data.collection(direction),
        );

        (space, extra_margin)
      };
      let result = self.tree.layout(node, &space);
      let GridTreeNode {
        grid_items,
        layout_data,
        ..
      } = sizing_tree.node_mut(index);
      let item = &mut grid_items.include_subgridded_items_mut()[item_index];
      // Blink's `MeasureAndStoreItemBaseline`. A column baseline is always synthesized.
      let has_synthesized_baseline = direction == ForColumns || result.first_baseline.is_none();

      item.set_alignment_fallback(direction, has_synthesized_baseline);
      if !item.is_baseline_aligned(direction) {
        continue;
      }

      // Blink's `StoreItemBaseline`.
      let baseline = extra_margin + item_baseline(item, direction, self.direction(), &result);

      set_track_baseline(item, direction, baseline, layout_data);
    }
  }

  /// Blink's `NeedsAdditionalLayoutPass`.
  fn needs_additional_layout_pass(&self, rows: &GridTrackCollection) -> bool {
    if self.style.row_gap.has_percent() || rows.is_dependent_on_available_size() {
      return true;
    }

    self.space.is_initial_block_size_indefinite
      && LengthResolver::new(self.space, &self.style)
        .block_size_for_fragment(None, self.inline_size, None)
        .is_some()
  }

  /// The early return of Blink's `ComputeGridGeometry` in a subgrid, whose root grid sized its tracks:
  /// the items in `layout_subtree`, placed with the line resolver the grid kept while it was sized.
  fn subgrid_geometry(
    &mut self,
    layout_subtree: GridLayoutSubtree,
  ) -> (GridLayoutState, Vec<NodeId>, LayoutUnit) {
    let line_resolver = match self
      .tree
      .node_state(self.node)
      .layout_grid
      .last_placement_data()
    {
      Some(placement_data) => placement_data.line_resolver.clone(),
      None => self.build_grid_line_resolver(None),
    };
    let mut out_of_flow_children = Vec::new();
    let (mut grid_items, placement_data, _) =
      self.construct_grid_items(&line_resolver, &mut false, Some(&mut out_of_flow_children));
    let layout_data = layout_subtree.layout_data().clone();

    GridTrackSizingAlgorithm::cache_grid_items_properties(
      &layout_data.columns,
      grid_items.include_subgridded_items_mut(),
    );
    GridTrackSizingAlgorithm::cache_grid_items_properties(
      &layout_data.rows,
      grid_items.include_subgridded_items_mut(),
    );

    let intrinsic_block_size = self.calculate_intrinsic_block_size(&layout_data.rows);

    (
      GridLayoutState {
        grid_items,
        layout_data,
        layout_subtree,
        placement_data,
      },
      out_of_flow_children,
      intrinsic_block_size,
    )
  }

  /// Blink's `CalculateIntrinsicBlockSize` in `ComputeGridGeometry`: the block size `rows` take, or
  /// without the items when the grid contains its block size.
  fn calculate_intrinsic_block_size(&mut self, rows: &GridTrackCollection) -> LayoutUnit {
    if self.style.contains_block_size() {
      return self.intrinsic_block_size_ignoring_children();
    }

    rows.total_set_span_size().unwrap_or_default() + self.border_padding.block_sum()
  }

  /// Blink's `ComputeIntrinsicBlockSizeIgnoringChildren`: the block size a block-size-contained
  /// grid's rows take without its items.
  fn intrinsic_block_size_ignoring_children(&mut self) -> LayoutUnit {
    let mut sizing_tree = self.build_grid_sizing_tree_ignoring_children();

    self.initialize_track_sizes(&mut sizing_tree, 0, None, Some(ForRows));
    self.complete_track_sizing_algorithm_for_tree(
      &mut sizing_tree,
      ForRows,
      SizingConstraint::Layout,
      None,
    );

    sizing_tree
      .node(0)
      .layout_data
      .rows
      .total_set_span_size()
      .unwrap_or_default()
      + self.border_padding.block_sum()
  }

  /// Blink's `ComputeGridGeometry`: the sized grid, the out-of-flow children, and the intrinsic block
  /// size.
  fn compute_grid_geometry(&mut self) -> (GridLayoutState, Vec<NodeId>, LayoutUnit) {
    if let Some(layout_subtree) = self.space.grid_layout_subtree.clone() {
      return self.subgrid_geometry(layout_subtree);
    }

    let (mut sizing_tree, out_of_flow_children, placement_data) = self.build_grid_sizing_tree();
    let mut needs_additional_pass = false;

    self.initialize_track_sizes(&mut sizing_tree, 0, None, None);
    self.complete_track_sizing_algorithm_for_tree(
      &mut sizing_tree,
      ForColumns,
      SizingConstraint::Layout,
      Some(&mut needs_additional_pass),
    );
    self.complete_track_sizing_algorithm_for_tree(
      &mut sizing_tree,
      ForRows,
      SizingConstraint::Layout,
      Some(&mut needs_additional_pass),
    );

    let intrinsic_block_size =
      self.calculate_intrinsic_block_size(&sizing_tree.node(0).layout_data.rows);
    let applies_auto_min_size = self.style.aspect_ratio.is_some()
      && self.style.overflow.y.is_clip_or_visible()
      && self.style.min_size.height.is_auto();

    if self.grid_available_size.block_size.is_none() || applies_auto_min_size {
      let block_size = LengthResolver::new(self.space, &self.style)
        .block_size_for_fragment(Some(intrinsic_block_size), self.inline_size, None)
        .unwrap_or(intrinsic_block_size);
      let content_block_size =
        (block_size - self.border_padding.block_sum()).clamp_negative_to_zero();

      self.grid_available_size.block_size = Some(content_block_size);
      self.grid_min_available_size.block_size = content_block_size;
      self.grid_max_available_size.block_size = content_block_size;
      needs_additional_pass |=
        self.needs_additional_layout_pass(&sizing_tree.node(0).layout_data.rows);

      if !needs_additional_pass && self.style.align_content != Default::default() {
        self.finalize_sets_geometry(&mut sizing_tree.node_mut(0).layout_data.rows);
      }
    }

    if needs_additional_pass {
      self.initialize_track_sizes(&mut sizing_tree, 0, None, Some(ForColumns));
      self.complete_track_sizing_algorithm_for_tree(
        &mut sizing_tree,
        ForColumns,
        SizingConstraint::Layout,
        None,
      );
      self.initialize_track_sizes(&mut sizing_tree, 0, None, Some(ForRows));
      self.complete_track_sizing_algorithm_for_tree(
        &mut sizing_tree,
        ForRows,
        SizingConstraint::Layout,
        None,
      );
    } else if sizing_tree.has_subgrid_with_indefinite_standalone_axis()
      && sizing_tree.has_block_size_dependent_grid_item()
    {
      // A subgrid's indefinite rows the first column pass measured against are sized now.
      self.initialize_track_sizes(&mut sizing_tree, 0, None, Some(ForColumns));
      self.complete_track_sizing_algorithm_for_tree(
        &mut sizing_tree,
        ForColumns,
        SizingConstraint::Layout,
        None,
      );
    }

    if sizing_tree.has_deferred_subgrid_baseline() {
      self.resolve_baselines_in_standalone_axes(&mut sizing_tree, 0);
    }
    // Blink's `CompleteFinalBaselineAlignment`.
    if sizing_tree.has_baselines() {
      let layout_tree = sizing_tree.finalize_tree();

      self.compute_baseline_alignment(&layout_tree, &mut sizing_tree, 0, None, None);
    }

    let layout_subtree = GridLayoutSubtree::new(sizing_tree.finalize_tree());
    let root = sizing_tree.node_mut(0);

    (
      GridLayoutState {
        grid_items: take(&mut root.grid_items),
        layout_data: take(&mut root.layout_data),
        layout_subtree,
        placement_data,
      },
      out_of_flow_children,
      intrinsic_block_size,
    )
  }

  /// Blink's `PlaceGridItems`: the items laid out in their grid areas, and the grid's baselines.
  fn place_grid_items(
    &mut self,
    grid_state: &GridLayoutState,
  ) -> (Vec<PlacedChild>, GridBaselineAccumulator) {
    let layout_data = &grid_state.layout_data;
    let mut baseline_accumulator = GridBaselineAccumulator::default();
    let mut children = Vec::with_capacity(grid_state.grid_items.len());

    self.reset_row_break_between(layout_data.rows.set_count());
    for (item, layout_subtree) in grid_state
      .grid_items
      .iter()
      .zip(grid_state.item_layout_subtrees())
    {
      let (space, mut containing_grid_area) =
        self.create_constraint_space_for_layout(item, layout_data, layout_subtree);
      let margins = margins_for(
        &item.style,
        space.percentage_resolution_inline_size,
        self.direction(),
      );
      let result = self.tree.layout(item.node, &space);
      let containing_grid_area_size = containing_grid_area.size;
      let baseline_offset = |direction: GridTrackSizingDirection| {
        // Blink's `ComputeBaselineOffset`.
        if !item.is_baseline_aligned(direction) {
          return LayoutUnit::ZERO;
        }

        let baseline_delta = track_baseline(item, layout_data, direction).unwrap_or_default()
          - item_baseline(item, direction, self.direction(), &result);

        match item.baseline_group(direction) {
          BaselineGroup::Major => baseline_delta,
          BaselineGroup::Minor => {
            let (available_size, item_size) = match direction {
              ForColumns => (
                containing_grid_area_size.inline_size,
                result.size.inline_size,
              ),
              ForRows => (containing_grid_area_size.block_size, result.size.block_size),
            };

            available_size - baseline_delta - item_size
          }
        }
      };

      containing_grid_area.offset.inline_offset += alignment_offset(
        containing_grid_area.size.inline_size,
        result.size.inline_size,
        margins.inline_start,
        margins.inline_end,
        baseline_offset(ForColumns),
        item.alignment(ForColumns),
        item.is_overflow_safe(ForColumns),
      );
      containing_grid_area.offset.block_offset += alignment_offset(
        containing_grid_area.size.block_size,
        result.size.block_size,
        margins.block_start,
        margins.block_end,
        baseline_offset(ForRows),
        item.alignment(ForRows),
        item.is_overflow_safe(ForRows),
      );

      let relative_offset = compute_relative_offset(
        self.tree.render_node(item.node),
        self.direction(),
        Some(containing_grid_area.size.inline_size.to_float()),
        Some(containing_grid_area.size.block_size.to_float()),
      )
      .map(LayoutUnit::from_float);

      self.join_row_break_between(item, &result);
      // A grid that breaks lays its items out on each page from where they sit here.
      if self.keeps_placement(item, containing_grid_area.offset, relative_offset) {
        continue;
      }

      let location = self.physical_offset(containing_grid_area.offset, result.size.inline_size)
        + relative_offset;

      baseline_accumulator.accumulate(item, &result, containing_grid_area.offset.block_offset);
      children.push(PlacedChild {
        node: item.node,
        location,
        result,
      });
    }

    if layout_data.has_baselines(ForRows) {
      baseline_accumulator.accumulate_rows(layout_data);
    }

    (children, baseline_accumulator)
  }

  /// The border-box location of a child `child_inline_size` wide at the logical `offset`.
  fn physical_offset(
    &self,
    offset: LogicalOffset,
    child_inline_size: LayoutUnit,
  ) -> Point<LayoutUnit> {
    let x = match self.direction() {
      Direction::Ltr => offset.inline_offset,
      Direction::Rtl => {
        self.inline_size.unwrap_or_default() - offset.inline_offset - child_inline_size
      }
    };

    Point::new(x, offset.block_offset)
  }

  /// Blink's `PlaceOutOfFlowItems`.
  fn place_out_of_flow_items(
    &self,
    grid_state: &GridLayoutState,
    block_size: LayoutUnit,
    out_of_flow_children: Vec<NodeId>,
  ) -> (Vec<OutOfFlowCandidate>, Vec<NodeId>) {
    let should_process_block_end = self.should_process_out_of_flow_block_end();
    let previously_consumed_block_size = self.previously_consumed_block_size();
    let capacity = self.fragmentainer_capacity_for_children();
    let mut held = Vec::new();
    let mut candidates = Vec::new();
    let inline_size = self.inline_size.unwrap_or_default();
    let claims = self.tree.claims(self.node);
    let border = LogicalBoxStrut::from_physical(
      self.style.border.map(LayoutUnit::from_float),
      self.direction(),
    );
    let padding_box_rect = LogicalRect::new(
      border.inline_start,
      border.block_start,
      (inline_size - border.inline_sum()).clamp_negative_to_zero(),
      (block_size - border.block_sum()).clamp_negative_to_zero(),
    );
    let default_containing_block_size = LogicalSize::new(
      (inline_size - self.border_padding.inline_sum()).clamp_negative_to_zero(),
      (block_size - self.border_padding.block_sum()).clamp_negative_to_zero(),
    );
    for child in out_of_flow_children {
      let mut item = GridItemData::new(
        child,
        Rc::clone(self.tree.layout_style_rc(child)),
        &self.style,
        self.tree.is_replaced(child),
      );
      let position = item.style.position;
      let containing_block_rect = ((claims.absolute && position == Position::Absolute)
        || (claims.fixed && position == Position::Fixed))
        .then(|| {
          compute_out_of_flow_item_containing_rect(
            &grid_state.placement_data,
            &grid_state.layout_data,
            padding_box_rect,
            &mut item,
          )
        });
      let mut static_position = LogicalStaticPosition {
        offset: containing_block_rect.map_or(
          LogicalOffset::new(
            self.border_padding.inline_start,
            self.border_padding.block_start,
          ),
          |rect| rect.offset,
        ),
        ..LogicalStaticPosition::default()
      };

      alignment_offset_for_out_of_flow(
        item.alignment(ForColumns),
        item.alignment(ForRows),
        containing_block_rect.map_or(default_containing_block_size, |rect| rect.size),
        &mut static_position,
      );
      static_position.offset.block_offset -= previously_consumed_block_size;

      if !should_process_block_end && static_position.offset.block_offset > capacity {
        held.push(child);
        continue;
      }
      candidates.push(OutOfFlowCandidate::new(
        child,
        static_position.to_physical(self.direction(), inline_size),
      ));
    }
    (candidates, held)
  }

  /// Blink's `Layout`.
  pub(crate) fn layout(mut self) -> LayoutResult {
    let (mut grid_state, out_of_flow_children, mut intrinsic_block_size) =
      match self.resumed_grid_state() {
        Some(resumed) => resumed,
        None => self.compute_grid_geometry(),
      };
    let (children, baselines) = if self.is_resuming() {
      (Vec::new(), GridBaselineAccumulator::default())
    } else {
      self.place_grid_items(&grid_state)
    };
    let (children, baselines) = self.place_grid_items_for_fragmentation(
      &mut grid_state,
      &mut intrinsic_block_size,
      (children, baselines),
    );
    let inline_size = self.inline_size.unwrap_or_default();
    let style = Rc::clone(&self.style);
    let mut resolver = LengthResolver::new(self.space, &style);

    resolver.border_padding = self.border_padding_for_size(resolver.border_padding);

    let block_size = resolver
      .block_size_for_fragment(Some(intrinsic_block_size), Some(inline_size), None)
      .unwrap_or(intrinsic_block_size);
    let fragment_block_size = match self.finish_fragmentation(block_size) {
      Ok(fragment_block_size) => fragment_block_size,
      Err(status) => return self.relayout(status),
    };
    let mut out_of_flow = self.resumed_out_of_flow();

    out_of_flow.extend(
      children
        .iter()
        .flat_map(|child| child.result.out_of_flow_moved_by(child.location)),
    );
    let (candidates, held_out_of_flow) =
      self.place_out_of_flow_items(&grid_state, block_size, out_of_flow_children);

    out_of_flow.extend(candidates);
    self.tree.node_state_mut(self.node).grid_geometry =
      Some(Rc::new(grid_state.geometry(self.direction(), inline_size)));
    self.keep_break_token_data(grid_state, intrinsic_block_size, held_out_of_flow);

    let size = LogicalSize::new(inline_size, fragment_block_size);
    let mut result = LayoutResult::of_box(size, &style, self.space);

    out_of_flow = self.tree.lay_out_container_out_of_flow(
      (self.node, self.fragment_index()),
      &self.style,
      size,
      self.space,
      out_of_flow,
      #[cfg(feature = "fragmentation")]
      &mut self.builder,
    );

    if self.space.cache_slot == CacheSlot::Layout {
      let fragment_index = self.fragment_index();

      for child in children {
        self.tree.place_child(
          child.node,
          child.result.fragment_index(),
          fragment_index,
          child.location,
          &child.result,
        );
      }
    }

    result.intrinsic_block_size = self.fragment_intrinsic_block_size(intrinsic_block_size);
    result.bfc_line_offset = self.space.bfc_offset.line_offset;
    result.bfc_block_offset = Some(self.space.bfc_offset.block_offset);
    result.first_baseline = baselines.first_baseline();
    result.last_baseline = baselines.last_baseline();
    result.out_of_flow = out_of_flow;
    self.finish_break_info(&mut result);
    result
  }

  /// Blink's `ComputeMinMaxSizes`, a subgrid's the columns its root grid sized.
  pub(crate) fn compute_min_max_sizes(mut self) -> MinMaxSizesResult {
    if let Some(layout_subtree) = &self.space.grid_layout_subtree {
      let size = layout_subtree
        .layout_data()
        .columns
        .total_set_span_size()
        .unwrap_or_default();

      return MinMaxSizesResult::new(
        MinMaxSizes::both(size) + self.border_padding.inline_sum(),
        false,
      );
    }

    // An inline-size-contained grid sizes its columns as if it had no items.
    let mut sizing_tree = if self.style.contains_inline_size() {
      self.build_grid_sizing_tree_ignoring_children()
    } else {
      self.build_grid_sizing_tree().0
    };
    let mut depends_on_block_constraints = false;
    let mut compute_total_column_size = |this: &mut Self, sizing_constraint| {
      let mut needs_additional_pass = false;

      this.initialize_track_sizes(&mut sizing_tree, 0, None, None);
      this.complete_track_sizing_algorithm_for_tree(
        &mut sizing_tree,
        ForColumns,
        sizing_constraint,
        Some(&mut needs_additional_pass),
      );

      let needs_additional_pass_for_column_subtree = sizing_tree
        .has_subgrid_with_indefinite_standalone_axis()
        && sizing_tree.has_block_size_dependent_grid_item();

      if needs_additional_pass || sizing_tree.has_block_size_dependent_grid_item() {
        depends_on_block_constraints = true;
        this.complete_track_sizing_algorithm_for_tree(
          &mut sizing_tree,
          ForRows,
          sizing_constraint,
          Some(&mut needs_additional_pass),
        );

        if needs_additional_pass || needs_additional_pass_for_column_subtree {
          this.initialize_track_sizes(&mut sizing_tree, 0, None, Some(ForColumns));
          this.complete_track_sizing_algorithm_for_tree(
            &mut sizing_tree,
            ForColumns,
            sizing_constraint,
            None,
          );
        }
      }

      sizing_tree
        .node(0)
        .layout_data
        .columns
        .total_set_span_size()
        .unwrap_or_default()
    };
    let mut sizes = MinMaxSizes {
      min_size: LayoutUnit::ZERO,
      max_size: compute_total_column_size(&mut self, SizingConstraint::MaxContent),
    };

    sizes.encompass(compute_total_column_size(
      &mut self,
      SizingConstraint::MinContent,
    ));

    MinMaxSizesResult::new(
      sizes + self.border_padding.inline_sum(),
      depends_on_block_constraints,
    )
  }
}

/// The block fragmentation hooks without the `fragmentation` feature, where nothing breaks.
#[cfg(not(feature = "fragmentation"))]
impl GridLayoutAlgorithm<'_, '_> {
  fn is_resuming(&self) -> bool {
    false
  }

  fn fragment_index(&self) -> usize {
    0
  }

  fn border_padding_for_size(&self, border_padding: LogicalBoxStrut) -> LogicalBoxStrut {
    border_padding
  }

  fn previously_consumed_block_size(&self) -> LayoutUnit {
    LayoutUnit::ZERO
  }

  fn fragmentainer_capacity_for_children(&self) -> LayoutUnit {
    LayoutUnit::MAX
  }

  fn should_process_out_of_flow_block_end(&self) -> bool {
    true
  }

  fn resumed_grid_state(&mut self) -> Option<(GridLayoutState, Vec<NodeId>, LayoutUnit)> {
    None
  }

  fn reset_row_break_between(&mut self, _set_count: usize) {}

  fn join_row_break_between(&mut self, _item: &GridItemData, _result: &LayoutResult) {}

  fn keeps_placement(
    &mut self,
    _item: &GridItemData,
    _offset: LogicalOffset,
    _relative_offset: Point<LayoutUnit>,
  ) -> bool {
    false
  }

  fn place_grid_items_for_fragmentation(
    &mut self,
    _grid_state: &mut GridLayoutState,
    _intrinsic_block_size: &mut LayoutUnit,
    unfragmented: (Vec<PlacedChild>, GridBaselineAccumulator),
  ) -> (Vec<PlacedChild>, GridBaselineAccumulator) {
    unfragmented
  }

  fn finish_fragmentation(&mut self, block_size: LayoutUnit) -> Result<LayoutUnit, LayoutStatus> {
    Ok(block_size)
  }

  fn relayout(self, status: LayoutStatus) -> LayoutResult {
    let mut result = LayoutResult::new(LogicalSize::default());

    result.status = status;
    result
  }

  fn fragment_intrinsic_block_size(&self, intrinsic_block_size: LayoutUnit) -> LayoutUnit {
    intrinsic_block_size
  }

  fn keep_break_token_data(
    &mut self,
    _grid_state: GridLayoutState,
    _intrinsic_block_size: LayoutUnit,
    _out_of_flow_children: Vec<NodeId>,
  ) {
  }

  fn resumed_out_of_flow(&self) -> Vec<OutOfFlowCandidate> {
    Vec::new()
  }

  fn finish_break_info(&mut self, _result: &mut LayoutResult) {}
}

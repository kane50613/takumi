//! Blink's [`GridLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_layout_algorithm.cc)
//! and the [`grid_layout_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_layout_utils.cc)
//! it runs, in a horizontal writing mode, without subgrid, `grid-lanes`, fragmentation, gap
//! decorations, `reading-flow`, scrollbars, size containment, or `last baseline`. Every item lies
//! parallel to its grid.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::{NodeId, Point, Size},
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    grid::{
      grid_data::GridLayoutData,
      grid_item::{AxisEdge, GridItemData, GridItemIndices},
      grid_line_resolver::GridLineResolver,
      grid_placement::{GridPlacement, GridPlacementData},
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
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult},
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
  column_auto_repetitions: usize,
  row_auto_repetitions: usize,
  column_start_offset: usize,
  row_start_offset: usize,
  direction: Direction,
  inline_size: LayoutUnit,
}

/// Blink's `GridSizingTree` for a grid without subgrids: its items and tracks.
struct GridSizingTree {
  grid_items: Vec<GridItemData>,
  layout_data: GridLayoutData,
  column_auto_repetitions: usize,
  row_auto_repetitions: usize,
  column_start_offset: usize,
  row_start_offset: usize,
}

impl GridSizingTree {
  /// Blink's `HasBlockSizeDependentGridItem`.
  fn has_block_size_dependent_item(&self) -> bool {
    self
      .grid_items
      .iter()
      .any(|item| item.is_sizing_dependent_on_block_size)
  }

  fn geometry(&self, direction: Direction, inline_size: LayoutUnit) -> GridGeometry {
    GridGeometry {
      direction,
      inline_size,
      layout_data: self.layout_data.clone(),
      column_auto_repetitions: self.column_auto_repetitions,
      row_auto_repetitions: self.row_auto_repetitions,
      column_start_offset: self.column_start_offset,
      row_start_offset: self.row_start_offset,
    }
  }
}

/// Blink's `BlockSizeDependentGridItem`.
struct BlockSizeDependentGridItem {
  row_set_indices: (usize, usize),
  cached_block_size: Option<LayoutUnit>,
}

/// What measuring an item's contribution to the tracks of `direction` reads.
struct ItemMeasure<'a> {
  layout_data: &'a GridLayoutData,
  space: &'a ConstraintSpace,
  margins: LogicalBoxStrut,
  direction: GridTrackSizingDirection,
}

impl ItemMeasure<'_> {
  /// Blink's `CalculateBaselineShim`: the space before an item's `baseline` that lines it up with
  /// its track's.
  fn baseline_shim(&self, item: &GridItemData, baseline: LayoutUnit) -> LayoutUnit {
    self
      .layout_data
      .major_baseline(self.direction, item.set_indices(self.direction).0)
      .map_or(LayoutUnit::ZERO, |track_baseline| {
        track_baseline - baseline - baseline_margin(self.margins, self.direction)
      })
  }
}

/// A laid-out item waiting to be placed.
struct PlacedChild {
  node: NodeId,
  location: Point<LayoutUnit>,
  result: Rc<LayoutResult>,
}

/// Blink's `GridBaselineAccumulator` over major baselines.
#[derive(Default)]
struct GridBaselineAccumulator {
  first_fallback_baseline: Option<(usize, usize, LayoutUnit)>,
  last_fallback_baseline: Option<(usize, usize, LayoutUnit)>,
  first_major_baseline: Option<(usize, LayoutUnit)>,
  last_major_baseline: Option<(usize, LayoutUnit)>,
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
      if let Some(baseline) = layout_data.major_baseline(ForRows, index) {
        let baseline = rows.set_offset(index) + baseline;

        if self.first_major_baseline.is_none() {
          self.first_major_baseline = Some((index, baseline));
        }
        self.last_major_baseline = Some((index, baseline));
      }
    }
  }

  fn first_baseline(&self) -> Option<LayoutUnit> {
    match self.first_major_baseline {
      Some((index, baseline)) if Some(index) == self.first_set_index => Some(baseline),
      _ => self
        .first_fallback_baseline
        .map(|(_, _, baseline)| baseline),
    }
  }

  fn last_baseline(&self) -> Option<LayoutUnit> {
    match self.last_major_baseline {
      Some((index, baseline)) if Some(index) == self.last_set_index => Some(baseline),
      _ => self.last_fallback_baseline.map(|(_, _, baseline)| baseline),
    }
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

/// Blink's `GetSynthesizedLogicalBaseline` for an item `block_size` long in `direction`'s baseline
/// writing mode: the alphabetic baseline at the line-under edge, which `vertical-lr` puts at the
/// block start.
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
  placement_data: &GridPlacementData<'_>,
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
  /// The grid area an out-of-flow `node` of the grid `container` resolves its insets against, or
  /// `containing_block`, its padding box, when `container` is not a grid.
  pub(crate) fn grid_area(
    &self,
    container: NodeId,
    node: NodeId,
    containing_block: ContainingBlock,
  ) -> ContainingBlock {
    let Some(geometry) = self.node_state(container).grid_geometry.as_deref() else {
      return containing_block;
    };
    let container_style = self.layout_style(container);
    let Some(grid_style) = container_style.grid.as_deref() else {
      return containing_block;
    };
    let (direction, inline_size) = (geometry.direction, geometry.inline_size);
    let placement_data = GridPlacementData {
      line_resolver: GridLineResolver::new(
        grid_style,
        geometry.column_auto_repetitions,
        geometry.row_auto_repetitions,
      ),
      grid_item_positions: Vec::new(),
      column_start_offset: geometry.column_start_offset,
      row_start_offset: geometry.row_start_offset,
    };
    let mut item = GridItemData::new(
      node,
      Rc::clone(self.layout_style_rc(node)),
      container_style,
      self.is_replaced(node),
    );
    let rect = compute_out_of_flow_item_containing_rect(
      &placement_data,
      &geometry.layout_data,
      logical_rect(
        containing_block.origin,
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
      origin: Point::new(x, rect.offset.block_offset),
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
}

impl<'t, 'r> GridLayoutAlgorithm<'t, 'r> {
  /// The algorithm for `node` in `space`, with an indefinite inline size when `is_intrinsic`.
  pub(crate) fn new(
    tree: &'t mut LayoutTree<'r>,
    node: NodeId,
    space: &'t ConstraintSpace,
    is_intrinsic: bool,
  ) -> Self {
    let style = Rc::clone(tree.layout_style_rc(node));
    let grid_style = style.grid.clone().unwrap_or_default();
    let resolver = LengthResolver::new(space, &style);
    let border_padding = resolver.border_padding;
    let inline_size = (!is_intrinsic).then(|| {
      resolver.inline_size_for_fragment(&mut |size_type| tree.min_max_sizes(node, space, size_type))
    });
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
    }
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

  /// Blink's `ComputeAutomaticRepetitions`.
  fn compute_automatic_repetitions(&self, direction: GridTrackSizingDirection) -> usize {
    let track_list = &self.grid_style.template_tracks(direction).track_list;

    if !track_list.has_auto_repeater() {
      return 0;
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
      calculate_gutter_size(&self.style, self.grid_available_size, direction),
      self.available_size(direction),
      min_available_size,
      max_available_size,
    )
  }

  /// Blink's `BuildGridSizingTree`: the in-flow items placed in the grid, their tracks, and the
  /// out-of-flow children.
  fn build_grid_sizing_tree(&mut self) -> (GridSizingTree, Vec<NodeId>) {
    let column_auto_repetitions = self.compute_automatic_repetitions(ForColumns);
    let row_auto_repetitions = self.compute_automatic_repetitions(ForRows);
    let mut grid_items = Vec::new();
    let mut out_of_flow_children = Vec::new();

    // Blink's `GridNode::ConstructGridItems`.
    for child in self.tree.child_nodes(self.node) {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));

      if child_style.display == Display::None {
        if self.space.cache_slot == CacheSlot::Layout {
          self.tree.hide(child);
        }
        continue;
      }
      if child_style.is_out_of_flow() {
        out_of_flow_children.push(child);
        continue;
      }

      let is_replaced = self.tree.is_replaced(child);

      grid_items.push(GridItemData::new(
        child,
        child_style,
        &self.style,
        is_replaced,
      ));
    }

    let grid_style = Rc::clone(&self.grid_style);
    let line_resolver =
      GridLineResolver::new(&grid_style, column_auto_repetitions, row_auto_repetitions);
    let item_styles: Vec<&LayoutStyle> = grid_items.iter().map(|item| &*item.style).collect();
    let placement_data =
      GridPlacement::new(&grid_style, line_resolver).run_auto_placement_algorithm(&item_styles);

    for (item, position) in grid_items
      .iter_mut()
      .zip(&placement_data.grid_item_positions)
    {
      item.resolved_position = *position;
    }

    let column_start_offset = placement_data.column_start_offset;
    let row_start_offset = placement_data.row_start_offset;
    let build_sizing_collection =
      |direction, auto_repetitions, start_offset, items: &mut [GridItemData]| {
        // Blink's `BuildSizingCollection`.
        let mut range_builder =
          GridRangeBuilder::new(&grid_style, direction, auto_repetitions, start_offset);
        let mut must_create_baselines = false;

        for (index, item) in items.iter().enumerate() {
          must_create_baselines |= item.is_baseline_specified(direction);
          range_builder.ensure_track_coverage(
            item.start_line(direction),
            item.span_size(direction),
            index,
          );
        }

        let (ranges, item_ranges) = range_builder.finalize_ranges(items.len());

        for (item, (begin, end)) in items.iter_mut().zip(item_ranges) {
          *item.range_indices_mut(direction) = GridItemIndices {
            begin: Some(begin),
            end: Some(end),
          };
        }
        (
          GridTrackCollection::new(ranges, direction),
          must_create_baselines,
        )
      };
    let (columns, column_baselines) = build_sizing_collection(
      ForColumns,
      column_auto_repetitions,
      column_start_offset,
      &mut grid_items,
    );
    let (rows, row_baselines) = build_sizing_collection(
      ForRows,
      row_auto_repetitions,
      row_start_offset,
      &mut grid_items,
    );
    let mut layout_data = GridLayoutData::new(columns, rows);

    if column_baselines {
      layout_data.create_baselines(ForColumns);
    }
    if row_baselines {
      layout_data.create_baselines(ForRows);
    }

    let sizing_tree = GridSizingTree {
      grid_items,
      layout_data,
      column_auto_repetitions,
      row_auto_repetitions,
      column_start_offset,
      row_start_offset,
    };

    (sizing_tree, out_of_flow_children)
  }

  /// Blink's `InitializeTrackSizes`, for `direction` or both.
  fn initialize_track_sizes(
    &self,
    sizing_tree: &mut GridSizingTree,
    direction: Option<GridTrackSizingDirection>,
  ) {
    let directions = match direction {
      Some(direction) => vec![direction],
      None => vec![ForColumns, ForRows],
    };

    for direction in directions {
      let available_size = self.available_size(direction);
      let gutter_size = calculate_gutter_size(&self.style, self.grid_available_size, direction);
      let track_collection = sizing_tree.layout_data.collection_mut(direction);

      track_collection.build_sets(&self.grid_style, available_size, gutter_size);
      GridTrackSizingAlgorithm::cache_grid_items_properties(
        track_collection,
        &mut sizing_tree.grid_items,
      );

      if track_collection.has_non_definite_track() {
        track_collection.cache_initialized_sets_geometry(self.border_padding_start(direction));
      } else {
        self.finalize_sets_geometry(track_collection);
      }

      let set_count = track_collection.set_count();

      if sizing_tree.layout_data.has_baselines(direction) {
        sizing_tree
          .layout_data
          .reset_baselines(direction, set_count);
      }
    }
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
  /// its sizes fixed to those of `fixed_available_size` that are definite.
  fn create_constraint_space(
    &self,
    cache_slot: CacheSlot,
    item: &GridItemData,
    containing_grid_area_size: AvailableLogicalSize,
    fixed_available_size: AvailableLogicalSize,
  ) -> ConstraintSpace {
    let mut space =
      ConstraintSpace::new(containing_grid_area_size.inline_size, item.style.direction);

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
    space
  }

  /// Blink's `CreateConstraintSpaceForLayout`: the space and the item's grid area.
  fn space_for_layout(
    &self,
    item: &GridItemData,
    layout_data: &GridLayoutData,
  ) -> (ConstraintSpace, LogicalRect) {
    let (inline_size, inline_offset) = item.calculate_available_size(&layout_data.columns);
    let (block_size, block_offset) = item.calculate_available_size(&layout_data.rows);
    let containing_grid_area = LogicalRect::new(
      inline_offset,
      block_offset,
      inline_size.unwrap_or_default(),
      block_size.unwrap_or_default(),
    );
    let space = self.create_constraint_space(
      self.space.cache_slot,
      item,
      AvailableLogicalSize {
        inline_size,
        block_size,
      },
      AvailableLogicalSize::default(),
    );

    (space, containing_grid_area)
  }

  /// Blink's `CreateConstraintSpaceForMeasure`: a space with the grid area's size in the axis
  /// other than `direction`.
  fn space_for_measure(
    &self,
    item: &GridItemData,
    layout_data: &GridLayoutData,
    direction: GridTrackSizingDirection,
    fixed_inline_size: Option<LayoutUnit>,
  ) -> ConstraintSpace {
    let containing_grid_area_size = match direction {
      ForColumns => AvailableLogicalSize {
        inline_size: None,
        block_size: item.calculate_available_size(&layout_data.rows).0,
      },
      ForRows => AvailableLogicalSize {
        inline_size: item.calculate_available_size(&layout_data.columns).0,
        block_size: None,
      },
    };

    self.create_constraint_space(
      CacheSlot::Measure,
      item,
      containing_grid_area_size,
      AvailableLogicalSize {
        inline_size: fixed_inline_size,
        block_size: None,
      },
    )
  }

  /// Blink's `ContributionSizeForGridItem`.
  fn contribution_size_for_grid_item(
    &mut self,
    layout_data: &GridLayoutData,
    contribution_type: GridItemContributionType,
    direction: GridTrackSizingDirection,
    item: &mut GridItemData,
  ) -> LayoutUnit {
    let item_style = Rc::clone(&item.style);
    let space = self.space_for_measure(item, layout_data, direction, None);
    let margins = margins_for(
      &item_style,
      space.percentage_resolution_inline_size,
      self.direction(),
    );
    let measure = ItemMeasure {
      layout_data,
      space: &space,
      margins,
      direction,
    };
    let is_for_columns = direction == ForColumns;
    let margin_sum = if is_for_columns {
      margins.inline_sum()
    } else {
      margins.block_sum()
    };
    let contribution = match contribution_type {
      GridItemContributionType::ContentBasedMinimums
      | GridItemContributionType::IntrinsicMaximums => {
        self.content_contribution(&measure, item, true).0
      }
      GridItemContributionType::MaxContentMinimums
      | GridItemContributionType::MaxContentMaximums => {
        self.content_contribution(&measure, item, false).0
      }
      GridItemContributionType::IntrinsicMinimums => {
        self.intrinsic_minimum_contribution(&measure, item, margin_sum)
      }
      GridItemContributionType::FreeSpace => LayoutUnit::ZERO,
    };

    (contribution + margin_sum).clamp_negative_to_zero()
  }

  /// Blink's `CalculateIntrinsicMinimumContribution`, clamped by the definite tracks it spans as
  /// `ContributionSizeForGridItem` does.
  fn intrinsic_minimum_contribution(
    &mut self,
    measure: &ItemMeasure<'_>,
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
      LayoutLength::MaxContent => return self.content_contribution(measure, item, false).0,
      LayoutLength::MinContent | LayoutLength::Fixed(_) | LayoutLength::MinIntrinsic => {
        return self.content_contribution(measure, item, true).0;
      }
      LayoutLength::Auto
      | LayoutLength::FitContent
      | LayoutLength::Stretch
      | LayoutLength::Percent(_)
      | LayoutLength::Calc { .. } => {}
    }

    let resolver = LengthResolver::new(measure.space, &item_style);

    if !min_length.is_auto() || !overflow.is_clip_or_visible() || special_spanning_criteria {
      if !is_for_columns {
        return resolver.resolve_initial_min_block_length(min_length, None);
      }

      let tree = &mut *self.tree;

      return resolver.resolve_min_inline_length(
        &mut |size_type| tree.min_max_sizes(item.node, measure.space, size_type),
        min_length,
        None,
        None,
        FitContentMode::Normal,
      );
    }

    let (contribution, baseline_shim) = self.content_contribution(measure, item, true);
    let (begin_set_index, end_set_index) = item.set_indices(direction);
    let Some(spanned_tracks_definite_max_size) = measure
      .layout_data
      .collection(direction)
      .calculate_set_span_size(begin_set_index, end_set_index)
    else {
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
  /// row: the contribution and the baseline shim in it.
  fn content_contribution(
    &mut self,
    measure: &ItemMeasure<'_>,
    item: &mut GridItemData,
    is_min_content: bool,
  ) -> (LayoutUnit, LayoutUnit) {
    let direction = measure.direction;

    if direction == ForColumns {
      return self.min_or_max_content_size(measure, item, is_min_content);
    }

    let result = if measure.space.available_inline_size.is_none() {
      let (max_content_size, _) = self.min_or_max_content_size(measure, item, false);
      let fallback_space =
        self.space_for_measure(item, measure.layout_data, direction, Some(max_content_size));

      self.tree.layout(item.node, &fallback_space)
    } else {
      self.tree.layout(item.node, measure.space)
    };
    let baseline_shim = if item.is_baseline_aligned(direction) {
      measure.baseline_shim(item, first_baseline_or_synthesize(item, &result))
    } else {
      LayoutUnit::ZERO
    };

    (result.size.block_size + baseline_shim, baseline_shim)
  }

  /// Blink's `MinOrMaxContentSize`.
  fn min_or_max_content_size(
    &mut self,
    measure: &ItemMeasure<'_>,
    item: &mut GridItemData,
    is_min_content: bool,
  ) -> (LayoutUnit, LayoutUnit) {
    let result = self
      .tree
      .min_and_max_content_contribution(item.node, measure.space);

    if result.depends_on_block_constraints {
      item.is_sizing_dependent_on_block_size = true;
    }

    let content_size = if is_min_content {
      result.sizes.min_size
    } else {
      result.sizes.max_size
    };
    let baseline_shim = if item.is_baseline_aligned(measure.direction) {
      measure.baseline_shim(
        item,
        synthesized_baseline(measure.direction, self.direction(), content_size),
      )
    } else {
      LayoutUnit::ZERO
    };

    (content_size + baseline_shim, baseline_shim)
  }

  /// Blink's `ComputeUsedTrackSizes`.
  fn compute_used_track_sizes(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    direction: GridTrackSizingDirection,
    sizing_constraint: SizingConstraint,
  ) {
    let available_size = self.available_size(direction);
    let gutter_size = calculate_gutter_size(&self.style, self.grid_available_size, direction);

    sizing_tree
      .layout_data
      .collection_mut(direction)
      .build_sets(&self.grid_style, available_size, gutter_size);

    // The contributions read the geometry the tracks had before sizing.
    let mut track_collection = sizing_tree.layout_data.collection(direction).clone();
    let algorithm = GridTrackSizingAlgorithm::new(
      &self.style,
      self.grid_available_size,
      self.grid_min_available_size,
      sizing_constraint,
    );
    let layout_data = &sizing_tree.layout_data;

    algorithm.compute_used_track_sizes(
      &mut |contribution_type, item| {
        self.contribution_size_for_grid_item(layout_data, contribution_type, direction, item)
      },
      &mut track_collection,
      &mut sizing_tree.grid_items,
    );

    *sizing_tree.layout_data.collection_mut(direction) = track_collection;
  }

  /// Blink's `CompleteTrackSizingAlgorithm`: the tracks of `direction` sized and placed, and
  /// whether the rows' sizes change the items' contributions to the columns in
  /// `needs_additional_pass`.
  fn complete_track_sizing_algorithm(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    direction: GridTrackSizingDirection,
    sizing_constraint: SizingConstraint,
    needs_additional_pass: Option<&mut bool>,
  ) {
    if sizing_tree.layout_data.has_any_baselines() {
      self.compute_grid_item_baselines(sizing_tree, direction, true);
    }
    if !sizing_tree
      .layout_data
      .collection(direction)
      .has_non_definite_track()
    {
      return;
    }

    self.compute_used_track_sizes(sizing_tree, direction, sizing_constraint);

    let needs_additional_pass =
      needs_additional_pass.filter(|needs| direction == ForRows && !**needs);
    let block_size_dependent_items: Vec<BlockSizeDependentGridItem> =
      if needs_additional_pass.is_some() {
        sizing_tree
          .grid_items
          .iter()
          .filter(|item| item.is_sizing_dependent_on_block_size)
          .map(|item| {
            let row_set_indices = item.set_indices(ForRows);

            BlockSizeDependentGridItem {
              row_set_indices,
              cached_block_size: sizing_tree
                .layout_data
                .rows
                .calculate_set_span_size(row_set_indices.0, row_set_indices.1),
            }
          })
          .collect()
      } else {
        Vec::new()
      };
    let track_collection = sizing_tree.layout_data.collection_mut(direction);

    self.finalize_sets_geometry(track_collection);

    // Blink's `MayChangeBlockSizeDependentGridItemContributions`.
    if let Some(needs_additional_pass) = needs_additional_pass {
      *needs_additional_pass = block_size_dependent_items.iter().any(|item| {
        sizing_tree
          .layout_data
          .rows
          .calculate_set_span_size(item.row_set_indices.0, item.row_set_indices.1)
          != item.cached_block_size
      });
    }
  }

  /// Blink's `ComputeGridItemBaselines`: each set's baseline in `direction`, measured for track
  /// sizing or from the final layout.
  fn compute_grid_item_baselines(
    &mut self,
    sizing_tree: &mut GridSizingTree,
    direction: GridTrackSizingDirection,
    is_for_track_sizing: bool,
  ) {
    if !sizing_tree.layout_data.has_baselines(direction) {
      return;
    }

    let set_count = sizing_tree.layout_data.collection(direction).set_count();

    sizing_tree
      .layout_data
      .reset_baselines(direction, set_count);

    for index in 0..sizing_tree.grid_items.len() {
      let item = &sizing_tree.grid_items[index];

      if !item.is_baseline_specified(direction) {
        continue;
      }

      let space = if is_for_track_sizing {
        self.space_for_measure(item, &sizing_tree.layout_data, direction, None)
      } else {
        self.space_for_layout(item, &sizing_tree.layout_data).0
      };
      let node = item.node;
      let result = self.tree.layout(node, &space);
      let item = &mut sizing_tree.grid_items[index];
      // Blink's `MeasureAndStoreItemBaseline`. A column baseline is always synthesized.
      let has_synthesized_baseline = direction == ForColumns || result.first_baseline.is_none();

      item.set_alignment_fallback(direction, has_synthesized_baseline);
      if !item.is_baseline_aligned(direction) {
        continue;
      }

      let margins = margins_for(
        &item.style,
        space.percentage_resolution_inline_size,
        self.direction(),
      );
      let baseline = baseline_margin(margins, direction)
        + item_baseline(item, direction, self.direction(), &result);
      let begin_set_index = item.set_indices(direction).0;

      sizing_tree
        .layout_data
        .set_major_baseline(direction, begin_set_index, baseline);
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

  /// Blink's `ComputeGridGeometry`: the sized grid, the out-of-flow children, and the intrinsic
  /// block size.
  fn compute_grid_geometry(&mut self) -> (GridSizingTree, Vec<NodeId>, LayoutUnit) {
    let (mut sizing_tree, out_of_flow_children) = self.build_grid_sizing_tree();
    let mut needs_additional_pass = false;

    self.initialize_track_sizes(&mut sizing_tree, None);
    self.complete_track_sizing_algorithm(
      &mut sizing_tree,
      ForColumns,
      SizingConstraint::Layout,
      Some(&mut needs_additional_pass),
    );
    self.complete_track_sizing_algorithm(
      &mut sizing_tree,
      ForRows,
      SizingConstraint::Layout,
      Some(&mut needs_additional_pass),
    );

    let intrinsic_block_size = sizing_tree
      .layout_data
      .rows
      .total_set_span_size()
      .unwrap_or_default()
      + self.border_padding.block_sum();
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
      needs_additional_pass |= self.needs_additional_layout_pass(&sizing_tree.layout_data.rows);

      if !needs_additional_pass && self.style.align_content != Default::default() {
        let rows = &mut sizing_tree.layout_data.rows;

        self.finalize_sets_geometry(rows);
      }
    }

    if needs_additional_pass {
      self.initialize_track_sizes(&mut sizing_tree, Some(ForColumns));
      self.complete_track_sizing_algorithm(
        &mut sizing_tree,
        ForColumns,
        SizingConstraint::Layout,
        None,
      );
      self.initialize_track_sizes(&mut sizing_tree, Some(ForRows));
      self.complete_track_sizing_algorithm(
        &mut sizing_tree,
        ForRows,
        SizingConstraint::Layout,
        None,
      );
    }

    // Blink's `CompleteFinalBaselineAlignment`.
    if sizing_tree.layout_data.has_any_baselines() {
      self.compute_grid_item_baselines(&mut sizing_tree, ForColumns, false);
      self.compute_grid_item_baselines(&mut sizing_tree, ForRows, false);
    }

    (sizing_tree, out_of_flow_children, intrinsic_block_size)
  }

  /// Blink's `PlaceGridItems`: the items laid out in their grid areas, and the grid's baselines.
  fn place_grid_items(
    &mut self,
    sizing_tree: &GridSizingTree,
  ) -> (Vec<PlacedChild>, GridBaselineAccumulator) {
    let layout_data = &sizing_tree.layout_data;
    let mut baseline_accumulator = GridBaselineAccumulator::default();
    let mut children = Vec::with_capacity(sizing_tree.grid_items.len());

    for item in &sizing_tree.grid_items {
      let (space, mut containing_grid_area) = self.space_for_layout(item, layout_data);
      let margins = margins_for(
        &item.style,
        space.percentage_resolution_inline_size,
        self.direction(),
      );
      let result = self.tree.layout(item.node, &space);
      let baseline_offset = |direction: GridTrackSizingDirection| {
        // Blink's `ComputeBaselineOffset`.
        if !item.is_baseline_aligned(direction) {
          return LayoutUnit::ZERO;
        }

        layout_data
          .major_baseline(direction, item.set_indices(direction).0)
          .unwrap_or_default()
          - item_baseline(item, direction, self.direction(), &result)
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
      );
      let location = self.physical_offset(containing_grid_area.offset, result.size.inline_size)
        + relative_offset.map(LayoutUnit::from_float);

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

  /// Blink's `PlaceOutOfFlowItems`: each out-of-flow child's static position, at the start,
  /// center, or end of its grid area when the grid is its containing block.
  fn place_out_of_flow_items(
    &self,
    sizing_tree: &GridSizingTree,
    block_size: LayoutUnit,
    out_of_flow_children: Vec<NodeId>,
  ) -> Vec<OutOfFlowCandidate> {
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
    let line_resolver = GridLineResolver::new(
      &self.grid_style,
      sizing_tree.column_auto_repetitions,
      sizing_tree.row_auto_repetitions,
    );
    let placement_data = GridPlacementData {
      line_resolver,
      grid_item_positions: Vec::new(),
      column_start_offset: sizing_tree.column_start_offset,
      row_start_offset: sizing_tree.row_start_offset,
    };

    out_of_flow_children
      .into_iter()
      .map(|child| {
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
              &placement_data,
              &sizing_tree.layout_data,
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

        OutOfFlowCandidate::new(
          child,
          static_position.to_physical(self.direction(), inline_size),
        )
      })
      .collect()
  }

  /// Blink's `Layout`.
  pub(crate) fn layout(mut self) -> LayoutResult {
    let (sizing_tree, out_of_flow_children, intrinsic_block_size) = self.compute_grid_geometry();
    let (children, baselines) = self.place_grid_items(&sizing_tree);
    let inline_size = self.inline_size.unwrap_or_default();
    let style = Rc::clone(&self.style);
    let block_size = LengthResolver::new(self.space, &style)
      .block_size_for_fragment(Some(intrinsic_block_size), Some(inline_size), None)
      .unwrap_or(intrinsic_block_size);
    let mut out_of_flow: Vec<OutOfFlowCandidate> = children
      .iter()
      .flat_map(|child| {
        child
          .result
          .out_of_flow
          .iter()
          .map(|candidate| candidate.moved_by(child.location))
      })
      .collect();

    out_of_flow.extend(self.place_out_of_flow_items(
      &sizing_tree,
      block_size,
      out_of_flow_children,
    ));

    self.tree.node_state_mut(self.node).grid_geometry =
      Some(Rc::new(sizing_tree.geometry(self.direction(), inline_size)));

    let size = LogicalSize::new(inline_size, block_size);
    let mut result = LayoutResult::of_box(size, &style, self.space);

    if !out_of_flow.is_empty() {
      let claims = self.tree.claims(self.node);
      let containing_block = ContainingBlock::padding_box(
        Size {
          width: size.inline_size,
          height: size.block_size,
        },
        style.border,
      );

      out_of_flow =
        self
          .tree
          .lay_out_out_of_flow(self.node, claims, out_of_flow, containing_block, self.space);
    }

    if self.space.cache_slot == CacheSlot::Layout {
      for child in children {
        self
          .tree
          .place_child(child.node, 0, 0, child.location, &child.result);
      }
    }

    result.intrinsic_block_size = intrinsic_block_size;
    result.bfc_line_offset = self.space.bfc_offset.line_offset;
    result.bfc_block_offset = Some(self.space.bfc_offset.block_offset);
    result.first_baseline = baselines.first_baseline();
    result.last_baseline = baselines.last_baseline();
    result.out_of_flow = out_of_flow;
    result
  }

  /// Blink's `ComputeMinMaxSizes`.
  pub(crate) fn compute_min_max_sizes(mut self) -> MinMaxSizesResult {
    let (mut sizing_tree, _) = self.build_grid_sizing_tree();
    let mut depends_on_block_constraints = false;
    let mut compute_total_column_size = |this: &mut Self, sizing_constraint| {
      let mut needs_additional_pass = false;

      this.initialize_track_sizes(&mut sizing_tree, None);
      this.complete_track_sizing_algorithm(
        &mut sizing_tree,
        ForColumns,
        sizing_constraint,
        Some(&mut needs_additional_pass),
      );

      if needs_additional_pass || sizing_tree.has_block_size_dependent_item() {
        depends_on_block_constraints = true;
        this.complete_track_sizing_algorithm(
          &mut sizing_tree,
          ForRows,
          sizing_constraint,
          Some(&mut needs_additional_pass),
        );

        if needs_additional_pass {
          this.initialize_track_sizes(&mut sizing_tree, Some(ForColumns));
          this.complete_track_sizing_algorithm(
            &mut sizing_tree,
            ForColumns,
            sizing_constraint,
            None,
          );
        }
      }

      sizing_tree
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

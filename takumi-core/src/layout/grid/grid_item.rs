//! Blink's [`GridItemData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_item.cc):
//! a grid item's area, alignment, and the tracks it spans, and the items a grid sizes. In a
//! horizontal writing mode, without `grid-lanes` or `last baseline`, so every subgrid is parallel
//! to its root grid.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{
  ops::{Deref, DerefMut},
  rc::Rc,
};

use crate::{
  geometry::NodeId,
  layout::{
    constraint_space::AutoSizeBehavior,
    grid::{
      grid_area::{GridArea, GridSpan},
      grid_placement::{GridPlacement, GridPlacementData},
      grid_style::GridTrackSizingDirection,
      grid_track_collection::{GridTrackCollection, TrackSpanProperties},
    },
    layout_style::LayoutStyle,
  },
  layout_unit::LayoutUnit,
  style::{AlignItems, Direction},
};

/// Blink's `AxisEdge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AxisEdge {
  Start,
  Center,
  End,
  FirstBaseline,
}

/// Blink's `BaselineGroup`: the baseline-sharing group an item aligns in, at its track's start or
/// end. <https://www.w3.org/TR/css-align-3/#baseline-sharing-group>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineGroup {
  Major,
  Minor,
}

/// The writing modes Blink's `DetermineBaselineWritingMode` reads a horizontal item's baselines in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaselineWritingMode {
  HorizontalTb,
  VerticalLr,
  VerticalRl,
}

/// Blink's `DetermineBaselineWritingMode` for a horizontal item of a horizontal grid laid out in
/// `container_direction`.
fn determine_baseline_writing_mode(
  container_direction: Direction,
  is_parallel_context: bool,
) -> BaselineWritingMode {
  if is_parallel_context {
    return BaselineWritingMode::HorizontalTb;
  }

  match container_direction {
    Direction::Ltr => BaselineWritingMode::VerticalLr,
    Direction::Rtl => BaselineWritingMode::VerticalRl,
  }
}

/// Blink's `DetermineBaselineGroup` for a first baseline in a horizontal grid laid out in
/// `container_direction`.
fn determine_baseline_group(
  container_direction: Direction,
  baseline_writing_mode: BaselineWritingMode,
  is_parallel_context: bool,
) -> BaselineGroup {
  let (start_group, end_group) = (BaselineGroup::Major, BaselineGroup::Minor);

  if is_parallel_context {
    return if baseline_writing_mode == BaselineWritingMode::HorizontalTb {
      start_group
    } else {
      end_group
    };
  }

  let is_ltr = container_direction == Direction::Ltr;

  match baseline_writing_mode {
    BaselineWritingMode::HorizontalTb | BaselineWritingMode::VerticalLr => {
      if is_ltr {
        start_group
      } else {
        end_group
      }
    }
    BaselineWritingMode::VerticalRl => {
      if is_ltr {
        end_group
      } else {
        start_group
      }
    }
  }
}

/// Blink's `GridItemIndices`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GridItemIndices {
  pub(crate) begin: Option<usize>,
  pub(crate) end: Option<usize>,
}

/// Blink's `OutOfFlowItemPlacement`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OutOfFlowItemPlacement {
  pub(crate) range_index: GridItemIndices,
  pub(crate) offset_in_range: GridItemIndices,
}

/// What an item's alignment in one axis resolves to, from Blink's `AxisEdgeFromItemPosition`.
struct AxisAlignment {
  edge: AxisEdge,
  auto_behavior: AutoSizeBehavior,
  is_overflow_safe: bool,
}

/// Blink's `AxisEdgeFromItemPosition`. A subgridded axis stretches to its area from the start.
fn axis_edge_from_item_position(
  direction: GridTrackSizingDirection,
  has_subgridded_axis: bool,
  is_replaced: bool,
  is_out_of_flow: bool,
  item_style: &LayoutStyle,
  grid_style: &LayoutStyle,
) -> AxisAlignment {
  if has_subgridded_axis {
    return AxisAlignment {
      edge: AxisEdge::Start,
      auto_behavior: AutoSizeBehavior::StretchImplicit,
      is_overflow_safe: true,
    };
  }

  let is_for_columns = direction == GridTrackSizingDirection::ForColumns;
  let (alignment, normal) = if is_for_columns {
    (item_style.justify_self, grid_style.justify_items)
  } else {
    (item_style.align_self, grid_style.align_items)
  };
  let alignment = if alignment == AlignItems::Normal {
    normal
  } else {
    alignment
  };
  let mut is_overflow_safe = matches!(
    alignment,
    AlignItems::SafeStart
      | AlignItems::SafeEnd
      | AlignItems::SafeFlexStart
      | AlignItems::SafeFlexEnd
      | AlignItems::SafeSelfStart
      | AlignItems::SafeSelfEnd
      | AlignItems::SafeCenter
  );
  let mut auto_behavior = AutoSizeBehavior::FitContent;
  let result = |edge, auto_behavior, is_overflow_safe| AxisAlignment {
    edge,
    auto_behavior,
    is_overflow_safe,
  };

  if !is_out_of_flow {
    let (start, end) = if is_for_columns {
      grid_style
        .direction
        .inline_sides(item_style.margin.left, item_style.margin.right)
    } else {
      (item_style.margin.top, item_style.margin.bottom)
    };
    let (is_start_auto, is_end_auto) = (start.is_auto(), end.is_auto());

    if is_start_auto || is_end_auto {
      is_overflow_safe = true;
    }
    match (is_start_auto, is_end_auto) {
      (true, true) => return result(AxisEdge::Center, auto_behavior, is_overflow_safe),
      (true, false) => return result(AxisEdge::End, auto_behavior, is_overflow_safe),
      (false, true) => return result(AxisEdge::Start, auto_behavior, is_overflow_safe),
      (false, false) => {}
    }
  }

  let edge = match alignment {
    AlignItems::SelfStart
    | AlignItems::SafeSelfStart
    | AlignItems::SelfEnd
    | AlignItems::SafeSelfEnd => {
      let is_start = matches!(alignment, AlignItems::SelfStart | AlignItems::SafeSelfStart);
      // An item's own block axis is its grid's; its inline axis may run the other way.
      let flips = is_for_columns && item_style.direction != grid_style.direction;

      if is_start != flips {
        AxisEdge::Start
      } else {
        AxisEdge::End
      }
    }
    AlignItems::Center | AlignItems::SafeCenter => AxisEdge::Center,
    AlignItems::Start
    | AlignItems::SafeStart
    | AlignItems::FlexStart
    | AlignItems::SafeFlexStart => AxisEdge::Start,
    AlignItems::End | AlignItems::SafeEnd | AlignItems::FlexEnd | AlignItems::SafeFlexEnd => {
      AxisEdge::End
    }
    AlignItems::Stretch => {
      auto_behavior = AutoSizeBehavior::StretchExplicit;
      AxisEdge::Start
    }
    AlignItems::Baseline => AxisEdge::FirstBaseline,
    AlignItems::Normal => {
      auto_behavior = if is_replaced {
        AutoSizeBehavior::FitContent
      } else {
        AutoSizeBehavior::StretchImplicit
      };
      AxisEdge::Start
    }
  };

  result(edge, auto_behavior, is_overflow_safe)
}

/// Blink's `GridItemData`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridItemData {
  pub(crate) node: NodeId,
  pub(crate) style: Rc<LayoutStyle>,
  pub(crate) resolved_position: GridArea,
  pub(crate) is_overflow_safe_for_columns: bool,
  pub(crate) is_overflow_safe_for_rows: bool,
  pub(crate) is_sizing_dependent_on_block_size: bool,
  pub(crate) column_alignment: AxisEdge,
  pub(crate) row_alignment: AxisEdge,
  pub(crate) column_fallback_alignment: Option<AxisEdge>,
  pub(crate) row_fallback_alignment: Option<AxisEdge>,
  pub(crate) column_auto_behavior: AutoSizeBehavior,
  pub(crate) row_auto_behavior: AutoSizeBehavior,
  pub(crate) column_span_properties: TrackSpanProperties,
  pub(crate) row_span_properties: TrackSpanProperties,
  pub(crate) column_set_indices: GridItemIndices,
  pub(crate) row_set_indices: GridItemIndices,
  pub(crate) column_range_indices: GridItemIndices,
  pub(crate) row_range_indices: GridItemIndices,
  pub(crate) column_placement: OutOfFlowItemPlacement,
  pub(crate) row_placement: OutOfFlowItemPlacement,
  pub(crate) has_subgridded_columns: bool,
  pub(crate) has_subgridded_rows: bool,
  pub(crate) is_considered_for_column_sizing: bool,
  pub(crate) is_considered_for_row_sizing: bool,
  pub(crate) is_opposite_direction_in_root_grid_columns: bool,
  pub(crate) is_subgridded_to_parent_grid: bool,
  pub(crate) must_consider_grid_items_for_column_sizing: bool,
  pub(crate) must_consider_grid_items_for_row_sizing: bool,
  pub(crate) column_baseline_group: BaselineGroup,
  pub(crate) row_baseline_group: BaselineGroup,
}

impl GridItemData {
  /// Blink's `GridItemData(node, parent_style)`: an item of the grid styled `grid_style` that no
  /// track sizing considers, as an out-of-flow box is.
  pub(crate) fn new(
    node: NodeId,
    style: Rc<LayoutStyle>,
    grid_style: &LayoutStyle,
    is_replaced: bool,
  ) -> Self {
    Self::new_in_grid(
      node,
      style,
      grid_style,
      grid_style,
      is_replaced,
      false,
      false,
    )
  }

  /// Blink's `GridItemData` constructor: an item of the grid styled `parent_grid_style`, whose
  /// tracks the root grid styled `root_grid_style` sizes in the axes the parent `must_consider`.
  pub(crate) fn new_in_grid(
    node: NodeId,
    style: Rc<LayoutStyle>,
    parent_grid_style: &LayoutStyle,
    root_grid_style: &LayoutStyle,
    is_replaced: bool,
    parent_must_consider_grid_items_for_column_sizing: bool,
    parent_must_consider_grid_items_for_row_sizing: bool,
  ) -> Self {
    let is_out_of_flow = style.is_out_of_flow();
    // A grid that is forced to establish an independent formatting context is not a subgrid.
    // <https://drafts.csswg.org/css-grid-2/#subgrid-listing>
    let subgrid_style = style
      .grid
      .as_deref()
      .filter(|_| style.display.is_grid() && !is_replaced && !style.applies_layout_containment());
    let has_subgridded_columns =
      subgrid_style.is_some_and(|grid| grid.template_columns.is_subgridded_axis());
    let has_subgridded_rows =
      subgrid_style.is_some_and(|grid| grid.template_rows.is_subgridded_axis());
    let columns = axis_edge_from_item_position(
      GridTrackSizingDirection::ForColumns,
      has_subgridded_columns,
      is_replaced,
      is_out_of_flow,
      &style,
      parent_grid_style,
    );
    let rows = axis_edge_from_item_position(
      GridTrackSizingDirection::ForRows,
      has_subgridded_rows,
      is_replaced,
      is_out_of_flow,
      &style,
      parent_grid_style,
    );
    // A subgrid acts as if it were empty in the axes it subgrids, its items sizing the tracks.
    // <https://drafts.csswg.org/css-grid-2/#subgrid-size-contribution>
    let (must_consider_grid_items_for_column_sizing, is_considered_for_column_sizing) =
      if parent_must_consider_grid_items_for_column_sizing {
        (has_subgridded_columns, !has_subgridded_columns)
      } else {
        (false, false)
      };
    let (must_consider_grid_items_for_row_sizing, is_considered_for_row_sizing) =
      if parent_must_consider_grid_items_for_row_sizing {
        (has_subgridded_rows, !has_subgridded_rows)
      } else {
        (false, false)
      };

    let root_direction = root_grid_style.direction;
    let baseline_group = |is_parallel_context| {
      determine_baseline_group(
        root_direction,
        determine_baseline_writing_mode(root_direction, is_parallel_context),
        is_parallel_context,
      )
    };

    Self {
      column_baseline_group: baseline_group(false),
      row_baseline_group: baseline_group(true),
      has_subgridded_columns,
      has_subgridded_rows,
      is_considered_for_column_sizing,
      is_considered_for_row_sizing,
      is_opposite_direction_in_root_grid_columns: style.direction != root_grid_style.direction,
      is_subgridded_to_parent_grid: false,
      must_consider_grid_items_for_column_sizing,
      must_consider_grid_items_for_row_sizing,
      node,
      style,
      resolved_position: GridArea::default(),
      is_overflow_safe_for_columns: columns.is_overflow_safe,
      is_overflow_safe_for_rows: rows.is_overflow_safe,
      is_sizing_dependent_on_block_size: false,
      column_alignment: columns.edge,
      row_alignment: rows.edge,
      column_fallback_alignment: None,
      row_fallback_alignment: None,
      column_auto_behavior: columns.auto_behavior,
      row_auto_behavior: rows.auto_behavior,
      column_span_properties: TrackSpanProperties::empty(),
      row_span_properties: TrackSpanProperties::empty(),
      column_set_indices: GridItemIndices::default(),
      row_set_indices: GridItemIndices::default(),
      column_range_indices: GridItemIndices::default(),
      row_range_indices: GridItemIndices::default(),
      column_placement: OutOfFlowItemPlacement::default(),
      row_placement: OutOfFlowItemPlacement::default(),
    }
  }

  /// Blink's `IsSubgrid`.
  pub(crate) fn is_subgrid(&self) -> bool {
    self.has_subgridded_columns || self.has_subgridded_rows
  }

  /// Blink's `IsConsideredForSizing`.
  pub(crate) fn is_considered_for_sizing(&self, direction: GridTrackSizingDirection) -> bool {
    match direction {
      GridTrackSizingDirection::ForColumns => self.is_considered_for_column_sizing,
      GridTrackSizingDirection::ForRows => self.is_considered_for_row_sizing,
    }
  }

  /// Blink's `IsOppositeDirectionInRootGrid`.
  pub(crate) fn is_opposite_direction_in_root_grid(
    &self,
    direction: GridTrackSizingDirection,
  ) -> bool {
    direction == GridTrackSizingDirection::ForColumns
      && self.is_opposite_direction_in_root_grid_columns
  }

  /// Blink's `MustConsiderGridItemsForSizing`.
  pub(crate) fn must_consider_grid_items_for_sizing(
    &self,
    direction: GridTrackSizingDirection,
  ) -> bool {
    match direction {
      GridTrackSizingDirection::ForColumns => self.must_consider_grid_items_for_column_sizing,
      GridTrackSizingDirection::ForRows => self.must_consider_grid_items_for_row_sizing,
    }
  }

  /// Blink's `MustCachePlacementIndices`.
  pub(crate) fn must_cache_placement_indices(&self, direction: GridTrackSizingDirection) -> bool {
    !self.is_subgridded_to_parent_grid
      || self.is_considered_for_sizing(direction)
      || self.must_consider_grid_items_for_sizing(direction)
  }

  /// Blink's `ResetPlacementIndices`.
  pub(crate) fn reset_placement_indices(&mut self) {
    self.column_range_indices = GridItemIndices::default();
    self.row_range_indices = GridItemIndices::default();
    self.column_set_indices = GridItemIndices::default();
    self.row_set_indices = GridItemIndices::default();
  }

  /// Blink's `AdjustSubgriddedItemSpan`: the span of an item of the subgrid `subgrid_item` in its
  /// parent's lines.
  pub(crate) fn adjust_subgridded_item_span(&mut self, subgrid_item: &GridItemData) {
    for direction in [
      GridTrackSizingDirection::ForColumns,
      GridTrackSizingDirection::ForRows,
    ] {
      if !subgrid_item.must_consider_grid_items_for_sizing(direction) {
        continue;
      }

      let span = self.resolved_position.span_mut(direction);

      if subgrid_item.is_opposite_direction_in_root_grid(direction) {
        let subgrid_span_size = subgrid_item.span_size(direction);

        *span = GridSpan::translated_definite(
          subgrid_span_size - span.end_line(),
          subgrid_span_size - span.start_line(),
        );
      }
      span.translate(subgrid_item.start_line(direction));
    }
  }

  pub(crate) fn alignment(&self, direction: GridTrackSizingDirection) -> AxisEdge {
    match direction {
      GridTrackSizingDirection::ForColumns => self
        .column_fallback_alignment
        .unwrap_or(self.column_alignment),
      GridTrackSizingDirection::ForRows => {
        self.row_fallback_alignment.unwrap_or(self.row_alignment)
      }
    }
  }

  pub(crate) fn is_overflow_safe(&self, direction: GridTrackSizingDirection) -> bool {
    match direction {
      GridTrackSizingDirection::ForColumns => {
        self.column_fallback_alignment.is_some() || self.is_overflow_safe_for_columns
      }
      GridTrackSizingDirection::ForRows => {
        self.row_fallback_alignment.is_some() || self.is_overflow_safe_for_rows
      }
    }
  }

  pub(crate) fn is_baseline_aligned(&self, direction: GridTrackSizingDirection) -> bool {
    self.alignment(direction) == AxisEdge::FirstBaseline
  }

  pub(crate) fn is_baseline_specified(&self, direction: GridTrackSizingDirection) -> bool {
    let alignment = match direction {
      GridTrackSizingDirection::ForColumns => self.column_alignment,
      GridTrackSizingDirection::ForRows => self.row_alignment,
    };

    alignment == AxisEdge::FirstBaseline
  }

  pub(crate) fn start_line(&self, direction: GridTrackSizingDirection) -> usize {
    self.resolved_position.start_line(direction)
  }

  pub(crate) fn end_line(&self, direction: GridTrackSizingDirection) -> usize {
    self.resolved_position.end_line(direction)
  }

  pub(crate) fn span_size(&self, direction: GridTrackSizingDirection) -> usize {
    self.resolved_position.span_size(direction)
  }

  pub(crate) fn set_indices(&self, direction: GridTrackSizingDirection) -> (usize, usize) {
    let indices = match direction {
      GridTrackSizingDirection::ForColumns => self.column_set_indices,
      GridTrackSizingDirection::ForRows => self.row_set_indices,
    };

    (indices.begin.unwrap_or(0), indices.end.unwrap_or(0))
  }

  pub(crate) fn range_indices(&self, direction: GridTrackSizingDirection) -> GridItemIndices {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_range_indices,
      GridTrackSizingDirection::ForRows => self.row_range_indices,
    }
  }

  pub(crate) fn range_indices_mut(
    &mut self,
    direction: GridTrackSizingDirection,
  ) -> &mut GridItemIndices {
    match direction {
      GridTrackSizingDirection::ForColumns => &mut self.column_range_indices,
      GridTrackSizingDirection::ForRows => &mut self.row_range_indices,
    }
  }

  pub(crate) fn track_span_properties(
    &self,
    direction: GridTrackSizingDirection,
  ) -> TrackSpanProperties {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_span_properties,
      GridTrackSizingDirection::ForRows => self.row_span_properties,
    }
  }

  pub(crate) fn set_track_span_property(
    &mut self,
    property: TrackSpanProperties,
    direction: GridTrackSizingDirection,
  ) {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_span_properties |= property,
      GridTrackSizingDirection::ForRows => self.row_span_properties |= property,
    }
  }

  pub(crate) fn is_spanning_flexible_track(&self, direction: GridTrackSizingDirection) -> bool {
    self
      .track_span_properties(direction)
      .contains(TrackSpanProperties::HAS_FLEXIBLE_TRACK)
  }

  pub(crate) fn is_spanning_intrinsic_track(&self, direction: GridTrackSizingDirection) -> bool {
    self
      .track_span_properties(direction)
      .contains(TrackSpanProperties::HAS_INTRINSIC_TRACK)
  }

  pub(crate) fn is_spanning_auto_minimum_track(&self, direction: GridTrackSizingDirection) -> bool {
    self
      .track_span_properties(direction)
      .contains(TrackSpanProperties::HAS_AUTO_MINIMUM_TRACK)
  }

  /// Blink's `BaselineGroup`.
  pub(crate) fn baseline_group(&self, direction: GridTrackSizingDirection) -> BaselineGroup {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_baseline_group,
      GridTrackSizingDirection::ForRows => self.row_baseline_group,
    }
  }

  /// Blink's `SetAlignmentFallback`: a baseline-aligned item that cannot take part in baseline
  /// alignment aligns to the edge of its baseline-sharing group.
  pub(crate) fn set_alignment_fallback(
    &mut self,
    direction: GridTrackSizingDirection,
    has_synthesized_baseline: bool,
  ) {
    if !self.is_baseline_specified(direction) {
      return;
    }

    let can_participate = !has_synthesized_baseline
      || !(self.is_spanning_intrinsic_track(direction)
        || self.is_spanning_flexible_track(direction))
      || {
        let style = &self.style;

        match direction {
          GridTrackSizingDirection::ForRows => {
            !style.size.height.has_percent_or_stretch()
              && !style.min_size.height.has_percent_or_stretch()
              && !style.max_size.height.has_percent_or_stretch()
          }
          GridTrackSizingDirection::ForColumns => {
            !style.size.width.has_percent_or_stretch()
              && !style.min_size.width.has_percent_or_stretch()
              && !style.max_size.width.has_percent_or_stretch()
          }
        }
      };
    let fallback = (!can_participate).then_some(match self.baseline_group(direction) {
      BaselineGroup::Major => AxisEdge::Start,
      BaselineGroup::Minor => AxisEdge::End,
    });

    match direction {
      GridTrackSizingDirection::ForColumns => self.column_fallback_alignment = fallback,
      GridTrackSizingDirection::ForRows => self.row_fallback_alignment = fallback,
    }
  }

  /// Blink's `ComputeSetIndices`.
  pub(crate) fn compute_set_indices(&mut self, track_collection: &GridTrackCollection) {
    let direction = track_collection.direction;
    let start_line = self.start_line(direction);
    let end_line = self.end_line(direction);
    let range_indices = self.range_indices_mut(direction);

    if range_indices.begin.is_none() {
      range_indices.begin = Some(track_collection.range_index_from_grid_line(start_line));
      range_indices.end = Some(track_collection.range_index_from_grid_line(end_line - 1));
    }

    let (begin, end) = (
      range_indices.begin.unwrap_or(0),
      range_indices.end.unwrap_or(0),
    );
    let set_indices = GridItemIndices {
      begin: Some(track_collection.ranges[begin].begin_set_index),
      end: Some(
        track_collection.ranges[end].begin_set_index + track_collection.ranges[end].set_count,
      ),
    };

    match direction {
      GridTrackSizingDirection::ForColumns => self.column_set_indices = set_indices,
      GridTrackSizingDirection::ForRows => self.row_set_indices = set_indices,
    }
  }

  /// Blink's `ComputeOutOfFlowItemPlacement`.
  pub(crate) fn compute_out_of_flow_item_placement(
    &mut self,
    track_collection: &GridTrackCollection,
    placement_data: &GridPlacementData,
  ) {
    let direction = track_collection.direction;
    let (mut start_line, mut end_line) = GridPlacement::resolve_out_of_flow_item_grid_lines(
      track_collection,
      &placement_data.line_resolver,
      &self.style,
      placement_data.start_offset(direction),
    );
    let range_count = track_collection.range_count();
    let mut placement = OutOfFlowItemPlacement::default();

    if let Some(line) = start_line.as_mut() {
      let range_index = if range_count == 0 {
        0
      } else if *line < track_collection.end_line_of_implicit_grid() {
        track_collection.range_index_from_grid_line(*line)
      } else {
        range_count - 1
      };

      if range_count > 0 {
        *line -= track_collection.ranges[range_index].start_line;
      }
      placement.range_index.begin = Some(range_index);
    }
    if let Some(line) = end_line.as_mut() {
      let range_index = if range_count == 0 || *line == 0 {
        0
      } else {
        track_collection.range_index_from_grid_line(*line - 1)
      };

      if range_count > 0 {
        *line -= track_collection.ranges[range_index].start_line;
      }
      placement.range_index.end = Some(range_index);
    }
    placement.offset_in_range = GridItemIndices {
      begin: start_line,
      end: end_line,
    };

    match direction {
      GridTrackSizingDirection::ForColumns => self.column_placement = placement,
      GridTrackSizingDirection::ForRows => self.row_placement = placement,
    }
  }

  /// Blink's `CalculateAvailableSize`: the size of the sets the item spans, none where one is
  /// indefinite, and the offset of the first.
  pub(crate) fn calculate_available_size(
    &self,
    track_collection: &GridTrackCollection,
  ) -> (Option<LayoutUnit>, LayoutUnit) {
    let (begin, end) = self.set_indices(track_collection.direction);

    (
      track_collection.calculate_set_span_size(begin, end),
      track_collection.set_offset(begin),
    )
  }
}

/// Blink's `GridItems`: a grid's items, then the items of its subgrids it sizes, which only
/// [`Self::include_subgridded_items`] reaches.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GridItems {
  item_data: Vec<GridItemData>,
  first_subgridded_item_index: usize,
}

impl GridItems {
  /// Blink's `Append`, a subgridded item after the grid's own.
  pub(crate) fn append(&mut self, item: GridItemData) {
    if !item.is_subgridded_to_parent_grid {
      self.first_subgridded_item_index += 1;
    }
    self.item_data.push(item);
  }

  /// Blink's `IncludeSubgriddedItems`.
  pub(crate) fn include_subgridded_items(&self) -> &[GridItemData] {
    &self.item_data
  }

  pub(crate) fn include_subgridded_items_mut(&mut self) -> &mut [GridItemData] {
    &mut self.item_data
  }
}

impl FromIterator<GridItemData> for GridItems {
  fn from_iter<T: IntoIterator<Item = GridItemData>>(iter: T) -> Self {
    let mut items = Self::default();

    for item in iter {
      items.append(item);
    }
    items
  }
}

impl Deref for GridItems {
  type Target = [GridItemData];

  fn deref(&self) -> &[GridItemData] {
    &self.item_data[..self.first_subgridded_item_index]
  }
}

impl DerefMut for GridItems {
  fn deref_mut(&mut self) -> &mut [GridItemData] {
    &mut self.item_data[..self.first_subgridded_item_index]
  }
}

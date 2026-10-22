//! Blink's [`GridItemData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_item.cc):
//! a grid item's area, alignment, and the tracks it spans. In a horizontal writing mode, without
//! subgrid, `grid-lanes`, or `last baseline`, so every item's baseline group is the major one.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::NodeId,
  layout::{
    constraint_space::AutoSizeBehavior,
    grid::{
      grid_area::GridArea,
      grid_placement::{GridPlacement, GridPlacementData},
      grid_style::GridTrackSizingDirection,
      grid_track_collection::{GridTrackCollection, TrackSpanProperties},
    },
    layout_style::LayoutStyle,
  },
  layout_unit::LayoutUnit,
  style::AlignItems,
};

/// Blink's `AxisEdge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AxisEdge {
  Start,
  Center,
  End,
  FirstBaseline,
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

/// Blink's `AxisEdgeFromItemPosition`.
fn axis_edge_from_item_position(
  direction: GridTrackSizingDirection,
  is_replaced: bool,
  is_out_of_flow: bool,
  item_style: &LayoutStyle,
  grid_style: &LayoutStyle,
) -> AxisAlignment {
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
#[derive(Debug, Clone)]
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
}

impl GridItemData {
  pub(crate) fn new(
    node: NodeId,
    style: Rc<LayoutStyle>,
    grid_style: &LayoutStyle,
    is_replaced: bool,
  ) -> Self {
    let is_out_of_flow = style.is_out_of_flow();
    let columns = axis_edge_from_item_position(
      GridTrackSizingDirection::ForColumns,
      is_replaced,
      is_out_of_flow,
      &style,
      grid_style,
    );
    let rows = axis_edge_from_item_position(
      GridTrackSizingDirection::ForRows,
      is_replaced,
      is_out_of_flow,
      &style,
      grid_style,
    );

    Self {
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

  /// Blink's `SetAlignmentFallback`: a baseline-aligned item that cannot take part in baseline
  /// alignment aligns to the start.
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
    let fallback = (!can_participate).then_some(AxisEdge::Start);

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
    placement_data: &GridPlacementData<'_>,
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

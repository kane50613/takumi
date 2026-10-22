//! Blink's [`GridPlacement`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_placement.cc):
//! the grid areas of a grid's items, auto-placed where their style leaves them open. Without
//! subgrid. The placed items' ordered list is a vector of their indices.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{cmp::Ordering, collections::BinaryHeap};

use rustc_hash::FxHashMap;

use crate::layout::{
  grid::{
    grid_area::{GridArea, GridSpan},
    grid_line_resolver::GridLineResolver,
    grid_style::{GridPosition, GridStyle, GridTrackSizingDirection},
    grid_track_collection::GridTrackCollection,
  },
  layout_style::LayoutStyle,
};

/// Blink's `GridPlacementData`: each item's grid area, and how far placement moved the lines.
#[derive(Debug, Clone)]
pub(crate) struct GridPlacementData<'a> {
  pub(crate) line_resolver: GridLineResolver<'a>,
  pub(crate) grid_item_positions: Vec<GridArea>,
  pub(crate) column_start_offset: usize,
  pub(crate) row_start_offset: usize,
}

impl<'a> GridPlacementData<'a> {
  pub(crate) fn start_offset(&self, direction: GridTrackSizingDirection) -> usize {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_start_offset,
      GridTrackSizingDirection::ForRows => self.row_start_offset,
    }
  }

  pub(crate) fn explicit_grid_track_count(&self, direction: GridTrackSizingDirection) -> usize {
    self.line_resolver.explicit_grid_track_count(direction)
  }
}

/// Blink's `AutoPlacementType`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AutoPlacementType {
  NotNeeded,
  Major,
  Minor,
  Both,
}

/// Blink's `GridPlacement::GridPosition`: a point in the grid, major line first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Position {
  major_line: usize,
  minor_line: usize,
}

/// Blink's `PlacedGridItem`.
#[derive(Debug, Clone, Copy)]
struct PlacedGridItem {
  start: Position,
  end: Position,
}

impl PlacedGridItem {
  fn new(
    area: &GridArea,
    major: GridTrackSizingDirection,
    minor: GridTrackSizingDirection,
  ) -> Self {
    Self {
      start: Position {
        major_line: area.start_line(major),
        minor_line: area.start_line(minor),
      },
      end: Position {
        major_line: area.end_line(major),
        minor_line: area.end_line(minor),
      },
    }
  }

  fn end_on_previous_major_line(&self) -> Position {
    Position {
      major_line: self.end.major_line - 1,
      minor_line: self.end.minor_line,
    }
  }
}

/// Blink's `PlacedGridItemsList`: the items, and their order by start position.
#[derive(Default)]
struct PlacedGridItemsList {
  items: Vec<PlacedGridItem>,
  ordered: Vec<usize>,
}

/// Blink's `CursorMovementBehavior`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorMovementBehavior {
  Auto,
  ForceMajorLine,
  ForceMinorLine,
}

/// An item in the cursor's heap, the one ending first on top.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ByEnd {
  end: Position,
  item: usize,
}

impl Ord for ByEnd {
  fn cmp(&self, other: &Self) -> Ordering {
    other.end.cmp(&self.end).then(other.item.cmp(&self.item))
  }
}

impl PartialOrd for ByEnd {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

/// Blink's `AutoPlacementCursor`.
struct AutoPlacementCursor {
  items_overlapping_major_line: BinaryHeap<ByEnd>,
  should_move_to_next_item_major_end_line: bool,
  /// The position in the ordered list of the next placed item.
  next_placed_item: usize,
  current_position: Position,
}

impl AutoPlacementCursor {
  fn new() -> Self {
    Self {
      items_overlapping_major_line: BinaryHeap::new(),
      should_move_to_next_item_major_end_line: true,
      next_placed_item: 0,
      current_position: Position::default(),
    }
  }

  fn move_to_major_line(&mut self, major_line: usize) {
    self.current_position.major_line = major_line;
  }

  fn move_to_minor_line(&mut self, minor_line: usize) {
    if minor_line < self.current_position.minor_line {
      self.current_position.major_line += 1;
    }
    self.current_position.minor_line = minor_line;
  }

  fn move_to_next_major_line(&mut self, allow_minor_line_movement: bool) {
    self.current_position.major_line += 1;

    if self.should_move_to_next_item_major_end_line
      && let Some(first) = self.items_overlapping_major_line.peek()
    {
      self.current_position.major_line = first.end.major_line;
    }
    if allow_minor_line_movement {
      self.current_position.minor_line = 0;
    }
    self.should_move_to_next_item_major_end_line = true;
  }

  /// Blink's `UpdateItemsOverlappingMajorLine`.
  fn update_items_overlapping_major_line(&mut self, list: &PlacedGridItemsList) {
    while let Some(first) = self.items_overlapping_major_line.peek() {
      let last_overlapping_position = list.items[first.item].end_on_previous_major_line();

      if self.current_position < last_overlapping_position {
        break;
      }
      if self.current_position.major_line == last_overlapping_position.major_line {
        self.should_move_to_next_item_major_end_line = false;
      }
      self.items_overlapping_major_line.pop();
    }

    while let Some(&item) = list.ordered.get(self.next_placed_item) {
      let placed = list.items[item];

      if placed.start > self.current_position {
        break;
      }

      let last_overlapping_position = placed.end_on_previous_major_line();

      if self.current_position.major_line <= last_overlapping_position.major_line {
        self.should_move_to_next_item_major_end_line = false;
      }
      if self.current_position < last_overlapping_position {
        self.items_overlapping_major_line.push(ByEnd {
          end: placed.end,
          item,
        });
      }
      self.next_placed_item += 1;
    }
  }

  /// Blink's `MoveCursorToFitGridSpan`.
  fn move_cursor_to_fit_grid_span(
    &mut self,
    list: &PlacedGridItemsList,
    major_span_size: usize,
    minor_span_size: usize,
    minor_max_end_line: usize,
    movement_behavior: CursorMovementBehavior,
  ) {
    let allow_minor_line_movement = movement_behavior != CursorMovementBehavior::ForceMinorLine;
    let minor_max_start_line = if movement_behavior == CursorMovementBehavior::ForceMajorLine {
      minor_max_end_line
    } else {
      minor_max_end_line.saturating_sub(minor_span_size)
    };
    let needs_to_move_to_next_major_line = |cursor: &Self, next_minor_line: usize| {
      next_minor_line > minor_max_start_line
        || (!allow_minor_line_movement && next_minor_line != cursor.current_position.minor_line)
    };
    // Blink's `DoesCurrentPositionFitGridSpan`.
    let fits = |cursor: &mut Self, next_minor_line: usize| {
      if needs_to_move_to_next_major_line(cursor, next_minor_line) {
        cursor.move_to_next_major_line(allow_minor_line_movement);
      } else {
        if cursor.current_position.minor_line == next_minor_line {
          return true;
        }
        cursor.move_to_minor_line(next_minor_line);
      }
      false
    };

    if self.current_position.minor_line > minor_max_start_line {
      self.move_to_next_major_line(allow_minor_line_movement);
    }

    loop {
      self.update_items_overlapping_major_line(list);

      let mut next_minor_line = self.current_position.minor_line;

      for overlapping in self.items_overlapping_major_line.iter() {
        let placed = list.items[overlapping.item];
        let minor_span_end_line = next_minor_line + minor_span_size;

        if next_minor_line < placed.end.minor_line && placed.start.minor_line < minor_span_end_line
        {
          next_minor_line = placed.end.minor_line;
          if needs_to_move_to_next_major_line(self, next_minor_line) {
            break;
          }
        }
      }

      if !fits(self, next_minor_line) {
        continue;
      }

      for &item in &list.ordered[self.next_placed_item.min(list.ordered.len())..] {
        let upcoming = list.items[item];
        let major_span_end_line = self.current_position.major_line + major_span_size;
        let minor_span_end_line = next_minor_line + minor_span_size;

        if next_minor_line < upcoming.end.minor_line
          && self.current_position.major_line < upcoming.end.major_line
          && upcoming.start.major_line < major_span_end_line
          && upcoming.start.minor_line < minor_span_end_line
        {
          next_minor_line = upcoming.end.minor_line;
          if needs_to_move_to_next_major_line(self, next_minor_line) {
            break;
          }
        }
      }

      if fits(self, next_minor_line) {
        break;
      }
    }
  }
}

/// Blink's `GridPlacement`.
pub(crate) struct GridPlacement<'a> {
  placement_data: GridPlacementData<'a>,
  is_sparse: bool,
  major_direction: GridTrackSizingDirection,
  minor_direction: GridTrackSizingDirection,
  minor_max_end_line: usize,
}

impl<'a> GridPlacement<'a> {
  pub(crate) fn new(style: &'a GridStyle, line_resolver: GridLineResolver<'a>) -> Self {
    let major_direction = if style.is_column_auto_flow {
      GridTrackSizingDirection::ForColumns
    } else {
      GridTrackSizingDirection::ForRows
    };

    Self {
      placement_data: GridPlacementData {
        line_resolver,
        grid_item_positions: Vec::new(),
        column_start_offset: 0,
        row_start_offset: 0,
      },
      is_sparse: !style.is_dense_auto_flow,
      major_direction,
      minor_direction: major_direction.opposite(),
      minor_max_end_line: 0,
    }
  }

  fn auto_placement(&self, area: &GridArea) -> AutoPlacementType {
    let is_major_indefinite = area.span(self.major_direction).is_indefinite();
    let is_minor_indefinite = area.span(self.minor_direction).is_indefinite();

    match (is_major_indefinite, is_minor_indefinite) {
      (true, true) => AutoPlacementType::Both,
      (false, true) => AutoPlacementType::Minor,
      (true, false) => AutoPlacementType::Major,
      (false, false) => AutoPlacementType::NotNeeded,
    }
  }

  fn intrinsic_end_line(&self, direction: GridTrackSizingDirection) -> usize {
    self.placement_data.start_offset(direction)
      + self.placement_data.explicit_grid_track_count(direction)
  }

  /// Blink's `RunAutoPlacementAlgorithm` for items styled `item_styles`.
  pub(crate) fn run_auto_placement_algorithm(
    mut self,
    item_styles: &[&LayoutStyle],
  ) -> GridPlacementData<'a> {
    let mut placed_items = PlacedGridItemsList::default();
    let (locked_to_major_axis, not_locked_to_major_axis) =
      self.place_non_auto_grid_items(item_styles, &mut placed_items);

    if locked_to_major_axis.is_empty() && not_locked_to_major_axis.is_empty() {
      return self.placement_data;
    }

    // Blink's `AppendCurrentItemsToOrderedList`.
    let mut ordered: Vec<usize> = (0..placed_items.items.len()).collect();

    ordered.sort_by_key(|&index| placed_items.items[index].start);
    placed_items.ordered = ordered;

    self.place_grid_items_locked_to_major_axis(&locked_to_major_axis, &mut placed_items);

    let mut cursor = AutoPlacementCursor::new();

    for index in not_locked_to_major_axis {
      match self.auto_placement(&self.placement_data.grid_item_positions[index]) {
        AutoPlacementType::Both => {
          self.place_auto_both_axis_grid_item(index, &mut placed_items, &mut cursor)
        }
        AutoPlacementType::Major => {
          self.place_auto_major_axis_grid_item(index, &mut placed_items, &mut cursor)
        }
        AutoPlacementType::Minor | AutoPlacementType::NotNeeded => {}
      }

      if !self.is_sparse {
        cursor = AutoPlacementCursor::new();
      }
    }

    self.placement_data
  }

  /// Blink's `PlaceNonAutoGridItems`: the items locked to the major axis, and the ones not.
  fn place_non_auto_grid_items(
    &mut self,
    item_styles: &[&LayoutStyle],
    placed_items: &mut PlacedGridItemsList,
  ) -> (Vec<usize>, Vec<usize>) {
    let resolver = self.placement_data.line_resolver;

    for style in item_styles {
      let area = GridArea {
        columns: resolver
          .resolve_grid_positions_from_style(style, GridTrackSizingDirection::ForColumns),
        rows: resolver.resolve_grid_positions_from_style(style, GridTrackSizingDirection::ForRows),
      };

      if area.columns.is_untranslated_definite() {
        self.placement_data.column_start_offset = self
          .placement_data
          .column_start_offset
          .max((-area.columns.untranslated_start_line()).max(0) as usize);
      }
      if area.rows.is_untranslated_definite() {
        self.placement_data.row_start_offset = self
          .placement_data
          .row_start_offset
          .max((-area.rows.untranslated_start_line()).max(0) as usize);
      }
      self.placement_data.grid_item_positions.push(area);
    }

    self.minor_max_end_line = self.intrinsic_end_line(self.minor_direction);

    let mut locked_to_major_axis = Vec::new();
    let mut not_locked_to_major_axis = Vec::new();
    let (major, minor) = (self.major_direction, self.minor_direction);
    let major_offset = self.placement_data.start_offset(major);
    let minor_offset = self.placement_data.start_offset(minor);

    for (index, area) in self
      .placement_data
      .grid_item_positions
      .iter_mut()
      .enumerate()
    {
      let mut major_span = *area.span(major);
      let mut minor_span = *area.span(minor);
      let has_indefinite_major_span = major_span.is_indefinite();
      let has_indefinite_minor_span = minor_span.is_indefinite();

      if !has_indefinite_major_span {
        major_span.translate(major_offset);
        area.set_span(major_span, major);
      }
      if !has_indefinite_minor_span {
        minor_span.translate(minor_offset);
        area.set_span(minor_span, minor);
      }

      self.minor_max_end_line = self.minor_max_end_line.max(if has_indefinite_minor_span {
        minor_span.indefinite_span_size()
      } else {
        minor_span.end_line()
      });

      if !has_indefinite_major_span && !has_indefinite_minor_span {
        placed_items
          .items
          .push(PlacedGridItem::new(area, major, minor));
      } else if has_indefinite_major_span {
        not_locked_to_major_axis.push(index);
      } else {
        locked_to_major_axis.push(index);
      }
    }

    (locked_to_major_axis, not_locked_to_major_axis)
  }

  /// Blink's `PlaceGridItemsLockedToMajorAxis`.
  fn place_grid_items_locked_to_major_axis(
    &mut self,
    positions: &[usize],
    placed_items: &mut PlacedGridItemsList,
  ) {
    let mut minor_cursors: FxHashMap<usize, usize> = FxHashMap::default();
    let (major, minor) = (self.major_direction, self.minor_direction);

    for &index in positions {
      let area = self.placement_data.grid_item_positions[index];
      let minor_span_size = area
        .span(minor)
        .indefinite_span_size()
        .min(self.minor_max_end_line);
      let major_start_line = area.start_line(major);
      let mut cursor = AutoPlacementCursor::new();

      cursor.move_to_major_line(major_start_line);
      if self.is_sparse
        && let Some(&minor_line) = minor_cursors.get(&major_start_line)
      {
        cursor.move_to_minor_line(minor_line);
      }
      cursor.move_cursor_to_fit_grid_span(
        placed_items,
        area.span_size(major),
        minor_span_size,
        self.minor_max_end_line,
        CursorMovementBehavior::ForceMajorLine,
      );

      let minor_end_line = cursor.current_position.minor_line + minor_span_size;

      if self.is_sparse {
        minor_cursors.insert(major_start_line, minor_end_line);
      }
      self.minor_max_end_line = self.minor_max_end_line.max(minor_end_line);
      self.placement_data.grid_item_positions[index].set_span(
        GridSpan::translated_definite(cursor.current_position.minor_line, minor_end_line),
        minor,
      );
      self.place_grid_item_at_cursor(index, placed_items, &mut cursor);
    }
  }

  /// Blink's `PlaceAutoMajorAxisGridItem`.
  fn place_auto_major_axis_grid_item(
    &mut self,
    index: usize,
    placed_items: &mut PlacedGridItemsList,
    cursor: &mut AutoPlacementCursor,
  ) {
    let (major, minor) = (self.major_direction, self.minor_direction);
    let area = self.placement_data.grid_item_positions[index];
    let major_span_size = area.span(major).indefinite_span_size();

    cursor.move_to_minor_line(area.start_line(minor));
    cursor.move_cursor_to_fit_grid_span(
      placed_items,
      major_span_size,
      area.span_size(minor),
      self.minor_max_end_line,
      CursorMovementBehavior::ForceMinorLine,
    );
    self.placement_data.grid_item_positions[index].set_span(
      GridSpan::translated_definite(
        cursor.current_position.major_line,
        cursor.current_position.major_line + major_span_size,
      ),
      major,
    );
    self.place_grid_item_at_cursor(index, placed_items, cursor);
  }

  /// Blink's `PlaceAutoBothAxisGridItem`.
  fn place_auto_both_axis_grid_item(
    &mut self,
    index: usize,
    placed_items: &mut PlacedGridItemsList,
    cursor: &mut AutoPlacementCursor,
  ) {
    let (major, minor) = (self.major_direction, self.minor_direction);
    let area = self.placement_data.grid_item_positions[index];
    let major_span_size = area.span(major).indefinite_span_size();
    let minor_span_size = area
      .span(minor)
      .indefinite_span_size()
      .min(self.minor_max_end_line);

    cursor.move_cursor_to_fit_grid_span(
      placed_items,
      major_span_size,
      minor_span_size,
      self.minor_max_end_line,
      CursorMovementBehavior::Auto,
    );

    let position = cursor.current_position;
    let area = &mut self.placement_data.grid_item_positions[index];

    area.set_span(
      GridSpan::translated_definite(position.major_line, position.major_line + major_span_size),
      major,
    );
    area.set_span(
      GridSpan::translated_definite(position.minor_line, position.minor_line + minor_span_size),
      minor,
    );
    self.place_grid_item_at_cursor(index, placed_items, cursor);
  }

  /// Blink's `PlaceGridItemAtCursor`.
  fn place_grid_item_at_cursor(
    &self,
    index: usize,
    placed_items: &mut PlacedGridItemsList,
    cursor: &mut AutoPlacementCursor,
  ) {
    let placed = PlacedGridItem::new(
      &self.placement_data.grid_item_positions[index],
      self.major_direction,
      self.minor_direction,
    );
    let item = placed_items.items.len();
    let insertion_point = cursor.next_placed_item.min(placed_items.ordered.len());

    placed_items.items.push(placed);
    placed_items.ordered.insert(insertion_point, item);

    // Blink's `InsertPlacedItemAtCurrentPosition`.
    cursor.next_placed_item = insertion_point;
    cursor.move_to_minor_line(placed.end.minor_line);
    cursor.update_items_overlapping_major_line(placed_items);
  }

  /// Blink's `ResolveOutOfFlowItemGridLines`: the lines an out-of-flow item's style puts its
  /// edges at within the implicit grid, none for an `auto` edge.
  pub(crate) fn resolve_out_of_flow_item_grid_lines(
    track_collection: &GridTrackCollection,
    line_resolver: &GridLineResolver<'_>,
    item_style: &LayoutStyle,
    start_offset: usize,
  ) -> (Option<usize>, Option<usize>) {
    let direction = track_collection.direction;
    let span = line_resolver.resolve_grid_positions_from_style(item_style, direction);

    if span.is_indefinite() {
      return (None, None);
    }

    let (start_position, end_position) = match direction {
      GridTrackSizingDirection::ForColumns => {
        (&item_style.grid_column_start, &item_style.grid_column_end)
      }
      GridTrackSizingDirection::ForRows => (&item_style.grid_row_start, &item_style.grid_row_end),
    };
    let line = |line: i32, position: &GridPosition| {
      let line = line + start_offset as i32;

      (line >= 0
        && !position.is_auto()
        && track_collection.is_grid_line_within_implicit_grid(line as usize))
      .then_some(line as usize)
    };

    (
      line(span.untranslated_start_line(), start_position),
      line(span.untranslated_end_line(), end_position),
    )
  }
}

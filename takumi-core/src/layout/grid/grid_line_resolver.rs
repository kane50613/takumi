//! Blink's [`GridLineResolver`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_line_resolver.cc)
//! and [`GridNamedLineCollection`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_named_line_collection.cc):
//! an item's `grid-row` and `grid-column` resolved to grid lines. Without subgrid.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use crate::layout::{
  grid::{
    grid_area::GridSpan,
    grid_style::{
      ComputedGridTrackList, GRID_MAX_TRACKS, GridPosition, GridStyle, GridTrackSizingDirection,
      NamedGridLinesMap,
    },
  },
  layout_style::LayoutStyle,
};

/// Blink's `GridPositionSide`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridPositionSide {
  ColumnStart,
  ColumnEnd,
  RowStart,
  RowEnd,
}

impl GridPositionSide {
  fn direction(self) -> GridTrackSizingDirection {
    match self {
      Self::ColumnStart | Self::ColumnEnd => GridTrackSizingDirection::ForColumns,
      Self::RowStart | Self::RowEnd => GridTrackSizingDirection::ForRows,
    }
  }

  fn is_start(self) -> bool {
    matches!(self, Self::ColumnStart | Self::RowStart)
  }
}

/// Blink's `GridNamedLineCollection`: where a line name falls in the explicit grid, its auto
/// repeater counted `auto_repeat_total_tracks` tracks long.
struct GridNamedLineCollection<'a> {
  named_lines_indexes: Option<&'a [usize]>,
  auto_repeat_named_lines_indexes: Option<&'a [usize]>,
  implicit_named_lines_indexes: Option<&'a [usize]>,
  insertion_point: usize,
  last_line: usize,
  auto_repeat_total_tracks: usize,
  auto_repeat_track_list_length: usize,
}

impl<'a> GridNamedLineCollection<'a> {
  fn new(
    named_line: &str,
    implicit_grid_line_names: Option<&'a NamedGridLinesMap>,
    computed_grid_track_list: &'a ComputedGridTrackList,
    last_line: usize,
    auto_repeat_tracks_count: usize,
  ) -> Self {
    let find = |map: &'a NamedGridLinesMap| map.get(named_line).map(Vec::as_slice);

    Self {
      named_lines_indexes: find(&computed_grid_track_list.named_grid_lines),
      auto_repeat_named_lines_indexes: find(&computed_grid_track_list.auto_repeat_named_grid_lines),
      implicit_named_lines_indexes: implicit_grid_line_names.and_then(find),
      insertion_point: computed_grid_track_list.auto_repeat_insertion_point,
      last_line,
      auto_repeat_total_tracks: auto_repeat_tracks_count,
      auto_repeat_track_list_length: computed_grid_track_list
        .track_list
        .auto_repeat_track_count(),
    }
  }

  fn has_explicit_named_lines(&self) -> bool {
    self.named_lines_indexes.is_some() || self.auto_repeat_named_lines_indexes.is_some()
  }

  fn has_named_lines(&self) -> bool {
    self.has_explicit_named_lines() || self.implicit_named_lines_indexes.is_some()
  }

  fn contains(&self, line: usize) -> bool {
    if line > self.last_line {
      return false;
    }

    let find = |indexes: Option<&[usize]>, line: usize| {
      indexes.is_some_and(|indexes| indexes.contains(&line))
    };

    if find(self.implicit_named_lines_indexes, line) {
      return true;
    }
    if self.auto_repeat_track_list_length == 0 || line < self.insertion_point {
      return find(self.named_lines_indexes, line);
    }
    if line > self.insertion_point + self.auto_repeat_total_tracks {
      return find(
        self.named_lines_indexes,
        line - (self.auto_repeat_total_tracks - 1),
      );
    }
    if self.auto_repeat_total_tracks == 0 {
      return false;
    }
    if line == self.insertion_point {
      return find(self.named_lines_indexes, line) || find(self.auto_repeat_named_lines_indexes, 0);
    }
    if line == self.insertion_point + self.auto_repeat_total_tracks {
      return find(
        self.auto_repeat_named_lines_indexes,
        self.auto_repeat_track_list_length,
      ) || find(self.named_lines_indexes, self.insertion_point + 1);
    }

    let auto_repeat_index_in_first_repetition =
      (line - self.insertion_point) % self.auto_repeat_track_list_length;

    if auto_repeat_index_in_first_repetition == 0
      && find(
        self.auto_repeat_named_lines_indexes,
        self.auto_repeat_track_list_length,
      )
    {
      return true;
    }

    find(
      self.auto_repeat_named_lines_indexes,
      auto_repeat_index_in_first_repetition,
    )
  }

  fn first_explicit_position(&self) -> usize {
    if let Some(indexes) = self.named_lines_indexes
      && (self.auto_repeat_track_list_length == 0 || indexes[0] <= self.insertion_point)
    {
      return indexes[0];
    }
    if let Some(indexes) = self.auto_repeat_named_lines_indexes {
      return indexes[0] + self.insertion_point;
    }

    let auto_repeat_counted_tracks = self.auto_repeat_total_tracks.saturating_sub(1);

    self.named_lines_indexes.map_or(0, |indexes| indexes[0]) + auto_repeat_counted_tracks
  }

  fn first_position(&self) -> usize {
    let Some(implicit) = self.implicit_named_lines_indexes else {
      return self.first_explicit_position();
    };

    if !self.has_explicit_named_lines() {
      return implicit[0];
    }

    self.first_explicit_position().min(implicit[0])
  }
}

/// Blink's `GridLineResolver`: a grid's lines as its auto repeaters repeat.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridLineResolver<'a> {
  style: &'a GridStyle,
  column_auto_repetitions: usize,
  row_auto_repetitions: usize,
}

impl<'a> GridLineResolver<'a> {
  pub(crate) fn new(
    style: &'a GridStyle,
    column_auto_repetitions: usize,
    row_auto_repetitions: usize,
  ) -> Self {
    Self {
      style,
      column_auto_repetitions,
      row_auto_repetitions,
    }
  }

  pub(crate) fn explicit_grid_track_count(&self, direction: GridTrackSizingDirection) -> usize {
    let mut count = self
      .style
      .template_tracks(direction)
      .track_list
      .track_count_without_auto_repeat()
      + self.auto_repeat_track_count(direction);

    if let Some(areas) = &self.style.template_areas {
      count = count.max(match direction {
        GridTrackSizingDirection::ForColumns => areas.column_count,
        GridTrackSizingDirection::ForRows => areas.row_count,
      });
    }

    count.min(GRID_MAX_TRACKS)
  }

  pub(crate) fn auto_repetitions(&self, direction: GridTrackSizingDirection) -> usize {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_auto_repetitions,
      GridTrackSizingDirection::ForRows => self.row_auto_repetitions,
    }
  }

  pub(crate) fn auto_repeat_track_count(&self, direction: GridTrackSizingDirection) -> usize {
    self.auto_repetitions(direction)
      * self
        .style
        .template_tracks(direction)
        .track_list
        .auto_repeat_track_count()
  }

  fn implicit_named_lines_map(
    &self,
    direction: GridTrackSizingDirection,
  ) -> Option<&'a NamedGridLinesMap> {
    self
      .style
      .template_areas
      .as_ref()
      .map(|areas| match direction {
        GridTrackSizingDirection::ForColumns => &areas.implicit_named_grid_column_lines,
        GridTrackSizingDirection::ForRows => &areas.implicit_named_grid_row_lines,
      })
  }

  fn lines_collection(&self, name: &str, side: GridPositionSide) -> GridNamedLineCollection<'a> {
    let direction = side.direction();

    GridNamedLineCollection::new(
      name,
      self.implicit_named_lines_map(direction),
      self.style.template_tracks(direction),
      self.explicit_grid_track_count(direction),
      self.auto_repeat_track_count(direction),
    )
  }

  /// Blink's `LookAheadForNamedGridLine`.
  fn look_ahead_for_named_grid_line(
    start: i32,
    mut number_of_lines: usize,
    grid_last_line: usize,
    lines_collection: &GridNamedLineCollection<'_>,
  ) -> i32 {
    let mut end = start.max(0) as usize;

    if !lines_collection.has_named_lines() {
      end = end.max(grid_last_line + 1);
      return (end + number_of_lines - 1) as i32;
    }

    while number_of_lines > 0 {
      if end > grid_last_line || lines_collection.contains(end) {
        number_of_lines -= 1;
      }
      end += 1;
    }

    end as i32 - 1
  }

  /// Blink's `LookBackForNamedGridLine`.
  fn look_back_for_named_grid_line(
    end: i32,
    mut number_of_lines: usize,
    grid_last_line: i32,
    lines_collection: &GridNamedLineCollection<'_>,
  ) -> i32 {
    let mut start = end.min(grid_last_line);

    if !lines_collection.has_named_lines() {
      start = start.min(-1);
      return start - number_of_lines as i32 + 1;
    }

    while number_of_lines > 0 {
      if start < 0 || lines_collection.contains(start as usize) {
        number_of_lines -= 1;
      }
      start -= 1;
    }

    start + 1
  }

  /// Blink's `ResolveNamedGridLinePositionAgainstOppositePosition`.
  fn resolve_named_grid_line_position_against_opposite_position(
    &self,
    opposite_line: i32,
    span: i32,
    name: &str,
    side: GridPositionSide,
  ) -> GridSpan {
    let last_line = self.explicit_grid_track_count(side.direction());
    let lines_collection = self.lines_collection(name, side);
    let span = span.max(1) as usize;

    if side.is_start() {
      let start = Self::look_back_for_named_grid_line(
        opposite_line - 1,
        span,
        last_line as i32,
        &lines_collection,
      );

      GridSpan::untranslated_definite(start, opposite_line)
    } else {
      let end =
        Self::look_ahead_for_named_grid_line(opposite_line + 1, span, last_line, &lines_collection);

      GridSpan::untranslated_definite(opposite_line, end)
    }
  }

  /// Blink's `ResolveGridPositionAgainstOppositePosition`.
  fn resolve_grid_position_against_opposite_position(
    &self,
    opposite_line: i32,
    position: &GridPosition,
    side: GridPositionSide,
  ) -> GridSpan {
    let span = match position {
      GridPosition::Auto => 1,
      GridPosition::Span {
        span,
        name: Some(name),
      } => {
        return self.resolve_named_grid_line_position_against_opposite_position(
          opposite_line,
          *span,
          name,
          side,
        );
      }
      GridPosition::Span { span, name: None } => *span,
      GridPosition::Explicit { .. } | GridPosition::NamedArea(_) => 1,
    };

    if side.is_start() {
      GridSpan::untranslated_definite(opposite_line - span, opposite_line)
    } else {
      GridSpan::untranslated_definite(opposite_line, opposite_line + span)
    }
  }

  /// Blink's `ResolveNamedGridLinePosition`.
  fn resolve_named_grid_line_position(
    &self,
    integer: i32,
    name: &str,
    side: GridPositionSide,
  ) -> i32 {
    let last_line = self.explicit_grid_track_count(side.direction());
    let lines_collection = self.lines_collection(name, side);
    let count = integer.unsigned_abs().max(1) as usize;

    if integer >= 0 {
      return Self::look_ahead_for_named_grid_line(0, count, last_line, &lines_collection);
    }

    Self::look_back_for_named_grid_line(
      last_line as i32,
      count,
      last_line as i32,
      &lines_collection,
    )
  }

  /// Blink's `ResolveGridPosition`.
  fn resolve_grid_position(&self, position: &GridPosition, side: GridPositionSide) -> i32 {
    match position {
      GridPosition::Explicit {
        integer,
        name: Some(name),
      } => self.resolve_named_grid_line_position(*integer, name, side),
      GridPosition::Explicit {
        integer,
        name: None,
      } => {
        if *integer > 0 {
          return integer - 1;
        }

        let resolved_position = integer.unsigned_abs() as i32 - 1;

        self.explicit_grid_track_count(side.direction()) as i32 - resolved_position
      }
      GridPosition::NamedArea(name) => {
        let suffix = if side.is_start() { "-start" } else { "-end" };
        let implicit_lines = self.lines_collection(&format!("{name}{suffix}"), side);

        if implicit_lines.has_named_lines() {
          return implicit_lines.first_position() as i32;
        }

        let explicit_lines = self.lines_collection(name, side);

        if explicit_lines.has_named_lines() {
          return explicit_lines.first_position() as i32;
        }

        self.explicit_grid_track_count(side.direction()) as i32 + 1
      }
      GridPosition::Auto | GridPosition::Span { .. } => 0,
    }
  }

  /// Blink's `ResolveGridPositionsFromStyle`.
  pub(crate) fn resolve_grid_positions_from_style(
    &self,
    item_style: &LayoutStyle,
    direction: GridTrackSizingDirection,
  ) -> GridSpan {
    let (mut initial_position, mut final_position) = match direction {
      GridTrackSizingDirection::ForColumns => (
        item_style.grid_column_start.clone(),
        item_style.grid_column_end.clone(),
      ),
      GridTrackSizingDirection::ForRows => (
        item_style.grid_row_start.clone(),
        item_style.grid_row_end.clone(),
      ),
    };

    // Blink's `InitialAndFinalPositionsFromStyle`.
    if initial_position.is_span() && final_position.is_span() {
      final_position = GridPosition::Auto;
    }
    if initial_position.is_auto()
      && matches!(final_position, GridPosition::Span { name: Some(_), .. })
    {
      final_position = GridPosition::Span {
        span: 1,
        name: None,
      };
    }
    if final_position.is_auto()
      && matches!(initial_position, GridPosition::Span { name: Some(_), .. })
    {
      initial_position = GridPosition::Span {
        span: 1,
        name: None,
      };
    }

    let initial_against_opposite = initial_position.should_be_resolved_against_opposite_position();
    let final_against_opposite = final_position.should_be_resolved_against_opposite_position();

    if initial_against_opposite && final_against_opposite {
      let span = match (&initial_position, &final_position) {
        (GridPosition::Span { span, .. }, _) | (_, GridPosition::Span { span, .. }) => {
          *span as usize
        }
        _ => 1,
      };

      return GridSpan::indefinite(span);
    }

    let (initial_side, final_side) = match direction {
      GridTrackSizingDirection::ForColumns => {
        (GridPositionSide::ColumnStart, GridPositionSide::ColumnEnd)
      }
      GridTrackSizingDirection::ForRows => (GridPositionSide::RowStart, GridPositionSide::RowEnd),
    };

    if initial_against_opposite {
      let end_line = self.resolve_grid_position(&final_position, final_side);

      return self.resolve_grid_position_against_opposite_position(
        end_line,
        &initial_position,
        initial_side,
      );
    }
    if final_against_opposite {
      let start_line = self.resolve_grid_position(&initial_position, initial_side);

      return self.resolve_grid_position_against_opposite_position(
        start_line,
        &final_position,
        final_side,
      );
    }

    let mut start_line = self.resolve_grid_position(&initial_position, initial_side);
    let mut end_line = self.resolve_grid_position(&final_position, final_side);

    if end_line < start_line {
      std::mem::swap(&mut start_line, &mut end_line);
    } else if end_line == start_line {
      end_line = start_line + 1;
    }

    GridSpan::untranslated_definite(start_line, end_line)
  }
}

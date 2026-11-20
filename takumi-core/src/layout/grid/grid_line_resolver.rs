//! Blink's [`GridLineResolver`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_line_resolver.cc)
//! and [`GridNamedLineCollection`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_named_line_collection.cc):
//! an item's `grid-row` and `grid-column` resolved to grid lines, a subgrid's merged with the
//! line names and areas of its parent grids.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::swap, rc::Rc};

use crate::{
  layout::{
    grid::{
      grid_area::{GridArea, GridSpan},
      grid_style::{
        ComputedGridTrackList, GRID_MAX_TRACKS, GridPosition, GridStyle, GridTemplateAreas,
        GridTrackSizingDirection, NamedGridAreaMap, NamedGridLinesMap,
      },
    },
    layout_style::LayoutStyle,
  },
  style::Direction,
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
  is_standalone_grid: bool,
}

impl<'a> GridNamedLineCollection<'a> {
  fn new(
    named_line: &str,
    implicit_grid_line_names: Option<&'a NamedGridLinesMap>,
    explicit_grid_line_names: &'a NamedGridLinesMap,
    computed_grid_track_list: &'a ComputedGridTrackList,
    last_line: usize,
    auto_repeat_tracks_count: usize,
    is_subgridded_to_parent: bool,
  ) -> Self {
    let find = |map: &'a NamedGridLinesMap| {
      map
        .get(named_line)
        .map(Vec::as_slice)
        .filter(|indexes| !indexes.is_empty())
    };
    let is_standalone_grid = !computed_grid_track_list.is_subgridded_axis();
    // A subgridded axis has its line names only when a parent grid subgrids it.
    // <https://www.w3.org/TR/css-grid-2/#subgrid-listing>
    let are_named_lines_valid = is_subgridded_to_parent || is_standalone_grid;
    let mut collection = Self {
      named_lines_indexes: find(explicit_grid_line_names).filter(|_| are_named_lines_valid),
      auto_repeat_named_lines_indexes: find(&computed_grid_track_list.auto_repeat_named_grid_lines)
        .filter(|_| are_named_lines_valid),
      implicit_named_lines_indexes: implicit_grid_line_names.and_then(find),
      insertion_point: computed_grid_track_list.auto_repeat_insertion_point,
      last_line,
      auto_repeat_total_tracks: auto_repeat_tracks_count,
      auto_repeat_track_list_length: computed_grid_track_list
        .track_list
        .auto_repeat_track_count(),
      is_standalone_grid,
    };

    // A subgrid's auto repeater may repeat no times, and `contains` counts the line after it one
    // higher, so the last line moves with it.
    if collection.has_collapsed_auto_repeat() {
      collection.last_line += 1;
    }
    collection
  }

  fn has_explicit_named_lines(&self) -> bool {
    self.named_lines_indexes.is_some() || self.auto_repeat_named_lines_indexes.is_some()
  }

  /// Blink's `HasCollapsedAutoRepeat`: a subgrid's auto repeater that repeats no times.
  fn has_collapsed_auto_repeat(&self) -> bool {
    !self.is_standalone_grid
      && self.auto_repeat_track_list_length != 0
      && self.auto_repeat_total_tracks == 0
  }

  fn has_named_lines(&self) -> bool {
    self.has_explicit_named_lines() || self.implicit_named_lines_indexes.is_some()
  }

  fn contains(&self, mut line: usize) -> bool {
    if line > self.last_line {
      return false;
    }

    let has_collapsed_auto_repeat = self.has_collapsed_auto_repeat();

    if has_collapsed_auto_repeat && line >= self.insertion_point {
      line += 1;
    }

    let find = |indexes: Option<&[usize]>, line: usize| {
      indexes.is_some_and(|indexes| indexes.contains(&line))
    };

    if find(self.implicit_named_lines_indexes, line) {
      return true;
    }
    if self.auto_repeat_track_list_length == 0
      || has_collapsed_auto_repeat
      || line < self.insertion_point
    {
      return find(self.named_lines_indexes, line);
    }
    if line > self.insertion_point + self.auto_repeat_total_tracks {
      // Blink subtracts `auto_repeat_total_tracks - 1` in unsigned arithmetic, which wraps to
      // `line + 1` when no repetition fits.
      return find(
        self.named_lines_indexes,
        line + 1 - self.auto_repeat_total_tracks,
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
      && ((self.is_standalone_grid && self.auto_repeat_track_list_length == 0)
        || indexes[0] <= self.insertion_point)
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

/// Blink's `MergeNamedGridLinesWithParent`: `parent_map`'s lines in `subgrid_span` merged into
/// `subgrid_map`, numbered from the subgrid's first line.
fn merge_named_grid_lines_with_parent(
  subgrid_map: &mut NamedGridLinesMap,
  parent_map: &NamedGridLinesMap,
  subgrid_span: GridSpan,
  is_opposite_direction_to_parent: bool,
) {
  for (name, positions) in parent_map {
    let mut merged_list: Vec<usize> = positions
      .iter()
      .filter(|&&position| subgrid_span.contains(position))
      .map(|&position| {
        if is_opposite_direction_to_parent {
          subgrid_span.end_line() - position
        } else {
          position - subgrid_span.start_line()
        }
      })
      .collect();

    if let Some(existing) = subgrid_map.get(name) {
      merged_list.extend_from_slice(existing);
      merged_list.sort_unstable();
      merged_list.dedup();
    }
    if !merged_list.is_empty() {
      subgrid_map.insert(name.clone(), merged_list);
    }
  }
}

/// Blink's `ExpandAutoRepeatTracksFromParent`: the parent's auto repeater's line names in
/// `subgrid_span` merged into `subgrid_map`, after the subgrid's lines past the repeater move by its
/// tracks.
fn expand_auto_repeat_tracks_from_parent(
  subgrid_map: &mut NamedGridLinesMap,
  parent_auto_repeat_map: &NamedGridLinesMap,
  track_list: &ComputedGridTrackList,
  subgrid_span: GridSpan,
  auto_repetitions: usize,
  is_opposite_direction_to_parent: bool,
  is_nested_subgrid: bool,
) {
  let auto_repeat_track_count = track_list.track_list.auto_repeat_track_count();
  let auto_repeat_total_tracks = auto_repeat_track_count * auto_repetitions;

  if auto_repeat_total_tracks == 0 {
    return;
  }

  let insertion_point = track_list.auto_repeat_insertion_point;

  // A nested subgrid's lines past the insertion point stay, as Blink leaves them: moving them here
  // too would move them twice.
  if !is_nested_subgrid {
    for positions in subgrid_map.values_mut() {
      *positions = positions
        .iter()
        .filter_map(|&position| {
          if position < insertion_point {
            return Some(position);
          }

          let expanded_position = position + auto_repeat_total_tracks;

          subgrid_span
            .contains(expanded_position)
            .then_some(expanded_position)
        })
        .collect();
    }
  }

  for (name, positions) in parent_auto_repeat_map {
    let mut merged_list = Vec::new();

    for &position in positions {
      for repetition in 0..auto_repetitions {
        for track in 0..auto_repeat_track_count {
          // Blink adds the repetition and the track, not the repetition's first track.
          let expanded_position = insertion_point + position + repetition + track;

          if subgrid_span.contains(expanded_position) {
            merged_list.push(if is_opposite_direction_to_parent {
              subgrid_span.end_line() - expanded_position
            } else {
              expanded_position - subgrid_span.start_line()
            });
          }
        }
      }
    }
    if let Some(existing) = subgrid_map.get(name) {
      merged_list.extend_from_slice(existing);
      merged_list.sort_unstable();
    }
    if !merged_list.is_empty() {
      subgrid_map.insert(name.clone(), merged_list);
    }
  }
}

/// Blink's `ClampSubgridAreas`: `style_map`'s areas clamped to the subgrid's span.
fn clamp_subgrid_areas(
  subgrid_map: &mut NamedGridAreaMap,
  style_map: &NamedGridAreaMap,
  subgrid_span: &GridArea,
) {
  for (name, area) in style_map {
    let mut clamped_area = *area;

    if subgrid_span.columns.is_translated_definite() {
      clamped_area
        .columns
        .intersect(0, subgrid_span.columns.integer_span() as i32);
    }
    if subgrid_span.rows.is_translated_definite() {
      clamped_area
        .rows
        .intersect(0, subgrid_span.rows.integer_span() as i32);
    }
    subgrid_map.insert(name.clone(), clamped_area);
  }
}

/// Blink's `MergeAndClampGridAreasWithParent`: the parent's areas that overlap the subgrid,
/// clamped to it and numbered from its first lines, merged into `subgrid_map`.
/// <https://www.w3.org/TR/css-grid-2/#subgrid-area-inheritance>
fn merge_and_clamp_grid_areas_with_parent(
  subgrid_map: &mut NamedGridAreaMap,
  parent_map: &NamedGridAreaMap,
  subgrid_span: &GridArea,
) {
  let has_subgridded_columns = subgrid_span.columns.is_translated_definite();
  let has_subgridded_rows = subgrid_span.rows.is_translated_definite();

  for (name, area) in parent_map {
    let mut position = *area;
    let rows_intersect = has_subgridded_rows && subgrid_span.rows.intersects(position.rows);
    let columns_intersect =
      has_subgridded_columns && subgrid_span.columns.intersects(position.columns);

    if !rows_intersect && !columns_intersect {
      continue;
    }

    for (has_subgridded_axis, direction) in [
      (has_subgridded_rows, GridTrackSizingDirection::ForRows),
      (has_subgridded_columns, GridTrackSizingDirection::ForColumns),
    ] {
      if !has_subgridded_axis {
        continue;
      }

      let span = subgrid_span.span(direction);
      let position_span = position.span_mut(direction);

      position_span.intersect(span.start_line() as i32, span.end_line() as i32);
      position_span.translate_by(-(span.start_line() as i32));

      if let Some(existing) = subgrid_map.get(name) {
        let existing_span = existing.span(direction);

        position_span.set_start(
          position_span
            .untranslated_start_line()
            .min(existing_span.untranslated_start_line()),
        );
        position_span.set_end(
          position_span
            .untranslated_end_line()
            .min(existing_span.untranslated_end_line()),
        );
      }
    }
    subgrid_map.insert(name.clone(), position);
  }
}

/// Blink's `GridLineResolver`: a grid's lines as its auto repeaters repeat, a subgrid's merged
/// with the line names and areas of its parent grid.
#[derive(Debug, Clone)]
pub(crate) struct GridLineResolver {
  style: Rc<GridStyle>,
  direction: Direction,
  column_auto_repetitions: usize,
  row_auto_repetitions: usize,
  subgridded_columns_span_size: Option<usize>,
  subgridded_rows_span_size: Option<usize>,
  subgridded_columns_merged_explicit_grid_line_names: Option<NamedGridLinesMap>,
  subgridded_rows_merged_explicit_grid_line_names: Option<NamedGridLinesMap>,
  subgridded_columns_merged_implicit_grid_line_names: Option<NamedGridLinesMap>,
  subgridded_rows_merged_implicit_grid_line_names: Option<NamedGridLinesMap>,
  subgrid_merged_named_areas: Option<NamedGridAreaMap>,
}

impl PartialEq for GridLineResolver {
  /// Blink's `operator==`, which compares the inputs placement reads alone.
  fn eq(&self, other: &Self) -> bool {
    self.column_auto_repetitions == other.column_auto_repetitions
      && self.row_auto_repetitions == other.row_auto_repetitions
      && self.subgridded_columns_span_size == other.subgridded_columns_span_size
      && self.subgridded_rows_span_size == other.subgridded_rows_span_size
  }
}

impl GridLineResolver {
  /// A grid's line resolver, the grid laid out in `direction`.
  pub(crate) fn new(
    style: Rc<GridStyle>,
    direction: Direction,
    column_auto_repetitions: usize,
    row_auto_repetitions: usize,
  ) -> Self {
    Self {
      style,
      direction,
      column_auto_repetitions,
      row_auto_repetitions,
      subgridded_columns_span_size: None,
      subgridded_rows_span_size: None,
      subgridded_columns_merged_explicit_grid_line_names: None,
      subgridded_rows_merged_explicit_grid_line_names: None,
      subgridded_columns_merged_implicit_grid_line_names: None,
      subgridded_rows_merged_implicit_grid_line_names: None,
      subgrid_merged_named_areas: None,
    }
  }

  /// Blink's subgrid constructor: the resolver of a subgrid in `subgrid_area` of the grid
  /// `parent_line_resolver` resolves, its subgridded axes' definite in the area.
  ///
  /// Approximate: takumi lays out horizontally alone, so a subgrid is always parallel to its parent.
  pub(crate) fn for_subgrid(
    style: Rc<GridStyle>,
    direction: Direction,
    parent_line_resolver: &GridLineResolver,
    subgrid_area: GridArea,
    column_auto_repetitions: usize,
    row_auto_repetitions: usize,
  ) -> Self {
    let has_subgridded_columns = subgrid_area.columns.is_translated_definite();
    let has_subgridded_rows = subgrid_area.rows.is_translated_definite();
    let is_opposite_direction_to_parent = direction != parent_line_resolver.direction;
    let mut resolver = Self {
      subgridded_columns_span_size: has_subgridded_columns
        .then(|| subgrid_area.span_size(GridTrackSizingDirection::ForColumns)),
      subgridded_rows_span_size: has_subgridded_rows
        .then(|| subgrid_area.span_size(GridTrackSizingDirection::ForRows)),
      subgridded_columns_merged_explicit_grid_line_names: Some(
        style.template_columns.named_grid_lines.clone(),
      ),
      subgridded_rows_merged_explicit_grid_line_names: Some(
        style.template_rows.named_grid_lines.clone(),
      ),
      ..Self::new(
        Rc::clone(&style),
        direction,
        column_auto_repetitions,
        row_auto_repetitions,
      )
    };

    for (is_subgridded, direction, subgrid_span) in [
      (
        has_subgridded_columns,
        GridTrackSizingDirection::ForColumns,
        subgrid_area.columns,
      ),
      (
        has_subgridded_rows,
        GridTrackSizingDirection::ForRows,
        subgrid_area.rows,
      ),
    ] {
      if !is_subgridded {
        continue;
      }

      let merged = match direction {
        GridTrackSizingDirection::ForColumns => {
          &mut resolver.subgridded_columns_merged_explicit_grid_line_names
        }
        GridTrackSizingDirection::ForRows => {
          &mut resolver.subgridded_rows_merged_explicit_grid_line_names
        }
      };
      let merged = merged.get_or_insert_default();

      merge_named_grid_lines_with_parent(
        merged,
        parent_line_resolver.explicit_named_lines_map(direction),
        subgrid_span,
        is_opposite_direction_to_parent,
      );
      expand_auto_repeat_tracks_from_parent(
        merged,
        parent_line_resolver.auto_repeat_line_names_map(direction),
        parent_line_resolver.computed_grid_track_list(direction),
        subgrid_span,
        parent_line_resolver.auto_repetitions(direction),
        is_opposite_direction_to_parent,
        parent_line_resolver.is_subgridded(direction),
      );
    }

    if let Some(areas) = &style.template_areas {
      clamp_subgrid_areas(
        resolver.subgrid_merged_named_areas.get_or_insert_default(),
        &areas.named_areas,
        &subgrid_area,
      );
    }
    if let Some(parent_areas) = parent_line_resolver.named_areas_map() {
      merge_and_clamp_grid_areas_with_parent(
        resolver.subgrid_merged_named_areas.get_or_insert_default(),
        parent_areas,
        &subgrid_area,
      );
    }
    if let Some(areas) = &resolver.subgrid_merged_named_areas {
      if has_subgridded_columns {
        resolver.subgridded_columns_merged_implicit_grid_line_names = Some(
          GridTemplateAreas::create_implicit_named_grid_lines_from_grid_area(
            areas,
            GridTrackSizingDirection::ForColumns,
          ),
        );
      }
      if has_subgridded_rows {
        resolver.subgridded_rows_merged_implicit_grid_line_names = Some(
          GridTemplateAreas::create_implicit_named_grid_lines_from_grid_area(
            areas,
            GridTrackSizingDirection::ForRows,
          ),
        );
      }
    }
    resolver
  }

  pub(crate) fn explicit_grid_track_count(&self, direction: GridTrackSizingDirection) -> usize {
    if let Some(span_size) = self.subgrid_span_size(direction) {
      return span_size;
    }

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
        .computed_grid_track_list(direction)
        .track_list
        .auto_repeat_track_count()
  }

  /// Blink's `SubgridSpanSize`, none for a standalone axis.
  pub(crate) fn subgrid_span_size(&self, direction: GridTrackSizingDirection) -> Option<usize> {
    match direction {
      GridTrackSizingDirection::ForColumns => self.subgridded_columns_span_size,
      GridTrackSizingDirection::ForRows => self.subgridded_rows_span_size,
    }
  }

  /// Blink's `HasStandaloneAxis`.
  pub(crate) fn has_standalone_axis(&self, direction: GridTrackSizingDirection) -> bool {
    self.subgrid_span_size(direction).is_none()
  }

  /// Blink's `IsSubgridded`: whether the merged explicit line names exist, as they do for a
  /// subgrid in both axes.
  fn is_subgridded(&self, direction: GridTrackSizingDirection) -> bool {
    match direction {
      GridTrackSizingDirection::ForColumns => {
        &self.subgridded_columns_merged_explicit_grid_line_names
      }
      GridTrackSizingDirection::ForRows => &self.subgridded_rows_merged_explicit_grid_line_names,
    }
    .is_some()
  }

  /// Blink's `ImplicitNamedLinesMap`.
  fn implicit_named_lines_map(
    &self,
    direction: GridTrackSizingDirection,
  ) -> Option<&NamedGridLinesMap> {
    let merged = match direction {
      GridTrackSizingDirection::ForColumns => {
        &self.subgridded_columns_merged_implicit_grid_line_names
      }
      GridTrackSizingDirection::ForRows => &self.subgridded_rows_merged_implicit_grid_line_names,
    };

    merged.as_ref().or_else(|| {
      self
        .style
        .template_areas
        .as_ref()
        .map(|areas| match direction {
          GridTrackSizingDirection::ForColumns => &areas.implicit_named_grid_column_lines,
          GridTrackSizingDirection::ForRows => &areas.implicit_named_grid_row_lines,
        })
    })
  }

  /// Blink's `ExplicitNamedLinesMap`.
  fn explicit_named_lines_map(&self, direction: GridTrackSizingDirection) -> &NamedGridLinesMap {
    let merged = match direction {
      GridTrackSizingDirection::ForColumns => {
        &self.subgridded_columns_merged_explicit_grid_line_names
      }
      GridTrackSizingDirection::ForRows => &self.subgridded_rows_merged_explicit_grid_line_names,
    };

    merged
      .as_ref()
      .unwrap_or(&self.computed_grid_track_list(direction).named_grid_lines)
  }

  /// Blink's `NamedAreasMap`.
  fn named_areas_map(&self) -> Option<&NamedGridAreaMap> {
    self.subgrid_merged_named_areas.as_ref().or_else(|| {
      self
        .style
        .template_areas
        .as_ref()
        .map(|areas| &areas.named_areas)
    })
  }

  /// Blink's `AutoRepeatLineNamesMap`.
  fn auto_repeat_line_names_map(&self, direction: GridTrackSizingDirection) -> &NamedGridLinesMap {
    &self
      .computed_grid_track_list(direction)
      .auto_repeat_named_grid_lines
  }

  /// Blink's `ComputedGridTrackList`.
  fn computed_grid_track_list(
    &self,
    direction: GridTrackSizingDirection,
  ) -> &ComputedGridTrackList {
    self.style.template_tracks(direction)
  }

  fn lines_collection(&self, name: &str, side: GridPositionSide) -> GridNamedLineCollection<'_> {
    let direction = side.direction();

    GridNamedLineCollection::new(
      name,
      self.implicit_named_lines_map(direction),
      self.explicit_named_lines_map(direction),
      self.computed_grid_track_list(direction),
      self.explicit_grid_track_count(direction),
      self.auto_repeat_track_count(direction),
      self.is_subgridded(direction),
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
      swap(&mut start_line, &mut end_line);
    } else if end_line == start_line {
      end_line = start_line + 1;
    }

    GridSpan::untranslated_definite(start_line, end_line)
  }
}

#[cfg(test)]
mod tests {
  use super::GridNamedLineCollection;

  #[test]
  fn line_after_an_auto_repeater_with_no_repetitions() {
    let named_lines = [3];
    let collection = GridNamedLineCollection {
      named_lines_indexes: Some(&named_lines),
      auto_repeat_named_lines_indexes: None,
      implicit_named_lines_indexes: None,
      insertion_point: 1,
      last_line: 4,
      auto_repeat_total_tracks: 0,
      auto_repeat_track_list_length: 1,
      is_standalone_grid: true,
    };

    assert!(collection.contains(2));
  }
}

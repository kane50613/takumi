//! A grid's track lists, line names, and areas as Blink's
//! [`GridTrackList`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/grid_track_list.h),
//! [`GridTrackSize`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/grid_track_size.h),
//! and [`GridPosition`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/grid_position.h)
//! hold them, built from takumi's computed style the way Blink's `StyleBuilderConverter` builds
//! them. Without subgrid or `grid-lanes`.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use rustc_hash::FxHashMap;

use crate::{
  layout::{
    grid::grid_area::{GridArea, GridSpan},
    layout_style::LayoutLength,
  },
  style::{
    ComputedStyle, GridDirection, GridLength, GridPlacement, GridPlacementSpan,
    GridRepetitionCount, GridRepetitionKeyword, GridTemplateComponent,
    GridTrackSize as StyleTrackSize, SizingContext,
  },
};

/// Blink's `GridTrackSizingDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GridTrackSizingDirection {
  ForColumns,
  ForRows,
}

impl GridTrackSizingDirection {
  pub(crate) fn opposite(self) -> Self {
    match self {
      Self::ForColumns => Self::ForRows,
      Self::ForRows => Self::ForColumns,
    }
  }
}

/// A track breadth: a length or keyword, or a flexible `fr`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TrackBreadth {
  Length(LayoutLength),
  Flex(f32),
}

impl TrackBreadth {
  pub(crate) const AUTO: Self = Self::Length(LayoutLength::Auto);

  pub(crate) fn is_auto(self) -> bool {
    self == Self::AUTO
  }

  pub(crate) fn is_flex(self) -> bool {
    matches!(self, Self::Flex(_))
  }

  pub(crate) fn flex(self) -> f32 {
    match self {
      Self::Flex(flex) => flex,
      Self::Length(_) => 0.0,
    }
  }

  pub(crate) fn is_min_content(self) -> bool {
    self == Self::Length(LayoutLength::MinContent)
  }

  pub(crate) fn is_max_content(self) -> bool {
    self == Self::Length(LayoutLength::MaxContent)
  }

  /// Blink's `HasOnlyFixedAndPercent`.
  pub(crate) fn is_fixed(self) -> bool {
    matches!(
      self,
      Self::Length(LayoutLength::Fixed(_) | LayoutLength::Percent(_) | LayoutLength::Calc { .. })
    )
  }

  pub(crate) fn has_percent(self) -> bool {
    matches!(self, Self::Length(length) if length.has_percent())
  }

  /// The length a fixed breadth resolves to, its percentages of `basis`.
  pub(crate) fn length(self) -> LayoutLength {
    match self {
      Self::Length(length) => length,
      Self::Flex(_) => LayoutLength::Auto,
    }
  }

  /// Blink's `IsTrackSizeIntrinsic` for a length or `minmax()` track.
  fn is_intrinsic(self) -> bool {
    matches!(
      self,
      Self::Length(LayoutLength::Auto | LayoutLength::MinContent | LayoutLength::MaxContent)
    )
  }
}

/// Blink's `GridTrackSizeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridTrackSizeType {
  Length,
  MinMax,
  FitContent,
}

/// Blink's `GridTrackSize`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GridTrackSize {
  pub(crate) min_track_breadth: TrackBreadth,
  pub(crate) max_track_breadth: TrackBreadth,
  pub(crate) fit_content_track_breadth: LayoutLength,
  pub(crate) kind: GridTrackSizeType,
}

impl GridTrackSize {
  pub(crate) const AUTO: Self = Self::length(TrackBreadth::AUTO);

  pub(crate) const fn length(breadth: TrackBreadth) -> Self {
    Self {
      min_track_breadth: breadth,
      max_track_breadth: breadth,
      fit_content_track_breadth: LayoutLength::Fixed(0.0),
      kind: GridTrackSizeType::Length,
    }
  }

  pub(crate) const fn min_max(min: TrackBreadth, max: TrackBreadth) -> Self {
    Self {
      min_track_breadth: min,
      max_track_breadth: max,
      fit_content_track_breadth: LayoutLength::Fixed(0.0),
      kind: GridTrackSizeType::MinMax,
    }
  }

  pub(crate) fn is_fit_content(&self) -> bool {
    self.kind == GridTrackSizeType::FitContent
  }

  /// Blink's `IsTrackDefinitionIntrinsic`.
  pub(crate) fn is_track_definition_intrinsic(&self) -> bool {
    match self.kind {
      GridTrackSizeType::FitContent => true,
      GridTrackSizeType::Length => self.min_track_breadth.is_intrinsic(),
      GridTrackSizeType::MinMax => {
        self.min_track_breadth.is_intrinsic() && self.max_track_breadth.is_intrinsic()
      }
    }
  }

  pub(crate) fn has_percentage(&self) -> bool {
    if self.is_fit_content() {
      return self.fit_content_track_breadth.has_percent();
    }

    self.min_track_breadth.has_percent() || self.max_track_breadth.has_percent()
  }

  pub(crate) fn has_intrinsic_min_track_breadth(&self) -> bool {
    self.min_track_breadth.is_intrinsic() || self.is_fit_content()
  }

  pub(crate) fn has_intrinsic_max_track_breadth(&self) -> bool {
    self.max_track_breadth.is_intrinsic() || self.is_fit_content()
  }

  pub(crate) fn has_auto_min_track_breadth(&self) -> bool {
    self.min_track_breadth.is_auto()
  }

  pub(crate) fn has_auto_max_track_breadth(&self) -> bool {
    self.max_track_breadth.is_auto()
  }

  pub(crate) fn has_max_content_min_track_breadth(&self) -> bool {
    self.min_track_breadth.is_max_content()
  }

  pub(crate) fn has_min_content_min_track_breadth(&self) -> bool {
    self.min_track_breadth.is_min_content()
  }

  pub(crate) fn has_min_or_max_content_min_track_breadth(&self) -> bool {
    self.has_max_content_min_track_breadth() || self.has_min_content_min_track_breadth()
  }

  pub(crate) fn has_max_content_max_track_breadth(&self) -> bool {
    self.max_track_breadth.is_max_content()
  }

  pub(crate) fn has_max_content_or_auto_max_track_breadth(&self) -> bool {
    self.has_max_content_max_track_breadth() || self.has_auto_max_track_breadth()
  }

  pub(crate) fn has_fixed_min_track_breadth(&self) -> bool {
    self.min_track_breadth.is_fixed()
  }

  pub(crate) fn has_fixed_max_track_breadth(&self) -> bool {
    self.max_track_breadth.is_fixed()
  }

  pub(crate) fn has_flex_min_track_breadth(&self) -> bool {
    self.min_track_breadth.is_flex()
  }

  pub(crate) fn has_flex_max_track_breadth(&self) -> bool {
    self.max_track_breadth.is_flex()
  }

  /// Blink's `IsDefinite`.
  pub(crate) fn is_definite(&self) -> bool {
    self.has_fixed_min_track_breadth()
      && self.has_fixed_max_track_breadth()
      && self.min_track_breadth == self.max_track_breadth
  }

  /// The track size takumi's computed `size` is.
  fn of(size: StyleTrackSize, sizing: &SizingContext) -> Self {
    let breadth = |length: GridLength| match length {
      GridLength::Fr(flex) => TrackBreadth::Flex(flex),
      GridLength::Unit(length) => TrackBreadth::Length(length.to_layout_length(sizing)),
    };

    match size {
      StyleTrackSize::Fixed(length) => Self::length(breadth(length)),
      StyleTrackSize::MinMax(min_max) => Self::min_max(breadth(min_max.min), breadth(min_max.max)),
    }
  }
}

/// Blink's `GridTrackRepeater::RepeatType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepeatType {
  NoRepeat,
  AutoFill,
  AutoFit,
  Integer,
}

/// Blink's `GridTrackRepeater`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GridTrackRepeater {
  pub(crate) repeat_index: usize,
  pub(crate) repeat_size: usize,
  pub(crate) repeat_count: usize,
  pub(crate) repeat_type: RepeatType,
}

/// Blink's `kGridMaxTracks`.
pub(crate) const GRID_MAX_TRACKS: usize = 10_000_000;

/// Blink's `GridTrackList`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GridTrackList {
  repeaters: Vec<GridTrackRepeater>,
  repeater_track_sizes: Vec<GridTrackSize>,
  auto_repeater_index: Option<usize>,
  track_count_without_auto_repeat: usize,
  has_intrinsic_sized_repeater: bool,
}

impl GridTrackList {
  /// A list of one repeated `track_size`, as `grid-auto-rows` and `grid-auto-columns` default to.
  pub(crate) fn of_track(track_size: GridTrackSize) -> Self {
    let mut list = Self::default();

    list.add_repeater(&[track_size], RepeatType::NoRepeat, 1);
    list
  }

  pub(crate) fn repeat_count(&self, index: usize, auto_value: usize) -> usize {
    if Some(index) == self.auto_repeater_index {
      return auto_value;
    }

    self.repeaters[index].repeat_count
  }

  pub(crate) fn repeat_size(&self, index: usize) -> usize {
    self.repeaters[index].repeat_size
  }

  pub(crate) fn repeat_type(&self, index: usize) -> RepeatType {
    self.repeaters[index].repeat_type
  }

  pub(crate) fn repeat_track_size(&self, index: usize, n: usize) -> &GridTrackSize {
    &self.repeater_track_sizes[self.repeaters[index].repeat_index + n]
  }

  pub(crate) fn repeater_count(&self) -> usize {
    self.repeaters.len()
  }

  pub(crate) fn track_count_without_auto_repeat(&self) -> usize {
    self.track_count_without_auto_repeat
  }

  pub(crate) fn auto_repeat_track_count(&self) -> usize {
    self
      .auto_repeater_index
      .map_or(0, |index| self.repeaters[index].repeat_size)
  }

  pub(crate) fn has_auto_repeater(&self) -> bool {
    self.auto_repeater_index.is_some()
  }

  /// Blink's `AddRepeater`: false when the repeater would hold no tracks or too many.
  pub(crate) fn add_repeater(
    &mut self,
    track_sizes: &[GridTrackSize],
    repeat_type: RepeatType,
    repeat_count: usize,
  ) -> bool {
    if repeat_count == 0 || track_sizes.is_empty() {
      return false;
    }

    let repeat_size = track_sizes.len();
    let available_track_count = GRID_MAX_TRACKS - self.track_count_without_auto_repeat;

    match repeat_type {
      RepeatType::NoRepeat | RepeatType::Integer => {
        if repeat_size > available_track_count / repeat_count {
          return false;
        }
        self.track_count_without_auto_repeat += repeat_size * repeat_count;
      }
      RepeatType::AutoFill | RepeatType::AutoFit => {
        self.has_intrinsic_sized_repeater = track_sizes
          .iter()
          .any(GridTrackSize::is_track_definition_intrinsic);

        if self.has_auto_repeater() || repeat_size > available_track_count {
          return false;
        }
        self.auto_repeater_index = Some(self.repeaters.len());
      }
    }

    self.repeaters.push(GridTrackRepeater {
      repeat_index: self.repeater_track_sizes.len(),
      repeat_size,
      repeat_count,
      repeat_type,
    });
    self.repeater_track_sizes.extend_from_slice(track_sizes);
    true
  }
}

/// Blink's `NamedGridLinesMap`: each line name's line indices.
pub(crate) type NamedGridLinesMap = FxHashMap<String, Vec<usize>>;

/// Blink's `ComputedGridTrackList`: a track list with its line names.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ComputedGridTrackList {
  pub(crate) track_list: GridTrackList,
  pub(crate) named_grid_lines: NamedGridLinesMap,
  /// The line names inside the auto repeater, by index in one repetition.
  pub(crate) auto_repeat_named_grid_lines: NamedGridLinesMap,
  /// The line index the auto repeater stands at among the explicit named lines.
  pub(crate) auto_repeat_insertion_point: usize,
}

impl ComputedGridTrackList {
  /// Blink's `StyleBuilderConverter::ConvertGridTrackList`.
  fn of(components: Option<&[GridTemplateComponent]>, sizing: &SizingContext) -> Self {
    let mut list = Self::default();
    let Some(components) = components else {
      return list;
    };
    let mut current_named_grid_line = 0;
    let add_names = |map: &mut NamedGridLinesMap, names: &[String], line: usize| {
      for name in names {
        map.entry(name.clone()).or_default().push(line);
      }
    };

    for component in components {
      match component {
        GridTemplateComponent::LineNames(names) => {
          add_names(&mut list.named_grid_lines, names, current_named_grid_line);
        }
        GridTemplateComponent::Single(size) => {
          current_named_grid_line += 1;
          list.track_list.add_repeater(
            &[GridTrackSize::of(*size, sizing)],
            RepeatType::NoRepeat,
            1,
          );
        }
        GridTemplateComponent::Repeat(GridRepetitionCount::Keyword(keyword), tracks) => {
          let mut track_sizes = Vec::with_capacity(tracks.len());

          for (index, track) in tracks.iter().enumerate() {
            add_names(&mut list.auto_repeat_named_grid_lines, &track.names, index);
            track_sizes.push(GridTrackSize::of(track.size, sizing));
          }
          if let Some(end_names) = tracks.last().and_then(|track| track.end_names.as_deref()) {
            add_names(
              &mut list.auto_repeat_named_grid_lines,
              end_names,
              tracks.len(),
            );
          }

          let repeat_type = match keyword {
            GridRepetitionKeyword::AutoFill => RepeatType::AutoFill,
            GridRepetitionKeyword::AutoFit => RepeatType::AutoFit,
          };

          list.track_list.add_repeater(&track_sizes, repeat_type, 1);
          list.auto_repeat_insertion_point = current_named_grid_line;
          current_named_grid_line += 1;
        }
        GridTemplateComponent::Repeat(GridRepetitionCount::Count(repetitions), tracks) => {
          let repetitions = usize::from(*repetitions);

          for _ in 0..repetitions {
            for track in tracks {
              add_names(
                &mut list.named_grid_lines,
                &track.names,
                current_named_grid_line,
              );
              current_named_grid_line += 1;
            }
            if let Some(end_names) = tracks.last().and_then(|track| track.end_names.as_deref()) {
              add_names(
                &mut list.named_grid_lines,
                end_names,
                current_named_grid_line,
              );
            }
          }

          let track_sizes: Vec<_> = tracks
            .iter()
            .map(|track| GridTrackSize::of(track.size, sizing))
            .collect();

          list
            .track_list
            .add_repeater(&track_sizes, RepeatType::Integer, repetitions);
        }
      }
    }

    list
  }
}

/// Blink's `ComputedGridTemplateAreas`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GridTemplateAreas {
  pub(crate) named_areas: FxHashMap<String, GridArea>,
  pub(crate) row_count: usize,
  pub(crate) column_count: usize,
  pub(crate) implicit_named_grid_column_lines: NamedGridLinesMap,
  pub(crate) implicit_named_grid_row_lines: NamedGridLinesMap,
}

impl GridTemplateAreas {
  fn of(rows: &[Vec<String>]) -> Option<Self> {
    if rows.is_empty() {
      return None;
    }

    let mut areas = Self {
      row_count: rows.len(),
      column_count: rows.iter().map(Vec::len).max().unwrap_or(0),
      ..Self::default()
    };
    let mut bounds: FxHashMap<&str, (usize, usize, usize, usize)> = FxHashMap::default();

    for (row, cells) in rows.iter().enumerate() {
      for (column, cell) in cells.iter().enumerate() {
        if cell == "." {
          continue;
        }

        let bound = bounds
          .entry(cell.as_str())
          .or_insert((row, row, column, column));

        bound.0 = bound.0.min(row);
        bound.1 = bound.1.max(row);
        bound.2 = bound.2.min(column);
        bound.3 = bound.3.max(column);
      }
    }

    for (name, (row_start, row_end, column_start, column_end)) in bounds {
      let add = |map: &mut NamedGridLinesMap, suffix: &str, line: usize| {
        map.entry(format!("{name}{suffix}")).or_default().push(line);
      };

      add(
        &mut areas.implicit_named_grid_row_lines,
        "-start",
        row_start,
      );
      add(
        &mut areas.implicit_named_grid_row_lines,
        "-end",
        row_end + 1,
      );
      add(
        &mut areas.implicit_named_grid_column_lines,
        "-start",
        column_start,
      );
      add(
        &mut areas.implicit_named_grid_column_lines,
        "-end",
        column_end + 1,
      );
      areas.named_areas.insert(
        name.to_owned(),
        GridArea {
          rows: GridSpan::translated_definite(row_start, row_end + 1),
          columns: GridSpan::translated_definite(column_start, column_end + 1),
        },
      );
    }

    for lines in areas
      .implicit_named_grid_row_lines
      .values_mut()
      .chain(areas.implicit_named_grid_column_lines.values_mut())
    {
      lines.sort_unstable();
    }

    Some(areas)
  }
}

/// Blink's `GridPosition`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum GridPosition {
  #[default]
  Auto,
  /// `<integer> || <custom-ident>`.
  Explicit { integer: i32, name: Option<String> },
  /// `span && [ <integer> || <custom-ident> ]`.
  Span { span: i32, name: Option<String> },
  /// A lone `<custom-ident>`.
  NamedArea(String),
}

impl GridPosition {
  pub(crate) fn of(placement: &GridPlacement) -> Self {
    match placement {
      GridPlacement::Auto => Self::Auto,
      GridPlacement::Line(line) => Self::Explicit {
        integer: i32::from(*line),
        name: None,
      },
      GridPlacement::Span(GridPlacementSpan::Span(span)) => Self::Span {
        span: i32::from(*span),
        name: None,
      },
      GridPlacement::Named(name) => Self::NamedArea(name.clone()),
    }
  }

  pub(crate) fn is_auto(&self) -> bool {
    *self == Self::Auto
  }

  pub(crate) fn is_span(&self) -> bool {
    matches!(self, Self::Span { .. })
  }

  /// Blink's `ShouldBeResolvedAgainstOppositePosition`.
  pub(crate) fn should_be_resolved_against_opposite_position(&self) -> bool {
    self.is_auto() || self.is_span()
  }
}

/// What a grid container's style says of its grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GridStyle {
  pub(crate) template_columns: ComputedGridTrackList,
  pub(crate) template_rows: ComputedGridTrackList,
  pub(crate) auto_columns: GridTrackList,
  pub(crate) auto_rows: GridTrackList,
  pub(crate) template_areas: Option<GridTemplateAreas>,
  /// Whether auto-placement fills columns before rows.
  pub(crate) is_column_auto_flow: bool,
  pub(crate) is_dense_auto_flow: bool,
}

impl GridStyle {
  pub(crate) fn of(style: &ComputedStyle, sizing: &SizingContext) -> Self {
    let auto_tracks = |sizes: Option<&[StyleTrackSize]>| {
      let mut list = GridTrackList::default();
      let sizes: Vec<_> = sizes
        .unwrap_or_default()
        .iter()
        .map(|size| GridTrackSize::of(*size, sizing))
        .collect();

      if !list.add_repeater(&sizes, RepeatType::NoRepeat, 1) {
        list = GridTrackList::of_track(GridTrackSize::AUTO);
      }
      list
    };
    let grid = &style.misc2_data;

    Self {
      template_columns: ComputedGridTrackList::of(grid.grid_template_columns.as_deref(), sizing),
      template_rows: ComputedGridTrackList::of(grid.grid_template_rows.as_deref(), sizing),
      auto_columns: auto_tracks(grid.grid_auto_columns.as_deref()),
      auto_rows: auto_tracks(grid.grid_auto_rows.as_deref()),
      template_areas: grid
        .grid_template_areas
        .as_ref()
        .and_then(|areas| GridTemplateAreas::of(&areas.0)),
      is_column_auto_flow: grid.grid_auto_flow.direction == GridDirection::Column,
      is_dense_auto_flow: grid.grid_auto_flow.dense,
    }
  }

  /// Blink's `TemplateTracks`.
  pub(crate) fn template_tracks(
    &self,
    direction: GridTrackSizingDirection,
  ) -> &ComputedGridTrackList {
    match direction {
      GridTrackSizingDirection::ForColumns => &self.template_columns,
      GridTrackSizingDirection::ForRows => &self.template_rows,
    }
  }

  /// Blink's `AutoTracks`.
  pub(crate) fn auto_tracks(&self, direction: GridTrackSizingDirection) -> &GridTrackList {
    match direction {
      GridTrackSizingDirection::ForColumns => &self.auto_columns,
      GridTrackSizingDirection::ForRows => &self.auto_rows,
    }
  }
}

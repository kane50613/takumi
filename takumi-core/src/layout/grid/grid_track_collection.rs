//! Blink's [`GridTrackCollection`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_track_collection.cc):
//! a grid's tracks in ranges of the same definition, each split into sets of tracks sized as one.
//! Blink's `GridLayoutTrackCollection` and `GridSizingTrackCollection` as one type, without
//! subgrid or `grid-lanes`.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use bitflags::bitflags;

use crate::{
  layout::{
    grid::grid_style::{
      GridStyle, GridTrackList, GridTrackSize, GridTrackSizingDirection, RepeatType, TrackBreadth,
    },
    layout_style::LayoutLength,
  },
  layout_unit::LayoutUnit,
};

bitflags! {
  /// Blink's `TrackSpanProperties`.
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
  pub(crate) struct TrackSpanProperties: u16 {
    const HAS_AUTO_MINIMUM_TRACK = 1 << 0;
    const HAS_FIXED_MAXIMUM_TRACK = 1 << 1;
    const HAS_FIXED_MINIMUM_TRACK = 1 << 2;
    const HAS_FLEXIBLE_TRACK = 1 << 3;
    const HAS_INTRINSIC_TRACK = 1 << 4;
    const HAS_NON_DEFINITE_TRACK = 1 << 5;
    const IS_COLLAPSED = 1 << 6;
    const IS_DEPENDENT_ON_AVAILABLE_SIZE = 1 << 7;
    const IS_IMPLICIT = 1 << 8;
    const IS_AUTO_REPEAT = 1 << 9;
  }
}

impl TrackSpanProperties {
  /// Blink's `ResetType`: drops all but the range's own kind.
  pub(crate) fn reset_type(&mut self) {
    *self &= Self::IS_COLLAPSED | Self::IS_IMPLICIT | Self::IS_AUTO_REPEAT;
  }
}

/// Blink's `GridRange`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GridRange {
  pub(crate) begin_set_index: usize,
  pub(crate) repeater_index: Option<usize>,
  pub(crate) repeater_offset: usize,
  pub(crate) set_count: usize,
  pub(crate) start_line: usize,
  pub(crate) track_count: usize,
  pub(crate) properties: TrackSpanProperties,
}

impl GridRange {
  pub(crate) fn is_collapsed(&self) -> bool {
    self.properties.contains(TrackSpanProperties::IS_COLLAPSED)
  }

  pub(crate) fn is_implicit(&self) -> bool {
    self.properties.contains(TrackSpanProperties::IS_IMPLICIT)
  }
}

/// A grid line, and where to note the index of the range it bounds.
#[derive(Debug, Clone, Copy)]
struct TrackBoundary {
  grid_line: usize,
  /// The item and which of its edges this line is, for Blink's `grid_item_range_index_to_cache`.
  item: Option<(usize, bool)>,
}

/// Blink's `GridRangeBuilder`.
pub(crate) struct GridRangeBuilder<'a> {
  auto_repetitions: usize,
  start_offset: usize,
  must_sort_grid_lines: bool,
  explicit_tracks: &'a GridTrackList,
  implicit_tracks: &'a GridTrackList,
  start_lines: Vec<TrackBoundary>,
  end_lines: Vec<TrackBoundary>,
}

/// The ranges an item's start and end lines fall in, by item, as the builder finds them.
pub(crate) type ItemRangeIndices = Vec<(usize, usize)>;

impl<'a> GridRangeBuilder<'a> {
  pub(crate) fn new(
    style: &'a GridStyle,
    direction: GridTrackSizingDirection,
    auto_repetitions: usize,
    start_offset: usize,
  ) -> Self {
    let explicit_tracks = &style.template_tracks(direction).track_list;
    let implicit_tracks = style.auto_tracks(direction);
    let mut builder = Self {
      auto_repetitions,
      start_offset,
      must_sort_grid_lines: false,
      explicit_tracks,
      implicit_tracks,
      start_lines: Vec::new(),
      end_lines: Vec::new(),
    };
    let mut current_repeater_start_line = start_offset;

    for index in 0..explicit_tracks.repeater_count() {
      let repeater_track_count =
        explicit_tracks.repeat_count(index, auto_repetitions) * explicit_tracks.repeat_size(index);

      if repeater_track_count == 0 {
        continue;
      }

      builder.start_lines.push(TrackBoundary {
        grid_line: current_repeater_start_line,
        item: None,
      });
      current_repeater_start_line += repeater_track_count;
      builder.end_lines.push(TrackBoundary {
        grid_line: current_repeater_start_line,
        item: None,
      });
    }

    // The template areas may reach past the template tracks.
    let named_grid_area_end_line = start_offset
      + style
        .template_areas
        .as_ref()
        .map_or(0, |areas| match direction {
          GridTrackSizingDirection::ForColumns => areas.column_count,
          GridTrackSizingDirection::ForRows => areas.row_count,
        });
    let current_repeater_start_line = builder
      .end_lines
      .last()
      .map_or(start_offset, |line| line.grid_line);

    if current_repeater_start_line < named_grid_area_end_line {
      builder.start_lines.push(TrackBoundary {
        grid_line: current_repeater_start_line,
        item: None,
      });
      builder.end_lines.push(TrackBoundary {
        grid_line: named_grid_area_end_line,
        item: None,
      });
    }

    builder
  }

  /// Blink's `EnsureTrackCoverage` for the item `item`, whose range indices come back from
  /// [`Self::finalize_ranges`].
  pub(crate) fn ensure_track_coverage(
    &mut self,
    start_line: usize,
    span_length: usize,
    item: usize,
  ) {
    self.must_sort_grid_lines = true;
    self.start_lines.push(TrackBoundary {
      grid_line: start_line,
      item: Some((item, true)),
    });
    self.end_lines.push(TrackBoundary {
      grid_line: start_line + span_length,
      item: Some((item, false)),
    });
  }

  /// Blink's `FinalizeRanges`: the ranges, and each of `item_count` items' start and end ranges.
  pub(crate) fn finalize_ranges(mut self, item_count: usize) -> (Vec<GridRange>, ItemRangeIndices) {
    if self.must_sort_grid_lines {
      self.start_lines.sort_by_key(|line| line.grid_line);
      self.end_lines.sort_by_key(|line| line.grid_line);
    }

    let explicit_repeater_count = self.explicit_tracks.repeater_count();
    let grid_line_count = self.start_lines.len();
    let mut item_ranges = vec![(0, 0); item_count];
    let mut ranges: Vec<GridRange> = Vec::new();
    let mut is_in_auto_fit_range = false;
    let mut current_explicit_grid_line = self.start_offset;
    let mut current_explicit_repeater_index: Option<usize> = None;
    let mut current_range_start_line = 0;
    let mut current_set_index = 0;
    let mut open_items_or_repeaters: usize = 0;
    let mut next_explicit_repeater_start =
      (explicit_repeater_count > 0).then_some(self.start_offset);
    let mut start_line_index = 0;
    let mut end_line_index = 0;

    loop {
      while start_line_index < grid_line_count
        && current_range_start_line >= self.start_lines[start_line_index].grid_line
      {
        start_line_index += 1;
        open_items_or_repeaters += 1;
      }
      while end_line_index < grid_line_count
        && current_range_start_line >= self.end_lines[end_line_index].grid_line
      {
        end_line_index += 1;
        open_items_or_repeaters -= 1;
      }

      if end_line_index >= grid_line_count {
        break;
      }

      let next_start_line = self
        .start_lines
        .get(start_line_index)
        .map_or(usize::MAX, |line| line.grid_line);
      let next_end_line = self.end_lines[end_line_index].grid_line;

      while Some(current_range_start_line) == next_explicit_repeater_start {
        current_explicit_grid_line = current_range_start_line;

        let next_index = current_explicit_repeater_index.map_or(0, |index| index + 1);

        if next_index == explicit_repeater_count {
          current_explicit_repeater_index = None;
          next_explicit_repeater_start = None;
          is_in_auto_fit_range = false;
          break;
        }

        current_explicit_repeater_index = Some(next_index);
        is_in_auto_fit_range = self.explicit_tracks.repeat_type(next_index) == RepeatType::AutoFit;
        next_explicit_repeater_start = next_explicit_repeater_start.map(|start| {
          start
            + self.explicit_tracks.repeat_size(next_index)
              * self
                .explicit_tracks
                .repeat_count(next_index, self.auto_repetitions)
        });
      }

      let mut range = GridRange {
        start_line: current_range_start_line,
        track_count: next_start_line.min(next_end_line) - current_range_start_line,
        begin_set_index: current_set_index,
        ..GridRange::default()
      };
      let mut current_repeater_size = 1;

      if let Some(repeater_index) = current_explicit_repeater_index {
        current_repeater_size = self.explicit_tracks.repeat_size(repeater_index);
        range.repeater_index = Some(repeater_index);
        range.repeater_offset =
          (current_range_start_line - current_explicit_grid_line) % current_repeater_size;

        if matches!(
          self.explicit_tracks.repeat_type(repeater_index),
          RepeatType::AutoFit | RepeatType::AutoFill
        ) {
          range.properties |= TrackSpanProperties::IS_AUTO_REPEAT;
        }
      } else {
        range.properties |= TrackSpanProperties::IS_IMPLICIT;

        if self.implicit_tracks.repeater_count() > 0 {
          current_repeater_size = self.implicit_tracks.repeat_size(0);
          range.repeater_index = Some(0);
          range.repeater_offset = (current_range_start_line + current_repeater_size
            - current_explicit_grid_line % current_repeater_size)
            % current_repeater_size;
        }
      }

      // The items starting or ending at this range's edges note its index.
      for line in self.start_lines[..start_line_index]
        .iter()
        .rev()
        .take_while(|line| line.grid_line == range.start_line)
      {
        if let Some((item, true)) = line.item {
          item_ranges[item].0 = ranges.len();
        }
      }

      let end_line = range.start_line + range.track_count;

      for line in self.end_lines[end_line_index..]
        .iter()
        .take_while(|line| line.grid_line == end_line)
      {
        if let Some((item, false)) = line.item {
          item_ranges[item].1 = ranges.len();
        }
      }

      // An auto-fit range no item covers collapses.
      if is_in_auto_fit_range && open_items_or_repeaters == 1 {
        range.properties |= TrackSpanProperties::IS_COLLAPSED;
        range.set_count = 0;
      } else {
        range.set_count = current_repeater_size.min(range.track_count);
      }

      current_range_start_line += range.track_count;
      current_set_index += range.set_count;
      ranges.push(range);
    }

    (ranges, item_ranges)
  }
}

/// Blink's `GridSet`: tracks of one definition sized together.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridSet {
  pub(crate) track_count: usize,
  pub(crate) track_size: GridTrackSize,
  pub(crate) base_size: LayoutUnit,
  pub(crate) growth_limit: Option<LayoutUnit>,
  pub(crate) planned_increase: Option<LayoutUnit>,
  pub(crate) fit_content_limit: Option<LayoutUnit>,
  pub(crate) item_incurred_increase: Option<LayoutUnit>,
  pub(crate) is_infinitely_growable: bool,
}

impl GridSet {
  /// A set of `track_count` tracks with no definition, as a range with no repeater has.
  fn undefined(track_count: usize) -> Self {
    Self::with_size(
      track_count,
      GridTrackSize::min_max(TrackBreadth::AUTO, TrackBreadth::AUTO),
    )
  }

  fn with_size(track_count: usize, track_size: GridTrackSize) -> Self {
    Self {
      track_count,
      track_size,
      base_size: LayoutUnit::ZERO,
      growth_limit: None,
      planned_increase: None,
      fit_content_limit: None,
      item_incurred_increase: None,
      is_infinitely_growable: false,
    }
  }

  /// A set of `track_definition` tracks, normalized as Blink's `GridSet` constructor does: a
  /// percentage against an indefinite size, or a flexible minimum, as `auto`.
  fn new(
    track_count: usize,
    track_definition: GridTrackSize,
    is_available_size_indefinite: bool,
  ) -> Self {
    let track_size = if track_definition.is_fit_content() {
      if is_available_size_indefinite && track_definition.fit_content_track_breadth.has_percent() {
        GridTrackSize::min_max(
          TrackBreadth::AUTO,
          TrackBreadth::Length(LayoutLength::MaxContent),
        )
      } else {
        track_definition
      }
    } else {
      let min = if (is_available_size_indefinite
        && track_definition.min_track_breadth.has_percent())
        || track_definition.has_flex_min_track_breadth()
      {
        TrackBreadth::AUTO
      } else {
        track_definition.min_track_breadth
      };
      let max = if is_available_size_indefinite && track_definition.max_track_breadth.has_percent()
      {
        TrackBreadth::AUTO
      } else {
        track_definition.max_track_breadth
      };

      GridTrackSize::min_max(min, max)
    };

    Self::with_size(track_count, track_size)
  }

  pub(crate) fn flex_factor(&self) -> f32 {
    self.track_size.max_track_breadth.flex() * self.track_count as f32
  }

  pub(crate) fn init_base_size(&mut self, base_size: LayoutUnit) {
    self.base_size = base_size;
    self.ensure_growth_limit_is_not_less_than_base_size();
  }

  pub(crate) fn increase_base_size(&mut self, base_size: LayoutUnit) {
    self.base_size = base_size;
    self.ensure_growth_limit_is_not_less_than_base_size();
  }

  pub(crate) fn increase_growth_limit(&mut self, growth_limit: LayoutUnit) {
    self.growth_limit = Some(growth_limit);
  }

  pub(crate) fn ensure_growth_limit_is_not_less_than_base_size(&mut self) {
    if self.is_growth_limit_less_than_base_size() {
      self.growth_limit = Some(self.base_size);
    }
  }

  pub(crate) fn is_growth_limit_less_than_base_size(&self) -> bool {
    self
      .growth_limit
      .is_some_and(|limit| limit < self.base_size)
  }
}

/// Blink's `GridLayoutTrackCollection::SetGeometry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SetGeometry {
  pub(crate) offset: LayoutUnit,
  pub(crate) track_count: usize,
}

/// Blink's `GridSizingTrackCollection`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridTrackCollection {
  pub(crate) direction: GridTrackSizingDirection,
  pub(crate) gutter_size: LayoutUnit,
  pub(crate) ranges: Vec<GridRange>,
  properties: TrackSpanProperties,
  sets_geometry: Vec<SetGeometry>,
  /// For each set boundary, the last set before it whose size is indefinite.
  last_indefinite_index: Vec<Option<usize>>,
  non_collapsed_track_count: usize,
  pub(crate) sets: Vec<GridSet>,
}

impl GridTrackCollection {
  pub(crate) fn new(ranges: Vec<GridRange>, direction: GridTrackSizingDirection) -> Self {
    let non_collapsed_track_count = ranges
      .iter()
      .filter(|range| !range.is_collapsed())
      .map(|range| range.track_count)
      .sum();

    Self {
      direction,
      gutter_size: LayoutUnit::ZERO,
      ranges,
      properties: TrackSpanProperties::empty(),
      sets_geometry: Vec::new(),
      last_indefinite_index: Vec::new(),
      non_collapsed_track_count,
      sets: Vec::new(),
    }
  }

  pub(crate) fn range_count(&self) -> usize {
    self.ranges.len()
  }

  /// Blink's `RangeIndexFromGridLine`.
  pub(crate) fn range_index_from_grid_line(&self, grid_line: usize) -> usize {
    let mut upper = self.ranges.len();
    let mut lower = 0;

    while lower < upper {
      let center = (lower + upper) / 2;
      let range = &self.ranges[center];

      if grid_line < range.start_line {
        upper = center;
      } else if grid_line < range.start_line + range.track_count {
        return center;
      } else {
        lower = center + 1;
      }
    }

    lower
  }

  pub(crate) fn end_line_of_implicit_grid(&self) -> usize {
    self
      .ranges
      .last()
      .map_or(0, |range| range.start_line + range.track_count)
  }

  pub(crate) fn is_grid_line_within_implicit_grid(&self, grid_line: usize) -> bool {
    grid_line <= self.end_line_of_implicit_grid()
  }

  pub(crate) fn set_count(&self) -> usize {
    self
      .ranges
      .last()
      .map_or(0, |range| range.begin_set_index + range.set_count)
  }

  pub(crate) fn set_offset(&self, set_index: usize) -> LayoutUnit {
    self.sets_geometry[set_index].offset
  }

  /// Blink's `AdjustSetOffsets`: the sets from `set_index` on move by `delta`.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn adjust_set_offsets(&mut self, set_index: usize, delta: LayoutUnit) {
    for geometry in self.sets_geometry.iter_mut().skip(set_index) {
      geometry.offset += delta;
    }
  }

  pub(crate) fn set_track_count(&self, set_index: usize) -> usize {
    self.sets_geometry[set_index + 1].track_count
  }

  /// Blink's `CalculateSetSpanSize` over every set.
  pub(crate) fn total_set_span_size(&self) -> Option<LayoutUnit> {
    self.calculate_set_span_size(0, self.set_count())
  }

  /// Blink's `CalculateSetSpanSize`: none when the sets span one whose size is indefinite.
  pub(crate) fn calculate_set_span_size(
    &self,
    begin_set_index: usize,
    end_set_index: usize,
  ) -> Option<LayoutUnit> {
    if begin_set_index == end_set_index {
      return Some(LayoutUnit::ZERO);
    }
    if self.is_spanning_indefinite_set(begin_set_index, end_set_index) {
      return None;
    }

    Some(
      (self.set_offset(end_set_index) - self.gutter_size - self.set_offset(begin_set_index))
        .clamp_negative_to_zero(),
    )
  }

  fn is_spanning_indefinite_set(&self, begin_set_index: usize, end_set_index: usize) -> bool {
    self
      .last_indefinite_index
      .get(end_set_index)
      .copied()
      .flatten()
      .is_some_and(|last_indefinite_index| begin_set_index <= last_indefinite_index)
  }

  pub(crate) fn has_flexible_track(&self) -> bool {
    self
      .properties
      .contains(TrackSpanProperties::HAS_FLEXIBLE_TRACK)
  }

  pub(crate) fn has_intrinsic_track(&self) -> bool {
    self
      .properties
      .contains(TrackSpanProperties::HAS_INTRINSIC_TRACK)
  }

  pub(crate) fn has_non_definite_track(&self) -> bool {
    self
      .properties
      .contains(TrackSpanProperties::HAS_NON_DEFINITE_TRACK)
  }

  pub(crate) fn is_dependent_on_available_size(&self) -> bool {
    self
      .properties
      .contains(TrackSpanProperties::IS_DEPENDENT_ON_AVAILABLE_SIZE)
  }

  pub(crate) fn non_collapsed_track_count(&self) -> usize {
    self.non_collapsed_track_count
  }

  /// Blink's `TotalTrackSize`.
  pub(crate) fn total_track_size(&self) -> LayoutUnit {
    if self.sets.is_empty() {
      return LayoutUnit::ZERO;
    }

    self.sets.iter().fold(LayoutUnit::ZERO, |total, set| {
      total + set.base_size + self.gutter_size * set.track_count as i32
    }) - self.gutter_size
  }

  /// Blink's `CacheInitializedSetsGeometry`.
  pub(crate) fn cache_initialized_sets_geometry(&mut self, mut first_set_offset: LayoutUnit) {
    self.last_indefinite_index = vec![None];
    self.sets_geometry = vec![SetGeometry {
      offset: first_set_offset,
      track_count: 0,
    }];

    for set in &self.sets {
      match set.growth_limit {
        None => self
          .last_indefinite_index
          .push(Some(self.last_indefinite_index.len() - 1)),
        Some(growth_limit) => {
          first_set_offset += growth_limit + self.gutter_size * set.track_count as i32;
          let last = *self.last_indefinite_index.last().unwrap_or(&None);

          self.last_indefinite_index.push(last);
        }
      }
      self.sets_geometry.push(SetGeometry {
        offset: first_set_offset,
        track_count: set.track_count,
      });
    }
  }

  /// Blink's `FinalizeSetsGeometry`.
  pub(crate) fn finalize_sets_geometry(
    &mut self,
    mut first_set_offset: LayoutUnit,
    override_gutter_size: LayoutUnit,
  ) {
    self.gutter_size = override_gutter_size;
    self.last_indefinite_index.clear();
    self.sets_geometry = vec![SetGeometry {
      offset: first_set_offset,
      track_count: 0,
    }];

    for set in &self.sets {
      first_set_offset += set.base_size + self.gutter_size * set.track_count as i32;
      self.sets_geometry.push(SetGeometry {
        offset: first_set_offset,
        track_count: set.track_count,
      });
    }
  }

  /// Blink's `SetIndefiniteGrowthLimitsToBaseSize`.
  pub(crate) fn set_indefinite_growth_limits_to_base_size(&mut self) {
    for set in &mut self.sets {
      if set.growth_limit.is_none() {
        set.growth_limit = Some(set.base_size);
      }
    }
  }

  /// Blink's `BuildSets`: each range's sets, its properties, and their initial sizes against
  /// `available_size`, with `gutter_size` between tracks.
  pub(crate) fn build_sets(
    &mut self,
    style: &GridStyle,
    available_size: Option<LayoutUnit>,
    gutter_size: LayoutUnit,
  ) {
    let explicit_track_list = &style.template_tracks(self.direction).track_list;
    let implicit_track_list = style.auto_tracks(self.direction);
    let is_available_size_indefinite = available_size.is_none();

    self.gutter_size = gutter_size;
    self.properties.reset_type();
    self.sets.clear();

    for range in &mut self.ranges {
      range.properties.reset_type();

      if range.is_collapsed() {
        continue;
      }

      let cache_set_properties = |properties: &mut TrackSpanProperties, set: &GridSet| {
        let size = &set.track_size;

        if size.has_auto_min_track_breadth() {
          *properties |= TrackSpanProperties::HAS_AUTO_MINIMUM_TRACK;
        }
        if size.has_fixed_min_track_breadth() {
          *properties |= TrackSpanProperties::HAS_FIXED_MINIMUM_TRACK;
        }
        if size.has_fixed_max_track_breadth() {
          *properties |= TrackSpanProperties::HAS_FIXED_MAXIMUM_TRACK;
        }
        if size.has_flex_max_track_breadth() {
          *properties |= TrackSpanProperties::HAS_FLEXIBLE_TRACK
            | TrackSpanProperties::IS_DEPENDENT_ON_AVAILABLE_SIZE;
        }
        if size.has_intrinsic_min_track_breadth() || size.has_intrinsic_max_track_breadth() {
          *properties |= TrackSpanProperties::HAS_INTRINSIC_TRACK;
        }
        if !size.is_definite() {
          *properties |= TrackSpanProperties::HAS_NON_DEFINITE_TRACK;
        }
      };

      match range.repeater_index {
        None => {
          let set = GridSet::undefined(range.track_count);

          cache_set_properties(&mut range.properties, &set);
          self.sets.push(set);
        }
        Some(repeater_index) => {
          let track_list = if range.is_implicit() {
            implicit_track_list
          } else {
            explicit_track_list
          };
          let repeater_size = track_list.repeat_size(repeater_index);
          let floor_set_track_count = range.track_count / repeater_size;
          let remaining_track_count = range.track_count % repeater_size;

          for index in 0..range.set_count {
            let set_track_count =
              floor_set_track_count + usize::from(index < remaining_track_count);
            let set_repeater_offset = (range.repeater_offset + index) % repeater_size;
            let track_size = *track_list.repeat_track_size(repeater_index, set_repeater_offset);

            if track_size.has_percentage() {
              range.properties |= TrackSpanProperties::IS_DEPENDENT_ON_AVAILABLE_SIZE;
            }

            let set = GridSet::new(set_track_count, track_size, is_available_size_indefinite);

            cache_set_properties(&mut range.properties, &set);
            self.sets.push(set);
          }
        }
      }

      self.properties |= range.properties;
    }

    self.initialize_sets(available_size);
  }

  /// Blink's `InitializeSets`.
  fn initialize_sets(&mut self, grid_available_size: Option<LayoutUnit>) {
    for set in &mut self.sets {
      let track_size = set.track_size;
      let count = set.track_count as i32;

      if track_size.is_fit_content() {
        let argument = track_size
          .fit_content_track_breadth
          .resolve(grid_available_size)
          .unwrap_or(LayoutUnit::ZERO);

        set.fit_content_limit = Some(argument * count);
      }

      set.growth_limit = if track_size.has_fixed_max_track_breadth() {
        Some(
          track_size
            .max_track_breadth
            .length()
            .resolve(grid_available_size)
            .unwrap_or(LayoutUnit::ZERO)
            * count,
        )
      } else {
        None
      };

      let base_size = if track_size.has_fixed_min_track_breadth() {
        track_size
          .min_track_breadth
          .length()
          .resolve(grid_available_size)
          .unwrap_or(LayoutUnit::ZERO)
          * count
      } else {
        LayoutUnit::ZERO
      };

      set.init_base_size(base_size);
    }
  }
}

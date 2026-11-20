//! Blink's [`GridTrackCollection`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_track_collection.cc):
//! a grid's tracks in ranges of the same definition, each split into sets of tracks sized as one.
//! Blink's `GridLayoutTrackCollection` and `GridSizingTrackCollection` as one type, a subgrid's
//! collection in a subgridded axis being a layout one with no sets to size. Without `grid-lanes`.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::mem::swap;

use bitflags::bitflags;

use crate::{
  layout::{
    grid::grid_data::GridTrackBaselines,
    grid::grid_style::{
      GridStyle, GridTrackList, GridTrackSize, GridTrackSizingDirection, RepeatType, TrackBreadth,
    },
    layout_style::LayoutLength,
    logical_rect::LogicalBoxStrut,
  },
  layout_unit::LayoutUnit,
  sort_key::sort_by_key,
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

    for index in 0..builder.explicit_repeater_count() {
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

  /// The explicit repeaters, none for a subgridded axis that has no parent grid, whose used value
  /// is `none`. <https://drafts.csswg.org/css-grid-2/#subgrid-listing>
  fn explicit_repeater_count(&self) -> usize {
    if self.explicit_tracks.is_subgridded_axis() {
      0
    } else {
      self.explicit_tracks.repeater_count()
    }
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
      sort_by_key(&mut self.start_lines, |line| line.grid_line);
      sort_by_key(&mut self.end_lines, |line| line.grid_line);
    }

    let explicit_repeater_count = self.explicit_repeater_count();
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

      if self.explicit_tracks.is_subgridded_axis() {
        // A subgridded axis with no parent grid sizes its tracks as `auto`.
      } else if let Some(repeater_index) = current_explicit_repeater_index {
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

/// Blink's `GridSizingTrackCollection`, or a subgrid's `GridLayoutTrackCollection` when not
/// [`Self::is_for_sizing`].
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
  /// Blink's `IsForSizing`: the grid sizes these tracks itself.
  is_for_sizing: bool,
  /// The margin, border, and padding of the subgrids these tracks lie in, on each edge, and the
  /// gutter they add, which their items' contributions add.
  accumulated_gutter_size_delta: LayoutUnit,
  accumulated_start_extra_margin: LayoutUnit,
  accumulated_end_extra_margin: LayoutUnit,
}

impl GridTrackCollection {
  pub(crate) fn new(ranges: Vec<GridRange>, direction: GridTrackSizingDirection) -> Self {
    let non_collapsed_track_count = ranges
      .iter()
      .filter(|range| !range.is_collapsed())
      .map(|range| range.track_count)
      .sum();

    Self {
      non_collapsed_track_count,
      ranges,
      is_for_sizing: true,
      ..Self::empty(direction)
    }
  }

  /// The collection of a subgridded axis before its parent grid's tracks give it one.
  pub(crate) fn empty(direction: GridTrackSizingDirection) -> Self {
    Self {
      direction,
      gutter_size: LayoutUnit::ZERO,
      ranges: Vec::new(),
      properties: TrackSpanProperties::empty(),
      sets_geometry: Vec::new(),
      last_indefinite_index: Vec::new(),
      non_collapsed_track_count: 0,
      sets: Vec::new(),
      is_for_sizing: false,
      accumulated_gutter_size_delta: LayoutUnit::ZERO,
      accumulated_start_extra_margin: LayoutUnit::ZERO,
      accumulated_end_extra_margin: LayoutUnit::ZERO,
    }
  }

  /// Blink's `IsForSizing`.
  pub(crate) fn is_for_sizing(&self) -> bool {
    self.is_for_sizing
  }

  /// Blink's `HasIndefiniteSet`.
  pub(crate) fn has_indefinite_set(&self) -> bool {
    self
      .last_indefinite_index
      .last()
      .is_some_and(Option::is_some)
  }

  /// Blink's `StartExtraMargin`: what the subgrids these tracks lie in add before set
  /// `set_index`.
  pub(crate) fn start_extra_margin(&self, set_index: usize) -> LayoutUnit {
    if set_index == 0 {
      self.accumulated_start_extra_margin
    } else {
      self.accumulated_gutter_size_delta / 2
    }
  }

  /// Blink's `EndExtraMargin`: what the subgrids these tracks lie in add before boundary
  /// `set_index`.
  pub(crate) fn end_extra_margin(&self, set_index: usize) -> LayoutUnit {
    if set_index < self.sets_geometry.len().saturating_sub(1) {
      self.accumulated_gutter_size_delta / 2
    } else {
      self.accumulated_end_extra_margin
    }
  }

  /// Blink's `CreateSubgridTrackCollection`: the tracks of ranges `begin_range_index..=end_range_index`
  /// as the subgrid in them, `subgrid_direction` its axis, sees them, offsets from its border box
  /// start, with its gutter and its margin, border, and padding on its edges.
  #[expect(clippy::too_many_arguments)]
  pub(crate) fn create_subgrid_track_collection(
    &self,
    begin_range_index: usize,
    end_range_index: usize,
    subgrid_gutter_size: LayoutUnit,
    subgrid_margin: LogicalBoxStrut,
    subgrid_border_padding: LogicalBoxStrut,
    subgrid_direction: GridTrackSizingDirection,
    is_opposite_direction_in_root_grid: bool,
  ) -> Self {
    let mut subgrid_collection = Self::empty(subgrid_direction);
    let begin_set_index = self.ranges[begin_range_index].begin_set_index;
    let end_set_index =
      self.ranges[end_range_index].begin_set_index + self.ranges[end_range_index].set_count;
    let range_count = end_range_index - begin_range_index;
    let mut current_begin_set_index = 0;
    let mut current_start_line = 0;

    for index in 0..=range_count {
      let current_index = if is_opposite_direction_in_root_grid {
        end_range_index - index
      } else {
        begin_range_index + index
      };
      let mut range = self.ranges[current_index];

      range.begin_set_index = current_begin_set_index;
      current_begin_set_index += range.set_count;
      range.start_line = current_start_line;
      current_start_line += range.track_count;
      subgrid_collection.properties |= range.properties;
      if !range.is_collapsed() {
        subgrid_collection.non_collapsed_track_count += range.track_count;
      }
      subgrid_collection.ranges.push(range);
    }

    let set_span_size = end_set_index - begin_set_index;
    let subgrid_gutter_size_delta = subgrid_gutter_size - self.gutter_size;
    let is_for_columns = subgrid_direction == GridTrackSizingDirection::ForColumns;
    let (margin_start, margin_end, border_padding_start, border_padding_end) = if is_for_columns {
      (
        subgrid_margin.inline_start,
        subgrid_margin.inline_end,
        subgrid_border_padding.inline_start,
        subgrid_border_padding.inline_end,
      )
    } else {
      (
        subgrid_margin.block_start,
        subgrid_margin.block_end,
        subgrid_border_padding.block_start,
        subgrid_border_padding.block_end,
      )
    };
    let margin_border_padding_start = margin_start + border_padding_start;
    let margin_border_padding_end = margin_end + border_padding_end;
    let parent_start_extra_margin = self.start_extra_margin(begin_set_index);
    let parent_end_extra_margin = self.end_extra_margin(end_set_index);

    subgrid_collection.accumulated_gutter_size_delta =
      subgrid_gutter_size_delta + self.accumulated_gutter_size_delta;
    (
      subgrid_collection.accumulated_start_extra_margin,
      subgrid_collection.accumulated_end_extra_margin,
    ) = if is_opposite_direction_in_root_grid {
      (
        margin_border_padding_start + parent_end_extra_margin,
        margin_border_padding_end + parent_start_extra_margin,
      )
    } else {
      (
        margin_border_padding_start + parent_start_extra_margin,
        margin_border_padding_end + parent_end_extra_margin,
      )
    };

    let first_set_offset = if is_opposite_direction_in_root_grid {
      self.sets_geometry[end_set_index].offset - margin_start
    } else {
      self.sets_geometry[begin_set_index].offset + margin_start
    };
    let geometry_at = |index: usize| {
      let mut geometry = self.sets_geometry[index];

      if is_opposite_direction_in_root_grid {
        // An opposite subgrid takes a boundary's offset, and the track count of the set after it.
        geometry.offset = first_set_offset - geometry.offset;
        geometry.track_count = self.sets_geometry[index + 1].track_count;
      } else {
        geometry.offset -= first_set_offset;
      }
      geometry
    };

    subgrid_collection.sets_geometry.push(SetGeometry {
      offset: border_padding_start,
      track_count: 0,
    });
    for index in 1..set_span_size {
      let mut geometry = geometry_at(if is_opposite_direction_in_root_grid {
        end_set_index - index
      } else {
        begin_set_index + index
      });

      geometry.offset += subgrid_gutter_size_delta / 2;
      subgrid_collection.sets_geometry.push(geometry);
    }

    let mut last_geometry = geometry_at(if is_opposite_direction_in_root_grid {
      begin_set_index
    } else {
      end_set_index
    });

    last_geometry.offset += subgrid_gutter_size_delta - margin_border_padding_end;
    subgrid_collection.sets_geometry.push(last_geometry);

    if !self.last_indefinite_index.is_empty() {
      let mut last_indefinite_index = None;

      subgrid_collection.last_indefinite_index.push(None);
      for index in 0..set_span_size {
        let current_index = if is_opposite_direction_in_root_grid {
          end_set_index - index - 1
        } else {
          begin_set_index + index
        };

        if self.last_indefinite_index[current_index + 1]
          != self.last_indefinite_index[current_index]
        {
          last_indefinite_index = Some(index);
        }
        subgrid_collection
          .last_indefinite_index
          .push(last_indefinite_index);
      }
    }

    subgrid_collection.gutter_size = subgrid_gutter_size;
    subgrid_collection
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

  /// Blink's `CreateSubgridBaselines`: the baselines of the sets of ranges
  /// `begin_range_index..=end_range_index` as the subgrid in them sees them, from its border box
  /// start, a subgrid running opposite to its root grid taking the minor ones for its major ones.
  #[expect(clippy::too_many_arguments)]
  pub(crate) fn create_subgrid_baselines(
    &self,
    begin_range_index: usize,
    end_range_index: usize,
    subgrid_gutter_size: LayoutUnit,
    subgrid_margin: LogicalBoxStrut,
    subgrid_border_padding: LogicalBoxStrut,
    subgrid_direction: GridTrackSizingDirection,
    is_opposite_direction_in_root_grid: bool,
    parent_baselines: &GridTrackBaselines,
  ) -> GridTrackBaselines {
    let begin_set_index = self.ranges[begin_range_index].begin_set_index;
    let end_set_index =
      self.ranges[end_range_index].begin_set_index + self.ranges[end_range_index].set_count;
    let set_span_size = end_set_index - begin_set_index;
    let subgrid_gutter_size_delta = subgrid_gutter_size - self.gutter_size;
    let (margin_border_padding_start, margin_border_padding_end) = match subgrid_direction {
      GridTrackSizingDirection::ForColumns => (
        subgrid_margin.inline_start + subgrid_border_padding.inline_start,
        subgrid_margin.inline_end + subgrid_border_padding.inline_end,
      ),
      GridTrackSizingDirection::ForRows => (
        subgrid_margin.block_start + subgrid_border_padding.block_start,
        subgrid_margin.block_end + subgrid_border_padding.block_end,
      ),
    };
    let adjust = |baselines: &[Option<LayoutUnit>], index: usize, adjust: LayoutUnit| {
      baselines
        .get(index)
        .copied()
        .flatten()
        .map(|baseline| baseline - adjust)
    };
    let mut subgrid_baselines = GridTrackBaselines::default();

    for index in 0..set_span_size {
      let mut major_adjust = if index == 0 {
        margin_border_padding_start
      } else {
        subgrid_gutter_size_delta / 2
      };
      let mut minor_adjust = if index == set_span_size - 1 {
        margin_border_padding_end
      } else {
        subgrid_gutter_size_delta / 2
      };

      if is_opposite_direction_in_root_grid {
        swap(&mut major_adjust, &mut minor_adjust);
      }

      let current_index = if is_opposite_direction_in_root_grid {
        end_set_index - index - 1
      } else {
        begin_set_index + index
      };

      subgrid_baselines
        .major
        .push(adjust(&parent_baselines.major, current_index, major_adjust));
      subgrid_baselines
        .minor
        .push(adjust(&parent_baselines.minor, current_index, minor_adjust));
    }
    if is_opposite_direction_in_root_grid {
      swap(&mut subgrid_baselines.major, &mut subgrid_baselines.minor);
    }
    subgrid_baselines
  }

  /// Blink's `CacheDefiniteSetsGeometry`: the offsets of the sets whose sizes are definite before
  /// sizing, for the subgrids in them to read.
  pub(crate) fn cache_definite_sets_geometry(&mut self) {
    let mut first_set_offset = LayoutUnit::ZERO;

    self.last_indefinite_index = vec![None];
    self.sets_geometry = vec![SetGeometry {
      offset: first_set_offset,
      track_count: 0,
    }];

    for set in &self.sets {
      if set.track_size.is_definite() {
        first_set_offset += set.base_size + self.gutter_size * set.track_count as i32;

        let last = *self.last_indefinite_index.last().unwrap_or(&None);

        self.last_indefinite_index.push(last);
      } else {
        self
          .last_indefinite_index
          .push(Some(self.last_indefinite_index.len() - 1));
      }
      self.sets_geometry.push(SetGeometry {
        offset: first_set_offset,
        track_count: set.track_count,
      });
    }
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

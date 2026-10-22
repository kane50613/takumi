//! Blink's [`GridTrackSizingAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_track_sizing_algorithm.cc):
//! the track sizing algorithm, sets standing for the tracks they hold.
//! <https://drafts.csswg.org/css-grid-2/#algo-track-sizing>
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use crate::{
  layout::{
    content_alignment::{ContentAlignment, ContentDistribution, ContentPosition},
    grid::{
      grid_item::GridItemData,
      grid_style::GridTrackSizingDirection,
      grid_track_collection::{GridSet, GridTrackCollection, TrackSpanProperties},
    },
    layout_style::LayoutStyle,
    logical_rect::{AvailableLogicalSize, LogicalSize},
  },
  layout_unit::LayoutUnit,
};

/// Blink's `GridItemContributionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridItemContributionType {
  IntrinsicMinimums,
  ContentBasedMinimums,
  MaxContentMinimums,
  IntrinsicMaximums,
  MaxContentMaximums,
  FreeSpace,
}

impl GridItemContributionType {
  /// Blink's `IsDistributionForGrowthLimits`.
  fn is_for_growth_limits(self) -> bool {
    matches!(self, Self::IntrinsicMaximums | Self::MaxContentMaximums)
  }
}

/// Blink's `SizingConstraint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SizingConstraint {
  Layout,
  MaxContent,
  MinContent,
}

/// Blink's `FirstSetGeometry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FirstSetGeometry {
  pub(crate) gutter_size: LayoutUnit,
  pub(crate) start_offset: LayoutUnit,
}

/// A grid item's contribution of `contribution_type` to the tracks it spans.
pub(crate) type ContributionSize<'f> =
  dyn FnMut(GridItemContributionType, &mut GridItemData) -> LayoutUnit + 'f;

/// Blink's `kFloatEpsilon`.
const FLOAT_EPSILON: f32 = f32::EPSILON;

fn are_equal(a: f32, b: f32) -> bool {
  (a - b).abs() < FLOAT_EPSILON
}

/// Blink's `DefiniteGrowthLimit`.
fn definite_growth_limit(set: &GridSet) -> LayoutUnit {
  set.growth_limit.unwrap_or(set.base_size)
}

/// Blink's `AffectedSizeForContribution`.
fn affected_size_for_contribution(
  set: &GridSet,
  contribution_type: GridItemContributionType,
) -> LayoutUnit {
  if contribution_type.is_for_growth_limits() {
    definite_growth_limit(set)
  } else {
    set.base_size
  }
}

/// Blink's `GrowAffectedSizeByPlannedIncrease`.
fn grow_affected_size_by_planned_increase(
  contribution_type: GridItemContributionType,
  set: &mut GridSet,
) {
  set.is_infinitely_growable = false;

  let Some(planned_increase) = set.planned_increase else {
    return;
  };

  match contribution_type {
    GridItemContributionType::IntrinsicMinimums
    | GridItemContributionType::ContentBasedMinimums
    | GridItemContributionType::MaxContentMinimums => {
      set.increase_base_size(set.base_size + planned_increase);
    }
    GridItemContributionType::IntrinsicMaximums => {
      set.is_infinitely_growable = set.growth_limit.is_none();
      set.increase_growth_limit(definite_growth_limit(set) + planned_increase);
    }
    GridItemContributionType::MaxContentMaximums => {
      set.increase_growth_limit(definite_growth_limit(set) + planned_increase);
    }
    GridItemContributionType::FreeSpace => {}
  }
}

/// Blink's `IsContributionAppliedToSet`.
fn is_contribution_applied_to_set(
  set: &GridSet,
  contribution_type: GridItemContributionType,
) -> bool {
  let size = &set.track_size;

  match contribution_type {
    GridItemContributionType::IntrinsicMinimums => size.has_intrinsic_min_track_breadth(),
    GridItemContributionType::ContentBasedMinimums => {
      size.has_min_or_max_content_min_track_breadth()
    }
    GridItemContributionType::MaxContentMinimums => size.has_max_content_min_track_breadth(),
    GridItemContributionType::IntrinsicMaximums => size.has_intrinsic_max_track_breadth(),
    GridItemContributionType::MaxContentMaximums => {
      size.has_max_content_or_auto_max_track_breadth()
    }
    GridItemContributionType::FreeSpace => true,
  }
}

/// Blink's `ShouldUsedSizeGrowBeyondLimit`.
fn should_used_size_grow_beyond_limit(
  set: &GridSet,
  contribution_type: GridItemContributionType,
) -> bool {
  match contribution_type {
    GridItemContributionType::IntrinsicMinimums
    | GridItemContributionType::ContentBasedMinimums => {
      set.track_size.has_intrinsic_max_track_breadth()
    }
    GridItemContributionType::MaxContentMinimums => {
      set.track_size.has_max_content_or_auto_max_track_breadth()
    }
    _ => false,
  }
}

/// Blink's `GrowthPotentialForSet`, none for an infinite one. `enforce_infinitely_growable`
/// is Blink's `InfinitelyGrowableBehavior::kEnforce`.
fn growth_potential_for_set(
  set: &GridSet,
  contribution_type: GridItemContributionType,
  enforce_infinitely_growable: bool,
) -> Option<LayoutUnit> {
  let item_incurred_increase = set.item_incurred_increase.unwrap_or(LayoutUnit::ZERO);

  match contribution_type {
    GridItemContributionType::IntrinsicMinimums
    | GridItemContributionType::ContentBasedMinimums
    | GridItemContributionType::MaxContentMinimums => {
      let growth_limit = set.growth_limit?;

      Some(growth_limit - (set.base_size + item_incurred_increase))
    }
    GridItemContributionType::IntrinsicMaximums | GridItemContributionType::MaxContentMaximums => {
      if enforce_infinitely_growable && set.growth_limit.is_some() && !set.is_infinitely_growable {
        return Some(LayoutUnit::ZERO);
      }

      set.fit_content_limit.map(|limit| {
        (limit - definite_growth_limit(set) - item_incurred_increase).clamp_negative_to_zero()
      })
    }
    GridItemContributionType::FreeSpace => {
      Some(set.growth_limit.unwrap_or(set.base_size) - set.base_size)
    }
  }
}

/// Whether a growth potential leaves room, Blink testing a `LayoutUnit` for non-zero, which
/// counts an indefinite one.
fn has_potential(potential: Option<LayoutUnit>) -> bool {
  potential != Some(LayoutUnit::ZERO)
}

/// Blink's `DistributeExtraSpaceToSets`, by track count when `flex_factor_sum` is none, else by
/// flex factor. Sets are indices into `sets`.
fn distribute_extra_space_to_sets(
  sets: &mut [GridSet],
  extra_space: Option<LayoutUnit>,
  flex_factor_sum: Option<f32>,
  contribution_type: GridItemContributionType,
  sets_to_grow: &mut [usize],
  sets_to_grow_beyond_limit: Option<&[usize]>,
) {
  let Some(mut extra) = extra_space else {
    for &index in sets_to_grow.iter() {
      sets[index].item_incurred_increase =
        growth_potential_for_set(&sets[index], contribution_type, true);
    }
    return;
  };
  let mut growable_track_count = 0;

  for &index in sets_to_grow.iter() {
    sets[index].item_incurred_increase = Some(LayoutUnit::ZERO);
    if has_potential(growth_potential_for_set(
      &sets[index],
      contribution_type,
      true,
    )) {
      growable_track_count += sets[index].track_count;
    }
  }

  let is_equal_distribution = flex_factor_sum.is_none();
  let mut share_ratio_sum = flex_factor_sum.unwrap_or(growable_track_count as f32);

  if (growable_track_count > 0 || contribution_type.is_for_growth_limits())
    && flex_factor_sum.is_none_or(|sum| are_equal(sum, 0.0))
  {
    // Sets with the least room take their share first; infinite ones last.
    sets_to_grow.sort_by(|&left, &right| {
      let left = growth_potential_for_set(&sets[left], contribution_type, false);
      let right = growth_potential_for_set(&sets[right], contribution_type, false);

      match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(left), Some(right)) => left.cmp(&right),
      }
    });
  }

  let extra_space_share = |set: &GridSet,
                           growth_potential: Option<LayoutUnit>,
                           growable_track_count: &mut usize,
                           share_ratio_sum: &mut f32,
                           extra: &mut LayoutUnit|
   -> LayoutUnit {
    if growth_potential == Some(LayoutUnit::ZERO) {
      return LayoutUnit::ZERO;
    }

    let mut set_track_count = set.track_count;
    let mut set_share_ratio = if is_equal_distribution {
      set_track_count as f32
    } else {
      set.flex_factor()
    };

    if set_share_ratio > *share_ratio_sum {
      set_share_ratio = *share_ratio_sum;
    }

    let mut share = if are_equal(set_share_ratio, *share_ratio_sum) {
      set_track_count = *growable_track_count;
      *extra
    } else if is_equal_distribution {
      LayoutUnit::from_raw_value(
        ((i64::from(extra.raw_value()) * set_share_ratio as i64) / *share_ratio_sum as i64) as i32,
      )
    } else {
      LayoutUnit::from_raw_value(
        ((extra.raw_value() as f32 * set_share_ratio) / *share_ratio_sum) as i32,
      )
    };

    if let Some(potential) = growth_potential {
      share = share.min(potential);
    }

    *growable_track_count = growable_track_count.saturating_sub(set_track_count);
    *share_ratio_sum -= set_share_ratio;
    *extra -= share;
    share
  };

  for &index in sets_to_grow.iter() {
    if growable_track_count == 0 {
      break;
    }

    let potential = growth_potential_for_set(&sets[index], contribution_type, true);
    let share = extra_space_share(
      &sets[index],
      potential,
      &mut growable_track_count,
      &mut share_ratio_sum,
      &mut extra,
    );

    sets[index].item_incurred_increase = Some(share);
  }

  // What is left grows the sets that may grow past their limits.
  if let Some(beyond_limit) = sets_to_grow_beyond_limit
    && extra != LayoutUnit::ZERO
  {
    let beyond_limits_growth_potential = |set: &GridSet| {
      if contribution_type.is_for_growth_limits() {
        growth_potential_for_set(set, contribution_type, false)
      } else {
        None
      }
    };

    for &index in beyond_limit {
      if has_potential(beyond_limits_growth_potential(&sets[index])) {
        growable_track_count += sets[index].track_count;
      }
    }
    share_ratio_sum = growable_track_count as f32;

    for &index in beyond_limit {
      if growable_track_count == 0 {
        break;
      }

      let potential = beyond_limits_growth_potential(&sets[index]);
      let share = extra_space_share(
        &sets[index],
        potential,
        &mut growable_track_count,
        &mut share_ratio_sum,
        &mut extra,
      );

      sets[index].item_incurred_increase = Some(
        sets[index]
          .item_incurred_increase
          .unwrap_or(LayoutUnit::ZERO)
          + share,
      );
    }
  }
}

/// Blink's `GridTrackSizingAlgorithm`.
pub(crate) struct GridTrackSizingAlgorithm {
  available_size: AvailableLogicalSize,
  min_available_size: LogicalSize,
  sizing_constraint: SizingConstraint,
  columns_alignment: ContentAlignment,
  rows_alignment: ContentAlignment,
}

impl GridTrackSizingAlgorithm {
  pub(crate) fn new(
    style: &LayoutStyle,
    available_size: AvailableLogicalSize,
    min_available_size: LogicalSize,
    sizing_constraint: SizingConstraint,
  ) -> Self {
    Self {
      available_size,
      min_available_size,
      sizing_constraint,
      columns_alignment: ContentAlignment::of(style.justify_content),
      rows_alignment: ContentAlignment::of(style.align_content),
    }
  }

  fn available_size(&self, direction: GridTrackSizingDirection) -> Option<LayoutUnit> {
    match direction {
      GridTrackSizingDirection::ForColumns => self.available_size.inline_size,
      GridTrackSizingDirection::ForRows => self.available_size.block_size,
    }
  }

  /// Blink's `CacheGridItemsProperties`: the sets each item spans, and what they hold.
  pub(crate) fn cache_grid_items_properties(
    track_collection: &GridTrackCollection,
    grid_items: &mut [GridItemData],
  ) {
    let direction = track_collection.direction;
    let mut spanning_multiple_ranges = Vec::new();

    for (index, item) in grid_items.iter_mut().enumerate() {
      item.compute_set_indices(track_collection);

      let range_indices = *item.range_indices_mut(direction);
      let properties = match direction {
        GridTrackSizingDirection::ForColumns => &mut item.column_span_properties,
        GridTrackSizingDirection::ForRows => &mut item.row_span_properties,
      };

      *properties = TrackSpanProperties::empty();
      if range_indices.begin == range_indices.end {
        *properties = track_collection.ranges[range_indices.begin.unwrap_or(0)].properties;
      } else {
        spanning_multiple_ranges.push(index);
      }
    }

    if spanning_multiple_ranges.is_empty() {
      return;
    }

    spanning_multiple_ranges.sort_by_key(|&index| grid_items[index].start_line(direction));

    for property in [
      TrackSpanProperties::HAS_FLEXIBLE_TRACK,
      TrackSpanProperties::HAS_INTRINSIC_TRACK,
      TrackSpanProperties::HAS_AUTO_MINIMUM_TRACK,
      TrackSpanProperties::HAS_FIXED_MINIMUM_TRACK,
      TrackSpanProperties::HAS_FIXED_MAXIMUM_TRACK,
    ] {
      let mut current_range_index = 0;
      let range_count = track_collection.range_count();
      let range_end_line = |index: usize| {
        track_collection.ranges[index].start_line + track_collection.ranges[index].track_count
      };

      for &item in &spanning_multiple_ranges {
        let start_line = grid_items[item].start_line(direction);

        while current_range_index < range_count
          && (range_end_line(current_range_index) <= start_line
            || !track_collection.ranges[current_range_index]
              .properties
              .contains(property))
        {
          current_range_index += 1;
        }

        if current_range_index == range_count {
          break;
        }
        if range_end_line(current_range_index) <= grid_items[item].end_line(direction) {
          grid_items[item].set_track_span_property(property, direction);
        }
      }
    }
  }

  /// Blink's `ComputeFirstSetGeometry`: the gutter and first track offset `align-content` or
  /// `justify-content` give the tracks.
  pub(crate) fn compute_first_set_geometry(
    track_collection: &GridTrackCollection,
    style: &LayoutStyle,
    available_size: Option<LayoutUnit>,
    border_padding_start: LayoutUnit,
  ) -> FirstSetGeometry {
    let mut geometry = FirstSetGeometry {
      gutter_size: track_collection.gutter_size,
      start_offset: border_padding_start,
    };
    let Some(available_size) = available_size else {
      return geometry;
    };
    let alignment = ContentAlignment::of(match track_collection.direction {
      GridTrackSizingDirection::ForColumns => style.justify_content,
      GridTrackSizingDirection::ForRows => style.align_content,
    });
    let free_space = || {
      let free_space = available_size - track_collection.total_track_size();

      if alignment.is_safe {
        free_space.clamp_negative_to_zero()
      } else {
        free_space
      }
    };
    let track_count = track_collection.non_collapsed_track_count() as i32;

    match alignment.distribution {
      ContentDistribution::SpaceBetween => {
        let free_space = free_space();

        if track_count >= 2 && free_space >= LayoutUnit::ZERO {
          geometry.gutter_size += free_space / (track_count - 1);
        }
        return geometry;
      }
      ContentDistribution::SpaceAround => {
        let free_space = free_space();

        if free_space < LayoutUnit::ZERO {
          return geometry;
        }
        if track_count < 1 {
          geometry.start_offset += free_space / 2;
          return geometry;
        }

        let track_space = free_space / track_count;

        geometry.start_offset += track_space / 2;
        geometry.gutter_size += track_space;
        return geometry;
      }
      ContentDistribution::SpaceEvenly => {
        let free_space = free_space();

        if free_space < LayoutUnit::ZERO {
          return geometry;
        }

        let track_space = free_space / (track_count + 1);

        geometry.start_offset += track_space;
        geometry.gutter_size += track_space;
        return geometry;
      }
      ContentDistribution::Stretch | ContentDistribution::Default => {}
    }

    match alignment.position {
      ContentPosition::Center => geometry.start_offset += free_space() / 2,
      ContentPosition::End | ContentPosition::FlowEnd => geometry.start_offset += free_space(),
      ContentPosition::Start | ContentPosition::FlowStart | ContentPosition::Normal => {}
    }

    geometry
  }

  /// Blink's `ComputeUsedTrackSizes`.
  pub(crate) fn compute_used_track_sizes(
    &self,
    contribution_size: &mut ContributionSize<'_>,
    track_collection: &mut GridTrackCollection,
    grid_items: &mut [GridItemData],
  ) {
    if track_collection.has_intrinsic_track() {
      self.resolve_intrinsic_track_sizes(contribution_size, track_collection, grid_items);
    }

    track_collection.set_indefinite_growth_limits_to_base_size();
    self.maximize_tracks(track_collection);
    if track_collection.has_flexible_track() {
      self.expand_flexible_tracks(contribution_size, track_collection, grid_items);
    }
    self.stretch_auto_tracks(track_collection);
  }

  /// Blink's `IncreaseTrackSizesToAccommodateGridItems` for the items `group`.
  fn increase_track_sizes_to_accommodate_grid_items(
    &self,
    contribution_size: &mut ContributionSize<'_>,
    group: &[usize],
    grid_items: &mut [GridItemData],
    contribution_type: GridItemContributionType,
    is_group_spanning_flex_track: bool,
    track_collection: &mut GridTrackCollection,
  ) {
    let direction = track_collection.direction;
    let gutter_size = track_collection.gutter_size;
    let sets = &mut track_collection.sets;

    for set in sets.iter_mut() {
      set.planned_increase = None;
    }

    let mut sets_to_grow = Vec::new();
    let mut sets_to_grow_beyond_limit = Vec::new();

    for &item_index in group {
      let item = &mut grid_items[item_index];
      let (begin, end) = item.set_indices(direction);
      let mut flex_factor_sum = 0.0_f32;
      let mut spanned_tracks_size = gutter_size * (item.span_size(direction) as i32 - 1);

      sets_to_grow.clear();
      sets_to_grow_beyond_limit.clear();

      for (index, set) in sets.iter_mut().enumerate().take(end).skip(begin) {
        spanned_tracks_size += affected_size_for_contribution(set, contribution_type);

        if is_group_spanning_flex_track && !set.track_size.has_flex_max_track_breadth() {
          continue;
        }
        if is_contribution_applied_to_set(set, contribution_type) {
          if set.planned_increase.is_none() {
            set.planned_increase = Some(LayoutUnit::ZERO);
          }
          if is_group_spanning_flex_track {
            flex_factor_sum = (flex_factor_sum + set.flex_factor()).min(f32::MAX);
          }
          sets_to_grow.push(index);
          if should_used_size_grow_beyond_limit(set, contribution_type) {
            sets_to_grow_beyond_limit.push(index);
          }
        }
      }

      if sets_to_grow.is_empty() {
        continue;
      }

      let extra_space =
        (contribution_size(contribution_type, item) - spanned_tracks_size).clamp_negative_to_zero();

      if extra_space == LayoutUnit::ZERO {
        continue;
      }

      if !is_group_spanning_flex_track || are_equal(flex_factor_sum, 0.0) {
        let beyond_limit = if sets_to_grow_beyond_limit.is_empty() {
          sets_to_grow.clone()
        } else {
          sets_to_grow_beyond_limit.clone()
        };

        distribute_extra_space_to_sets(
          sets,
          Some(extra_space),
          None,
          contribution_type,
          &mut sets_to_grow,
          Some(&beyond_limit),
        );
      } else {
        distribute_extra_space_to_sets(
          sets,
          Some(extra_space),
          Some(flex_factor_sum),
          contribution_type,
          &mut sets_to_grow,
          None,
        );
      }

      for &index in &sets_to_grow {
        let set = &mut sets[index];

        set.planned_increase = Some(
          set
            .item_incurred_increase
            .unwrap_or(LayoutUnit::ZERO)
            .max(set.planned_increase.unwrap_or(LayoutUnit::ZERO)),
        );
      }
    }

    for set in sets.iter_mut() {
      grow_affected_size_by_planned_increase(contribution_type, set);
    }
  }

  /// Blink's `ResolveIntrinsicTrackSizes`.
  fn resolve_intrinsic_track_sizes(
    &self,
    contribution_size: &mut ContributionSize<'_>,
    track_collection: &mut GridTrackCollection,
    grid_items: &mut [GridItemData],
  ) {
    let direction = track_collection.direction;
    let mut reordered: Vec<usize> = (0..grid_items.len())
      .filter(|&index| grid_items[index].is_spanning_intrinsic_track(direction))
      .collect();

    // Items spanning no flexible track go first, by span.
    reordered.sort_by(|&left, &right| {
      let (left, right) = (&grid_items[left], &grid_items[right]);

      match (
        left.is_spanning_flexible_track(direction),
        right.is_spanning_flexible_track(direction),
      ) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        (true, true) => std::cmp::Ordering::Equal,
        (false, false) => left.span_size(direction).cmp(&right.span_size(direction)),
      }
    });

    let mut group_begin = 0;

    while group_begin < reordered.len()
      && !grid_items[reordered[group_begin]].is_spanning_flexible_track(direction)
    {
      let span_size = grid_items[reordered[group_begin]].span_size(direction);
      let mut group_end = group_begin + 1;

      while group_end < reordered.len()
        && !grid_items[reordered[group_end]].is_spanning_flexible_track(direction)
        && grid_items[reordered[group_end]].span_size(direction) == span_size
      {
        group_end += 1;
      }

      for contribution_type in [
        GridItemContributionType::IntrinsicMinimums,
        GridItemContributionType::ContentBasedMinimums,
        GridItemContributionType::MaxContentMinimums,
        GridItemContributionType::IntrinsicMaximums,
        GridItemContributionType::MaxContentMaximums,
      ] {
        self.increase_track_sizes_to_accommodate_grid_items(
          contribution_size,
          &reordered[group_begin..group_end],
          grid_items,
          contribution_type,
          false,
          track_collection,
        );
      }
      group_begin = group_end;
    }

    if group_begin != reordered.len() {
      for contribution_type in [
        GridItemContributionType::IntrinsicMinimums,
        GridItemContributionType::ContentBasedMinimums,
        GridItemContributionType::MaxContentMinimums,
      ] {
        self.increase_track_sizes_to_accommodate_grid_items(
          contribution_size,
          &reordered[group_begin..],
          grid_items,
          contribution_type,
          true,
          track_collection,
        );
      }
    }
  }

  /// Blink's `MaximizeTracks`.
  fn maximize_tracks(&self, track_collection: &mut GridTrackCollection) {
    let free_space = self.determine_free_space(track_collection);

    if free_space == Some(LayoutUnit::ZERO) {
      return;
    }

    let mut sets_to_grow: Vec<usize> = (0..track_collection.sets.len()).collect();

    distribute_extra_space_to_sets(
      &mut track_collection.sets,
      free_space,
      None,
      GridItemContributionType::FreeSpace,
      &mut sets_to_grow,
      None,
    );

    for set in &mut track_collection.sets {
      let increase = set.item_incurred_increase.unwrap_or(LayoutUnit::ZERO);

      set.increase_base_size(set.base_size + increase);
    }
  }

  /// Blink's `StretchAutoTracks`.
  fn stretch_auto_tracks(&self, track_collection: &mut GridTrackCollection) {
    let is_for_columns = track_collection.direction == GridTrackSizingDirection::ForColumns;
    let alignment = if is_for_columns {
      self.columns_alignment
    } else {
      self.rows_alignment
    };

    if alignment.distribution != ContentDistribution::Stretch
      && (alignment.distribution != ContentDistribution::Default
        || alignment.position != ContentPosition::Normal)
    {
      return;
    }

    let mut sets_to_grow: Vec<usize> = track_collection
      .sets
      .iter()
      .enumerate()
      .filter(|(_, set)| {
        set.track_size.has_auto_max_track_breadth() && !set.track_size.is_fit_content()
      })
      .map(|(index, _)| index)
      .collect();

    if sets_to_grow.is_empty() {
      return;
    }

    let free_space = self
      .determine_free_space(track_collection)
      .unwrap_or_else(|| {
        let min_available_size = if is_for_columns {
          self.min_available_size.inline_size
        } else {
          self.min_available_size.block_size
        };

        min_available_size - track_collection.total_track_size()
      });

    if free_space <= LayoutUnit::ZERO {
      return;
    }

    let beyond_limit = sets_to_grow.clone();

    distribute_extra_space_to_sets(
      &mut track_collection.sets,
      Some(free_space),
      None,
      GridItemContributionType::FreeSpace,
      &mut sets_to_grow,
      Some(&beyond_limit),
    );

    for &index in &sets_to_grow {
      let set = &mut track_collection.sets[index];
      let increase = set.item_incurred_increase.unwrap_or(LayoutUnit::ZERO);

      set.increase_base_size(set.base_size + increase);
    }
  }

  /// Blink's `FindFrSize` over the sets `begin..end` filling `leftover_space`.
  fn find_fr_size(
    track_collection: &GridTrackCollection,
    begin: usize,
    end: usize,
    mut leftover_space: LayoutUnit,
  ) -> f32 {
    let mut flex_factor_sum = 0.0_f32;
    let mut total_track_count = 0;
    let mut flexible_sets = Vec::new();

    for set in &track_collection.sets[begin..end] {
      if set.track_size.has_flex_max_track_breadth() && !are_equal(set.flex_factor(), 0.0) {
        flex_factor_sum += set.flex_factor();
        flexible_sets.push(set);
      } else {
        leftover_space -= set.base_size;
      }
      total_track_count += set.track_count;
    }

    leftover_space -= track_collection.gutter_size * (total_track_count as i32 - 1);

    if leftover_space < LayoutUnit::ZERO || flexible_sets.is_empty() {
      return 0.0;
    }

    flexible_sets.sort_by(|left, right| {
      let left_ratio = left.base_size.raw_value() as f32 * right.flex_factor();
      let right_ratio = right.base_size.raw_value() as f32 * left.flex_factor();

      right_ratio.total_cmp(&left_ratio)
    });

    let mut current = 0;

    while leftover_space > LayoutUnit::ZERO && current < flexible_sets.len() {
      flex_factor_sum = flex_factor_sum.max(1.0);

      let mut next = current;

      while next < flexible_sets.len()
        && flexible_sets[next].flex_factor() * (leftover_space.raw_value() as f32)
          < flexible_sets[next].base_size.raw_value() as f32 * flex_factor_sum
      {
        next += 1;
      }

      if current == next {
        return leftover_space.raw_value() as f32 / flex_factor_sum;
      }

      for set in &flexible_sets[current..next] {
        flex_factor_sum -= set.flex_factor();
        leftover_space -= set.base_size;
      }
      current = next;
    }

    0.0
  }

  /// Blink's `ExpandFlexibleTracks`.
  fn expand_flexible_tracks(
    &self,
    contribution_size: &mut ContributionSize<'_>,
    track_collection: &mut GridTrackCollection,
    grid_items: &mut [GridItemData],
  ) {
    let free_space = self.determine_free_space(track_collection);

    if free_space == Some(LayoutUnit::ZERO) {
      return;
    }

    let direction = track_collection.direction;
    let set_count = track_collection.sets.len();
    let mut fr_size = 0.0_f32;

    if free_space.is_some() {
      fr_size = Self::find_fr_size(
        track_collection,
        0,
        set_count,
        self.available_size(direction).unwrap_or(LayoutUnit::ZERO),
      );
    } else {
      for item in grid_items.iter_mut() {
        if !item.is_spanning_flexible_track(direction) {
          continue;
        }

        let contribution = contribution_size(GridItemContributionType::MaxContentMaximums, item);
        let (begin, end) = item.set_indices(direction);

        fr_size = fr_size.max(Self::find_fr_size(
          track_collection,
          begin,
          end,
          contribution,
        ));
      }

      for set in &track_collection.sets {
        if !set.track_size.has_flex_max_track_breadth() {
          continue;
        }

        let set_flex_factor = set.flex_factor().max(set.track_count as f32);

        fr_size = fr_size.max(set.base_size.raw_value() as f32 / set_flex_factor);
      }
    }

    let mut leftover_size = 0.0_f32;

    for set in &mut track_collection.sets {
      if !set.track_size.has_flex_max_track_breadth() {
        continue;
      }

      let fr_share =
        (fr_size * set.flex_factor() + leftover_size).clamp(i32::MIN as f32, i32::MAX as f32);
      let expanded_size = LayoutUnit::from_raw_value((fr_share + FLOAT_EPSILON) as i32);

      if !expanded_size.might_be_saturated() && expanded_size >= set.base_size {
        set.increase_base_size(expanded_size);
        leftover_size = (fr_share - expanded_size.raw_value() as f32).max(0.0);
      }
    }
  }

  /// Blink's `DetermineFreeSpace`, none where infinite.
  fn determine_free_space(&self, track_collection: &GridTrackCollection) -> Option<LayoutUnit> {
    let is_for_columns = track_collection.direction == GridTrackSizingDirection::ForColumns;
    let constraint = if is_for_columns {
      self.sizing_constraint
    } else {
      SizingConstraint::Layout
    };

    match constraint {
      SizingConstraint::Layout => self
        .available_size(track_collection.direction)
        .map(|size| (size - track_collection.total_track_size()).clamp_negative_to_zero()),
      SizingConstraint::MaxContent => None,
      SizingConstraint::MinContent => Some(LayoutUnit::ZERO),
    }
  }
}

/// Blink's `CalculateGutterSize`: `column-gap` or `row-gap` against the content box.
pub(crate) fn calculate_gutter_size(
  style: &LayoutStyle,
  available_size: AvailableLogicalSize,
  direction: GridTrackSizingDirection,
) -> LayoutUnit {
  let (gap, basis) = match direction {
    GridTrackSizingDirection::ForColumns => (style.column_gap, available_size.inline_size),
    GridTrackSizingDirection::ForRows => (style.row_gap, available_size.block_size),
  };

  gap
    .resolve(Some(basis.unwrap_or(LayoutUnit::ZERO)))
    .unwrap_or(LayoutUnit::ZERO)
}

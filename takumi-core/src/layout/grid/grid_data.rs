//! Blink's [`GridLayoutData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_data.h):
//! a grid's track collections and the baselines its tracks share. Every item's baseline group is
//! the major one, so a track keeps no minor baseline.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use crate::{
  layout::grid::{
    grid_style::GridTrackSizingDirection, grid_track_collection::GridTrackCollection,
  },
  layout_unit::LayoutUnit,
};

/// Blink's `GridLayoutData`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridLayoutData {
  pub(crate) columns: GridTrackCollection,
  pub(crate) rows: GridTrackCollection,
  /// Blink's `GridTrackBaselines::major` by set, none for a set no item aligns a baseline in.
  column_baselines: Option<Vec<Option<LayoutUnit>>>,
  row_baselines: Option<Vec<Option<LayoutUnit>>>,
}

impl GridLayoutData {
  pub(crate) fn new(columns: GridTrackCollection, rows: GridTrackCollection) -> Self {
    Self {
      columns,
      rows,
      column_baselines: None,
      row_baselines: None,
    }
  }

  pub(crate) fn collection(&self, direction: GridTrackSizingDirection) -> &GridTrackCollection {
    match direction {
      GridTrackSizingDirection::ForColumns => &self.columns,
      GridTrackSizingDirection::ForRows => &self.rows,
    }
  }

  pub(crate) fn collection_mut(
    &mut self,
    direction: GridTrackSizingDirection,
  ) -> &mut GridTrackCollection {
    match direction {
      GridTrackSizingDirection::ForColumns => &mut self.columns,
      GridTrackSizingDirection::ForRows => &mut self.rows,
    }
  }

  fn baselines(&self, direction: GridTrackSizingDirection) -> &Option<Vec<Option<LayoutUnit>>> {
    match direction {
      GridTrackSizingDirection::ForColumns => &self.column_baselines,
      GridTrackSizingDirection::ForRows => &self.row_baselines,
    }
  }

  fn baselines_mut(
    &mut self,
    direction: GridTrackSizingDirection,
  ) -> &mut Option<Vec<Option<LayoutUnit>>> {
    match direction {
      GridTrackSizingDirection::ForColumns => &mut self.column_baselines,
      GridTrackSizingDirection::ForRows => &mut self.row_baselines,
    }
  }

  pub(crate) fn has_baselines(&self, direction: GridTrackSizingDirection) -> bool {
    self.baselines(direction).is_some()
  }

  pub(crate) fn has_any_baselines(&self) -> bool {
    self.column_baselines.is_some() || self.row_baselines.is_some()
  }

  pub(crate) fn major_baseline(
    &self,
    direction: GridTrackSizingDirection,
    set_index: usize,
  ) -> Option<LayoutUnit> {
    self
      .baselines(direction)
      .as_ref()
      .and_then(|baselines| baselines.get(set_index).copied().flatten())
  }

  pub(crate) fn create_baselines(&mut self, direction: GridTrackSizingDirection) {
    *self.baselines_mut(direction) = Some(Vec::new());
  }

  pub(crate) fn reset_baselines(&mut self, direction: GridTrackSizingDirection, set_count: usize) {
    *self.baselines_mut(direction) = Some(vec![None; set_count]);
  }

  /// Blink's `SetMajorBaseline`: the set's baseline grows to `candidate_baseline`.
  pub(crate) fn set_major_baseline(
    &mut self,
    direction: GridTrackSizingDirection,
    set_index: usize,
    candidate_baseline: LayoutUnit,
  ) {
    if let Some(baseline) = self
      .baselines_mut(direction)
      .as_mut()
      .and_then(|baselines| baselines.get_mut(set_index))
      && baseline.is_none_or(|baseline| candidate_baseline > baseline)
    {
      *baseline = Some(candidate_baseline);
    }
  }
}

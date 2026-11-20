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

impl Default for GridLayoutData {
  /// The layout data of a grid whose collections its sizing tree has yet to build.
  fn default() -> Self {
    Self::new(
      GridTrackCollection::empty(GridTrackSizingDirection::ForColumns),
      GridTrackCollection::empty(GridTrackSizingDirection::ForRows),
    )
  }
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

  /// Blink's `HasSubgriddedAxis`: whether the parent grid's tracks give `direction`'s.
  pub(crate) fn has_subgridded_axis(&self, direction: GridTrackSizingDirection) -> bool {
    !self.collection(direction).is_for_sizing()
  }

  /// Blink's `IsSubgridWithStandaloneAxis`.
  pub(crate) fn is_subgrid_with_standalone_axis(
    &self,
    direction: GridTrackSizingDirection,
  ) -> bool {
    !self.has_subgridded_axis(direction) && self.has_subgridded_axis(direction.opposite())
  }

  /// Blink's `OnlySubgriddedCollection`.
  pub(crate) fn only_subgridded_collection(&self) -> &GridTrackCollection {
    if self.columns.is_for_sizing() {
      &self.rows
    } else {
      &self.columns
    }
  }

  /// Blink's `HasIndefiniteSet`.
  pub(crate) fn has_indefinite_set(&self) -> bool {
    self.columns.has_indefinite_set() || self.rows.has_indefinite_set()
  }

  /// Blink's `SetTrackCollection`.
  pub(crate) fn set_track_collection(&mut self, track_collection: GridTrackCollection) {
    let direction = track_collection.direction;

    *self.collection_mut(direction) = track_collection;
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

  /// Blink's `GetBaselines`, the major ones.
  pub(crate) fn major_baselines(
    &self,
    direction: GridTrackSizingDirection,
  ) -> Option<&[Option<LayoutUnit>]> {
    self.baselines(direction).as_deref()
  }

  /// Blink's `SetBaselines`.
  pub(crate) fn set_major_baselines(
    &mut self,
    direction: GridTrackSizingDirection,
    baselines: Vec<Option<LayoutUnit>>,
  ) {
    *self.baselines_mut(direction) = Some(baselines);
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

//! Blink's [`GridLayoutData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_data.h):
//! a grid's track collections and the baselines its tracks share.
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
  column_baselines: Option<GridTrackBaselines>,
  row_baselines: Option<GridTrackBaselines>,
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

  /// Blink's `GetBaselines`.
  pub(crate) fn baselines(
    &self,
    direction: GridTrackSizingDirection,
  ) -> Option<&GridTrackBaselines> {
    match direction {
      GridTrackSizingDirection::ForColumns => self.column_baselines.as_ref(),
      GridTrackSizingDirection::ForRows => self.row_baselines.as_ref(),
    }
  }

  fn baselines_mut(
    &mut self,
    direction: GridTrackSizingDirection,
  ) -> &mut Option<GridTrackBaselines> {
    match direction {
      GridTrackSizingDirection::ForColumns => &mut self.column_baselines,
      GridTrackSizingDirection::ForRows => &mut self.row_baselines,
    }
  }

  pub(crate) fn has_baselines(&self, direction: GridTrackSizingDirection) -> bool {
    self.baselines(direction).is_some()
  }

  /// Blink's `MajorBaseline`, none where Blink's is `LayoutUnit::Min()`.
  pub(crate) fn major_baseline(
    &self,
    direction: GridTrackSizingDirection,
    set_index: usize,
  ) -> Option<LayoutUnit> {
    self
      .baselines(direction)
      .and_then(|baselines| baselines.major.get(set_index).copied().flatten())
  }

  /// Blink's `MinorBaseline`.
  pub(crate) fn minor_baseline(
    &self,
    direction: GridTrackSizingDirection,
    set_index: usize,
  ) -> Option<LayoutUnit> {
    self
      .baselines(direction)
      .and_then(|baselines| baselines.minor.get(set_index).copied().flatten())
  }

  /// Blink's `SetBaselines`.
  pub(crate) fn set_baselines(
    &mut self,
    direction: GridTrackSizingDirection,
    baselines: GridTrackBaselines,
  ) {
    *self.baselines_mut(direction) = Some(baselines);
  }

  pub(crate) fn create_baselines(&mut self, direction: GridTrackSizingDirection) {
    *self.baselines_mut(direction) = Some(GridTrackBaselines::default());
  }

  pub(crate) fn reset_baselines(&mut self, direction: GridTrackSizingDirection, set_count: usize) {
    *self.baselines_mut(direction) = Some(GridTrackBaselines::new(set_count));
  }

  /// Blink's `SetMajorBaseline`: the set's major baseline grows to `candidate_baseline`.
  pub(crate) fn set_major_baseline(
    &mut self,
    direction: GridTrackSizingDirection,
    set_index: usize,
    candidate_baseline: LayoutUnit,
  ) {
    if let Some(baselines) = self.baselines_mut(direction) {
      grow_baseline(&mut baselines.major, set_index, candidate_baseline);
    }
  }

  /// Blink's `SetMinorBaseline`: the set's minor baseline grows to `candidate_baseline`.
  pub(crate) fn set_minor_baseline(
    &mut self,
    direction: GridTrackSizingDirection,
    set_index: usize,
    candidate_baseline: LayoutUnit,
  ) {
    if let Some(baselines) = self.baselines_mut(direction) {
      grow_baseline(&mut baselines.minor, set_index, candidate_baseline);
    }
  }
}

fn grow_baseline(baselines: &mut [Option<LayoutUnit>], set_index: usize, candidate: LayoutUnit) {
  if let Some(baseline) = baselines.get_mut(set_index)
    && baseline.is_none_or(|baseline| candidate > baseline)
  {
    *baseline = Some(candidate);
  }
}

/// Blink's `GridTrackBaselines`: each set's baseline of its major and minor baseline-sharing
/// groups, none where no item aligns one. <https://www.w3.org/TR/css-align-3/#baseline-sharing-group>
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GridTrackBaselines {
  pub(crate) major: Vec<Option<LayoutUnit>>,
  pub(crate) minor: Vec<Option<LayoutUnit>>,
}

impl GridTrackBaselines {
  /// Blink's `Reset`.
  pub(crate) fn new(set_count: usize) -> Self {
    Self {
      major: vec![None; set_count],
      minor: vec![None; set_count],
    }
  }
}

//! What Blink's [`LayoutGrid`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/layout_grid.cc)
//! keeps of a grid across its layouts, on the grid's layout node: its items' placement and, for a
//! subgrid, its [`SubgridMinMaxSizesCache`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/subgrid_min_max_sizes_cache.h).
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::layout::{
  grid::{
    grid_data::GridLayoutData, grid_placement::GridPlacementData,
    grid_track_collection::GridTrackCollection,
  },
  layout_style::LayoutStyle,
  min_max_sizes::MinMaxSizes,
};

/// Blink's `SubgridMinMaxSizesCache`: a subgrid's intrinsic sizes in its standalone axis, valid
/// while the tracks it takes from its parent in the other axis stay the same.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SubgridMinMaxSizesCache {
  opposite_axis_subgridded_tracks: GridTrackCollection,
  cached_min_max_sizes: MinMaxSizes,
}

impl SubgridMinMaxSizesCache {
  pub(crate) fn new(min_max_sizes: MinMaxSizes, layout_data: &GridLayoutData) -> Self {
    Self {
      opposite_axis_subgridded_tracks: layout_data.only_subgridded_collection().clone(),
      cached_min_max_sizes: min_max_sizes,
    }
  }

  /// Blink's `IsValidFor`.
  pub(crate) fn is_valid_for(&self, layout_data: &GridLayoutData) -> bool {
    *layout_data.only_subgridded_collection() == self.opposite_axis_subgridded_tracks
  }
}

/// Blink's `LayoutGrid` caches.
#[derive(Debug, Clone, Default)]
pub(crate) struct LayoutGrid {
  cached_placement_data: Option<Rc<GridPlacementData>>,
  /// The grid's style and its items', which Blink's `StyleDidChange` marks the placement dirty on
  /// a change of.
  placement_styles: Vec<Rc<LayoutStyle>>,
  cached_subgrid_min_max_sizes: Option<SubgridMinMaxSizesCache>,
}

impl LayoutGrid {
  /// Blink's `CachedPlacementData`, none when `HasCachedPlacementData` is false: never placed, or
  /// placed with styles other than `placement_styles`, the grid's and its items' in order.
  pub(crate) fn cached_placement_data(
    &self,
    placement_styles: &[Rc<LayoutStyle>],
  ) -> Option<&Rc<GridPlacementData>> {
    let is_grid_placement_dirty = self.placement_styles.len() != placement_styles.len()
      || self
        .placement_styles
        .iter()
        .zip(placement_styles)
        .any(|(cached, style)| !Rc::ptr_eq(cached, style));

    self
      .cached_placement_data
      .as_ref()
      .filter(|_| !is_grid_placement_dirty)
  }

  /// The placement the grid last kept, whatever styles it read: Blink's `CachedPlacementData` read
  /// where the placement is known not to have changed.
  pub(crate) fn last_placement_data(&self) -> Option<&Rc<GridPlacementData>> {
    self.cached_placement_data.as_ref()
  }

  /// Blink's `SetCachedPlacementData`.
  pub(crate) fn set_cached_placement_data(
    &mut self,
    placement_data: Rc<GridPlacementData>,
    placement_styles: Vec<Rc<LayoutStyle>>,
  ) {
    self.cached_placement_data = Some(placement_data);
    self.placement_styles = placement_styles;
  }

  /// Blink's `CachedSubgridMinMaxSizes`, none when `HasCachedSubgridMinMaxSizes` is false.
  pub(crate) fn cached_subgrid_min_max_sizes(&self) -> Option<MinMaxSizes> {
    self
      .cached_subgrid_min_max_sizes
      .as_ref()
      .map(|cache| cache.cached_min_max_sizes)
  }

  /// Blink's `SetSubgridMinMaxSizesCache`.
  pub(crate) fn set_subgrid_min_max_sizes_cache(
    &mut self,
    min_max_sizes: MinMaxSizes,
    layout_data: &GridLayoutData,
  ) {
    self.cached_subgrid_min_max_sizes =
      Some(SubgridMinMaxSizesCache::new(min_max_sizes, layout_data));
  }

  /// Blink's `ShouldInvalidateSubgridMinMaxSizesCacheFor`.
  pub(crate) fn should_invalidate_subgrid_min_max_sizes_cache_for(
    &self,
    layout_data: &GridLayoutData,
  ) -> bool {
    self
      .cached_subgrid_min_max_sizes
      .as_ref()
      .is_some_and(|cache| !cache.is_valid_for(layout_data))
  }

  /// Blink's `InvalidateSubgridMinMaxSizesCache`.
  pub(crate) fn invalidate_subgrid_min_max_sizes_cache(&mut self) {
    self.cached_subgrid_min_max_sizes = None;
  }
}

//! Blink's [`GridSizingTree`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_sizing_tree.cc)
//! and [`GridLayoutTree`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_data.h):
//! a grid and its nested subgrids in preorder, each node with the size of the subtree it roots, as
//! [`grid_subtree.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/grid_subtree.h)
//! describes. A subtree is its root's index.
//!
//! Approximate: Blink keeps a grid's placement and a subgrid's
//! [`SubgridMinMaxSizesCache`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/grid/subgrid_min_max_sizes_cache.h)
//! on its `LayoutGrid`, which takumi lacks, so a node keeps them for as long as its tree lives,
//! and the layout tree hands a subgrid its placement along with its tracks.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::{
  geometry::NodeId,
  layout::{
    grid::{
      grid_data::GridLayoutData,
      grid_item::{GridItemData, GridItems},
      grid_placement::GridPlacementData,
      grid_style::GridTrackSizingDirection,
      grid_track_collection::GridTrackCollection,
    },
    min_max_sizes::MinMaxSizes,
  },
};

/// Blink's `SubgriddedItemData`: an item of a subgrid, with its data in the subgrid that holds it,
/// whichever ancestor grid sizes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SubgriddedItemData {
  pub(crate) item_index_in_parent: usize,
  pub(crate) parent_grid_index: usize,
}

impl SubgriddedItemData {
  /// The item, in its parent grid's lines.
  pub(crate) fn item<'t>(&self, sizing_tree: &'t GridSizingTree) -> &'t GridItemData {
    &sizing_tree.tree_data[self.parent_grid_index].grid_items[self.item_index_in_parent]
  }

  /// Blink's `ParentLayoutData`.
  pub(crate) fn parent_layout_data<'t>(
    &self,
    sizing_tree: &'t GridSizingTree,
  ) -> &'t GridLayoutData {
    &sizing_tree.tree_data[self.parent_grid_index].layout_data
  }
}

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

  /// Blink's `CachedMinMaxSizes`.
  pub(crate) fn cached_min_max_sizes(&self) -> MinMaxSizes {
    self.cached_min_max_sizes
  }
}

/// Blink's `GridSizingTree::GridTreeNode`.
#[derive(Debug, Clone)]
pub(crate) struct GridTreeNode {
  pub(crate) node: NodeId,
  pub(crate) grid_items: GridItems,
  pub(crate) layout_data: GridLayoutData,
  pub(crate) subtree_size: usize,
  /// Blink's `CachedPlacementData`, its line resolver Blink's `CachedLineResolver`.
  pub(crate) placement_data: GridPlacementData,
  pub(crate) subgrid_min_max_sizes_cache: Option<SubgridMinMaxSizesCache>,
}

/// Blink's `GridSizingTree`.
#[derive(Debug, Default)]
pub(crate) struct GridSizingTree {
  tree_data: Vec<GridTreeNode>,
  subgrid_index_lookup_map: FxHashMap<NodeId, usize>,
  subgridded_item_data_lookup_map: FxHashMap<NodeId, SubgriddedItemData>,
  tree_has_baselines: bool,
  has_subgrid_with_indefinite_standalone_axis: bool,
  has_block_size_dependent_grid_item: bool,
}

impl GridSizingTree {
  /// Blink's `AddToPreorderTraversal`: the index of `node`, placed as `placement_data` says.
  pub(crate) fn add_to_preorder_traversal(
    &mut self,
    node: NodeId,
    placement_data: GridPlacementData,
  ) -> usize {
    let index = self.tree_data.len();

    self.subgrid_index_lookup_map.insert(node, index);
    self.tree_data.push(GridTreeNode {
      node,
      grid_items: GridItems::default(),
      layout_data: GridLayoutData::default(),
      subtree_size: 1,
      placement_data,
      subgrid_min_max_sizes_cache: None,
    });
    index
  }

  /// Blink's `SetSizingNodeData` for the node at `index`, whose subgrids' subtrees follow it: its
  /// subtree's size, and where to look up the items it hands an ancestor to size.
  pub(crate) fn set_sizing_node_data(
    &mut self,
    index: usize,
    grid_items: GridItems,
    layout_data: GridLayoutData,
  ) {
    let has_standalone_columns =
      !layout_data.has_subgridded_axis(GridTrackSizingDirection::ForColumns);
    let has_standalone_rows = !layout_data.has_subgridded_axis(GridTrackSizingDirection::ForRows);
    let mut child_subgrid_index = index + 1;
    let mut subtree_size = 1;

    for (item_index, item) in grid_items.iter().enumerate() {
      if item.is_subgrid() {
        let size = self.tree_data[child_subgrid_index].subtree_size;

        subtree_size += size;
        child_subgrid_index += size;
      }
      if (has_standalone_columns || item.has_subgridded_columns)
        && (has_standalone_rows || item.has_subgridded_rows)
      {
        continue;
      }
      self.subgridded_item_data_lookup_map.insert(
        item.node,
        SubgriddedItemData {
          item_index_in_parent: item_index,
          parent_grid_index: index,
        },
      );
    }

    if layout_data.has_baselines(GridTrackSizingDirection::ForColumns)
      || layout_data.has_baselines(GridTrackSizingDirection::ForRows)
    {
      self.tree_has_baselines = true;
    }

    let tree_node = &mut self.tree_data[index];

    tree_node.subtree_size = subtree_size;
    tree_node.grid_items = grid_items;
    tree_node.layout_data = layout_data;
  }

  pub(crate) fn node(&self, index: usize) -> &GridTreeNode {
    &self.tree_data[index]
  }

  pub(crate) fn node_mut(&mut self, index: usize) -> &mut GridTreeNode {
    &mut self.tree_data[index]
  }

  /// The indices of the subtrees the subtree at `index` holds, in its items' order: Blink's
  /// `FirstChild` and `NextSibling`.
  pub(crate) fn children(&self, index: usize) -> Vec<usize> {
    let end = index + self.tree_data[index].subtree_size;
    let mut children = Vec::new();
    let mut child = index + 1;

    while child < end {
      children.push(child);
      child += self.tree_data[child].subtree_size;
    }
    children
  }

  /// Blink's `LookupSubgriddedItemData`.
  pub(crate) fn lookup_subgridded_item_data(&self, item: &GridItemData) -> SubgriddedItemData {
    self.subgridded_item_data_lookup_map[&item.node]
  }

  /// Blink's `LookupSubgridIndex`.
  pub(crate) fn lookup_subgrid_index(&self, node: NodeId) -> usize {
    self.subgrid_index_lookup_map[&node]
  }

  /// Blink's `HasBaselines`, which counts the subgrids too.
  pub(crate) fn has_baselines(&self) -> bool {
    self.tree_has_baselines
  }

  pub(crate) fn has_subgrid_with_indefinite_standalone_axis(&self) -> bool {
    self.has_subgrid_with_indefinite_standalone_axis
  }

  pub(crate) fn set_subgrid_has_indefinite_standalone_axis(&mut self) {
    self.has_subgrid_with_indefinite_standalone_axis = true;
  }

  /// Blink's `HasBlockSizeDependentGridItem`, any item at any depth.
  pub(crate) fn has_block_size_dependent_grid_item(&self) -> bool {
    self.has_block_size_dependent_grid_item
  }

  pub(crate) fn set_has_block_size_dependent_grid_item(&mut self) {
    self.has_block_size_dependent_grid_item = true;
  }

  /// Blink's `FinalizeTree`.
  pub(crate) fn finalize_tree(&self) -> Rc<GridLayoutTree> {
    Rc::new(self.finalize_subtree_at(0))
  }

  /// Blink's `FinalizeSubtreeAt`: the layout data of the subtree at `subtree_root`, itself at
  /// index zero.
  pub(crate) fn finalize_subtree_at(&self, subtree_root: usize) -> GridLayoutTree {
    let subtree_size = self.tree_data[subtree_root].subtree_size;
    let mut tree_data: Vec<GridLayoutTreeNode> = self.tree_data
      [subtree_root..subtree_root + subtree_size]
      .iter()
      .map(|node| GridLayoutTreeNode {
        has_unresolved_geometry: node.layout_data.has_indefinite_set(),
        layout_data: node.layout_data.clone(),
        subtree_size: node.subtree_size,
        placement_data: node.placement_data.clone(),
      })
      .collect();

    // A subtree is unresolved when any node in it is.
    for index in (0..subtree_size).rev() {
      if tree_data[index].has_unresolved_geometry || tree_data[index].subtree_size == 1 {
        continue;
      }

      let next_subtree_index = index + tree_data[index].subtree_size;
      let mut child = index + 1;

      while child < next_subtree_index && !tree_data[index].has_unresolved_geometry {
        tree_data[index].has_unresolved_geometry = tree_data[child].has_unresolved_geometry;
        child += tree_data[child].subtree_size;
      }
    }
    GridLayoutTree { tree_data }
  }
}

/// Blink's `GridLayoutTree::GridTreeNode`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridLayoutTreeNode {
  pub(crate) has_unresolved_geometry: bool,
  pub(crate) layout_data: GridLayoutData,
  pub(crate) subtree_size: usize,
  pub(crate) placement_data: GridPlacementData,
}

/// Blink's `GridLayoutTree`: the finalized tracks of a grid and its subgrids, which a subgrid lays
/// out in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GridLayoutTree {
  tree_data: Vec<GridLayoutTreeNode>,
}

/// Blink's `GridLayoutSubtree`: the subtree of a layout tree a subgrid lays out with, which its
/// constraint space hands it.
#[derive(Debug, Clone)]
pub(crate) struct GridLayoutSubtree {
  layout_tree: Rc<GridLayoutTree>,
  subtree_root: usize,
}

impl PartialEq for GridLayoutSubtree {
  /// Blink's `operator==`, which compares the subtrees' nodes, so a subgrid's kept layout serves
  /// only the tracks it laid out in.
  fn eq(&self, other: &Self) -> bool {
    self.nodes() == other.nodes()
  }
}

impl GridLayoutSubtree {
  pub(crate) fn new(layout_tree: Rc<GridLayoutTree>) -> Self {
    Self {
      layout_tree,
      subtree_root: 0,
    }
  }

  fn nodes(&self) -> &[GridLayoutTreeNode] {
    let size = self.root().subtree_size;

    &self.layout_tree.tree_data[self.subtree_root..self.subtree_root + size]
  }

  fn root(&self) -> &GridLayoutTreeNode {
    &self.layout_tree.tree_data[self.subtree_root]
  }

  /// Blink's `FirstChild` and `NextSibling`: the subtrees of the subgrids of this one's root, in
  /// its items' order.
  pub(crate) fn children(&self) -> Vec<GridLayoutSubtree> {
    let end = self.subtree_root + self.root().subtree_size;
    let mut children = Vec::new();
    let mut child = self.subtree_root + 1;

    while child < end {
      children.push(Self {
        layout_tree: Rc::clone(&self.layout_tree),
        subtree_root: child,
      });
      child += self.layout_tree.tree_data[child].subtree_size;
    }
    children
  }

  /// Blink's `LayoutData`.
  pub(crate) fn layout_data(&self) -> &GridLayoutData {
    &self.root().layout_data
  }

  /// The placement the root grid's items took while the tree was sized.
  pub(crate) fn placement_data(&self) -> &GridPlacementData {
    &self.root().placement_data
  }
}

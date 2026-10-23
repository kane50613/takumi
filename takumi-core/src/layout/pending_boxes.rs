//! The boxes of a render tree that no page has laid out yet and whose paint an earlier page can
//! still receive.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  context::BoxId,
  geometry::NodeId,
  layout::{paint_reach::PaintReach, tree::LayoutTree},
};

/// An out-of-flow box no page has laid out, and its layout node once the tree has one.
struct PendingPositioned {
  containing_block: Option<BoxId>,
  node: Option<NodeId>,
}

/// What a paged layout has yet to reach, as [`PaintReach`] lists it before any page is laid out.
pub(crate) struct PendingBoxes {
  /// The boxes no bound covers, until they and what they hold are laid out. Their paint is known
  /// then, and a box that repeats on every page is not listed.
  unbounded: FxHashMap<BoxId, Option<NodeId>>,
  /// The out-of-flow boxes no page has laid out. Each paints from its containing block's first
  /// page, or from the first page for the initial containing block, whichever page lays it out.
  positioned: FxHashMap<BoxId, PendingPositioned>,
  /// The blocks that contain a box of [`Self::positioned`].
  containing_blocks: FxHashSet<BoxId>,
  /// Whether a pending box paints from the first page.
  pins_every_page: bool,
  /// How many layout nodes have been looked through for the boxes above.
  scanned_nodes: usize,
}

impl PendingBoxes {
  pub(crate) fn new(reach: &PaintReach) -> Self {
    let mut pending = Self {
      unbounded: reach.unbounded.iter().map(|&id| (id, None)).collect(),
      positioned: reach
        .positioned
        .iter()
        .map(|positioned| {
          (
            positioned.id,
            PendingPositioned {
              containing_block: positioned.containing_block,
              node: None,
            },
          )
        })
        .collect(),
      containing_blocks: FxHashSet::default(),
      pins_every_page: false,
      scanned_nodes: 0,
    };

    pending.summarize();
    pending
  }

  /// Whether a pending box can paint on the first page, so no page is final.
  pub(crate) fn pins_every_page(&self) -> bool {
    self.pins_every_page
  }

  /// Whether the block `id` contains a pending out-of-flow box, which it lays out in a fragment
  /// that places it above the block's first.
  pub(crate) fn is_containing_block(&self, id: BoxId) -> bool {
    self.containing_blocks.contains(&id)
  }

  /// Whether a box has an unknown reach and is not laid out.
  pub(crate) fn has_unbounded(&self) -> bool {
    !self.unbounded.is_empty()
  }

  /// Drops the boxes `tree` has laid out. An unbounded box is dropped once it is laid out and not
  /// in `open`, the boxes with fragments still to come. None means any box may still get more.
  pub(crate) fn update(&mut self, tree: &LayoutTree<'_>, open: Option<&FxHashSet<NodeId>>) {
    if self.unbounded.is_empty() && self.positioned.is_empty() {
      return;
    }

    for index in self.scanned_nodes..tree.node_count() {
      let id = tree.render_node(index).context.box_id();
      let node = Some(NodeId::new(index));

      if let Some(found) = self.unbounded.get_mut(&id) {
        *found = node;
      }
      if let Some(found) = self.positioned.get_mut(&id) {
        found.node = node;
      }
    }
    self.scanned_nodes = tree.node_count();

    let view = tree.view();

    self
      .positioned
      .retain(|_, pending| !pending.node.is_some_and(|node| view.is_laid_out(node)));
    if let Some(open) = open {
      self.unbounded.retain(|_, node| {
        !node.is_some_and(|node| view.is_laid_out(node) && !open.contains(&node))
      });
    }
    self.summarize();
  }

  fn summarize(&mut self) {
    self.containing_blocks = self
      .positioned
      .values()
      .filter_map(|pending| pending.containing_block)
      .collect();
    self.pins_every_page = !self.unbounded.is_empty()
      || self
        .positioned
        .values()
        .any(|pending| pending.containing_block.is_none());
  }
}

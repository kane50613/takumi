//! A layout read through one interface whether it is finished or still laying out pages.

#[cfg(feature = "fragmentation")]
use rustc_hash::{FxHashMap, FxHashSet};
#[cfg(feature = "fragmentation")]
use smallvec::SmallVec;

#[cfg(feature = "fragmentation")]
use crate::layout::tree::{BoxFragment, LayoutNodeState};
use crate::{
  Error, Result,
  geometry::{ComputedLayout, NodeId},
  layout::{
    table::TableFragment,
    tree::{Fragments, LayoutResults, OrderedChild},
  },
};

/// Which boxes each out-of-flow box was hoisted to, as the layout so far says.
#[cfg(feature = "fragmentation")]
#[derive(Default)]
pub(crate) struct HoistedBy(FxHashMap<NodeId, SmallVec<[NodeId; 1]>>);

#[cfg(feature = "fragmentation")]
impl HoistedBy {
  /// Records that `container` positioned `child` as its containing block.
  pub(crate) fn insert(&mut self, container: NodeId, child: NodeId) {
    let containers = self.0.entry(child).or_default();

    if !containers.contains(&container) {
      containers.push(container);
    }
  }

  /// Forgets that `container` positioned `child`.
  pub(crate) fn remove(&mut self, container: NodeId, child: NodeId) {
    let Some(containers) = self.0.get_mut(&child) else {
      return;
    };

    containers.retain(|other| *other != container);
    if containers.is_empty() {
      self.0.remove(&child);
    }
  }

  /// The last box in layout order that positioned `child`, which wins when several did.
  pub(crate) fn containing_block(&self, child: NodeId) -> Option<NodeId> {
    self
      .0
      .get(&child)?
      .iter()
      .copied()
      .max_by_key(|&container| usize::from(container))
  }
}

/// A tree that is still laying out pages, as [`LayoutView::Live`] reads it.
#[cfg(feature = "fragmentation")]
#[derive(Clone, Copy)]
pub struct LiveLayout<'a> {
  pub(in crate::layout) nodes: &'a [LayoutNodeState],
  pub(in crate::layout) tables: &'a FxHashMap<usize, Vec<TableFragment>>,
  pub(in crate::layout) hidden: &'a FxHashSet<usize>,
  pub(in crate::layout) hoisted_by: &'a HoistedBy,
}

/// The methods of [`LayoutResults`] that painting reads, answered for a finished layout or for one
/// still laying out pages. A box the live layout has not placed yet reads as past the end of its
/// parent's children, where the finished layout holds only boxes it placed or hid.
#[derive(Clone, Copy)]
pub enum LayoutView<'a> {
  /// A finished layout.
  Final(&'a LayoutResults),
  /// A layout that has laid out some pages.
  #[cfg(feature = "fragmentation")]
  Live(LiveLayout<'a>),
}

impl<'a> LayoutView<'a> {
  /// Computed layout of a node.
  pub fn layout(self, node_id: NodeId) -> Result<ComputedLayout> {
    self.fragment_layout(node_id, 0)
  }

  /// Computed layout of a node's fragment `fragment_index`.
  pub fn fragment_layout(self, node_id: NodeId, fragment_index: usize) -> Result<ComputedLayout> {
    self
      .parts(node_id)?
      .0
      .get(fragment_index)
      .map(|fragment| fragment.layout)
      .ok_or(Error::InvalidLayoutNode(node_id.into()))
  }

  /// Whether the node's fragment `fragment_index` repeats an earlier one, as a table header does on
  /// each page.
  pub fn is_repeated_fragment(self, node_id: NodeId, fragment_index: usize) -> bool {
    self.parts(node_id).is_ok_and(|(fragments, _)| {
      fragments
        .get(fragment_index)
        .is_some_and(|fragment| fragment.is_repeated)
    })
  }

  /// Each fragment of `node_id` the layout has placed, with which of its container's fragments
  /// holds it.
  pub(crate) fn fragment_containers(
    self,
    node_id: NodeId,
  ) -> impl Iterator<Item = (usize, usize)> + 'a {
    #[cfg(feature = "fragmentation")]
    return self
      .parts(node_id)
      .ok()
      .into_iter()
      .flat_map(|(fragments, _)| fragments.iter().enumerate())
      .filter(move |(_, fragment)| self.is_placed(fragment))
      .map(|(index, fragment)| (index, fragment.container_fragment_index));
    #[cfg(not(feature = "fragmentation"))]
    return self.parts(node_id).ok().map(|_| (0, 0)).into_iter();
  }

  /// Which fragment of `node_id` the parent's fragment `container_fragment_index` holds first,
  /// none when it holds none.
  pub(crate) fn fragment_in(
    self,
    node_id: NodeId,
    container_fragment_index: usize,
  ) -> Option<usize> {
    self.fragments_in(node_id, container_fragment_index).next()
  }

  /// Every fragment of `node_id` the parent's fragment `container_fragment_index` holds: one,
  /// unless the box is an out-of-flow box broken across the pages after its containing block's.
  pub(crate) fn fragments_in(
    self,
    node_id: NodeId,
    container_fragment_index: usize,
  ) -> impl Iterator<Item = usize> + 'a {
    #[cfg(feature = "fragmentation")]
    return self
      .parts(node_id)
      .ok()
      .into_iter()
      .flat_map(move |(fragments, _)| {
        BoxFragment::held_by(fragments, container_fragment_index)
          .filter(move |&index| self.is_placed(&fragments[index]))
      });
    #[cfg(not(feature = "fragmentation"))]
    return (self.parts(node_id).is_ok() && container_fragment_index == 0)
      .then_some(0)
      .into_iter();
  }

  /// Paint-ordered children of a node. A live layout leaves
  /// [`hoisted_cb`](OrderedChild::hoisted_cb) unset: read it with [`Self::hoisted_cb`].
  pub fn box_children(self, node_id: NodeId) -> Result<&'a [OrderedChild]> {
    self.parts(node_id).map(|(_, children)| children)
  }

  /// The box that positioned `child`, a box child of `parent`, when that is not `parent`.
  pub fn hoisted_cb(
    self,
    #[cfg_attr(not(feature = "fragmentation"), allow(unused_variables))] parent: NodeId,
    child: &OrderedChild,
  ) -> Option<NodeId> {
    match self {
      Self::Final(_) => child.hoisted_cb,
      #[cfg(feature = "fragmentation")]
      Self::Live(live) => live
        .hoisted_by
        .containing_block(child.node_id)
        .filter(|&container| container != parent),
    }
  }

  /// The boxes `node_id` positioned as their containing block, parent or not.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn hoisted_children(self, node_id: NodeId) -> impl Iterator<Item = NodeId> + 'a {
    let children: &[NodeId] = match self {
      Self::Final(results) => results
        .hoisted_children
        .get(&node_id)
        .map_or(&[], Vec::as_slice),
      Self::Live(live) => live
        .nodes
        .get(usize::from(node_id))
        .map_or(&[], |node| node.hoisted_children.as_slice()),
    };

    children.iter().copied()
  }

  /// The run of `node_id`'s box children its fragment `fragment_index` holds, for children that
  /// break in order, as a table section's rows do.
  pub(crate) fn ordered_children_in(
    self,
    node_id: NodeId,
    fragment_index: usize,
  ) -> Result<&'a [OrderedChild]> {
    let children = self.box_children(node_id)?;

    #[cfg(feature = "fragmentation")]
    {
      let containers = |child: &OrderedChild| {
        let mut fragments = self.fragment_containers(child.node_id);
        let (_, first) = fragments.next()?;
        let last = fragments.last().map_or(first, |(_, last)| last);

        Some((first, last))
      };
      let start = children
        .partition_point(|child| containers(child).is_some_and(|(_, last)| last < fragment_index));
      let len = children[start..].partition_point(|child| {
        containers(child).is_some_and(|(first, _)| first <= fragment_index)
      });

      Ok(&children[start..start + len])
    }
    #[cfg(not(feature = "fragmentation"))]
    {
      let _ = fragment_index;

      Ok(children)
    }
  }

  /// The paint data of the table `node_id`'s fragment `fragment_index`, when it is one.
  pub fn table_fragment(self, node_id: NodeId, fragment_index: usize) -> Option<&'a TableFragment> {
    let tables = match self {
      Self::Final(results) => &results.tables,
      #[cfg(feature = "fragmentation")]
      Self::Live(live) => live.tables,
    };

    tables.get(&usize::from(node_id))?.get(fragment_index)
  }

  /// Whether layout hid the box `node_id`, as Blink's `IsHiddenForPaint`: a cell in collapsed
  /// columns or empty under `empty-cells: hide`, or a collapsed row.
  pub fn is_hidden_for_paint(self, node_id: NodeId) -> bool {
    let hidden = match self {
      Self::Final(results) => &results.hidden,
      #[cfg(feature = "fragmentation")]
      Self::Live(live) => live.hidden,
    };

    hidden.contains(&usize::from(node_id))
  }

  /// Blink's `IsLineClampClippedFloat`: whether `node_id` is a float its line-clamp container
  /// clips to its content box's block end.
  pub fn is_line_clamp_clipped_float(self, node_id: NodeId) -> bool {
    self
      .parts(node_id)
      .is_ok_and(|(fragments, _)| fragments[0].is_line_clamp_clipped_float)
  }

  /// Whether layout placed or hid the box `node_id`, which a finished layout has done to every box
  /// that has a fragment.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn is_laid_out(self, node_id: NodeId) -> bool {
    self.fragment_containers(node_id).next().is_some()
  }

  /// Each fragment of `node_id`, by fragment index.
  #[cfg(all(test, feature = "fragmentation"))]
  pub(crate) fn fragments(self, node_id: NodeId) -> Result<&'a [BoxFragment]> {
    self
      .parts(node_id)
      .map(|(fragments, _)| fragments.as_slice())
  }

  /// A box's fragments and box children.
  fn parts(self, node_id: NodeId) -> Result<(&'a Fragments, &'a [OrderedChild])> {
    match self {
      Self::Final(results) => results
        .nodes
        .get(usize::from(node_id))
        .map(|node| (&node.fragments, &*node.box_children)),
      #[cfg(feature = "fragmentation")]
      Self::Live(live) => live
        .nodes
        .get(usize::from(node_id))
        .map(|node| (&node.fragments, &*node.box_children)),
    }
    .ok_or(Error::InvalidLayoutNode(node_id.into()))
  }

  /// Whether the fragment is one the layout placed. A live layout still holds the fragment of a box
  /// it has not reached, and one an earlier run of the container placed that a later run did not
  /// place again.
  #[cfg(feature = "fragmentation")]
  fn is_placed(self, fragment: &BoxFragment) -> bool {
    match self {
      Self::Final(_) => true,
      Self::Live(live) => live.holds(fragment),
    }
  }
}

#[cfg(feature = "fragmentation")]
impl LiveLayout<'_> {
  /// Whether the run of the container that made the fragment it names placed `fragment`.
  fn holds(&self, fragment: &BoxFragment) -> bool {
    fragment.is_laid_out
      && (fragment.placed_by.run == 0
        || self
          .nodes
          .get(fragment.placed_by.node as usize)
          .and_then(|container| container.fragments.get(fragment.container_fragment_index))
          .is_some_and(|holder| holder.is_laid_out && holder.run == fragment.placed_by.run))
  }
}

#[cfg(all(test, feature = "fragmentation"))]
mod tests {
  use super::HoistedBy;
  use crate::geometry::NodeId;

  #[test]
  fn the_last_box_to_hoist_a_child_is_its_containing_block() {
    let (inner, outer, child) = (NodeId::new(2), NodeId::new(7), NodeId::new(9));
    let mut hoisted_by = HoistedBy::default();

    hoisted_by.insert(outer, child);
    hoisted_by.insert(inner, child);
    hoisted_by.insert(outer, child);
    assert_eq!(hoisted_by.containing_block(child), Some(outer));

    hoisted_by.remove(outer, child);
    assert_eq!(hoisted_by.containing_block(child), Some(inner));

    hoisted_by.remove(inner, child);
    assert_eq!(hoisted_by.containing_block(child), None);
  }
}

//! What a streamed layout is done with once the pages it sat on are drawn, so a long document's
//! memory stops growing with its page count.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use crate::geometry::NodeId;

/// What layout has stopped reading.
#[derive(Clone, Copy)]
pub(crate) enum Finished {
  /// The cached layouts of the box's fragments before `fragment`, which are never resumed again.
  LayoutsBefore { node: NodeId, fragment: usize },
  /// The box has no fragments to come, so it keeps no layout or intrinsic size.
  Ended(NodeId),
  /// The fragment items of the box's fragment, which the page it sits on has painted.
  Items { node: NodeId, fragment: usize },
}

/// What a streamed layout frees, by the page count that made it free.
#[derive(Default)]
pub(crate) struct Releases {
  /// The pages laid out once the page being laid out is, which stamps what the page logs.
  laid: usize,
  log: VecDeque<(usize, Finished)>,
  /// By box, what waits for it to end.
  held: FxHashMap<NodeId, Vec<(usize, Finished)>>,
  /// By box and fragment, the page count of the newest placement of items not yet released, which
  /// a fragment laid out again before its page is final replaces.
  items_placed: FxHashMap<(NodeId, usize), usize>,
}

impl Releases {
  /// Stamps what is logged from now with `laid`, the pages laid out once this one is.
  pub(crate) fn begin_page(&mut self, laid: usize) {
    self.laid = laid;
  }

  /// The stamp of what is logged now.
  pub(crate) fn laid(&self) -> usize {
    self.laid
  }

  pub(crate) fn push(&mut self, finished: Finished) {
    self.log.push_back((self.laid, finished));
  }

  /// Logs `finished`, which waits for `holder` to end.
  pub(crate) fn push_until_ended(&mut self, holder: NodeId, finished: Finished) {
    self
      .held
      .entry(holder)
      .or_default()
      .push((self.laid, finished));
  }

  /// Logs the items of `node`'s fragment, placed now, which wait for `holder` to end when a
  /// repeated section of it copies them.
  pub(crate) fn push_items(&mut self, node: NodeId, fragment: usize, holder: Option<NodeId>) {
    self.items_placed.insert((node, fragment), self.laid);

    let items = Finished::Items { node, fragment };

    match holder {
      Some(holder) => self.push_until_ended(holder, items),
      None => self.push(items),
    }
  }

  /// Whether the items of the fragment were last placed when `laid` pages were laid out, so that
  /// releasing them now is right.
  pub(crate) fn take_placed_items(&mut self, node: NodeId, fragment: usize, laid: usize) -> bool {
    if self.items_placed.get(&(node, fragment)) != Some(&laid) {
      return false;
    }

    self.items_placed.remove(&(node, fragment));
    true
  }

  /// The next entry that waits for no page after `pages`.
  pub(crate) fn pop_up_to(&mut self, pages: usize) -> Option<(usize, Finished)> {
    if self.log.front()?.0 > pages {
      return None;
    }
    self.log.pop_front()
  }

  /// What waited for `holder` to end.
  pub(crate) fn take_held(&mut self, holder: NodeId) -> Vec<(usize, Finished)> {
    self.held.remove(&holder).unwrap_or_default()
  }
}

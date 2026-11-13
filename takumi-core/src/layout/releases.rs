//! The layout data a streamed layout frees once the pages it sat on are drawn. The source tree and
//! the tree of boxes stay for the whole render.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use crate::geometry::NodeId;

/// What layout has stopped reading.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    self.log_at(self.laid, finished);
  }

  /// Logs `finished` for `laid`, behind what is logged for it or before, so that the log stays in
  /// stamp order for [`Self::pop_up_to`].
  fn log_at(&mut self, laid: usize, finished: Finished) {
    let at = self.log.partition_point(|&(stamp, _)| stamp <= laid);

    self.log.insert(at, (laid, finished));
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

  /// Logs again the items of `node`'s fragment, which wait for `page` to be drawn, or for the page
  /// laid out next when no page holds them yet.
  pub(crate) fn defer_items(&mut self, node: NodeId, fragment: usize, page: Option<usize>) {
    let laid = page.map_or(self.laid + 1, |page| page + 1);

    self.items_placed.insert((node, fragment), laid);
    self.log_at(laid, Finished::Items { node, fragment });
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

#[cfg(test)]
mod tests {
  use super::*;

  // A deferred entry for a later page lets an earlier one logged after it go first.
  #[test]
  fn entries_leave_the_log_in_stamp_order() {
    let mut releases = Releases::default();
    let ended = |node| Finished::Ended(NodeId::new(node));

    releases.begin_page(3);
    releases.defer_items(NodeId::new(1), 0, None);
    releases.defer_items(NodeId::new(2), 0, Some(1));
    releases.push(ended(3));

    assert_eq!(releases.pop_up_to(3).map(|(laid, _)| laid), Some(2));
    assert_eq!(releases.pop_up_to(3), Some((3, ended(3))));
    assert_eq!(releases.pop_up_to(3), None);
  }
}

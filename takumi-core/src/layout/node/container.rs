use std::mem::take;

use crate::layout::node::{Node, NodeKind};

impl Node {
  pub(crate) fn children(&self) -> Option<&[Node]> {
    let NodeKind::Container { children } = &self.kind else {
      return None;
    };

    (!children.is_empty()).then_some(children.as_slice())
  }

  pub(crate) fn take_children(&mut self) -> Option<Box<[Node]>> {
    let NodeKind::Container { children } = &mut self.kind else {
      return None;
    };

    (!children.is_empty()).then(|| take(children).into_boxed_slice())
  }

  /// Drops the subtree iteratively; recursive drop glue overflows the stack on deep trees.
  pub(super) fn drop_children(&mut self) {
    let NodeKind::Container { children } = &mut self.kind else {
      return;
    };

    let mut stack = take(children);
    while let Some(mut child) = stack.pop() {
      if let Some(grandchildren) = child.take_children() {
        stack.extend(grandchildren.into_vec());
      }
    }
  }
}

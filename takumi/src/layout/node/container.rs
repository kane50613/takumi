use std::mem::take;

use serde::{Deserialize, Deserializer};

use crate::layout::node::{Node, NodeKind};

pub(crate) fn deserialize_children<'de, D>(deserializer: D) -> Result<Vec<Node>, D::Error>
where
  D: Deserializer<'de>,
{
  Option::<Vec<Node>>::deserialize(deserializer).map(Option::unwrap_or_default)
}

pub(crate) fn container_children_ref(kind: &NodeKind) -> Option<&[Node]> {
  let NodeKind::Container { children } = kind else {
    return None;
  };

  (!children.is_empty()).then_some(children.as_slice())
}

pub(crate) fn take_container_children(kind: &mut NodeKind) -> Option<Box<[Node]>> {
  let NodeKind::Container { children } = kind else {
    return None;
  };

  (!children.is_empty()).then(|| take(children).into_boxed_slice())
}

pub(crate) fn drop_container_children(kind: &mut NodeKind) {
  let NodeKind::Container { children } = kind else {
    return;
  };

  let mut stack = take(children);
  while let Some(mut child) = stack.pop() {
    if let Some(grandchildren) = child.take_children() {
      stack.extend(grandchildren.into_vec());
    }
  }
}

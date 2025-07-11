//! Container node implementation for the takumi layout system.
//!
//! This module contains the ContainerNode struct which is used to group
//! other nodes and apply layout properties like flexbox layout.

use std::fmt::Debug;

use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{core::GlobalContext, layout::trait_node::Node, style::Style};

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ContainerNode<Nodes: Node<Nodes>> {
  /// The styling properties for this container
  #[serde(default, flatten)]
  pub style: Style,
  /// The child nodes contained within this container
  pub children: Option<Vec<Nodes>>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ContainerNode<Nodes> {
  fn get_children(&self) -> Option<Vec<&Nodes>> {
    self
      .children
      .as_ref()
      .map(|children| children.iter().collect())
  }

  fn get_style(&self) -> &Style {
    &self.style
  }

  fn get_style_mut(&mut self) -> &mut Style {
    &mut self.style
  }

  fn should_hydrate(&self) -> bool {
    self
      .children
      .as_ref()
      .map(|children| children.iter().any(Node::should_hydrate))
      .unwrap_or(false)
  }

  fn hydrate(&self, context: &GlobalContext) {
    let Some(children) = &self.children else {
      return;
    };

    children
      .iter()
      .filter(|child| child.should_hydrate())
      .par_bridge()
      .for_each(|child| child.hydrate(context));
  }

  fn inherit_style_for_children(&mut self) {
    let style = self.get_style().clone();

    let Some(children) = &mut self.children else {
      return;
    };

    for child in children.iter_mut() {
      child.inherit_style(&style);
    }
  }
}

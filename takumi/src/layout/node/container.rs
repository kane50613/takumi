//! Container node implementation for the takumi layout system.
//!
//! This module contains the ContainerNode struct which is used to group
//! other nodes and apply layout properties like flexbox layout.

use std::fmt::Debug;

use serde::Deserialize;

use crate::layout::{
  node::{Node, NodeMetadata, NodeStyleLayers},
  style::Style,
};

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContainerNode<Nodes: Node<Nodes>> {
  /// Shared node metadata.
  #[serde(flatten)]
  pub(crate) metadata: NodeMetadata,
  /// The child nodes contained within this container
  pub(crate) children: Option<Box<[Nodes]>>,
}

impl<Nodes: Node<Nodes>> ContainerNode<Nodes> {
  /// Set the children of the container node.
  pub fn with_children(mut self, children: impl Into<Box<[Nodes]>>) -> Self {
    self.children = Some(children.into());
    self
  }
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ContainerNode<Nodes> {
  fn metadata(&self) -> &NodeMetadata {
    &self.metadata
  }

  fn metadata_mut(&mut self) -> &mut NodeMetadata {
    &mut self.metadata
  }

  fn children_ref(&self) -> Option<&[Nodes]> {
    self.children.as_deref()
  }

  fn take_style_layers(&mut self) -> NodeStyleLayers {
    NodeStyleLayers {
      preset: self.metadata.preset.take(),
      author_tw: self.metadata.tw.take(),
      inline: self.metadata.style.take(),
    }
  }

  fn take_children(&mut self) -> Option<Box<[Nodes]>> {
    self.children.take()
  }

  fn get_style(&self) -> Option<&Style> {
    self.metadata.style.as_ref()
  }
}

impl<Nodes: Node<Nodes>> Default for ContainerNode<Nodes> {
  fn default() -> Self {
    Self {
      metadata: NodeMetadata::default(),
      children: None,
    }
  }
}

// Avoid stack overflow in deep recursive nodes.
impl<Nodes: Node<Nodes>> Drop for ContainerNode<Nodes> {
  fn drop(&mut self) {
    let mut stack = Vec::new();
    if let Some(children) = self.children.take() {
      stack.extend(children.into_vec());
    }
    while let Some(mut child) = stack.pop() {
      if let Some(grandchildren) = child.take_children() {
        stack.extend(grandchildren.into_vec());
      }
    }
  }
}

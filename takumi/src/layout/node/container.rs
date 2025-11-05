//! Container node implementation for the takumi layout system.
//!
//! This module contains the ContainerNode struct which is used to group
//! other nodes and apply layout properties like flexbox layout.

use std::fmt::Debug;

use serde::Deserialize;

use crate::layout::{
  node::Node,
  style::{InheritedStyle, Style, tw::TailwindProperties},
};

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone)]
pub struct ContainerNode<Nodes: Node<Nodes>> {
  /// The styling properties for this container
  pub style: Option<Style>,
  /// The child nodes contained within this container
  pub children: Option<Vec<Nodes>>,
  /// The tailwind properties for this container node
  pub tw: Option<TailwindProperties>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ContainerNode<Nodes> {
  fn get_tailwind_properties(&self) -> Option<&TailwindProperties> {
    self.tw.as_ref()
  }

  fn children_ref(&self) -> Option<&[Nodes]> {
    self.children.as_deref()
  }

  fn create_inherited_style(&mut self, parent_style: &InheritedStyle) -> InheritedStyle {
    let mut style = self.style.take().unwrap_or_default();

    if let Some(tw) = self.tw.as_ref() {
      tw.apply(&mut style);
    }

    style.inherit(parent_style)
  }

  fn take_children(&mut self) -> Option<Vec<Nodes>> {
    self.children.take()
  }

  fn get_style(&self) -> Option<&Style> {
    self.style.as_ref()
  }
}

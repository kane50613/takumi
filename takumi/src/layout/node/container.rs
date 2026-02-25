//! Container node implementation for the takumi layout system.
//!
//! This module contains the ContainerNode struct which is used to group
//! other nodes and apply layout properties like flexbox layout.

use std::fmt::Debug;

use serde::Deserialize;

use crate::layout::{
  node::{Node, NodeStyleLayers},
  style::{Style, tw::TailwindValues},
};

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContainerNode<Nodes: Node<Nodes>> {
  /// The element's tag name
  pub tag_name: Option<Box<str>>,
  /// The element's class name
  pub class_name: Option<Box<str>>,
  /// The element's id
  pub id: Option<Box<str>>,
  /// Default style presets from HTML element type (lowest priority)
  pub preset: Option<Style>,
  /// The styling properties for this container
  pub style: Option<Style>,
  /// The child nodes contained within this container
  pub children: Option<Box<[Nodes]>>,
  /// The tailwind properties for this container node
  pub tw: Option<TailwindValues>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ContainerNode<Nodes> {
  fn tag_name(&self) -> Option<&str> {
    self.tag_name.as_deref()
  }

  fn class_name(&self) -> Option<&str> {
    self.class_name.as_deref()
  }

  fn id(&self) -> Option<&str> {
    self.id.as_deref()
  }

  fn children_ref(&self) -> Option<&[Nodes]> {
    self.children.as_deref()
  }

  fn take_style_layers(&mut self) -> NodeStyleLayers {
    NodeStyleLayers {
      preset: self.preset.take(),
      author_tw: self.tw.take(),
      inline: self.style.take(),
    }
  }

  fn take_children(&mut self) -> Option<Box<[Nodes]>> {
    self.children.take()
  }

  fn get_style(&self) -> Option<&Style> {
    self.style.as_ref()
  }
}

impl<Nodes: Node<Nodes>> Default for ContainerNode<Nodes> {
  fn default() -> Self {
    Self {
      tag_name: None,
      class_name: None,
      id: None,
      preset: None,
      style: None,
      children: None,
      tw: None,
    }
  }
}

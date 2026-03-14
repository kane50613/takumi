//! Container node implementation for the takumi layout system.
//!
//! This module contains the ContainerNode struct which is used to group
//! other nodes and apply layout properties like flexbox layout.

use std::{fmt::Debug, mem::take};

use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

use crate::layout::{
  node::{Node, NodeMetadata, NodeStyleLayers},
  style::{Style, tw::TailwindValues},
};

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone)]
#[serde(
  rename_all = "camelCase",
  bound(deserialize = "Nodes: Deserialize<'de>")
)]
pub struct ContainerNode<Nodes: Node<Nodes>> {
  /// Shared node metadata.
  #[serde(flatten)]
  pub(crate) metadata: NodeMetadata,
  /// The child nodes contained within this container
  #[serde(default, deserialize_with = "deserialize_children")]
  pub(crate) children: Vec<Nodes>,
}

fn deserialize_children<'de, D, Nodes>(deserializer: D) -> Result<Vec<Nodes>, D::Error>
where
  D: Deserializer<'de>,
  Nodes: Deserialize<'de> + Node<Nodes>,
{
  Option::<Vec<Nodes>>::deserialize(deserializer).map(Option::unwrap_or_default)
}

impl<Nodes: Node<Nodes>> ContainerNode<Nodes> {
  /// Set the tag name and return the updated container node.
  pub fn with_tag_name(mut self, tag_name: impl Into<Box<str>>) -> Self {
    self.metadata.tag_name = Some(tag_name.into());
    self
  }

  /// Set the class name and return the updated container node.
  pub fn with_class_name(mut self, class_name: impl Into<Box<str>>) -> Self {
    self.metadata.class_name = Some(class_name.into());
    self
  }

  /// Set the id and return the updated container node.
  pub fn with_id(mut self, id: impl Into<Box<str>>) -> Self {
    self.metadata.id = Some(id.into());
    self
  }

  /// Set the attributes and return the updated container node.
  pub fn with_attributes(mut self, attributes: BTreeMap<Box<str>, Box<str>>) -> Self {
    self.metadata.attributes = Some(attributes);
    self
  }

  /// Set the preset style and return the updated container node.
  pub fn with_preset(mut self, preset: Style) -> Self {
    self.metadata.preset = Some(preset);
    self
  }

  /// Set the inline style and return the updated container node.
  pub fn with_style(mut self, style: Style) -> Self {
    self.metadata.style = Some(style);
    self
  }

  /// Set the Tailwind values and return the updated container node.
  pub fn with_tw(mut self, tw: TailwindValues) -> Self {
    self.metadata.tw = Some(tw);
    self
  }

  /// Set the children of the container node.
  pub fn with_children<T>(mut self, children: impl IntoIterator<Item = T>) -> Self
  where
    T: Into<Nodes>,
  {
    self.children = children.into_iter().map(Into::into).collect();
    self
  }

  /// Append a single child to the container node.
  pub fn with_child(mut self, child: impl Into<Nodes>) -> Self {
    self.children.push(child.into());
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
    (!self.children.is_empty()).then_some(self.children.as_slice())
  }

  fn take_style_layers(&mut self) -> NodeStyleLayers {
    NodeStyleLayers {
      preset: self.metadata.preset.take(),
      author_tw: self.metadata.tw.take(),
      inline: self.metadata.style.take(),
    }
  }

  fn take_children(&mut self) -> Option<Box<[Nodes]>> {
    (!self.children.is_empty()).then(|| take(&mut self.children).into_boxed_slice())
  }

  fn get_style(&self) -> Option<&Style> {
    self.metadata.style.as_ref()
  }
}

impl<Nodes: Node<Nodes>> Default for ContainerNode<Nodes> {
  fn default() -> Self {
    Self {
      metadata: NodeMetadata::default(),
      children: Vec::new(),
    }
  }
}

// Avoid stack overflow in deep recursive nodes.
impl<Nodes: Node<Nodes>> Drop for ContainerNode<Nodes> {
  fn drop(&mut self) {
    let mut stack = take(&mut self.children);
    while let Some(mut child) = stack.pop() {
      if let Some(grandchildren) = child.take_children() {
        stack.extend(grandchildren.into_vec());
      }
    }
  }
}

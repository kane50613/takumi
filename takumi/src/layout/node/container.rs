//! Container node implementation for the takumi layout system.
//!
//! This module contains the ContainerNode struct which is used to group
//! other nodes and apply layout properties like flexbox layout.

use std::fmt::Debug;

use serde::Deserialize;

use crate::layout::{
  Viewport,
  node::Node,
  style::{ComputedStyle, Style, tw::TailwindValues},
};

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone)]
pub struct ContainerNode<Nodes: Node<Nodes>> {
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
  fn children_ref(&self) -> Option<&[Nodes]> {
    self.children.as_deref()
  }

  fn compute_style(&mut self, parent_style: &ComputedStyle, viewport: Viewport) -> ComputedStyle {
    // Start with empty style
    let mut style = Style::default();

    // 1. Apply preset first (lowest priority)
    if let Some(preset) = self.preset.take() {
      style.merge_from(preset);
    }

    // 2. Apply Tailwind (medium priority)
    if let Some(tw) = self.tw.as_ref() {
      tw.apply(&mut style, viewport);
    }

    // 3. Merge inline style last (highest priority)
    if let Some(inline_style) = self.style.take() {
      style.merge_from(inline_style);
    }

    style.compute_style(parent_style)
  }

  fn take_children(&mut self) -> Option<Box<[Nodes]>> {
    self.children.take()
  }

  fn get_style(&self) -> Option<&Style> {
    self.style.as_ref()
  }
}

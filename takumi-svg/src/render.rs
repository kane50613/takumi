//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::collections::HashMap;
use std::rc::Rc;

use taffy::NodeId;
use takumi_core::{
  GlobalContext,
  context::RenderContext,
  error::Result,
  layout::{
    Viewport,
    node::Node,
    style::StyleSheet,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
};

use crate::{Rgba, SvgDocument};

/// Renders a node tree to a vector SVG string.
pub fn render_svg(node: Node, viewport: Viewport, global: &GlobalContext) -> Result<String> {
  let canvas_w = viewport.size.width;
  let canvas_h = viewport.size.height;
  let context = RenderContext::new(
    global,
    viewport,
    HashMap::default(),
    Rc::new(StyleSheet::default()),
    0,
  );
  let root = RenderNode::from_node(&context, node);
  let mut tree = LayoutTree::from_render_node(&root);
  let root_id = tree.root_node_id();
  tree.compute_layout(context.sizing.viewport.into());
  let results = tree.into_results();

  let root_layout = results.layout(root_id)?;
  let width = canvas_w.map_or(root_layout.size.width, |w| w as f32);
  let height = canvas_h.map_or(root_layout.size.height, |h| h as f32);
  let mut doc = SvgDocument::new(width, height);
  emit_node(&root, root_id, &results, 0.0, 0.0, &mut doc);
  Ok(doc.render())
}

/// Emits one node's box decorations and recurses into its children.
fn emit_node(
  node: &RenderNode,
  node_id: NodeId,
  results: &LayoutResults,
  parent_x: f32,
  parent_y: f32,
  doc: &mut SvgDocument,
) {
  let Ok(layout) = results.layout(node_id) else {
    return;
  };
  let x = parent_x + layout.location.x;
  let y = parent_y + layout.location.y;
  let width = layout.size.width;
  let height = layout.size.height;

  let background = node
    .context
    .style
    .background_color
    .resolve(node.context.current_color);
  if background.0[3] != 0 {
    doc.rect(x, y, width, height, Rgba(background.0));
  }

  let Ok(children) = results.box_children(node_id) else {
    return;
  };
  let Some(child_nodes) = node.children.as_deref() else {
    return;
  };
  for child in children {
    if let Some(child_node) = child_nodes.get(child.render_index) {
      emit_node(child_node, child.node_id, results, x, y, doc);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_svg_wrapper_at_viewport_size() {
    let global = GlobalContext::default();
    let svg = render_svg(Node::container([]), Viewport::new((120, 80)), &global).unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("width=\"120\""));
    assert!(svg.contains("height=\"80\""));
    assert!(!svg.contains("base64"));
  }
}

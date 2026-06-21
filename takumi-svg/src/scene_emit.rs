//! Scene-driven SVG emission: walk the backend-agnostic stacking-context scene
//! built by takumi-core — the same paint order, z-index buckets, and out-of-flow
//! hoisting the raster backend consumes — instead of re-deriving them here.
//!
//! Each painted node is placed by its transform relative to its parent frame
//! (`parent⁻¹ · node`), so nesting composes the absolute transform. A pure
//! translation is folded into the draw origin to keep the output compact; a
//! rotation/scale becomes the group's `transform`. Box decorations and content go
//! through the shared [`emit_box_chrome`]/[`emit_own_content`]; children come from
//! the scene's buckets, not a hand-rolled stacking sort.

use std::io;

use takumi_core::{
  layout::{
    style::Affine,
    tree::{LayoutResults, RenderNode},
  },
  scene::{NodePaint, PaintItemKind, StackingContextNode},
};

use crate::{
  IDENTITY, SvgDocument,
  render::{BoxChrome, emit_box_chrome, emit_own_content},
};

pub(crate) fn emit_scene(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  emit_context(root, contexts, 0, IDENTITY, results, doc)
}

fn node_at<'a>(root: &'a RenderNode, path: &[usize]) -> Option<&'a RenderNode> {
  let mut current = root;
  for &index in path {
    current = current.children.as_deref()?.get(index)?;
  }
  Some(current)
}

/// Emits a node's decorations and own content positioned by its transform
/// relative to `parent`, leaving its chrome groups open for the caller to close
/// after the node's children. Returns the chrome and the frame the node's
/// children sit in (`parent · group_transform`): a pure translation is folded into
/// the draw origin so it leaves no group, so the children's frame is the parent's,
/// not the node's.
fn emit_box(
  root: &RenderNode,
  np: &NodePaint,
  parent: Affine,
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<Option<(BoxChrome, Affine)>> {
  let Some(node) = node_at(root, &np.path) else {
    return Ok(None);
  };
  let Ok(layout) = results.layout(np.node_id) else {
    return Ok(None);
  };

  let relative = parent.invert().unwrap_or(IDENTITY) * np.transform;
  let (x, y, group_transform) = if relative.only_translation() {
    let origin = relative.decompose_translation();
    (origin.x, origin.y, IDENTITY)
  } else {
    (0.0, 0.0, relative)
  };

  let chrome = emit_box_chrome(node, layout, x, y, group_transform, doc)?;
  emit_own_content(node, layout, x, y, doc)?;
  Ok(Some((chrome, parent * group_transform)))
}

fn emit_context(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  id: usize,
  parent: Affine,
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some(ctx) = contexts.get(id) else {
    return Ok(());
  };

  // Children sit in the root node's child frame; a synthetic root context keeps
  // the caller's frame.
  let (chrome, child_frame) = match &ctx.root {
    Some(np) => match emit_box(root, np, parent, results, doc)? {
      Some((chrome, frame)) => (Some(chrome), frame),
      None => (None, parent),
    },
    None => (None, parent),
  };

  for bucket in ctx.buckets.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(np) => {
          if let Some((chrome, _)) = emit_box(root, np, child_frame, results, doc)? {
            chrome.close(doc)?;
          }
        }
        PaintItemKind::Context(child) => {
          emit_context(root, contexts, *child, child_frame, results, doc)?;
        }
      }
    }
  }

  if let Some(chrome) = chrome {
    chrome.close(doc)?;
  }
  Ok(())
}

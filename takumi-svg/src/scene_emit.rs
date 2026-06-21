//! Scene-driven SVG emission: walk the backend-agnostic stacking-context scene
//! built by takumi-core — the same paint order, z-index buckets, and out-of-flow
//! hoisting the raster backend consumes — instead of re-deriving them here.
//!
//! Each painted node's box decorations and own content are emitted through the
//! shared [`emit_box_chrome`]/[`emit_own_content`]; its children come from the
//! scene's buckets rather than a hand-rolled stacking sort. This path handles
//! translation-only scenes; trees carrying a CSS `transform` still take the
//! recursive [`emit_node`](crate::render) path (the scene bakes the transform
//! into device space, which the nested `<g>` emission can't consume yet).

use std::io;

use takumi_core::{
  layout::tree::{LayoutResults, RenderNode},
  scene::{NodePaint, PaintItemKind, StackingContextNode},
};

use crate::{
  SvgDocument,
  render::{BoxChrome, emit_box_chrome, emit_own_content},
};

/// True when no painted node carries a CSS `transform`. The scene bakes a node's
/// transform into its device origin, but [`emit_box_chrome`] re-derives the CSS
/// transform from the style, so a transformed node would be placed twice. Those
/// trees still take the recursive [`emit_node`](crate::render) path; everything
/// else can read the scene directly.
pub(crate) fn scene_has_no_css_transform(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  results: &LayoutResults,
) -> bool {
  let untransformed = |np: &NodePaint| {
    let Some(node) = node_at(root, &np.path) else {
      return true;
    };
    let Ok(layout) = results.layout(np.node_id) else {
      return true;
    };
    !node
      .context
      .style
      .has_non_identity_transform(layout.size, &node.context.sizing)
  };

  contexts.iter().all(|ctx| {
    ctx.root.as_ref().is_none_or(&untransformed)
      && ctx.buckets.in_paint_order().iter().all(|bucket| {
        bucket.iter().all(|item| match &item.kind {
          PaintItemKind::Node(np) => untransformed(np),
          PaintItemKind::Context(_) => true,
        })
      })
  })
}

pub(crate) fn emit_scene(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  emit_context(root, contexts, 0, results, doc)
}

fn node_at<'a>(root: &'a RenderNode, path: &[usize]) -> Option<&'a RenderNode> {
  let mut current = root;
  for &index in path {
    current = current.children.as_deref()?.get(index)?;
  }
  Some(current)
}

/// Emits a node's decorations and own content at its device origin, leaving its
/// chrome groups open for the caller to close after the node's children.
fn emit_box(
  root: &RenderNode,
  np: &NodePaint,
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<Option<BoxChrome>> {
  let Some(node) = node_at(root, &np.path) else {
    return Ok(None);
  };
  let Ok(layout) = results.layout(np.node_id) else {
    return Ok(None);
  };
  let origin = np.transform.decompose_translation();
  let chrome = emit_box_chrome(node, layout, origin.x, origin.y, doc)?;
  emit_own_content(node, layout, origin.x, origin.y, doc)?;
  Ok(Some(chrome))
}

fn emit_context(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  id: usize,
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some(ctx) = contexts.get(id) else {
    return Ok(());
  };

  let chrome = match &ctx.root {
    Some(np) => emit_box(root, np, results, doc)?,
    None => None,
  };

  for bucket in ctx.buckets.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(np) => {
          if let Some(chrome) = emit_box(root, np, results, doc)? {
            chrome.close(doc)?;
          }
        }
        PaintItemKind::Context(child) => emit_context(root, contexts, *child, results, doc)?,
      }
    }
  }

  if let Some(chrome) = chrome {
    chrome.close(doc)?;
  }
  Ok(())
}

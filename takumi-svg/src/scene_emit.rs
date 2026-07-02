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

use taffy::NodeId;
use takumi_core::{
  layout::{
    border::BorderProperties,
    style::{Affine, Filter},
    tree::{LayoutResults, RenderNode},
  },
  scene::{NodePaint, PaintItemKind, StackingContextNode},
};

use crate::{
  IDENTITY, SvgDocument,
  render::{
    BoxChrome, border_box_path_data, emit_box_chrome, emit_clip_path_group, emit_mask_group,
    emit_own_content,
  },
};

pub(crate) fn emit_scene(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  results: &LayoutResults,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  emit_context(root, contexts, 0, IDENTITY, results, doc, None)?;
  Ok(())
}

/// Emits the filtered backdrop of a `backdrop-filter` node: the scene's paint
/// order replayed up to (but excluding) this node, run through the node's filter
/// chain, then clipped to its border box and attenuated by its mask/clip-path —
/// the same semantics the raster backend applies (and Chromium's backdrop root).
///
/// SVG has no native backdrop source (SVG 1.1 `BackgroundImage` is dead), so the
/// backdrop is re-emitted vector content, wrapped in the inverse of the current
/// frame to stay in root coordinates.
#[allow(clippy::too_many_arguments)]
fn emit_backdrop(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  results: &LayoutResults,
  doc: &mut SvgDocument,
  node: &RenderNode,
  node_id: NodeId,
  layout: &taffy::Layout,
  frame: Affine,
  x: f32,
  y: f32,
  group_transform: Affine,
) -> io::Result<()> {
  let filters: Vec<Filter> = node
    .context
    .style
    .backdrop_filter
    .iter()
    .filter(|f| !f.is_drop_shadow())
    .copied()
    .collect();
  if filters.is_empty() || layout.size.width <= 0.0 || layout.size.height <= 0.0 {
    return Ok(());
  }

  let outer = (!group_transform.is_identity())
    .then(|| doc.begin_group(group_transform, 1.0, None, None))
    .transpose()?;

  let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
  let clip = doc.clip_path(&border_box_path_data(&border, layout.size, x, y))?;
  let clip_group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;

  let shape_clip = emit_clip_path_group(node, layout.size, x, y, doc)?;
  let mask = emit_mask_group(node, x, y, layout.size.width, layout.size.height, doc)?;

  let filter_ref = doc.filter(
    &filters,
    &node.context.sizing,
    node.context.current_color,
    layout.size,
  )?;
  let filter_group = doc.begin_group(IDENTITY, 1.0, None, filter_ref.as_deref())?;

  // The replay is emitted in root coordinates; cancel the current frame.
  let to_root = frame.invert().unwrap_or(IDENTITY);
  let root_group = (!to_root.is_identity())
    .then(|| doc.begin_group(to_root, 1.0, None, None))
    .transpose()?;

  emit_context(root, contexts, 0, IDENTITY, results, doc, Some(node_id))?;

  if let Some(group) = root_group {
    doc.end_group(group)?;
  }
  doc.end_group(filter_group)?;
  if let Some(group) = mask {
    doc.end_group(group)?;
  }
  if let Some(group) = shape_clip {
    doc.end_group(group)?;
  }
  doc.end_group(clip_group)?;
  if let Some(group) = outer {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits a node's decorations and own content positioned by its transform
/// relative to `parent`, leaving its chrome groups open for the caller to close
/// after the node's children. Returns the chrome and the frame the node's
/// children sit in (`parent · group_transform`): a pure translation is folded into
/// the draw origin so it leaves no group, so the children's frame is the parent's,
/// not the node's.
fn emit_box(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  np: &NodePaint,
  parent: Affine,
  results: &LayoutResults,
  doc: &mut SvgDocument,
  stop_at: Option<NodeId>,
) -> io::Result<Option<(BoxChrome, Affine)>> {
  let Some(node) = root.node_at_path(&np.path) else {
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

  let frame = parent * group_transform;

  // Inside a replay (stop_at set), nested backdrop-filter nodes are emitted
  // without their own backdrop (each level would replay its own prefix, doubling
  // the output per backdrop node in paint order). Stacked backdrop elements
  // therefore see the unfiltered content beneath them in the replay.
  if stop_at.is_none() && !node.context.style.backdrop_filter.is_empty() {
    emit_backdrop(
      root,
      contexts,
      results,
      doc,
      node,
      np.node_id,
      layout,
      frame,
      x,
      y,
      group_transform,
    )?;
  }

  let chrome = emit_box_chrome(node, layout, x, y, group_transform, doc)?;
  emit_own_content(node, layout, x, y, doc)?;
  Ok(Some((chrome, frame)))
}

/// Walks a stacking context in paint order. With `stop_at` set, emission halts
/// (without emitting) at that node — used to replay the backdrop of a
/// `backdrop-filter` node. Returns whether the stop node was reached.
fn emit_context(
  root: &RenderNode,
  contexts: &[StackingContextNode],
  id: usize,
  parent: Affine,
  results: &LayoutResults,
  doc: &mut SvgDocument,
  stop_at: Option<NodeId>,
) -> io::Result<bool> {
  let Some(ctx) = contexts.get(id) else {
    return Ok(false);
  };

  // Children sit in the root node's child frame; a synthetic root context keeps
  // the caller's frame.
  let (chrome, child_frame) = match ctx.root() {
    Some(np) => {
      if stop_at == Some(np.node_id) {
        return Ok(true);
      }
      match emit_box(root, contexts, np, parent, results, doc, stop_at)? {
        Some((chrome, frame)) => (Some(chrome), frame),
        None => (None, parent),
      }
    }
    None => (None, parent),
  };

  let mut stopped = false;
  'buckets: for bucket in ctx.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(np) => {
          if stop_at == Some(np.node_id) {
            stopped = true;
            break 'buckets;
          }
          if let Some((chrome, _)) =
            emit_box(root, contexts, np, child_frame, results, doc, stop_at)?
          {
            chrome.close(doc)?;
          }
        }
        PaintItemKind::Context(child) => {
          if emit_context(root, contexts, *child, child_frame, results, doc, stop_at)? {
            stopped = true;
            break 'buckets;
          }
        }
      }
    }
  }

  if let Some(chrome) = chrome {
    chrome.close(doc)?;
  }
  Ok(stopped)
}

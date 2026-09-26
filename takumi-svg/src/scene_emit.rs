//! Scene-driven SVG emission: walk the backend-agnostic stacking-context scene
//! built by takumi-core — the same paint order, z-index buckets, and out-of-flow
//! hoisting the raster backend consumes — instead of re-deriving them here.
//!
//! Each painted node is placed by its transform relative to its parent frame
//! (`parent⁻¹ · node`), so nesting composes the absolute transform. A pure
//! translation is folded into the draw origin to keep the output compact; a
//! rotation/scale becomes the group's `transform`.

use std::io;

use takumi_core::{
  geometry::{NodeId, Point},
  scene::{NodePaint, PaintItemKind, Scene},
  style::{Affine, Filter},
};

use crate::{
  SvgDocument,
  box_model::BoxFrame,
  render::{BoxChrome, PlacedBox},
};

/// A scene, emitted in paint order.
pub(crate) struct SceneEmitter<'a> {
  pub(crate) scene: &'a Scene,
}

impl SceneEmitter<'_> {
  pub(crate) fn emit(&self, doc: &mut SvgDocument) -> io::Result<()> {
    self.emit_context(0, Affine::IDENTITY, None, doc)?;
    Ok(())
  }

  /// Emits the filtered backdrop of a `backdrop-filter` node: the scene's paint
  /// order replayed up to (but excluding) this node, run through the node's filter
  /// chain, then clipped to its border box and attenuated by its mask/clip-path —
  /// the same semantics the raster backend applies (and Chromium's backdrop root).
  ///
  /// SVG has no native backdrop source (SVG 1.1 `BackgroundImage` is dead), so the
  /// backdrop is re-emitted vector content, wrapped in the inverse of the current
  /// `transform` to stay in root coordinates.
  fn emit_backdrop(
    &self,
    placed: &PlacedBox,
    node_id: NodeId,
    transform: Affine,
    group_transform: Affine,
    doc: &mut SvgDocument,
  ) -> io::Result<()> {
    let context = &placed.node.context;
    let size = placed.frame.layout.size;
    let filters: Vec<Filter> = context
      .style
      .backdrop_filter
      .iter()
      .filter(|f| !f.is_drop_shadow())
      .cloned()
      .collect();
    if filters.is_empty() || size.width <= 0.0 || size.height <= 0.0 {
      return Ok(());
    }

    let outer = (!group_transform.is_identity())
      .then(|| doc.begin_group(group_transform, 1.0, None, None))
      .transpose()?;

    let clip_group = doc.begin_clipped_group(&placed.border_box_path_data())?;

    let shape_clip = placed.begin_clip_path_group(doc)?;
    let mask = placed.begin_mask_group(doc)?;

    // Alpha restore approximates the edge-duplicated backdrop sampling browsers
    // use; skipped for opacity(), which lowers alpha on purpose.
    let restore_alpha = !filters.iter().any(|f| matches!(f, Filter::Opacity(_)));
    let filter_refs = doc.filter(&filters, context, size, restore_alpha)?;
    let filter_wrappers = doc.begin_filter_wrappers(&filter_refs)?;
    let filter_group = doc.begin_group(
      Affine::IDENTITY,
      1.0,
      None,
      filter_refs.first().map(String::as_str),
    )?;

    // The replay is emitted in root coordinates; cancel the current transform.
    let to_root = transform.invert().unwrap_or(Affine::IDENTITY);
    let root_group = (!to_root.is_identity())
      .then(|| doc.begin_group(to_root, 1.0, None, None))
      .transpose()?;

    self.emit_context(0, Affine::IDENTITY, Some(node_id), doc)?;

    if let Some(group) = root_group {
      doc.end_group(group)?;
    }
    doc.end_group(filter_group)?;
    doc.end_filter_wrappers(filter_wrappers)?;
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
  /// after the node's children. Returns the chrome and the transform the node's
  /// children sit in (`parent · group_transform`): a pure translation is folded into
  /// the draw origin so it leaves no group, so the children's transform is the
  /// parent's, not the node's.
  fn emit_box(
    &self,
    np: &NodePaint,
    parent: Affine,
    stop_at: Option<NodeId>,
    doc: &mut SvgDocument,
  ) -> io::Result<Option<(BoxChrome, Affine)>> {
    let Some(node) = self.scene.root.node_at_path(&np.path) else {
      return Ok(None);
    };
    let Ok(layout) = self.scene.results.layout(np.node_id) else {
      return Ok(None);
    };

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * np.transform;
    let (origin, group_transform) = if relative.only_translation() {
      (
        Point {
          x: relative.x,
          y: relative.y,
        },
        Affine::IDENTITY,
      )
    } else {
      (Point::ZERO, relative)
    };
    let child_transform = parent * group_transform;
    let placed = PlacedBox::new(node, BoxFrame::new(layout, origin));

    // Inside a replay (stop_at set), nested backdrop-filter nodes are emitted
    // without their own backdrop (each level would replay its own prefix, doubling
    // the output per backdrop node in paint order). Stacked backdrop elements
    // therefore see the unfiltered content beneath them in the replay.
    if stop_at.is_none() && !node.context.style.backdrop_filter.is_empty() {
      self.emit_backdrop(&placed, np.node_id, child_transform, group_transform, doc)?;
    }

    let chrome = BoxChrome::open(&placed, group_transform, doc)?;
    placed.emit_own_content(doc)?;
    Ok(Some((chrome, child_transform)))
  }

  /// Walks a stacking context in paint order. With `stop_at` set, emission halts
  /// (without emitting) at that node — used to replay the backdrop of a
  /// `backdrop-filter` node. Returns whether the stop node was reached.
  fn emit_context(
    &self,
    id: usize,
    parent: Affine,
    stop_at: Option<NodeId>,
    doc: &mut SvgDocument,
  ) -> io::Result<bool> {
    let Some(ctx) = self.scene.contexts.get(id) else {
      return Ok(false);
    };

    // Children sit in the root node's child transform; a synthetic root context
    // keeps the caller's.
    let (chrome, child_transform) = match ctx.root() {
      Some(np) => {
        if stop_at == Some(np.node_id) {
          return Ok(true);
        }
        match self.emit_box(np, parent, stop_at, doc)? {
          Some((chrome, transform)) => (Some(chrome), transform),
          None => (None, parent),
        }
      }
      None => (None, parent),
    };

    let mut stopped = false;
    // A plain node in a bucket owns no effect groups — anything that would need
    // one makes the node its own context — so its outline can wait for the
    // descendants that follow it. Blink runs the same pass as
    // `kDescendantOutlinesOnly`.
    let mut descendant_outlines = Vec::new();

    'buckets: for bucket in ctx.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(np) => {
            if stop_at == Some(np.node_id) {
              stopped = true;
              break 'buckets;
            }
            if let Some((mut chrome, _)) = self.emit_box(np, child_transform, stop_at, doc)? {
              descendant_outlines.extend(chrome.take_outline());
              chrome.close(doc)?;
            }
          }
          PaintItemKind::Context(child) => {
            if self.emit_context(*child, child_transform, stop_at, doc)? {
              stopped = true;
              break 'buckets;
            }
          }
        }
      }
    }

    for pending in &descendant_outlines {
      pending.emit(doc)?;
    }
    if let Some(chrome) = chrome {
      chrome.close(doc)?;
    }
    Ok(stopped)
  }
}

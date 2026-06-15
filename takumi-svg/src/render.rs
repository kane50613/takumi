//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::collections::HashMap;
use std::io;
use std::rc::Rc;
use std::sync::Arc;

use taffy::NodeId;
use takumi_core::{
  GlobalContext,
  context::RenderContext,
  error::Result,
  layout::{
    Viewport,
    node::{Node, NodeKind},
    style::StyleSheet,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::image::ImageSource,
  shadow::SizedShadow,
};
use typed_builder::TypedBuilder;

use crate::box_model::{
  clips_overflow, element_transform, has_radius, resolved_radii, rounded_rect_path,
};
use crate::gradient::emit_background_images;
use crate::image::emit_image;
use crate::text::emit_text;
use crate::{IDENTITY, Rgba, SvgDocument};

/// Inputs for [`render`]. Built with [`SvgOptions::builder`]; only `node`,
/// `viewport`, and `global` are required. Carrying inputs in a builder struct
/// keeps new options from being breaking changes.
#[derive(TypedBuilder)]
pub struct SvgOptions<'g> {
  /// The viewport to render in.
  pub(crate) viewport: Viewport,
  /// The global context (fonts, persistent images).
  pub(crate) global: &'g GlobalContext,
  /// The root node to render.
  pub(crate) node: Node,
  /// Resources fetched externally, keyed by URL.
  #[builder(default)]
  pub(crate) fetched_resources: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub(crate) stylesheet: StyleSheet,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
}

/// Renders a node tree to a vector SVG string.
pub fn render(options: SvgOptions<'_>) -> Result<String> {
  let viewport = options.viewport;
  let canvas_w = viewport.size.width;
  let canvas_h = viewport.size.height;
  let context = RenderContext::new(
    options.global,
    viewport,
    options.fetched_resources,
    Rc::new(options.stylesheet),
    options.time_ms,
  );
  let root = RenderNode::from_node(&context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);
  let root_id = tree.root_node_id();
  tree.compute_layout(context.sizing.viewport.into());
  let results = tree.into_results();

  let root_layout = results.layout(root_id)?;
  let width = canvas_w.map_or(root_layout.size.width, |w| w as f32);
  let height = canvas_h.map_or(root_layout.size.height, |h| h as f32);
  let mut doc = SvgDocument::new(width, height)?;
  emit_node(&root, root_id, &results, 0.0, 0.0, &mut doc)?;
  Ok(doc.render()?)
}

/// Emits one node's box decorations and recurses into its children.
fn emit_node(
  node: &RenderNode,
  node_id: NodeId,
  results: &LayoutResults,
  parent_x: f32,
  parent_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Ok(layout) = results.layout(node_id) else {
    return Ok(());
  };
  let x = parent_x + layout.location.x;
  let y = parent_y + layout.location.y;
  let width = layout.size.width;
  let height = layout.size.height;
  let style = &node.context.style;
  let cc = node.context.current_color;

  // CSS opacity + transform apply to the element's whole subtree → one group.
  let opacity = style.opacity.0;
  let transform = element_transform(&node.context, layout.size, x, y);
  let outer = (transform.is_some() || opacity < 1.0)
    .then(|| doc.begin_group(transform.unwrap_or(IDENTITY), opacity, None, None))
    .transpose()?;

  emit_box_shadows(node, layout, x, y, width, height, doc)?;

  let radii = resolved_radii(style, &node.context.sizing, width, height);
  let rounded = has_radius(&radii);

  // Background (clipped to the rounded border-box when radii are present).
  let background = style.background_color.resolve(cc);
  let has_bg_image = style
    .background_image
    .as_deref()
    .is_some_and(|images| !images.is_empty());
  if background.0[3] != 0 || has_bg_image {
    let bg_clip = if rounded {
      Some(doc.clip_path(&rounded_rect_path(x, y, width, height, radii))?)
    } else {
      None
    };
    let bg_group = bg_clip
      .as_deref()
      .map(|clip| doc.begin_group(IDENTITY, 1.0, Some(clip), None))
      .transpose()?;
    if background.0[3] != 0 {
      doc.rect(x, y, width, height, Rgba(background.0))?;
    }
    if let Some(images) = style.background_image.as_deref() {
      emit_background_images(images, &node.context, x, y, width, height, doc)?;
    }
    if let Some(group) = bg_group {
      doc.end_group(group)?;
    }
  }

  emit_borders(node, layout, x, y, width, height, doc)?;

  // Children, clipped to the (rounded) padding box when overflow is not visible.
  let child_group = clips_overflow(style)
    .then(|| {
      let b = layout.border;
      let inner = [
        [
          (radii[0][0] - b.left).max(0.0),
          (radii[0][1] - b.top).max(0.0),
        ],
        [
          (radii[1][0] - b.right).max(0.0),
          (radii[1][1] - b.top).max(0.0),
        ],
        [
          (radii[2][0] - b.right).max(0.0),
          (radii[2][1] - b.bottom).max(0.0),
        ],
        [
          (radii[3][0] - b.left).max(0.0),
          (radii[3][1] - b.bottom).max(0.0),
        ],
      ];
      let path = rounded_rect_path(
        x + b.left,
        y + b.top,
        (width - b.left - b.right).max(0.0),
        (height - b.top - b.bottom).max(0.0),
        inner,
      );
      doc
        .clip_path(&path)
        .and_then(|clip| doc.begin_group(IDENTITY, 1.0, Some(&clip), None))
    })
    .transpose()?;

  match node.node.as_ref().map(|n| &n.kind) {
    Some(NodeKind::Image(image)) => emit_image(
      image,
      &node.context,
      parent_x + layout.content_box_x(),
      parent_y + layout.content_box_y(),
      layout.content_box_width(),
      layout.content_box_height(),
      doc,
    )?,
    Some(NodeKind::Text(text)) => emit_text(text, &node.context, *layout, x, y, doc)?,
    _ => {}
  }

  if let Ok(children) = results.box_children(node_id)
    && let Some(child_nodes) = node.children.as_deref()
  {
    for child in children {
      if let Some(child_node) = child_nodes.get(child.render_index) {
        emit_node(child_node, child.node_id, results, x, y, doc)?;
      }
    }
  }

  if let Some(group) = child_group {
    doc.end_group(group)?;
  }
  if let Some(group) = outer {
    doc.end_group(group)?;
  }
  Ok(())
}

/// Emits each visible border side as a filled trapezoid path. Border-radius is
/// applied to the background and clip, but border *corners* are square for now.
fn emit_borders(
  node: &RenderNode,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let b = layout.border;
  let cc = node.context.current_color;
  let style = &node.context.style;
  let (r, t, l, bo) = (x + w, y, x, y + h);
  let sides = [
    (
      b.top,
      style.border_top_color.resolve(cc),
      format!(
        "M{x} {y} L{r} {y} L{} {} L{} {} Z",
        r - b.right,
        y + b.top,
        l + b.left,
        y + b.top
      ),
    ),
    (
      b.right,
      style.border_right_color.resolve(cc),
      format!(
        "M{r} {t} L{r} {bo} L{} {} L{} {} Z",
        r - b.right,
        bo - b.bottom,
        r - b.right,
        t + b.top
      ),
    ),
    (
      b.bottom,
      style.border_bottom_color.resolve(cc),
      format!(
        "M{r} {bo} L{l} {bo} L{} {} L{} {} Z",
        l + b.left,
        bo - b.bottom,
        r - b.right,
        bo - b.bottom
      ),
    ),
    (
      b.left,
      style.border_left_color.resolve(cc),
      format!(
        "M{l} {bo} L{l} {t} L{} {} L{} {} Z",
        l + b.left,
        t + b.top,
        l + b.left,
        bo - b.bottom
      ),
    ),
  ];
  for (width, color, d) in sides {
    if width > 0.0 && color.0[3] != 0 {
      doc.path(&d, Rgba(color.0))?;
    }
  }
  Ok(())
}

/// Emits outset `box-shadow`s behind the element as offset, blurred rects.
/// Inset shadows are not yet supported.
fn emit_box_shadows(
  node: &RenderNode,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some(shadows) = node.context.style.box_shadow.as_ref() else {
    return Ok(());
  };
  let cc = node.context.current_color;
  for shadow in shadows.iter() {
    if shadow.inset {
      continue;
    }
    let resolved = SizedShadow::from_box_shadow(*shadow, &node.context.sizing, cc, layout.size);
    if resolved.color.0[3] == 0 {
      continue;
    }
    let sx = x + resolved.offset_x - resolved.spread_radius;
    let sy = y + resolved.offset_y - resolved.spread_radius;
    let sw = w + 2.0 * resolved.spread_radius;
    let sh = h + 2.0 * resolved.spread_radius;
    if sw <= 0.0 || sh <= 0.0 {
      continue;
    }
    let fill = Rgba(resolved.color.0);
    if resolved.blur_radius > 0.0 {
      let filter = doc.blur_filter(resolved.blur_radius / 2.0)?;
      let group = doc.begin_group(IDENTITY, 1.0, None, Some(&filter))?;
      doc.rect(sx, sy, sw, sh, fill)?;
      doc.end_group(group)?;
    } else {
      doc.rect(sx, sy, sw, sh, fill)?;
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_svg_wrapper_at_viewport_size() {
    let global = GlobalContext::default();
    let svg = render(
      SvgOptions::builder()
        .node(Node::container([]))
        .viewport(Viewport::new((120, 80)))
        .global(&global)
        .build(),
    )
    .unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("width=\"120\""));
    assert!(svg.contains("height=\"80\""));
    assert!(!svg.contains("base64"));
  }
}

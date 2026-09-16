//! A paint tree: what the backends paint for a node tree, with used values, in paint order.
//!
//! [`paint_tree`] runs layout, builds the same stacking-context scene the raster, SVG, and
//! PDF backends walk, and records each box's decorations, image placement, and shaped text
//! runs instead of drawing them. Lengths are device pixels. A node's `transform` is absolute;
//! everything inside a node is relative to its border box.
//!
//! Descendant outlines drift from the backends: they paint after the owning node's children,
//! while raster and SVG defer a plain node's outline past the siblings that follow it in the
//! same stacking context.

mod fonts;
mod tree;
mod walk;

use std::{collections::HashMap, rc::Rc, sync::Arc};

use typed_builder::TypedBuilder;

pub use self::tree::*;
use crate::{
  Fonts,
  context::RenderContext,
  error::Result,
  geometry::{NodeId, Size},
  layout::{
    node::Node,
    tree::{LayoutTree, RenderNode},
  },
  resources::image::ImageSource,
  scene::{SceneRequest, build_scene},
  style::{Affine, ComputedStyle, FontFamily, Lang, SizingContext, StyleSheet},
  viewport::Viewport,
};

/// Inputs for [`paint_tree`], built with [`PaintTreeOptions::builder`].
#[derive(TypedBuilder)]
pub struct PaintTreeOptions<'g> {
  /// The viewport to lay out in.
  pub(crate) viewport: Viewport,
  /// The font registry.
  pub(crate) fonts: &'g Fonts,
  /// The root node.
  pub(crate) node: Node,
  /// Pre-decoded images keyed by `src`.
  #[builder(default)]
  pub(crate) images: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub(crate) stylesheet: Arc<StyleSheet>,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
  /// Per-render font fallback chain.
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language applied to the root.
  #[builder(default)]
  pub(crate) lang: Option<Lang>,
}

/// Lays out `options.node` and records what painting it would draw.
pub fn paint_tree(options: PaintTreeOptions<'_>) -> Result<PaintTree> {
  let viewport = options.viewport;
  let context = RenderContext::builder()
    .fonts(
      options
        .fonts
        .snapshot_with_fallbacks(options.font_families.as_ref()),
    )
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(Rc::new(options.images))
    .stylesheet(options.stylesheet)
    .time_ms(options.time_ms)
    .style(Box::new(ComputedStyle {
      lang: options.lang,
      font_family: options.font_families.unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  let root = RenderNode::from_node(&context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);
  tree.compute_layout(viewport.into());
  let results = tree.into_results();

  let root_layout = results.layout(NodeId::ROOT)?;
  let width = viewport
    .size
    .width
    .map_or(root_layout.size.width, |w| w as f32);
  let height = viewport
    .size
    .height
    .map_or(root_layout.size.height, |h| h as f32);
  let contexts = build_scene(SceneRequest {
    root: &root,
    layout_results: &results,
    node_id: NodeId::ROOT,
    transform: Affine::IDENTITY,
    container_size: Size {
      width: Some(width),
      height: Some(height),
    },
    paint_bounds: false,
  })?;

  let mut walker = walk::Walker {
    fonts: fonts::FontTable::default(),
  };
  let mut nodes = walker.scene(&root, &results, &contexts)?;
  let root = match nodes.len() {
    1 => nodes.remove(0),
    _ => PaintNode {
      source: None,
      width,
      height,
      transform: Affine::IDENTITY.to_cols_array(),
      opacity: 1.0,
      blend_mode: None,
      isolate: false,
      clip: None,
      box_decoration: None,
      image: None,
      text_shadows: Vec::new(),
      inline_backgrounds: Vec::new(),
      runs: Vec::new(),
      unresolved_effects: None,
      children: nodes,
    },
  };

  Ok(PaintTree {
    width,
    height,
    fonts: walker.fonts.into_fonts(),
    root,
  })
}

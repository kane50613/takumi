//! A paint tree: what the backends paint for a node tree, with used values, in paint order.
//!
//! [`paint_tree`] runs layout, builds the same stacking-context scene the raster, SVG, and
//! PDF backends walk, and records each box's decorations, image placement, and shaped text
//! runs instead of drawing them. Lengths are device pixels. A node's `x`, `y`, and `transform`
//! are absolute; everything inside a node is relative to its border box.
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
  geometry::Size,
  layout::{node::Node, tree::RenderNode},
  resources::image::ImageSource,
  scene::Scene,
  style::{ComputedStyle, FontFamily, Lang, SizingContext, StyleSheet},
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
    .style(Box::new(ComputedStyle::root(
      options.lang,
      options.font_families,
    )))
    .build();

  let scene = Scene::lay_out(
    RenderNode::from_node(&context, options.node),
    viewport,
    false,
  )?;
  let Size { width, height } = scene.size;

  let mut walker = walk::Walker {
    fonts: fonts::FontTable::default(),
  };
  let mut nodes = walker.scene(&scene.root, &scene.results, &scene.contexts)?;
  let root = match nodes.len() {
    1 => nodes.remove(0),
    _ => PaintNode {
      source: None,
      width,
      height,
      x: 0.0,
      y: 0.0,
      transform: None,
      opacity: 1.0,
      blend_mode: None,
      isolate: false,
      clip: None,
      background: None,
      border: None,
      shadows: None,
      outline: None,
      image: None,
      text_shadows: Vec::new(),
      inline_backgrounds: Vec::new(),
      text_runs: Vec::new(),
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

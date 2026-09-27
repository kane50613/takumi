//! A paint document: what the backends paint for a node tree, with every CSS value resolved.
//!
//! [`paint_tree`] runs layout, walks the same stacking-context scene the raster, SVG, and PDF
//! backends walk, and records what the shared painters draw instead of drawing it. Lengths are
//! device pixels. A node's `transform` maps its local space onto the canvas, and its drawables
//! sit in that local space.

mod document;
mod fonts;
mod paints;
mod record;
mod runs;
mod walk;

use std::{collections::HashMap, rc::Rc, sync::Arc};

use parley::fontique::Blob;
use typed_builder::TypedBuilder;

pub use self::document::*;
use crate::{
  Fonts,
  context::RenderContext,
  error::Result,
  geometry::Size,
  layout::{node::Node, tree::RenderNode},
  resources::image::ImageSource as DecodedImage,
  scene::Scene,
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
  pub(crate) images: HashMap<Arc<str>, DecodedImage>,
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

/// A painted document, and the font files its runs use.
pub struct PaintTree {
  /// The document.
  pub document: PaintDocument,
  fonts: Vec<Blob<u8>>,
}

impl PaintTree {
  /// The file of the document's font at `index`.
  pub fn font_data(&self, index: usize) -> Option<&[u8]> {
    self.fonts.get(index).map(Blob::as_ref)
  }
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
  let has_root = match scene.contexts.first().and_then(|context| context.root()) {
    Some(paint) => walk::recorded(&scene, paint)?.is_some(),
    None => false,
  };
  let mut walker = walk::Walker::new((!has_root).then(|| PaintNode {
    id: 0,
    parent: None,
    element: None,
    transform: Affine::IDENTITY.to_cols_array(),
    width,
    height,
    bounds: PaintRect {
      x: 0.0,
      y: 0.0,
      width,
      height,
    },
    drawables: Vec::new(),
    children: Vec::new(),
    kind: NodeKind::Box {
      content_box: PaintRect {
        x: 0.0,
        y: 0.0,
        width,
        height,
      },
      outline: Vec::new(),
      effects: None,
      overflow_clip: None,
    },
  }));

  walker.scene(&scene, &[])?;
  walker.link();

  let (fonts, data) = walker.fonts.finish();

  Ok(PaintTree {
    document: PaintDocument {
      width,
      height,
      nodes: walker.nodes,
      fonts,
      steps: walker.steps,
    },
    fonts: data,
  })
}

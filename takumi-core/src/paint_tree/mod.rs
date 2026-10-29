//! A paint document: what the backends paint for a node tree, in device pixels, recorded instead
//! of drawn.

mod document;
mod fonts;
mod outlines;
mod paints;
mod record;
mod runs;
mod walk;

use std::{collections::HashMap, rc::Rc, sync::Arc};

use crate::resources::font::FontBlob;
use typed_builder::TypedBuilder;

pub use self::document::*;
use self::outlines::Outlines;
use crate::{
  Fonts,
  context::RenderContext,
  error::Result,
  geometry::Size,
  layout::{needs::RenderNeeds, node::Node, tree::RenderNode},
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

/// A painted document, the font files its runs use, and its runs' glyph outlines.
pub struct PaintTree {
  /// The document.
  pub document: PaintDocument,
  fonts: Vec<FontBlob>,
  outlines: Outlines,
}

impl PaintTree {
  /// The file of the document's font at `index`.
  pub fn font_data(&self, index: usize) -> Option<&[u8]> {
    self.fonts.get(index).map(FontBlob::data)
  }

  /// Each glyph outline of run `run` as SVG path data, `None` for one the run's colour does not fill.
  pub fn run_outlines(&self, node: usize, run: usize) -> Option<Vec<Option<String>>> {
    self.outlines.path_data(node, run)
  }
}

impl PaintTreeOptions<'_> {
  /// The styled render tree of `node`.
  fn into_render_tree(self) -> RenderNode {
    let context = RenderContext::builder()
      .fonts(
        self
          .fonts
          .snapshot_with_fallbacks(self.font_families.as_ref()),
      )
      .sizing(SizingContext::builder().viewport(self.viewport).build())
      .images(Rc::new(self.images))
      .stylesheet(self.stylesheet)
      .time_ms(self.time_ms)
      .style(Box::new(ComputedStyle::root(self.lang, self.font_families)))
      .build();

    RenderNode::from_node(&context, self.node)
  }
}

/// What `options.node` needs fetched before it paints; see [`RenderNode::needs`].
pub fn paint_tree_needs(options: PaintTreeOptions<'_>) -> RenderNeeds {
  options.into_render_tree().needs()
}

/// Lays out `options.node` and records what painting it would draw.
pub fn paint_tree(options: PaintTreeOptions<'_>) -> Result<PaintTree> {
  let viewport = options.viewport;
  let scene = Scene::lay_out(options.into_render_tree(), viewport, false)?;
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
    outlines: walker.outlines,
  })
}

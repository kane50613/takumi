//! Layout of an independent node tree: the main content or a header/footer band.

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  geometry::{NodeId, Size},
  layout::{
    node::Node,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::image::ImageSource,
  scene::{StackingContextNode, build_stacking_contexts},
  style::{
    Affine, ComputedStyle, Display, FlexDirection, FontFamily, Lang, Length, SizingContext, Style,
    StyleDeclaration, StyleSheet,
  },
  viewport::Viewport,
};

use crate::{
  emitter::{Emitter, FontMap},
  inline::InlineMap,
  options::PdfError,
  tags::TagCollector,
};

/// Shared inputs for laying out an independent node tree: the main content or
/// a header/footer band.
pub(crate) struct TreeInputs<'g> {
  pub(crate) fonts: &'g Fonts,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) images: Rc<HashMap<Arc<str>, ImageSource>>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

/// A node tree taken through layout and scene building, ready to emit.
pub(crate) struct PreparedTree {
  pub(crate) root: RenderNode,
  pub(crate) results: LayoutResults,
  pub(crate) contexts: Vec<StackingContextNode>,
  pub(crate) width: f32,
  pub(crate) height: f32,
}

impl PreparedTree {
  /// The size the caller's own node laid out at, which is what `measure`
  /// reports. [`fill_root`] wraps that node in a page-wide box, so the root's
  /// size only ever gives the page back.
  pub(crate) fn content_size(&self) -> Size<f32> {
    self
      .results
      .box_children(NodeId::ROOT)
      .ok()
      .and_then(|children| children.first())
      .and_then(|child| self.results.layout(child.node_id).ok())
      .map_or(
        Size {
          width: self.width,
          height: self.height,
        },
        |layout| layout.size,
      )
  }

  pub(crate) fn emitter<'a>(
    &'a self,
    fonts: &'a mut FontMap,
    inline: Option<&'a InlineMap<'a>>,
    tags: Option<&'a RefCell<TagCollector>>,
    uncovered: &'a RefCell<String>,
    document_lang: Option<&'a str>,
  ) -> Emitter<'a> {
    Emitter {
      color_filter: None,
      uncovered,
      document_lang,
      transformed: false,
      root: &self.root,
      contexts: &self.contexts,
      results: &self.results,
      fonts,
      inline,
      window: None,
      line_window: None,
      tags,
    }
  }
}

// The root fills the content box like a browser body: a fit-content root
// resolves child percentages against a tentative width first and the final
// width later, and taffy does not reconcile heights across those passes.
fn fill_root(node: Node, viewport: Viewport) -> Node {
  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::flex_direction(FlexDirection::Column))
    .with(StyleDeclaration::width(Length::Percentage(100.0)));

  if viewport.size.height.is_some() {
    style = style.with(StyleDeclaration::height(Length::Percentage(100.0)));
  }

  Node::container([node]).with_style(style)
}

pub(crate) fn prepare_tree(
  inputs: &TreeInputs<'_>,
  node: Node,
  viewport: Viewport,
) -> Result<PreparedTree, PdfError> {
  let node = fill_root(node, viewport);
  let context = RenderContext::builder()
    .fonts(
      inputs
        .fonts
        .snapshot_with_fallbacks(inputs.font_families.as_ref()),
    )
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(inputs.images.clone())
    .stylesheet(inputs.stylesheet.clone())
    .style(Box::new(ComputedStyle {
      lang: inputs.lang,
      font_family: inputs.font_families.clone().unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  let root = RenderNode::from_node(&context, node);
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
  let contexts = build_stacking_contexts(
    &root,
    &results,
    NodeId::ROOT,
    Affine::IDENTITY,
    (Some(width), Some(height)),
  )?;

  Ok(PreparedTree {
    root,
    results,
    contexts,
    width,
    height,
  })
}

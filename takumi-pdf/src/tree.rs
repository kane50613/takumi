//! Layout of an independent node tree: the main content or a header/footer band.

use std::{collections::HashMap, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  geometry::{NodeId, Size},
  layout::{
    layout_view::LayoutView,
    node::{Node, NodeKind},
    paged_layout::PagedLayout,
    tree::{LayoutResults, RenderNode},
  },
  painter::OwnContent,
  resources::image::ImageSource,
  scene::{Scene, SceneView},
  style::{
    ComputedStyle, Display, FontFamily, Lang, LengthUnit, Position, SizingContext, Style,
    StyleSheet, ZIndex,
  },
  viewport::Viewport,
};

use crate::{
  bands::{FixedTemplate, Repeatable},
  counters::{has_page_counters, substitute_page_counters, substitute_target_counters},
  options::PdfError,
  page::PageFrame,
  pagination::MAX_PAGES,
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

impl TreeInputs<'_> {
  pub(crate) fn context(&self, viewport: Viewport) -> RenderContext {
    RenderContext::builder()
      .fonts(
        self
          .fonts
          .snapshot_with_fallbacks(self.font_families.as_ref()),
      )
      .sizing(SizingContext::builder().viewport(viewport).build())
      .images(self.images.clone())
      .stylesheet(self.stylesheet.clone())
      .style(Box::new(ComputedStyle::root(
        self.lang,
        self.font_families.clone(),
      )))
      .build()
  }

  pub(crate) fn prepare(&self, node: Node, viewport: Viewport) -> Result<PreparedTree, PdfError> {
    let node = fill_root(node, viewport);
    let root = RenderNode::from_initial_containing_block(&self.context(viewport), node);

    PreparedTree::lay_out(root, viewport)
  }

  /// Lays out a band template with the given counter values.
  pub(crate) fn prepare_band(
    &self,
    template: &Node,
    page: usize,
    pages: usize,
    viewport: Viewport,
  ) -> Result<PreparedTree, PdfError> {
    let mut node = template.clone();

    substitute_page_counters(&mut node, page, pages);
    // A band lays out per page, after the pass that resolves target counters, so
    // its hooks name no page. They empty like any other unresolved target instead
    // of leaving the placeholder the template put there.
    substitute_target_counters(&mut node, None, &|_: &str| None, &mut Vec::new());
    self.prepare(node, viewport)
  }

  /// The content laid out across pages, and the `fixed` subtrees attached to the initial
  /// containing block.
  pub(crate) fn prepare_paged(
    &self,
    node: Node,
    frame: &PageFrame,
  ) -> Result<(PagedContent, Vec<Repeatable>), PdfError> {
    let (root, repeated) = self.prepare_root(node, frame)?;

    Ok((PagedContent::lay_out(root, frame.page_size())?, repeated))
  }

  /// The content tree to lay out across pages, and the `fixed` subtrees taken out of it. Those
  /// repeat on every page, so they lay out against the page area on their own.
  pub(crate) fn prepare_root(
    &self,
    node: Node,
    frame: &PageFrame,
  ) -> Result<(RenderNode, Vec<Repeatable>), PdfError> {
    let viewport = frame.column();
    let node = fill_root(node, viewport);
    // A repeated box holding a counter lays out again per page, from the subtree
    // its preorder position names in the tree it was taken from.
    let source = has_page_counters(&node).then(|| node.clone());
    let mut root = RenderNode::from_initial_containing_block(&self.context(viewport), node);
    let repeated = take_repeating_fixed(&mut root);
    let page_context = self.context(frame.page_area);
    let repeated = repeated
      .into_iter()
      .map(|(node, parent)| {
        let template = source
          .as_ref()
          .zip(node.source_order())
          .and_then(|(source, index)| Some((index, node_in_source_order(source, index)?)))
          .filter(|(_, template)| has_page_counters(template))
          .map(|(source_order, template)| FixedTemplate {
            node: template.clone(),
            parent,
            source_order,
          });
        let prepared = PreparedTree::lay_out(page_root(&page_context, node), frame.page_area)?;

        Ok(Repeatable::fixed(prepared, template))
      })
      .collect::<Result<Vec<_>, PdfError>>()?;

    Ok((root, repeated))
  }
}

/// Whether a node's own content draws anything, i.e. whether a tagged content sequence around it
/// would be non-empty.
pub(crate) fn draws(content: &OwnContent<'_>) -> bool {
  match content {
    OwnContent::Inline(_) => true,
    OwnContent::Image(_) => cfg!(feature = "images"),
    OwnContent::None => false,
  }
}

/// A node tree taken through layout and scene building, ready to emit.
pub(crate) struct PreparedTree {
  pub(crate) scene: Scene,
}

impl PreparedTree {
  pub(crate) fn lay_out(root: RenderNode, viewport: Viewport) -> Result<Self, PdfError> {
    Ok(Self {
      scene: Scene::lay_out(root, viewport, true)?,
    })
  }

  /// The size the caller's own node laid out at, which is what `measure`
  /// reports. [`fill_root`] wraps that node in a page-wide box, so the root's
  /// size only ever gives the page back.
  pub(crate) fn content_size(&self) -> Size<f32> {
    let results = &self.scene.results;

    results
      .box_children(NodeId::ROOT)
      .ok()
      .and_then(|children| children.first())
      .and_then(|child| results.layout(child.node_id).ok())
      .map_or(self.scene.size, |layout| layout.size)
  }

  /// Whether a repeated box paints under the content, which is what a negative
  /// `z-index` asks for.
  pub(crate) fn paints_below(&self) -> bool {
    self
      .scene
      .root
      .children
      .as_deref()
      .and_then(<[RenderNode]>::first)
      .is_some_and(
        |child| matches!(child.context.style.box_data.z_index, ZIndex::Integer(index) if index < 0),
      )
  }

  pub(crate) fn view(&self) -> SceneView<'_> {
    self.scene.view()
  }
}

pub(crate) struct PagedContent {
  pub(crate) root: RenderNode,
  results: LayoutResults,
  pub(crate) page_size: Size<f32>,
  pub(crate) page_count: usize,
}

impl PagedContent {
  pub(crate) fn lay_out(root: RenderNode, page_size: Size<f32>) -> Result<Self, PdfError> {
    let mut layout = PagedLayout::new(&root, page_size, MAX_PAGES);

    while layout.next_page()? {}

    let page_count = layout.page_count();
    let results = layout.into_results();

    Ok(Self {
      root,
      results,
      page_size,
      page_count,
    })
  }

  pub(crate) fn layout(&self) -> LayoutView<'_> {
    self.results.view()
  }

  pub(crate) fn into_parts(self) -> (RenderNode, LayoutResults) {
    (self.root, self.results)
  }
}

// The root fills the content box like a browser body, and keeps the margins of what it holds
// inside it.
fn fill_root(node: Node, viewport: Viewport) -> Node {
  let mut style = Style::default()
    .display(Display::FlowRoot)
    .width(100.0.percent());

  if viewport.size.height.is_some() {
    style = style.height(100.0.percent());
  }

  Node::container([node]).with_style(style)
}

/// The node at a source order, counted the way the render tree numbers the
/// source tree it is built from.
fn node_in_source_order(node: &Node, index: usize) -> Option<&Node> {
  fn walk<'n>(node: &'n Node, index: usize, cursor: &mut usize) -> Option<&'n Node> {
    if *cursor == index {
      return Some(node);
    }
    *cursor += 1;
    let NodeKind::Container { children } = &node.kind else {
      return None;
    };

    children.iter().find_map(|child| walk(child, index, cursor))
  }

  walk(node, index, &mut 0)
}

// ponytail: a repeated box with no insets paints at the page area's origin.
// Blink keeps the box's hypothetical static position instead
// (`out_of_flow_layout_part.cc`), which needs the offset the box had in the
// flow it was taken out of.
/// Removes the `fixed` boxes the initial containing block holds. A box that
/// establishes a containing block of its own keeps its `fixed` descendants,
/// which stay in the flow and paginate with it.
fn take_repeating_fixed(node: &mut RenderNode) -> Vec<(RenderNode, RenderContext)> {
  let Some(children) = node.children.take() else {
    return Vec::new();
  };
  let parent = node.context.clone();
  let mut repeating = Vec::new();
  let mut kept = Vec::with_capacity(children.len());

  for mut child in children.into_vec() {
    if child.context.style.base_data.position == Position::Fixed {
      repeating.push((child, parent.clone()));
      continue;
    }
    if !child.context.style.contains_fixed_descendants() {
      repeating.append(&mut take_repeating_fixed(&mut child));
    }
    kept.push(child);
  }
  node.children = Some(kept.into_boxed_slice());
  repeating
}

/// The page area a repeated box positions against, as a layout root ignores its own insets.
pub(crate) fn page_root(context: &RenderContext, child: RenderNode) -> RenderNode {
  let area = Node::container([]).with_style(
    Style::default()
      .display(Display::Block)
      .width(100.0.percent())
      .height(100.0.percent()),
  );
  let mut root = RenderNode::from_node(context, area);

  root.children = Some(Box::new([child]));
  root
}

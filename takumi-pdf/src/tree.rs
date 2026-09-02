//! Layout of an independent node tree: the main content or a header/footer band.

use std::{collections::HashMap, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  geometry::{NodeId, Size},
  layout::{
    node::{Node, NodeKind},
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::image::ImageSource,
  scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
  style::{
    Affine, ComputedStyle, Display, FlexDirection, FontFamily, Lang, Length, Position,
    SizingContext, Style, StyleDeclaration, StyleSheet, ZIndex,
  },
  viewport::Viewport,
};

use crate::{
  atoms::AtomCollector,
  bands::{FixedTemplate, Repeatable},
  counters::{has_page_counters, substitute_page_counters, substitute_target_counters},
  emitter::{DocumentState, Emitter},
  inline::InlineMap,
  options::PdfError,
  page::PageFrame,
  window::Window,
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
      .style(Box::new(ComputedStyle {
        lang: self.lang,
        font_family: self.font_families.clone().unwrap_or_default(),
        ..Default::default()
      }))
      .build()
  }

  pub(crate) fn prepare(&self, node: Node, viewport: Viewport) -> Result<PreparedTree, PdfError> {
    let node = fill_root(node, viewport);
    let root = RenderNode::from_node(&self.context(viewport), node);

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

  /// The content column, plus the `fixed` subtrees attached to the initial
  /// containing block. Those repeat on every page, so they lay out against the
  /// page area instead of the column.
  pub(crate) fn prepare_paged(
    &self,
    node: Node,
    frame: &PageFrame,
  ) -> Result<(PreparedTree, Vec<Repeatable>), PdfError> {
    let viewport = frame.column();
    let node = fill_root(node, viewport);
    // A repeated box holding a counter lays out again per page, from the subtree
    // its preorder position names in the tree it was taken from.
    let source = has_page_counters(&node).then(|| node.clone());
    let mut root = RenderNode::from_node(&self.context(viewport), node);
    let repeated = take_repeating_fixed(&mut root);
    let content = PreparedTree::lay_out(root, viewport)?;
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

    Ok((content, repeated))
  }
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
  pub(crate) fn lay_out(root: RenderNode, viewport: Viewport) -> Result<Self, PdfError> {
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
      Size {
        width: Some(width),
        height: Some(height),
      },
    )?;

    Ok(Self {
      root,
      results,
      contexts,
      width,
      height,
    })
  }

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

  /// Whether a repeated box paints under the content, which is what a negative
  /// `z-index` asks for.
  pub(crate) fn paints_below(&self) -> bool {
    self
      .root
      .children
      .as_deref()
      .and_then(<[RenderNode]>::first)
      .is_some_and(
        |child| matches!(child.context.style.z_index, ZIndex::Integer(index) if index < 0),
      )
  }

  /// The scene's atom collector, for pagination.
  pub(crate) fn atom_collector<'a>(
    &'a self,
    inline: Option<&'a InlineMap<'a>>,
  ) -> AtomCollector<'a> {
    AtomCollector {
      root: &self.root,
      contexts: &self.contexts,
      results: &self.results,
      inline,
    }
  }

  /// Visits every node paint of the scene, in paint order.
  pub(crate) fn for_each_paint(&self, mut visit: impl FnMut(&NodePaint)) {
    fn walk(tree: &PreparedTree, id: usize, visit: &mut impl FnMut(&NodePaint)) {
      let Some(context) = tree.contexts.get(id) else {
        return;
      };

      if let Some(paint) = context.root() {
        visit(paint);
      }
      for bucket in context.in_paint_order() {
        for item in bucket {
          match &item.kind {
            PaintItemKind::Node(paint) => visit(paint),
            PaintItemKind::Context(child) => walk(tree, *child, visit),
          }
        }
      }
    }

    walk(self, 0, &mut visit);
  }

  pub(crate) fn emitter<'a>(
    &'a self,
    state: &'a DocumentState<'a>,
    inline: Option<&'a InlineMap<'a>>,
    tagged: bool,
  ) -> Emitter<'a> {
    Emitter {
      root: &self.root,
      contexts: &self.contexts,
      results: &self.results,
      document: state,
      inline,
      window: Window::default(),
      tagged: tagged && state.tags.is_some(),
      tag_prefix: Vec::new(),
      color_filter: None,
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
    if child.context.style.position == Position::Fixed {
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

/// The page area a repeated box positions against. Taffy gives a layout root
/// the origin, so the box has to be a child of one to see its own insets.
pub(crate) fn page_root(context: &RenderContext, child: RenderNode) -> RenderNode {
  let area = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Length::Percentage(100.0)))
      .with(StyleDeclaration::height(Length::Percentage(100.0))),
  );
  let mut root = RenderNode::from_node(context, area);

  root.children = Some(Box::new([child]));
  root
}

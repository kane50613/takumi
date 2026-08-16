//! Layout of an independent node tree: the main content or a header/footer band.

use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  geometry::{NodeId, Size},
  layout::{
    node::{Node, NodeKind},
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::image::ImageSource,
  scene::{StackingContextNode, build_stacking_contexts},
  style::{
    Affine, ComputedStyle, Display, FlexDirection, FontFamily, Lang, Length, Position,
    SizingContext, Style, StyleDeclaration, StyleSheet, ZIndex,
  },
  viewport::Viewport,
};

use crate::{
  counters::{has_page_counters, substitute_page_counters},
  emitter::{Emitter, FontMap, RenderIssues},
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

  pub(crate) fn emitter<'a>(
    &'a self,
    fonts: &'a mut FontMap,
    inline: Option<&'a InlineMap<'a>>,
    tags: Option<&'a RefCell<TagCollector>>,
    issues: &'a RefCell<RenderIssues>,
    document_lang: Option<&'a str>,
  ) -> Emitter<'a> {
    Emitter {
      color_filter: None,
      issues,
      document_lang,
      root: &self.root,
      contexts: &self.contexts,
      results: &self.results,
      fonts,
      inline,
      window: None,
      line_window: None,
      tags,
      tag_prefix: Vec::new(),
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
  let context = tree_context(inputs, viewport);
  let root = RenderNode::from_node(&context, node);

  lay_out(root, viewport)
}

pub(crate) fn tree_context(inputs: &TreeInputs<'_>, viewport: Viewport) -> RenderContext {
  RenderContext::builder()
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
    .build()
}

/// A `fixed` box repeated on every page: its layout, and the subtree it lays
/// out again from when it holds a page counter.
pub(crate) struct RepeatedBox {
  pub(crate) prepared: PreparedTree,
  pub(crate) template: Option<RepeatedTemplate>,
}

/// A repeated box's source subtree, with the context its styles resolved
/// under, so a re-layout inherits what its ancestors gave it.
pub(crate) struct RepeatedTemplate {
  node: Node,
  parent: RenderContext,
  index: usize,
}

impl RepeatedTemplate {
  /// The preorder positions the subtree occupies in the tree it was taken from.
  pub(crate) fn source_range(&self) -> Range<usize> {
    self.index..self.index + node_count(&self.node)
  }
}

fn node_count(node: &Node) -> usize {
  match &node.kind {
    NodeKind::Container { children } => 1 + children.iter().map(node_count).sum::<usize>(),
    _ => 1,
  }
}

/// The tree, plus the `fixed` subtrees attached to the initial containing
/// block. Those repeat on every page, so they lay out against the page area
/// instead of the content column.
pub(crate) fn prepare_paged_tree(
  inputs: &TreeInputs<'_>,
  node: Node,
  viewport: Viewport,
  page_area: Viewport,
) -> Result<(PreparedTree, Vec<RepeatedBox>), PdfError> {
  let node = fill_root(node, viewport);
  // A repeated box holding a counter lays out again per page, from the subtree
  // its preorder position names in the tree it was taken from.
  let source = has_page_counters(&node).then(|| node.clone());
  let context = tree_context(inputs, viewport);
  let mut root = RenderNode::from_node(&context, node);
  let repeated = take_repeating_fixed(&mut root);
  let content = lay_out(root, viewport)?;
  let page_context = tree_context(inputs, page_area);
  let repeated = repeated
    .into_iter()
    .map(|(node, parent)| {
      let template = source
        .as_ref()
        .zip(node.source_index)
        .and_then(|(source, index)| Some((index, node_at_preorder(source, index)?)))
        .filter(|(_, template)| has_page_counters(template))
        .map(|(index, template)| RepeatedTemplate {
          node: template.clone(),
          parent,
          index,
        });

      Ok(RepeatedBox {
        prepared: lay_out(page_root(&page_context, node), page_area)?,
        template,
      })
    })
    .collect::<Result<Vec<_>, PdfError>>()?;

  Ok((content, repeated))
}

/// Lays a repeated box out again with the counters a page asks for.
pub(crate) fn prepare_repeated(
  page_context: &RenderContext,
  template: &RepeatedTemplate,
  page: usize,
  pages: usize,
  page_area: Viewport,
) -> Result<PreparedTree, PdfError> {
  let mut node = template.node.clone();

  substitute_page_counters(&mut node, page, pages);
  let child = RenderNode::from_node(&template.parent, node);

  lay_out(page_root(page_context, child), page_area)
}

/// The node at a preorder position, counted the way the render tree numbers
/// the source tree it is built from.
fn node_at_preorder(node: &Node, index: usize) -> Option<&Node> {
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
fn page_root(context: &RenderContext, child: RenderNode) -> RenderNode {
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

fn lay_out(root: RenderNode, viewport: Viewport) -> Result<PreparedTree, PdfError> {
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

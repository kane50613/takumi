//! Layout of an independent node tree: the main content or a header/footer band.

use std::{collections::HashMap, rc::Rc, sync::Arc};

use takumi_core::{
  Fonts,
  context::RenderContext,
  geometry::{ComputedLayout, NodeId, Size},
  layout::{
    layout_view::LayoutView,
    node::{Node, NodeKind},
    tree::RenderNode,
  },
  paint_property::PropertyTrees,
  painter::OwnContent,
  resources::image::ImageSource,
  scene::{
    FragmentPlacements, NodePaint, PagedScene, PaintItemKind, Scene, SceneRef, StackingContextNode,
  },
  style::{
    ComputedStyle, Display, FontFamily, Lang, Length, Position, SizingContext, Style,
    StyleDeclaration, StyleSheet, ZIndex,
  },
  viewport::Viewport,
};

use crate::{
  bands::{FixedTemplate, Repeatable},
  chunks::SceneChunks,
  counters::{has_page_counters, substitute_page_counters, substitute_target_counters},
  emitter::{DocumentState, Emitter},
  inline::InlineCache,
  options::PdfError,
  page::PageFrame,
  pagination::MAX_PAGES,
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

  /// The content laid out across pages, how many, and the `fixed` subtrees attached to the
  /// initial containing block. Those repeat on every page, so they lay out against the page area
  /// on their own.
  pub(crate) fn prepare_paged(
    &self,
    node: Node,
    frame: &PageFrame,
  ) -> Result<(PreparedTree, Vec<Repeatable>, usize), PdfError> {
    let viewport = frame.column();
    let node = fill_root(node, viewport);
    // A repeated box holding a counter lays out again per page, from the subtree
    // its preorder position names in the tree it was taken from.
    let source = has_page_counters(&node).then(|| node.clone());
    let mut root = RenderNode::from_initial_containing_block(&self.context(viewport), node);
    let repeated = take_repeating_fixed(&mut root);
    let (content, page_count) = PreparedTree::lay_out_pages(
      root,
      Size {
        width: frame.content_width,
        height: frame.window_height,
      },
    )?;
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

    Ok((content, repeated, page_count))
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

  /// `root` laid out on pages `page_size`, the pages stacked in one scene, and how many there are.
  pub(crate) fn lay_out_pages(
    root: RenderNode,
    page_size: Size<f32>,
  ) -> Result<(Self, usize), PdfError> {
    let paged = PagedScene::lay_out(root, page_size, MAX_PAGES, true)?;
    let page_count = paged.page_count();

    Ok((
      Self {
        scene: paged.into_scene(),
      },
      page_count,
    ))
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
    SceneView::of_scene(&self.scene)
  }

  pub(crate) fn emitter<'a>(
    &'a self,
    state: &'a DocumentState<'a>,
    inline: Option<&'a InlineCache<'a>>,
    chunks: Option<&'a SceneChunks<'a>>,
    tagged: bool,
  ) -> Emitter<'a> {
    self.view().emitter(state, inline, chunks, tagged)
  }
}

/// A scene as the emitter and the collectors read it: the scene of a whole tree, or a window of a
/// paged layout.
#[derive(Clone, Copy)]
pub(crate) struct SceneView<'a> {
  pub(crate) root: &'a RenderNode,
  pub(crate) results: LayoutView<'a>,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) properties: &'a PropertyTrees,
  /// The boxes it paints more than one fragment of, none for a scene of one fragment of each.
  fragments: Option<&'a FragmentPlacements>,
}

impl<'a> SceneView<'a> {
  pub(crate) fn of_scene(scene: &'a Scene) -> Self {
    Self {
      root: &scene.root,
      results: scene.results.view(),
      contexts: &scene.contexts,
      properties: &scene.properties,
      fragments: scene.fragments.as_ref(),
    }
  }

  pub(crate) fn of_window(scene: &'a SceneRef<'_>) -> Self {
    Self {
      root: scene.root,
      results: scene.results,
      contexts: &scene.contexts,
      properties: &scene.properties,
      fragments: Some(&scene.fragments),
    }
  }

  /// The layout of the fragment `paint` paints, with its box set to paint it.
  pub(crate) fn enter(&self, paint: &NodePaint) -> takumi_core::Result<ComputedLayout> {
    let layout = self
      .results
      .fragment_layout(paint.node_id, paint.fragment_index)?;

    if let Some(fragments) = self.fragments {
      fragments.enter(self.root, paint, layout.location);
    }
    Ok(layout)
  }

  /// Visits every node paint of the scene, in paint order.
  pub(crate) fn for_each_paint(self, mut visit: impl FnMut(&NodePaint)) {
    fn walk(view: SceneView<'_>, id: usize, visit: &mut impl FnMut(&NodePaint)) {
      let Some(context) = view.contexts.get(id) else {
        return;
      };

      for paint in context.roots() {
        visit(paint);
      }
      for bucket in context.in_paint_order() {
        for item in bucket {
          match &item.kind {
            PaintItemKind::Node(paint) => visit(paint),
            PaintItemKind::Context(child) => walk(view, *child, visit),
            PaintItemKind::CollapsedBorders(_) => {}
          }
        }
      }
    }

    walk(self, 0, &mut visit);
  }

  pub(crate) fn emitter(
    self,
    state: &'a DocumentState<'a>,
    inline: Option<&'a InlineCache<'a>>,
    chunks: Option<&'a SceneChunks<'a>>,
    tagged: bool,
  ) -> Emitter<'a> {
    Emitter {
      scene: self,
      document: state,
      inline,
      chunks,
      window: Window::default(),
      tagged: tagged && state.tags.is_some(),
      color_filter: None,
    }
  }
}

// The root fills the content box like a browser body, and keeps the margins of what it holds
// inside it.
fn fill_root(node: Node, viewport: Viewport) -> Node {
  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::FlowRoot))
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
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Length::Percentage(100.0)))
      .with(StyleDeclaration::height(Length::Percentage(100.0))),
  );
  let mut root = RenderNode::from_node(context, area);

  root.children = Some(Box::new([child]));
  root
}

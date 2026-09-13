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
    Affine, BoxSizing, BreakBetween, ComputedStyle, Display, FlexDirection, FlexGrow, FontFamily,
    Lang, Length, PageName, Position, SizingContext, Style, StyleDeclaration, StyleSheet, ZIndex,
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
  page_context::PageContexts,
  pagination::{PageGroup, PageGroups},
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

  /// The content column, the `fixed` subtrees attached to the initial
  /// containing block, and the page groups the `page` property opens. The
  /// fixed boxes repeat on every page, so they lay out against the page area
  /// instead of the column.
  ///
  /// A page group is wrapped in a page-area box laid out in the same column
  /// at the group's page content width, with a page break before and after
  /// the group; its pages translate the column so the box lands at their
  /// left margin. The group's ancestors keep the unnamed page's width, and a
  /// box nested in a group cannot open another. Viewport units take the
  /// first page's area, so a document that opens with a named page sizes
  /// `100vh` to that page.
  pub(crate) fn prepare_paged(
    &self,
    node: Node,
    contexts: &mut PageContexts,
  ) -> Result<PagedTree, PdfError> {
    let unnamed = &contexts.unnamed().frame;
    let viewport = unnamed.column();
    let page_area = unnamed.page_area;
    let node = fill_root(node, viewport);
    // A repeated box holding a counter lays out again per page, from the subtree
    // its preorder position names in the tree it was taken from.
    let source = has_page_counters(&node).then(|| node.clone());
    let mut root = RenderNode::from_node(&self.context(viewport), node);
    let repeated = take_repeating_fixed(&mut root);
    let runs = PageGroupRun::collect(&root);

    for run in &runs {
      contexts.ensure(self, Some(&run.name))?;
    }
    if let Some(first) = runs.first().filter(|run| run.opens_the_document()) {
      let frame = &contexts.get(Some(&first.name)).frame;

      set_viewport(
        &mut root,
        viewport.with_unit_reference(Size {
          width: frame.content_width,
          height: frame.window_height,
        }),
      );
    }
    for run in &runs {
      let width = contexts.get(Some(&run.name)).frame.content_width;
      let last = run.members.len() - 1;

      for (position, path) in run.members.iter().enumerate() {
        wrap_in_page_area(&mut root, path, width, position == 0, position == last);
      }
    }
    let content = PreparedTree::lay_out(root, viewport)?;
    let groups = runs
      .iter()
      .filter_map(|run| {
        let (left, top, _) = content.box_bounds(run.members.first()?)?;
        let (_, _, bottom) = content.box_bounds(run.members.last()?)?;

        Some(PageGroup {
          name: run.name.clone(),
          left,
          top,
          bottom,
        })
      })
      .collect();
    let in_a_group = |path: &[usize]| {
      runs.iter().any(|run| {
        run
          .members
          .iter()
          .any(|member| path.starts_with(member) || member.starts_with(path))
      })
    };
    let groups = PageGroups::new(groups, content.shown_extents(in_a_group));
    let page_context = self.context(page_area);
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
        let prepared = PreparedTree::lay_out(page_root(&page_context, node.clone()), page_area)?;

        Ok(Repeatable::fixed(prepared, template, node, page_area))
      })
      .collect::<Result<Vec<_>, PdfError>>()?;

    Ok(PagedTree {
      content,
      repeated,
      groups,
    })
  }
}

/// Sets the viewport every box in the subtree sizes against.
pub(crate) fn set_viewport(node: &mut RenderNode, viewport: Viewport) {
  node.context.sizing.viewport = viewport;
  for child in node.children.as_deref_mut().unwrap_or_default() {
    set_viewport(child, viewport);
  }
}

/// What the content column lays out to: the column, the repeated boxes, and
/// the page groups in it.
pub(crate) struct PagedTree {
  pub(crate) content: PreparedTree,
  pub(crate) repeated: Vec<Repeatable>,
  pub(crate) groups: PageGroups,
}

/// A run of adjacent sibling boxes that name the same page: one page group,
/// per css-page-3 §8.1, which breaks only where the page name changes.
struct PageGroupRun {
  name: Arc<str>,
  /// The paths of the boxes, in document order.
  members: Vec<Vec<usize>>,
}

impl PageGroupRun {
  fn collect(root: &RenderNode) -> Vec<Self> {
    fn walk(node: &RenderNode, path: &mut Vec<usize>, runs: &mut Vec<PageGroupRun>) {
      if let PageName::Named(name) = &node.context.style.page {
        match runs.last_mut() {
          Some(run) if run.name == *name && run.follows(path) => run.members.push(path.clone()),
          _ => runs.push(PageGroupRun {
            name: name.clone(),
            members: vec![path.clone()],
          }),
        }
        return;
      }
      for (index, child) in node
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
      {
        path.push(index);
        walk(child, path, runs);
        path.pop();
      }
    }

    let mut runs = Vec::new();

    walk(root, &mut Vec::new(), &mut runs);
    runs
  }

  /// Whether `path` is the sibling right after the run's last box.
  fn follows(&self, path: &[usize]) -> bool {
    let Some(last) = self.members.last() else {
      return false;
    };
    let (Some((&last_index, parent)), Some((&index, candidate_parent))) =
      (last.split_last(), path.split_last())
    else {
      return false;
    };

    parent == candidate_parent && index == last_index + 1
  }

  /// Whether nothing lays out before this run, so its page is the first.
  fn opens_the_document(&self) -> bool {
    self
      .members
      .first()
      .is_some_and(|path| path.iter().all(|&index| index == 0))
  }
}

/// Puts the box at `path` inside a block as wide as its page's content box,
/// which breaks the page where the run starts and ends.
fn wrap_in_page_area(root: &mut RenderNode, path: &[usize], width: f32, first: bool, last: bool) {
  let Some((&index, parent_path)) = path.split_last() else {
    return;
  };
  let Some(parent) = root.node_at_path_mut(parent_path) else {
    return;
  };
  let Some(children) = parent.children.take() else {
    return;
  };
  let mut children = children.into_vec();
  let child = children.remove(index);
  let mut area = RenderNode::anonymous_block(&parent.context, child);
  let style = &mut area.context.style;

  style.width = Length::Px(width).into();
  style.box_sizing = BoxSizing::BorderBox;
  style.flex_shrink = Some(FlexGrow(0.0));
  style.border_top_width = Length::zero().into();
  style.border_right_width = Length::zero().into();
  style.border_bottom_width = Length::zero().into();
  style.border_left_width = Length::zero().into();
  style.break_before = if first {
    BreakBetween::Page
  } else {
    BreakBetween::Auto
  };
  style.break_after = if last {
    BreakBetween::Page
  } else {
    BreakBetween::Auto
  };
  children.insert(index, area);
  parent.children = Some(children.into_boxed_slice());
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

  /// The column extents of the boxes that painted something, leaving out
  /// those `skip` names.
  pub(crate) fn shown_extents(&self, skip: impl Fn(&[usize]) -> bool) -> Vec<(f32, f32)> {
    let mut extents = Vec::new();

    self.for_each_paint(|paint| {
      if let Some(bounds) = paint.paint_bounds
        && !skip(&paint.path)
      {
        extents.push((bounds.top as f32, bounds.bottom as f32));
      }
    });
    extents
  }

  /// The column left, top, and bottom of the box at `path`.
  pub(crate) fn box_bounds(&self, path: &[usize]) -> Option<(f32, f32, f32)> {
    let mut bounds = None;

    self.for_each_paint(|paint| {
      if paint.path == path
        && let Ok(layout) = self.results.layout(paint.node_id)
      {
        let (x, y) = (paint.transform.x, paint.transform.y);

        bounds = Some((x, y, y + layout.size.height));
      }
    });
    bounds
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

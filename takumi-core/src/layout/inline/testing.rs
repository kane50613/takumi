//! The tree and atomic inline layout the inline layout tests share.

use std::{rc::Rc, sync::Arc};

use crate::{
  context::RenderContext,
  fonts::shaping::testing::{GEIST_MONO, register},
  geometry::{AvailableSpace, NodeId, Size},
  layout::{
    block_layout_algorithm::BlockLayoutAlgorithm,
    constraint_space::{AutoSizeBehavior, ConstraintSpace},
    inline::{
      atomic_inline::BoxAtomicInline, inline_item_result::LayoutResult,
      inline_layout_algorithm::LineBoxFragment, inline_node_data::InlineNodeData,
      line_breaker::AtomicInlineLayout,
    },
    layout_result::LayoutResult as BoxLayoutResult,
    node::Node,
    tree::{LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
  resources::font::Fonts,
  style::{SizingContext, StyleSheet},
  viewport::Viewport,
};

/// Blink's tests lay out with Ahem, whose glyphs are 1em wide; Geist Mono's are 0.6em, so their
/// widths here are Blink's times 0.6.
const CSS: &str = ".container { display: block; font-size: 10px; font-family: 'Geist Mono'; line-height: 1 } \
  .span { display: inline } .atomic { display: inline-block; width: 18px; height: 10px }";

pub(crate) struct TestAtomicInline;

impl<'c> AtomicInlineLayout<'c> for TestAtomicInline {
  fn layout(&mut self, node: &'c RenderNode) -> LayoutResult {
    let mut tree = LayoutTree::from_render_node(node);
    let result =
      BoxAtomicInline::new(&mut tree, Size::all(AvailableSpace::MaxContent)).layout(node);

    LayoutResult {
      baseline: None,
      ..result
    }
  }

  fn layout_float(&mut self, node: &'c RenderNode, space: &ConstraintSpace) -> Rc<BoxLayoutResult> {
    let mut tree = LayoutTree::from_render_node(node);

    BoxAtomicInline::new(&mut tree, Size::all(AvailableSpace::MaxContent)).layout_float(node, space)
  }

  fn layout_block_in_inline(
    &mut self,
    node: &'c RenderNode,
    space: &ConstraintSpace,
  ) -> Rc<BoxLayoutResult> {
    let mut tree = LayoutTree::from_render_node(node);

    BoxAtomicInline::new(&mut tree, Size::all(AvailableSpace::MaxContent))
      .layout_block_in_inline(node, space)
  }

  fn min_max_sizes(&mut self, node: &'c RenderNode) -> (LayoutUnit, LayoutUnit) {
    let width = self.layout(node).inline_size;

    (width, width)
  }
}

/// `root` laid out with `css`, its only font the family `family` at `path`.
pub(crate) fn render_root(path: &str, family: &str, root: Node, css: &str) -> RenderNode {
  let mut fonts = Fonts::default();

  register(&mut fonts, path, family);

  let context = RenderContext::builder()
    .fonts(fonts.snapshot())
    .sizing(
      SizingContext::builder()
        .viewport(Viewport::default())
        .build(),
    )
    .stylesheet(Arc::new(StyleSheet::parse(css).expect("stylesheet parses")))
    .build();

  RenderNode::from_node(&context, root)
}

/// `children` in a block of Geist Mono.
pub(crate) fn render_tree(children: impl Into<Vec<Node>>, css: &str) -> RenderNode {
  render_root(
    GEIST_MONO,
    "Geist Mono",
    Node::container(children).with_class_name("container"),
    &format!("{CSS} {css}"),
  )
}

pub(crate) fn span(text: &str) -> Node {
  Node::text(text).with_class_name("span")
}

/// `block`'s lines laid out `available_width` wide as the root of a layout tree of its own, for
/// `read` to read with the items they lay out.
pub(crate) fn read_lines<'r, R>(
  block: &'r RenderNode,
  available_width: f32,
  read: impl FnOnce(&InlineNodeData<'r>, &[LineBoxFragment<'_, 'r>]) -> R,
) -> R {
  let mut tree = LayoutTree::from_render_node(block);
  let items_data = tree.inline_node_data(usize::from(NodeId::ROOT));
  let mut space = ConstraintSpace::new(
    Some(LayoutUnit::from_float(available_width)),
    block.context.style.inherited_data.direction,
  );

  space.is_new_formatting_context = true;
  space.inline_auto_behavior = AutoSizeBehavior::StretchImplicit;

  let mut ignore_line_clamp = false;

  loop {
    let mut algorithm = BlockLayoutAlgorithm::new(&mut tree, NodeId::ROOT, &space);

    if ignore_line_clamp {
      algorithm = algorithm.ignoring_line_clamp();
    }

    let lines = algorithm
      .start_layout()
      .and_then(|mut previous| algorithm.layout_lines(&items_data, &mut previous))
      .expect("the lines lay out");

    if !ignore_line_clamp && algorithm.should_relayout_ignoring_line_clamp() {
      ignore_line_clamp = true;
      continue;
    }

    return read(&items_data, &lines);
  }
}

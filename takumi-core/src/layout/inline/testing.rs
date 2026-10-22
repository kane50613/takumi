//! The tree and atomic inline layout the inline layout tests share.

use std::sync::Arc;

use crate::{
  context::RenderContext,
  fonts::shaping::testing::{GEIST_MONO, register},
  geometry::{AvailableSpace, Size},
  layout::{
    inline::{
      atomic_inline::BoxAtomicInline, inline_item_result::LayoutResult,
      line_breaker::AtomicInlineLayout,
    },
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

  fn layout_block_in_inline(&mut self, node: &'c RenderNode) -> LayoutResult {
    let mut tree = LayoutTree::from_render_node(node);

    BoxAtomicInline::new(&mut tree, Size::all(AvailableSpace::MaxContent))
      .layout_block_in_inline(node)
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

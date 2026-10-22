//! Blink's [`BlockNode::LayoutAtomicInline`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_node.cc)
//! and its `Layout` of a block-in-inline, each box laid out as a node of the layout tree its
//! inline formatting context belongs to.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use super::{
  inline_item_result::{CollapsedMargins, LayoutResult},
  line_breaker::AtomicInlineLayout,
};
use crate::{
  geometry::{AvailableSpace, NodeId, Size},
  layout::{
    constraint_space::{AutoSizeBehavior, BaselineAlgorithmType, CacheSlot, ConstraintSpace},
    layout_result::LayoutResult as BoxLayoutResult,
    margin_strut::MarginStrut,
    tree::{LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::BaselineSource,
};

/// Lays out the atomic inlines of an inline formatting context in its content box,
/// `available_space`.
pub(crate) struct BoxAtomicInline<'t, 'r> {
  pub(crate) tree: &'t mut LayoutTree<'r>,
  pub(crate) available_space: Size<AvailableSpace>,
  /// Whether its boxes lay out to be placed, or only to be measured.
  pub(crate) cache_slot: CacheSlot,
}

impl<'t, 'r> BoxAtomicInline<'t, 'r> {
  pub(crate) fn new(tree: &'t mut LayoutTree<'r>, available_space: Size<AvailableSpace>) -> Self {
    Self {
      tree,
      available_space,
      cache_slot: CacheSlot::Measure,
    }
  }

  /// The atomic inline `render_node` laid out as [`AtomicInlineLayout::layout`] lays it out.
  fn lay_out_atomic_inline(
    &mut self,
    render_node: &'r RenderNode,
  ) -> (NodeId, Rc<BoxLayoutResult>) {
    let node = self.tree.atomic_inline_node(render_node);
    let space = self.atomic_inline_space(node);

    (node, self.tree.layout(node, &space))
  }

  /// The inline content box `render_node` laid out as its line lays it out, a block-in-inline
  /// or a float as its line laid it out to `line_result`.
  pub(crate) fn lay_out_inline_content_box(
    &mut self,
    render_node: &'r RenderNode,
    line_result: Option<Rc<BoxLayoutResult>>,
  ) -> (NodeId, Rc<BoxLayoutResult>) {
    if let Some(result) = line_result {
      return (self.tree.atomic_inline_node(render_node), result);
    }

    self.lay_out_atomic_inline(render_node)
  }

  /// The space Blink's `LayoutAtomicInline` lays `node` out in: a new formatting context in the
  /// containing block's available size, its baselines an inline-block's unless `baseline-source`
  /// picks one.
  fn atomic_inline_space(&self, node: NodeId) -> ConstraintSpace {
    let mut space = self.block_space(node);

    space.is_new_formatting_context = true;
    space.baseline_algorithm = match self.tree.layout_style(node).baseline_source {
      BaselineSource::Auto => BaselineAlgorithmType::InlineBlock,
      BaselineSource::First | BaselineSource::Last => BaselineAlgorithmType::Default,
    };
    space
  }

  /// The space of a box in the containing block's available size.
  fn block_space(&self, node: NodeId) -> ConstraintSpace {
    let mut space = ConstraintSpace::new(
      self.available_space.width.definite(),
      self.tree.layout_style(node).direction,
    );

    space.available_block_size = self.available_space.height.definite();
    space.percentage_resolution_block_size = space.available_block_size;
    space.cache_slot = self.cache_slot;
    space
  }
}

impl<'r> AtomicInlineLayout<'r> for BoxAtomicInline<'_, 'r> {
  fn layout(&mut self, render_node: &'r RenderNode) -> LayoutResult {
    let (node, result) = self.lay_out_atomic_inline(render_node);

    LayoutResult {
      inline_size: result.size.inline_size,
      block_size: result.size.block_size,
      baseline: result.inline_baseline(self.tree.layout_style(node)),
      collapsed_margins: None,
    }
  }

  fn layout_float(
    &mut self,
    render_node: &'r RenderNode,
    space: &ConstraintSpace,
  ) -> Rc<BoxLayoutResult> {
    let node = self.tree.atomic_inline_node(render_node);

    self.tree.layout(node, space)
  }

  fn layout_block_in_inline(
    &mut self,
    render_node: &'r RenderNode,
    space: &ConstraintSpace,
  ) -> Rc<BoxLayoutResult> {
    let node = self.tree.atomic_inline_node(render_node);
    let mut space = space.clone();

    space.inline_auto_behavior = AutoSizeBehavior::StretchImplicit;
    self.tree.layout(node, &space)
  }

  fn min_max_sizes(&mut self, render_node: &'r RenderNode) -> (LayoutUnit, LayoutUnit) {
    let node = self.tree.atomic_inline_node(render_node);
    let mut space = self.atomic_inline_space(node);

    space.available_inline_size = None;

    let sizes = self
      .tree
      .min_and_max_content_contribution(node, &space)
      .sizes;

    (sizes.min_size, sizes.max_size)
  }
}

impl LayoutResult {
  /// A block-in-inline laid out as `result` in its line's `space`, its margins collapsing through
  /// its block start and end. Approximate: the start margins include those before the line, which
  /// Blink's anonymous block does not paint over.
  pub(crate) fn of_block_in_inline(result: &BoxLayoutResult, space: &ConstraintSpace) -> Self {
    let start = if result.is_self_collapsing {
      result.end_margin_strut
    } else {
      let mut start = MarginStrut::default();

      start.append(result.bfc_block_offset.map_or(LayoutUnit::ZERO, |offset| {
        offset - space.bfc_offset.block_offset
      }));
      start
    };

    Self {
      inline_size: result.size.inline_size,
      block_size: result.size.block_size,
      baseline: result.first_baseline,
      collapsed_margins: Some(CollapsedMargins {
        start,
        end: result.end_margin_strut,
        is_self_collapsing: result.is_self_collapsing,
      }),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::BoxAtomicInline;
  use crate::{
    geometry::{AvailableSpace, Size},
    layout::{
      inline::{line_breaker::AtomicInlineLayout, testing::render_tree},
      node::Node,
      tree::LayoutTree,
    },
    layout_unit::LayoutUnit,
  };

  fn atomic_inline<'t, 'r>(tree: &'t mut LayoutTree<'r>, width: f32) -> BoxAtomicInline<'t, 'r> {
    BoxAtomicInline::new(
      tree,
      Size {
        width: AvailableSpace::Definite(width),
        height: AvailableSpace::MaxContent,
      },
    )
  }

  /// An inline block's border box holds its padding and borders, not its margins.
  #[test]
  fn an_inline_block_lays_out_its_border_box() {
    let block = render_tree(
      [Node::container([]).with_class_name("box")],
      ".box { display: inline-block; box-sizing: content-box; width: 50px; height: 20px; \
       padding: 5px; border: 1px solid; margin: 7px }",
    );
    let child = &block.children.as_deref().expect("children")[0];
    let mut tree = LayoutTree::from_render_node(&block);
    let result = atomic_inline(&mut tree, 100.0).layout(child);

    assert_eq!(
      (result.inline_size, result.block_size),
      (LayoutUnit::from_int(62), LayoutUnit::from_int(32))
    );
  }

  /// A percentage width resolves against the line's available width.
  #[test]
  fn a_percentage_width_takes_the_available_width() {
    let block = render_tree(
      [Node::container([]).with_class_name("box")],
      ".box { display: inline-block; width: 50%; height: 10px }",
    );
    let child = &block.children.as_deref().expect("children")[0];
    let mut tree = LayoutTree::from_render_node(&block);

    assert_eq!(
      atomic_inline(&mut tree, 80.0).layout(child).inline_size,
      LayoutUnit::from_int(40)
    );
  }

  /// A replaced box's percentage padding takes the containing block's width.
  #[test]
  fn a_replaced_box_resolves_percentage_padding() {
    let block = render_tree(
      [Node::image("data:image/png;base64,iVBOR").with_class_name("image")],
      ".image { width: 20px; height: 10px; box-sizing: content-box; padding-left: 10% }",
    );
    let child = &block.children.as_deref().expect("children")[0];
    let mut tree = LayoutTree::from_render_node(&block);
    let wide = atomic_inline(&mut tree, 200.0).layout(child).inline_size;
    let narrow = atomic_inline(&mut tree, 100.0).layout(child).inline_size;

    assert_eq!(wide - narrow, LayoutUnit::from_int(10));
  }

  /// An inline block's baseline is its last line's, below its border box's top.
  #[test]
  fn an_inline_block_takes_its_last_line_baseline() {
    let block = render_tree(
      [Node::container([Node::text("a")]).with_class_name("box")],
      ".box { display: inline-block; padding-top: 4px; margin-top: 3px }",
    );
    let child = &block.children.as_deref().expect("children")[0];
    let mut tree = LayoutTree::from_render_node(&block);

    // Geist Mono at 10px with `line-height: 1` puts the baseline 8px below the line top.
    assert_eq!(
      atomic_inline(&mut tree, 100.0).layout(child).baseline,
      Some(LayoutUnit::from_int(12))
    );
  }

  /// An inline block's min-content and max-content widths.
  #[test]
  fn an_inline_block_has_min_and_max_content_widths() {
    let block = render_tree(
      [Node::container([Node::text("aa bbb")]).with_class_name("box")],
      ".box { display: inline-block; padding: 1px }",
    );
    let child = &block.children.as_deref().expect("children")[0];
    let mut tree = LayoutTree::from_render_node(&block);

    assert_eq!(
      atomic_inline(&mut tree, 100.0).min_max_sizes(child),
      (LayoutUnit::from_int(20), LayoutUnit::from_int(38))
    );
  }
}

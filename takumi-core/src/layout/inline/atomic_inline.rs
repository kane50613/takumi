//! Blink's [`BlockNode::LayoutAtomicInline`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_node.cc)
//! for takumi's boxes, which taffy lays out.

use super::{inline_item_result::LayoutResult, line_breaker::AtomicInlineLayout};
use crate::{
  geometry::{AvailableSpace, Size},
  layout::tree::RenderNode,
  layout_unit::LayoutUnit,
};

/// Lays out atomic inlines in their containing block's available space.
pub(crate) struct BoxAtomicInline {
  pub(crate) available_space: Size<AvailableSpace>,
}

impl BoxAtomicInline {
  /// The border box of `node` in `available_space`, and its baseline below the margin box's top.
  fn border_box(
    node: &RenderNode,
    available_space: Size<AvailableSpace>,
  ) -> (Size<f32>, Option<f32>) {
    let metrics = node.layout_atomic_inline(available_space);

    // An atomic subtree measures its border box; a replaced box, its content box.
    if node.participates_as_inline_box() {
      return (metrics.size, metrics.baseline_offset);
    }

    // Percentages resolve against the containing block's width, zero when it is indefinite.
    let containing_block_width = match available_space.width {
      AvailableSpace::Definite(width) => width,
      _ => 0.0,
    };
    let padding = node.padding_px_against(containing_block_width);
    let border = node.border_px();

    (
      Size {
        width: metrics.size.width + padding.left + padding.right + border.left + border.right,
        height: metrics.size.height + padding.top + padding.bottom + border.top + border.bottom,
      },
      metrics.baseline_offset,
    )
  }
}

impl<'c> AtomicInlineLayout<'c> for BoxAtomicInline {
  fn layout(&mut self, node: &'c RenderNode) -> LayoutResult {
    let (size, baseline_offset) = Self::border_box(node, self.available_space);
    let margin_top = node.margin_px().top;

    LayoutResult {
      inline_size: LayoutUnit::from_float(size.width),
      block_size: LayoutUnit::from_float(size.height),
      baseline: baseline_offset.map(|baseline| LayoutUnit::from_float(baseline - margin_top)),
    }
  }

  fn min_max_sizes(&mut self, node: &'c RenderNode) -> (LayoutUnit, LayoutUnit) {
    let at_width = |width| Size {
      width,
      height: self.available_space.height,
    };
    let (min_size, _) = Self::border_box(node, at_width(AvailableSpace::MinContent));
    let (max_size, _) = Self::border_box(node, at_width(AvailableSpace::MaxContent));

    (
      LayoutUnit::from_float(min_size.width),
      LayoutUnit::from_float(max_size.width),
    )
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
    },
    layout_unit::LayoutUnit,
  };

  fn atomic_inline(width: f32) -> BoxAtomicInline {
    BoxAtomicInline {
      available_space: Size {
        width: AvailableSpace::Definite(width),
        height: AvailableSpace::MaxContent,
      },
    }
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
    let result = atomic_inline(100.0).layout(child);

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

    assert_eq!(
      atomic_inline(80.0).layout(child).inline_size,
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

    assert_eq!(
      atomic_inline(200.0).layout(child).inline_size
        - atomic_inline(100.0).layout(child).inline_size,
      LayoutUnit::from_int(10)
    );
  }

  /// An inline block's baseline is its last line's, below its border box's top.
  #[test]
  fn an_inline_block_takes_its_last_line_baseline() {
    let block = render_tree(
      [Node::container([Node::text("a")]).with_class_name("box")],
      ".box { display: inline-block; padding-top: 4px; margin-top: 3px }",
    );
    let child = &block.children.as_deref().expect("children")[0];

    // Geist Mono at 10px with `line-height: 1` puts the baseline 8px below the line top.
    assert_eq!(
      atomic_inline(100.0).layout(child).baseline,
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

    assert_eq!(
      atomic_inline(100.0).min_max_sizes(child),
      (LayoutUnit::from_int(20), LayoutUnit::from_int(38))
    );
  }
}

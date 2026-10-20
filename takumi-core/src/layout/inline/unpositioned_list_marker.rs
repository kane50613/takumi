//! Blink's [`UnpositionedListMarker`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/list/unpositioned_list_marker.cc):
//! an outside list marker, placed beside the first line box of the block whose inline content
//! holds it. Approximate: Blink's list item places the marker from its own border box, while
//! takumi's marker travels into the first block that holds a line, so a list item whose content
//! sits in a nested block measures from that block instead and grows that block, not the item,
//! to push the content down. It also skips `ComputeIntrudedFloatOffset`, since the inline layout
//! sees no float outside its own block, and `AddToBoxWithoutLineBoxes`, since takumi always gives
//! the marker a line.

use super::{
  inline_item::InlineItemType,
  inline_item_result::{LayoutResult, LineBoxStrut},
  inline_layout_algorithm::LineBoxFragment,
  inline_node_data::InlineNodeData,
  line_breaker::AtomicInlineLayout,
};
use crate::{
  geometry::{ComputedLayout, Point},
  layout::{list_marker::inline_margins_for_outside, tree::RenderNode},
  layout_unit::LayoutUnit,
  style::Direction,
};

/// Blink's `UnpositionedListMarker`.
#[derive(Clone, Copy)]
pub(crate) struct UnpositionedListMarker<'c> {
  pub(crate) node: &'c RenderNode,
}

impl<'c> UnpositionedListMarker<'c> {
  /// The outside list marker among the items of `items_data`.
  pub(crate) fn of(items_data: &InlineNodeData<'c>) -> Option<Self> {
    let item = items_data
      .items
      .iter()
      .find(|item| item.item_type == InlineItemType::ListMarker)?;

    item.layout_object.map(|object| Self {
      node: object.node(),
    })
  }

  /// Blink's `Layout`: the marker laid out as an atomic inline.
  pub(crate) fn layout(self, atomic_inline: &mut dyn AtomicInlineLayout<'c>) -> LayoutResult {
    atomic_inline.layout(self.node)
  }

  /// Blink's `InlineOffset`: where the marker starts from its box's inline-start border edge.
  fn inline_offset(self, marker_inline_size: LayoutUnit) -> LayoutUnit {
    inline_margins_for_outside(&self.node.context, marker_inline_size).0
  }

  /// Blink's `ContentAlignmentBaseline` over line boxes: the line the marker aligns with, past
  /// the empty lines floats leave before it.
  pub(crate) fn content_alignment_line<'l, 'a>(
    lines: &'l [LineBoxFragment<'a, 'c>],
  ) -> Option<(usize, &'l LineBoxFragment<'a, 'c>)> {
    lines
      .iter()
      .enumerate()
      .find(|(_, line)| line.line_box_bfc_block_offset.is_some())
  }

  /// Blink's `AddToBox` block offset: a marker whose ascent passes the line's pushes the line, and
  /// the content after it, down.
  pub(crate) fn push_content_down(
    self,
    lines: &mut [LineBoxFragment<'_, 'c>],
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) {
    let Some((index, line)) = Self::content_alignment_line(lines) else {
      return;
    };
    let marker_ascent = self
      .layout(atomic_inline)
      .baseline_metrics(&LineBoxStrut::default())
      .ascent;
    let baseline_adjust = line.metrics.ascent - marker_ascent;

    if baseline_adjust >= LayoutUnit::ZERO {
      return;
    }

    for line in &mut lines[index..] {
      line.move_in_block_direction(-baseline_adjust);
    }
  }

  /// Blink's `AddToBox`: the marker's border box, laid out as `result`, beside `line` in the
  /// content box of `layout`, whose block lays out in `direction`.
  pub(crate) fn offset(
    self,
    result: &LayoutResult,
    line: &LineBoxFragment<'_, 'c>,
    layout: ComputedLayout,
    direction: Direction,
  ) -> Point<f32> {
    let baseline = line.baseline().unwrap_or(LayoutUnit::ZERO);
    let marker_ascent = result.baseline_metrics(&LineBoxStrut::default()).ascent;
    let inline_offset = self.inline_offset(result.inline_size).to_float();
    let border_box_x = match direction {
      Direction::Ltr => inline_offset,
      Direction::Rtl => layout.size.width - inline_offset - result.inline_size.to_float(),
    };

    Point {
      x: border_box_x - layout.content_box_offset().x,
      y: (baseline - marker_ascent).to_float(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::UnpositionedListMarker;
  use crate::{
    geometry::{AvailableSpace, ComputedLayout, Point, Rect, Size},
    layout::{
      inline::{atomic_inline::BoxAtomicInline, inline_node::InlineNode, testing::render_tree},
      node::Node,
    },
    layout_unit::LayoutUnit,
    style::Direction,
  };

  /// A 24px square PNG, taller than a 10px line's ascent.
  const SQUARE_DATA_URI: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR42mNQTX79n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAABNdoDMrNJeUAAAAABJRU5ErkJggg==";

  const ITEM_CSS: &str = ".item { display: list-item; list-style-type: decimal; padding-left: 6px; \
     border-left: 2px solid }";

  fn atomic_inline() -> BoxAtomicInline {
    BoxAtomicInline {
      available_space: Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::MaxContent,
      },
    }
  }

  /// The item's 100px content box behind its 2px border and 6px padding.
  fn item_layout() -> ComputedLayout {
    ComputedLayout::new(
      Point::ZERO,
      Size {
        width: 108.0,
        height: 10.0,
      },
      Rect {
        left: 2.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
      },
      Rect {
        left: 6.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
      },
    )
  }

  /// The marker's offset in the item's content box, laid out in `host` or else the item's
  /// direction, and its inline size.
  fn marker_offset(css: &str, host: Option<Direction>) -> (Point<f32>, f32) {
    let tree = render_tree(
      [Node::container([Node::text("item")]).with_class_name("item")],
      css,
    );
    let item = &tree.children.as_deref().expect("children")[0];
    let node = InlineNode::new(item);
    let data = node.prepare_layout();
    let mut atomic_inline = atomic_inline();
    let lines = node.layout_lines(&data, LayoutUnit::from_int(100), &mut atomic_inline);
    let marker = UnpositionedListMarker::of(&data).expect("an outside marker");
    let (_, line) = UnpositionedListMarker::content_alignment_line(&lines).expect("a line");
    let result = marker.layout(&mut atomic_inline);

    (
      marker.offset(
        &result,
        line,
        item_layout(),
        host.unwrap_or(item.context.style.inherited_data.direction),
      ),
      result.inline_size.to_float(),
    )
  }

  /// Blink's `InlineOffset` hangs a counter marker from the item's border box, past its padding
  /// and border.
  #[test]
  fn a_marker_hangs_outside_the_item_border_box() {
    let (offset, width) = marker_offset(ITEM_CSS, None);

    assert_eq!(offset.x, -8.0 - width);
  }

  /// Right to left, the marker hangs past the item's right border edge.
  #[test]
  fn a_right_to_left_marker_hangs_past_the_right_edge() {
    let (offset, _) = marker_offset(&format!("{ITEM_CSS} .item {{ direction: rtl }}"), None);

    assert_eq!(offset.x, 100.0);
  }

  /// Blink's `AddToBox` offsets the marker in the direction of the block that holds the line,
  /// which can differ from the list item's.
  #[test]
  fn a_marker_follows_the_direction_of_the_block_holding_the_line() {
    let (offset, _) = marker_offset(ITEM_CSS, Some(Direction::Rtl));

    assert_eq!(offset.x, 100.0);
  }

  /// Blink's `AddToBox` pushes the content down when the marker's ascent passes the line's.
  #[test]
  fn a_taller_marker_pushes_the_line_down() {
    let tree = render_tree(
      [Node::container([Node::text("item")]).with_class_name("item")],
      &format!(".item {{ display: list-item; list-style-image: url({SQUARE_DATA_URI}) }}"),
    );
    let item = &tree.children.as_deref().expect("children")[0];
    let node = InlineNode::new(item);
    let data = node.prepare_layout();
    let mut atomic_inline = atomic_inline();
    let mut lines = node.layout_lines(&data, LayoutUnit::from_int(100), &mut atomic_inline);
    let marker = UnpositionedListMarker::of(&data).expect("an outside marker");

    marker.push_content_down(&mut lines, &mut atomic_inline);

    assert_eq!(
      lines[0].line_box_bfc_block_offset,
      Some(LayoutUnit::from_int(24) - lines[0].metrics.ascent)
    );
  }
}

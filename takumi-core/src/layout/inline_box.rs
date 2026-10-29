//! What a replaced inline box paints where its line put it.

use crate::{
  geometry::{ComputedLayout, Point},
  layout::{
    inline::{InlineBoxItem, VisualInlineBox},
    tree::RenderNode,
  },
};

/// A replaced inline box, which paints its own chrome and content into `layout`.
pub struct ReplacedInlineBox<'n> {
  /// The node the box wraps.
  pub node: &'n RenderNode,
  /// The box's own layout, its location the box's origin in the container's border box.
  pub layout: ComputedLayout,
}

/// The replaced box `positioned` paints, or nothing at zero opacity or for an inline content box.
pub fn resolve_inline_box<'n>(
  positioned: &VisualInlineBox,
  item: &InlineBoxItem<'n>,
  container: ComputedLayout,
) -> Option<ReplacedInlineBox<'n>> {
  let node = item.render_node;

  if node.context.style.svg_data.opacity.0 == 0.0 || node.is_inline_content_box() {
    return None;
  }

  // The line advances by the margin box, but the box itself is its border box inside it, as
  // Blink's `LogicalLineBuilder::PlaceAtomicInline` places the fragment past its start margin.
  let location = container.content_box_offset()
    + Point {
      x: positioned.x + item.margin.left,
      y: positioned.y + item.margin.top,
    };

  Some(ReplacedInlineBox {
    node,
    layout: ComputedLayout {
      location,
      ..ComputedLayout::from(item)
    },
  })
}

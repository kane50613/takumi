//! Resolving what an inline box paints.
//!
//! An inline box is positioned by the inline layout rather than the paint list,
//! so every backend has to decide what it holds and, for an inline-level
//! container, lay its subtree out again at the size the line gave it. That
//! decision is the same everywhere; only the drawing differs.

use crate::geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Size};
use crate::layout::inline::{InlineBoxItem, VisualInlineBox};
use crate::layout::tree::{LayoutResults, LayoutTree, RenderNode};

/// What an inline box paints.
pub enum InlineBoxPaint<'n> {
  /// Replaced content, such as an image. The node paints its own chrome and
  /// content into `layout`.
  Replaced {
    /// The node the box wraps.
    node: &'n RenderNode,
    /// The box's own layout, in its own coordinate space.
    layout: ComputedLayout,
  },
  /// An inline-level container: `inline-block`, `inline-flex`, `inline-grid`,
  /// or a float. It carries a scene of its own.
  Container(Box<InlineSubtree>),
}

/// An inline-level container, laid out again at the size the line gave it.
pub struct InlineSubtree {
  /// The subtree root, cloned so it can own its layout.
  pub root: RenderNode,
  /// Layout of the subtree, rooted at [`NodeId::ROOT`].
  pub results: LayoutResults,
  /// The size the subtree was laid out at, its margins excluded.
  pub size: Size<f32>,
  /// Offset from the box origin to the subtree origin, i.e. its margins.
  pub margin_offset: Point<f32>,
}

/// Resolves what `positioned` paints, and where.
///
/// The returned point is the box's origin relative to the container's
/// border-box origin. A box with zero opacity resolves to nothing.
pub fn resolve_inline_box<'n>(
  positioned: &VisualInlineBox,
  item: &InlineBoxItem<'n>,
  container: ComputedLayout,
) -> Option<(Point<f32>, InlineBoxPaint<'n>)> {
  let node = item.render_node;

  if node.context.style.opacity.0 == 0.0 {
    return None;
  }
  let content = container.content_box_offset();
  let origin = Point {
    x: content.x + positioned.x,
    y: content.y + positioned.y,
  };

  if !node.participates_as_inline_box() {
    return Some((
      origin,
      InlineBoxPaint::Replaced {
        node,
        layout: ComputedLayout::from(item),
      },
    ));
  }
  let size = Size {
    width: (positioned.width - item.margin.horizontal()).max(0.0),
    height: (positioned.height - item.margin.vertical()).max(0.0),
  };
  let root = node.clone();
  let results = {
    let mut tree = LayoutTree::from_render_node(&root);

    tree.compute_layout(Size {
      width: AvailableSpace::Definite(size.width),
      height: AvailableSpace::Definite(size.height),
    });
    tree.into_results()
  };

  Some((
    origin,
    InlineBoxPaint::Container(Box::new(InlineSubtree {
      root,
      results,
      size,
      margin_offset: Point {
        x: item.margin.left,
        y: item.margin.top,
      },
    })),
  ))
}

impl InlineSubtree {
  /// The subtree's root node id.
  pub const ROOT: NodeId = NodeId::ROOT;
}

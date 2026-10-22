//! Resolving what an inline box paints.
//!
//! An inline box is positioned by the inline layout rather than the paint list,
//! so every backend has to decide what it holds and, for an outside list
//! marker, lay its subtree out again at the size the line gave it. That
//! decision is the same everywhere; only the drawing differs.

use crate::{
  error::Result,
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Size},
  layout::{
    inline::{InlineBoxItem, VisualInlineBox},
    tree::{LayoutResults, RenderNode},
  },
  scene::{Scene, SceneRequest},
  style::Affine,
};

/// What an inline box paints.
pub enum InlineBoxPaint<'n> {
  /// Replaced content, such as an image. The node paints its own chrome and
  /// content into `layout`.
  Replaced {
    /// The node the box wraps.
    node: &'n RenderNode,
    /// The box's own layout, its location the box's origin in the container's border box.
    layout: ComputedLayout,
  },
  /// An outside list marker, which carries a scene of its own.
  Container(Box<InlineSubtree>),
}

/// An outside list marker, laid out again at the size the line gave it.
pub struct InlineSubtree {
  /// The subtree root, cloned so it can own its layout.
  pub root: RenderNode,
  /// Layout of the subtree, rooted at [`NodeId::ROOT`].
  pub results: LayoutResults,
  /// The size the subtree was laid out at, its margins excluded.
  pub size: Size<f32>,
  /// Offset from the box origin to the subtree origin, i.e. its margins.
  pub margin_offset: Point<f32>,
  /// Blink's paint offset of the space the subtree root's layout location is measured in.
  paint_offset: Point<f32>,
}

/// Resolves what `positioned` paints, and where, or nothing for a box at zero opacity or an inline
/// content box, which its scene paints as a box of the layout tree.
pub fn resolve_inline_box<'n>(
  positioned: &VisualInlineBox,
  item: &InlineBoxItem<'n>,
  container: ComputedLayout,
) -> Option<(Point<f32>, InlineBoxPaint<'n>)> {
  let node = item.render_node;

  (node.context.style.svg_data.opacity.0 != 0.0 && !node.is_inline_content_box())
    .then(|| InlineBoxPaint::of(positioned, item, container))
}

impl<'n> InlineBoxPaint<'n> {
  /// What `positioned` holds, and where it paints from relative to the container's border-box
  /// origin: a replaced box's border box, or a container's margin box, which
  /// [`InlineSubtree::border_box_origin`] moves to its border box.
  pub fn of(
    positioned: &VisualInlineBox,
    item: &InlineBoxItem<'n>,
    container: ComputedLayout,
  ) -> (Point<f32>, Self) {
    let node = item.render_node;
    let content = container.content_box_offset();
    let origin = Point {
      x: content.x + positioned.x,
      y: content.y + positioned.y,
    };
    let margin_offset = Point {
      x: item.margin.left,
      y: item.margin.top,
    };

    // The line advances by the margin box, but the box itself is its border box inside it, as
    // Blink's `LogicalLineBuilder::PlaceAtomicInline` places the fragment past its start margin.
    if !node.participates_as_inline_box() {
      let border_box = origin + margin_offset;

      return (
        border_box,
        Self::Replaced {
          node,
          layout: ComputedLayout {
            location: border_box,
            ..ComputedLayout::from(item)
          },
        },
      );
    }

    let size = Size {
      width: (positioned.width - item.margin.horizontal()).max(0.0),
      height: (positioned.height - item.margin.vertical()).max(0.0),
    };
    let root = node.clone();
    // Layout lays a root out in the space beside its margins, which the margin box holds.
    let results = LayoutResults::compute(
      &root,
      Size {
        width: AvailableSpace::Definite(positioned.width.max(0.0)),
        height: AvailableSpace::Definite(positioned.height.max(0.0)),
      },
    );

    (
      origin,
      Self::Container(Box::new(InlineSubtree {
        root,
        results,
        size,
        margin_offset,
        paint_offset: node.context.paint_offset + origin + margin_offset,
      })),
    )
  }
}

impl InlineSubtree {
  /// The subtree's root node id.
  pub const ROOT: NodeId = NodeId::ROOT;

  /// Where the subtree's root border box sits, for the inline box at `origin`.
  pub fn border_box_origin(&self, origin: Point<f32>) -> Point<f32> {
    origin + self.margin_offset
  }

  /// Builds the subtree's scene, its root placed by `transform`.
  pub fn into_scene(self, transform: Affine, paint_bounds: bool) -> Result<Scene> {
    let layers = SceneRequest {
      root: &self.root,
      layout_results: &self.results,
      transform,
      paint_offset: self.paint_offset,
      container_size: self.size.map(Some),
      paint_bounds,
    }
    .build()?;

    Ok(Scene::new(self.root, self.results, layers, self.size))
  }
}

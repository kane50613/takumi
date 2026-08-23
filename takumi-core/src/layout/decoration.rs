//! Backend-agnostic box-decoration geometry.
//!
//! The clip regions a `background-clip` fills — border, padding, and content
//! boxes with their inset radii — are pure functions of the border geometry and
//! computed layout. Resolving them here keeps the raster and SVG backends from
//! re-deriving the same inset/expand math by hand and drifting apart.

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout as Layout, Point, Rect, Size},
  layout::border::BorderProperties,
  style::Sides,
};

/// A rounded-rect clip region: its corner geometry (radii after any inset or
/// expand), box size, and offset from the border-box top-left.
#[derive(Debug, Clone, Copy)]
pub struct ClipBox {
  /// Corner geometry for the region's rounded rectangle.
  pub border: BorderProperties,
  /// The region's size.
  pub size: Size<f32>,
  /// The region's top-left, relative to the border box.
  pub offset: Point<f32>,
}

impl ClipBox {
  /// The padding box: the border box inset by the border widths, with inner
  /// radii.
  pub fn padding_box(border: BorderProperties, layout: Layout) -> Self {
    let mut inner = border;
    inner.inset_by_border_width();

    Self {
      border: inner,
      size: Size {
        width: (layout.size.width - layout.border.left - layout.border.right).max(0.0),
        height: (layout.size.height - layout.border.top - layout.border.bottom).max(0.0),
      },
      offset: Point {
        x: layout.border.left,
        y: layout.border.top,
      },
    }
  }

  /// The content box: the padding box further inset by padding, with inner
  /// radii.
  pub fn content_box(border: BorderProperties, layout: Layout) -> Self {
    let mut inner = border;
    inner.inset_by_border_width();
    inner.expand_by(layout.padding.map(|size| -size));

    Self {
      border: inner,
      size: layout.content_box_size(),
      offset: layout.content_box_offset(),
    }
  }

  /// The region an inset `box-shadow` leaves uncovered: the padding box shrunk
  /// by `spread` on every side and shifted by the shadow `offset`. An inset
  /// shadow fills the padding box minus this hole.
  pub fn inset_shadow_hole(
    border: BorderProperties,
    padding_box: Size<f32>,
    spread: f32,
    offset: Point<f32>,
  ) -> Self {
    let mut hole = border;
    hole.expand_by(Rect {
      top: -spread,
      right: -spread,
      bottom: -spread,
      left: -spread,
    });

    Self {
      border: hole,
      size: Size {
        width: (padding_box.width - 2.0 * spread).max(0.0),
        height: (padding_box.height - 2.0 * spread).max(0.0),
      },
      offset: Point {
        x: offset.x + spread,
        y: offset.y + spread,
      },
    }
  }
}

/// The CSS `outline`: a uniform border ring expanded outward from the border box
/// by `outline-offset + outline-width`, following the element's border radius.
#[derive(Debug, Clone, Copy)]
pub struct OutlineGeometry {
  /// The outline drawn as a uniform border on all four sides.
  pub border: BorderProperties,
  /// The expanded box size.
  pub size: Size<f32>,
  /// Outward growth on each side; the box is positioned translated by `-grow`.
  pub grow: f32,
}

/// The outline a box paints, or `None` when it paints none.
///
/// Matches Blink's `ComputedStyle::HasOutline`: a width that rounds to zero
/// paints nothing, and otherwise `outline-style` has to draw something. A
/// transparent colour still counts as an outline, so the decision does not
/// look at alpha; a backend is free to skip the invisible fill.
///
/// Each backend used to guard this differently, and one of them not at all.
pub(crate) fn outline_paint(context: &RenderContext, size: Size<f32>) -> Option<OutlineGeometry> {
  let style = &context.style;
  let width = style.outline_width.to_used_px(&context.sizing).max(0.0);

  if width <= 0.0 || !style.outline_style.is_rendered() {
    return None;
  }
  Some(outline_geometry(context, size))
}

/// The outline ring's border geometry and how far it grows past the border box.
/// Says nothing about whether the outline paints; see [`outline_paint`].
pub(crate) fn outline_geometry(context: &RenderContext, size: Size<f32>) -> OutlineGeometry {
  let style = &context.style;
  let width = style.outline_width.to_used_px(&context.sizing).max(0.0);
  let offset = style
    .outline_offset
    .to_border_px(&context.sizing, size.width);
  // CSS: the outline shape must not shrink below `2 * outline-width` in either
  // dimension, so a large negative `outline-offset` can't invert the ring.
  let min_grow = (2.0 * width - size.width)
    .max(2.0 * width - size.height)
    .min(0.0)
    / 2.0;
  let grow = (offset + width).max(min_grow);

  let mut border = BorderProperties {
    width: Sides([width; 4]).into(),
    color: Sides([style.outline_color.resolve(context.current_color); 4]).into(),
    style: Sides([style.outline_style; 4]).into(),
    image_rendering: style.image_rendering,
    radius: BorderProperties::resolve_radius_part(context, size),
    shape: BorderProperties::resolve_shape_part(context),
    collapsed: false,
  };
  border.expand_by(Rect {
    top: grow,
    right: grow,
    bottom: grow,
    left: grow,
  });

  OutlineGeometry {
    border,
    size: Size {
      width: size.width + 2.0 * grow,
      height: size.height + 2.0 * grow,
    },
    grow,
  }
}

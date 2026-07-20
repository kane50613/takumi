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
  style::{Length, Sides},
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
  /// The full border box: the border geometry as-is.
  pub fn border_box(border: BorderProperties, layout: Layout) -> Self {
    Self {
      border,
      size: layout.size,
      offset: Point::ZERO,
    }
  }

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

/// Resolves the outline ring geometry from the element's style. Callers guard
/// visibility (zero width, transparent color, `none` style) themselves.
pub fn outline_geometry(context: &RenderContext, size: Size<f32>) -> OutlineGeometry {
  let style = &context.style;
  let width = Length::from(style.outline_width)
    .to_px(&context.sizing, size.width)
    .max(0.0);
  let offset = style.outline_offset.to_px(&context.sizing, size.width);
  let grow = offset + width;

  let mut border = BorderProperties {
    width: Sides([width; 4]).into(),
    color: Sides([style.outline_color.resolve(context.current_color); 4]).into(),
    style: Sides([style.outline_style; 4]).into(),
    image_rendering: style.image_rendering,
    radius: BorderProperties::resolve_radius_part(context, size),
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

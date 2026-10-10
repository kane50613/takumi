//! Backend-agnostic box-decoration geometry.
//!
//! The clip regions a `background-clip` fills — border, padding, and content
//! boxes with their inset radii — are pure functions of the border geometry and
//! computed layout. Resolving them here keeps the raster and SVG backends from
//! re-deriving the same inset/expand math by hand and drifting apart.

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout as Layout, PathCommand, Point, Rect, Size},
  layout::{
    border::BorderProperties,
    contoured_rect::{Corner, append_inset_contour, has_round_curvature},
  },
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
  /// The border box the region was inset from, whose corners its own follow when they are not
  /// round.
  pub origin: Option<ContourOrigin>,
}

/// A border box whose corners an inset region follows, as Blink's `ContouredRect` keeps its
/// origin rectangle.
#[derive(Debug, Clone, Copy)]
pub struct ContourOrigin {
  /// The border box's corner geometry.
  pub border: BorderProperties,
  /// The border box's size.
  pub size: Size<f32>,
  /// The border box's top-left, in the region's space.
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
        width: layout.padding_box_width().max(0.0),
        height: layout.padding_box_height().max(0.0),
      },
      offset: layout.border.top_left(),
      origin: Some(ContourOrigin {
        border,
        size: layout.size,
        offset: Point::ZERO,
      }),
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
      origin: Some(ContourOrigin {
        border,
        size: layout.size,
        offset: Point::ZERO,
      }),
    }
  }

  /// Whether the region follows its origin's corners rather than being a rounded rectangle of its
  /// own: it is inset from a border box with a corner that is not round.
  pub(crate) fn follows_origin(&self) -> bool {
    let Some(origin) = self.origin.filter(|origin| !origin.border.is_zero()) else {
      return false;
    };
    let radii = origin.border.scaled_corner_radii(origin.size);

    self.edges()
      != (Rect {
        left: origin.offset.x,
        top: origin.offset.y,
        right: origin.offset.x + origin.size.width,
        bottom: origin.offset.y + origin.size.height,
      })
      && !has_round_curvature(&radii, Corner::curvatures(&radii, &origin.border.shape))
  }

  /// Appends the region's contour, its corners aligned to its origin's, as Blink's
  /// `AddContouredRect` draws an inset contoured rectangle.
  pub(crate) fn append_contour(&self, path: &mut Vec<PathCommand>) {
    let Some(origin) = self.origin else {
      return self
        .border
        .append_mask_commands(path, self.size, self.offset);
    };
    let radii = origin.border.scaled_corner_radii(origin.size);

    append_inset_contour(
      origin.offset,
      origin.size,
      &radii,
      Corner::curvatures(&radii, &origin.border.shape),
      self.edges(),
      path,
    );
  }

  /// The region grown by `spread` on every side, its corner radii with it, or shrunk when
  /// `spread` is negative.
  pub fn outset(self, spread: f32) -> Self {
    let mut border = self.border;

    if spread > 0.0 {
      border.outset_radii(self.size, spread);
    } else {
      border.expand_by(Sides::from(spread).into());
    }

    Self {
      border,
      size: Size {
        width: (self.size.width + 2.0 * spread).max(0.0),
        height: (self.size.height + 2.0 * spread).max(0.0),
      },
      offset: Point {
        x: self.offset.x - spread,
        y: self.offset.y - spread,
      },
      origin: self.origin.filter(|_| spread <= 0.0),
    }
  }

  /// The region moved by `delta`.
  pub fn shifted(self, delta: Point<f32>) -> Self {
    Self {
      offset: self.offset + delta,
      ..self
    }
  }

  /// Whether the region covers no area.
  pub fn is_empty(&self) -> bool {
    self.size.width <= 0.0 || self.size.height <= 0.0
  }

  /// The region's edges, relative to the border box.
  pub fn edges(&self) -> Rect<f32> {
    Rect {
      left: self.offset.x,
      top: self.offset.y,
      right: self.offset.x + self.size.width,
      bottom: self.offset.y + self.size.height,
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

impl OutlineGeometry {
  /// The outline a box paints, or `None` when it paints none.
  ///
  /// Matches Blink's `ComputedStyle::HasOutline`: a width that rounds to zero
  /// paints nothing, and otherwise `outline-style` has to draw something. A
  /// transparent colour still counts as an outline, so the decision does not
  /// look at alpha; a backend is free to skip the invisible fill.
  pub(crate) fn painted(context: &RenderContext, size: Size<f32>) -> Option<Self> {
    let style = &context.style;
    let width = style
      .rare_non_inherited_data
      .outline_width
      .to_used_px(&context.sizing)
      .max(0.0);

    if width <= 0.0 || !style.rare_non_inherited_data.outline_style.is_rendered() {
      return None;
    }
    Some(Self::of(context, size))
  }

  /// The outline ring's border geometry and how far it grows past the border box, whether or not
  /// it paints.
  fn of(context: &RenderContext, size: Size<f32>) -> Self {
    let style = &context.style;
    let width = style
      .rare_non_inherited_data
      .outline_width
      .to_used_px(&context.sizing)
      .max(0.0);
    let offset = style
      .rare_non_inherited_data
      .outline_offset
      .to_border_px(&context.sizing, size.width);

    Self::ring(
      size,
      offset,
      BorderProperties {
        width: Sides([width; 4]).into(),
        color: Sides(
          [style
            .rare_non_inherited_data
            .outline_color
            .resolve(context.current_color); 4],
        )
        .into(),
        style: Sides([style.rare_non_inherited_data.outline_style; 4]).into(),
        image_rendering: style.rare_inherited_data.image_rendering,
        radius: BorderProperties::resolve_radius_part(context, size),
        shape: BorderProperties::resolve_shape_part(context),
        collapsed: false,
      },
    )
  }

  /// The ring `border` draws `offset` past a box of `size`, `border` holding the outline's width,
  /// colour and style on every side.
  pub(crate) fn ring(size: Size<f32>, offset: f32, mut border: BorderProperties) -> Self {
    let width = border.width.top;
    // CSS: the outline shape must not shrink below `2 * outline-width` in either
    // dimension, so a large negative `outline-offset` can't invert the ring.
    let min_grow = (2.0 * width - size.width)
      .max(2.0 * width - size.height)
      .min(0.0)
      / 2.0;
    let grow = (offset + width).max(min_grow);

    border.expand_by(Sides::from(grow).into());

    Self {
      border,
      size: Size {
        width: size.width + 2.0 * grow,
        height: size.height + 2.0 * grow,
      },
      grow,
    }
  }
}

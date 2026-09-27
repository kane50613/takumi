//! A box's `box-shadow`, painted after Blink's `BoxPainterBase::PaintNormalBoxShadow` and
//! `PaintInsetBoxShadow` in
//! [`box_painter_base.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_painter_base.cc).

use super::{BoxPainter, FillShape, PaintDevice};
use crate::{
  geometry::{Point, Rect, Size},
  layout::{border::BorderProperties, decoration::ClipBox},
  style::{Affine, BlurType, FillRule},
};

/// The shape a shadow casts before its blur.
#[derive(Debug, Clone, Copy)]
pub enum ShadowShape {
  /// A rounded rectangle, cast by an outer shadow.
  Inside(ClipBox),
  /// Everything in `bounds` outside a rounded rectangle, cast by an inset shadow.
  Outside {
    /// The rounded rectangle left unshadowed.
    hole: ClipBox,
    /// The edges of the shadowed area, far enough out that the blur never runs past them.
    bounds: Rect<f32>,
  },
}

impl ShadowShape {
  /// The shape grown by `spread`: a rounded rectangle grows and a hole shrinks.
  pub fn spread(self, spread: f32) -> Self {
    match self {
      Self::Inside(rect) => Self::Inside(rect.outset(spread)),
      Self::Outside { hole, bounds } => Self::Outside {
        hole: hole.outset(-spread),
        bounds,
      },
    }
  }

  /// The shape as a fill.
  pub fn fill_shape(&self) -> FillShape {
    match *self {
      Self::Inside(rect) => rect.into(),
      Self::Outside { hole, bounds } => {
        let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
        let size = Size {
          width: bounds.right - bounds.left,
          height: bounds.bottom - bounds.top,
        };

        BorderProperties::default().append_mask_commands(
          &mut commands,
          size,
          Point {
            x: bounds.left,
            y: bounds.top,
          },
        );
        hole
          .border
          .append_mask_commands(&mut commands, hole.size, hole.offset);

        FillShape::Path {
          commands,
          rule: FillRule::EvenOdd,
        }
      }
    }
  }
}

impl BoxPainter<'_> {
  /// Paints the outer `box-shadow` layers at `origin`, last listed first, clipped out of the
  /// border box.
  pub fn paint_normal_box_shadows<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    let shadows = self.shadows().outer;

    if shadows.is_empty() {
      return;
    }

    let at = Affine::translation(origin.x, origin.y);
    let border_box = ClipBox {
      border: *self.border(),
      size: self.layout.size,
      offset: Point::ZERO,
    };

    device.push_clip_out(&border_box.into(), at);

    for shadow in shadows.iter().rev() {
      let rect = border_box.outset(shadow.spread_radius);

      if rect.is_empty() {
        continue;
      }

      device.fill_shadow(&ShadowShape::Inside(rect), shadow, at);
    }

    device.pop_clip();
  }

  /// Paints the inset `box-shadow` layers at `origin`, last listed first, clipped to the padding
  /// box.
  pub fn paint_inset_box_shadows<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    let shadows = self.shadows().inset;
    let padding_box = ClipBox::padding_box(*self.border(), self.layout);

    if shadows.is_empty() || padding_box.is_empty() {
      return;
    }

    let at = Affine::translation(origin.x, origin.y);

    device.push_clip(&padding_box.into(), at);

    for shadow in shadows.iter().rev() {
      let hole = padding_box.outset(-shadow.spread_radius);

      if hole.is_empty() {
        device.fill_shape(&padding_box.into(), shadow.color, at);
        continue;
      }

      // The shape moves by the shadow's offset, so its bounds cover the padding box from where
      // the shape starts.
      let reach = shadow.blur_radius * BlurType::Shadow.extent_multiplier() + 1.0;
      let padding = padding_box.shifted(Point {
        x: -shadow.offset_x,
        y: -shadow.offset_y,
      });
      let [padding, hole_edges] = [padding.edges(), hole.edges()];
      let bounds = Rect {
        left: padding.left.min(hole_edges.left) - reach,
        top: padding.top.min(hole_edges.top) - reach,
        right: padding.right.max(hole_edges.right) + reach,
        bottom: padding.bottom.max(hole_edges.bottom) + reach,
      };

      device.fill_shadow(&ShadowShape::Outside { hole, bounds }, shadow, at);
    }

    device.pop_clip();
  }
}

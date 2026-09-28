//! One straight border side, drawn after Blink's
//! [`DrawLineForBoxSide`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_border_painter.cc).

// Ported from Blink, under the notice in LICENSE-CHROMIUM.

use super::{FillShape, PaintDevice, border::StyledLine};
use crate::{
  geometry::{Point, Size},
  layout::border::BorderSide,
  style::{Affine, BorderStyle, Color},
};

/// The rectangle one straight side fills, with the widths of the sides it mitres into.
///
/// An adjacent width shapes the ends: a positive one slants the inner edge in by that much, a
/// negative one slants the outer edge.
#[derive(Clone, Copy)]
pub(super) struct BoxSideRect {
  pub(super) side: BorderSide,
  pub(super) x1: f32,
  pub(super) y1: f32,
  pub(super) x2: f32,
  pub(super) y2: f32,
}

impl BoxSideRect {
  /// The rectangle's corners, clockwise from the top-left.
  pub(super) fn corners(self) -> [Point<f32>; 4] {
    [
      Point {
        x: self.x1,
        y: self.y1,
      },
      Point {
        x: self.x2,
        y: self.y1,
      },
      Point {
        x: self.x2,
        y: self.y2,
      },
      Point {
        x: self.x1,
        y: self.y2,
      },
    ]
  }

  /// Paints the side in `style`, mitred into sides `adjacent` wide.
  pub(super) fn paint(
    self,
    color: Color,
    style: BorderStyle,
    adjacent: [i32; 2],
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    let (thickness, length) = match self.side {
      BorderSide::Top | BorderSide::Bottom => (self.y2 - self.y1, self.x2 - self.x1),
      BorderSide::Left | BorderSide::Right => (self.x2 - self.x1, self.y2 - self.y1),
    };

    if length <= 0.0 || thickness <= 0.0 {
      return;
    }

    let thickness = thickness.round() as i32;
    let style = style.effective(thickness as f32);

    match style {
      BorderStyle::Dotted | BorderStyle::Dashed => {
        self.paint_pattern(color, style, thickness, at, device)
      }
      BorderStyle::Double => self.paint_double(color, thickness, adjacent, at, device),
      BorderStyle::Ridge | BorderStyle::Groove => {
        self.paint_ridge_or_groove(color, style, adjacent, at, device)
      }
      BorderStyle::Inset | BorderStyle::Outset => self.paint_solid(
        color.inset_outset(self.side.darkened_by(style)),
        adjacent,
        at,
        device,
      ),
      BorderStyle::Solid => self.paint_solid(color, adjacent, at, device),
      BorderStyle::None | BorderStyle::Hidden => {}
    }
  }

  /// Blink's `DrawDashedOrDottedBoxSide`: the line through the middle of the rectangle.
  fn paint_pattern(
    self,
    color: Color,
    style: BorderStyle,
    thickness: i32,
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    let half = thickness as f32 / 2.0;
    let (start, end) = match self.side {
      BorderSide::Top | BorderSide::Bottom => (
        Point {
          x: self.x1,
          y: self.y1 + half,
        },
        Point {
          x: self.x2,
          y: self.y1 + half,
        },
      ),
      BorderSide::Left | BorderSide::Right => (
        Point {
          x: self.x1 + half,
          y: self.y1,
        },
        Point {
          x: self.x1 + half,
          y: self.y2,
        },
      ),
    };

    StyledLine::box_side(start, end, thickness as f32, style, color).paint(at, device);
  }

  /// Blink's `DrawSolidBoxSide`.
  fn paint_solid(
    self,
    color: Color,
    [first, second]: [i32; 2],
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    let Self {
      side,
      x1,
      y1,
      x2,
      y2,
    } = self;

    if first == 0 && second == 0 {
      device.fill_shape(
        &FillShape::Rect(Size {
          width: x2 - x1,
          height: y2 - y1,
        }),
        color,
        at * Affine::translation(x1, y1),
      );
      return;
    }

    let inward = |width: i32| width.max(0) as f32;
    let outward = |width: i32| (-width).max(0) as f32;
    let point = |x, y| Point { x, y };
    let quad = match side {
      BorderSide::Top => [
        point(x1 + outward(first), y1),
        point(x1 + inward(first), y2),
        point(x2 - inward(second), y2),
        point(x2 - outward(second), y1),
      ],
      BorderSide::Bottom => [
        point(x1 + inward(first), y1),
        point(x1 + outward(first), y2),
        point(x2 - outward(second), y2),
        point(x2 - inward(second), y1),
      ],
      BorderSide::Left => [
        point(x1, y1 + outward(first)),
        point(x1, y2 - outward(second)),
        point(x2, y2 - inward(second)),
        point(x2, y1 + inward(first)),
      ],
      BorderSide::Right => [
        point(x1, y1 + inward(first)),
        point(x1, y2 - inward(second)),
        point(x2, y2 - outward(second)),
        point(x2, y1 + outward(first)),
      ],
    };

    device.fill_shape(&FillShape::polygons([quad]), color, at);
  }

  /// Blink's `DrawDoubleBoxSide`: two solid thirds, each mitred a third of the way in.
  fn paint_double(
    self,
    color: Color,
    thickness: i32,
    [first, second]: [i32; 2],
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    let third = ((thickness + 1) / 3) as f32;
    let Self {
      side,
      x1,
      y1,
      x2,
      y2,
    } = self;
    let rect = |x1, y1, x2, y2| Self {
      side,
      x1,
      y1,
      x2,
      y2,
    };

    if first == 0 && second == 0 {
      let (outer, inner) = match side {
        BorderSide::Top | BorderSide::Bottom => {
          (rect(x1, y1, x2, y1 + third), rect(x1, y2 - third, x2, y2))
        }
        BorderSide::Left | BorderSide::Right => {
          (rect(x1, y1, x1 + third, y2), rect(x2 - third, y1, x2, y2))
        }
      };

      outer.paint_solid(color, [0, 0], at, device);
      inner.paint_solid(color, [0, 0], at, device);
      return;
    }

    let big_third = |width: i32| (if width > 0 { width + 1 } else { width - 1 }) / 3;
    let adjacent = [big_third(first), big_third(second)];
    let inset = |width: i32| ((width * 2 + 1) / 3).max(0) as f32;
    let (near, far) = match side {
      BorderSide::Top => (
        rect(x1 + inset(-first), y1, x2 - inset(-second), y1 + third),
        rect(x1 + inset(first), y2 - third, x2 - inset(second), y2),
      ),
      BorderSide::Left => (
        rect(x1, y1 + inset(-first), x1 + third, y2 - inset(-second)),
        rect(x2 - third, y1 + inset(first), x2, y2 - inset(second)),
      ),
      BorderSide::Bottom => (
        rect(x1 + inset(first), y1, x2 - inset(second), y1 + third),
        rect(x1 + inset(-first), y2 - third, x2 - inset(-second), y2),
      ),
      BorderSide::Right => (
        rect(x1, y1 + inset(first), x1 + third, y2 - inset(second)),
        rect(x2 - third, y1 + inset(-first), x2, y2 - inset(-second)),
      ),
    };

    near.paint(color, BorderStyle::Solid, adjacent, at, device);
    far.paint(color, BorderStyle::Solid, adjacent, at, device);
  }

  /// Blink's `DrawRidgeOrGrooveBoxSide`: an `inset` half and an `outset` half.
  fn paint_ridge_or_groove(
    self,
    color: Color,
    style: BorderStyle,
    [first, second]: [i32; 2],
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    let (s1, s2) = if style == BorderStyle::Groove {
      (BorderStyle::Inset, BorderStyle::Outset)
    } else {
      (BorderStyle::Outset, BorderStyle::Inset)
    };
    let big_half = |width: i32| (if width > 0 { width + 1 } else { width - 1 }) / 2;
    let big_halves = [big_half(first), big_half(second)];
    let halves = [first / 2, second / 2];
    let half = |width: i32| (width.max(0) / 2) as f32;
    let Self {
      side,
      x1,
      y1,
      x2,
      y2,
    } = self;
    let rect = |x1, y1, x2, y2| Self {
      side,
      x1,
      y1,
      x2,
      y2,
    };
    let middle = |from: f32, to: f32| from + (((to - from).round() as i32 + 1) / 2) as f32;

    match side {
      BorderSide::Top => {
        let mid = middle(y1, y2);

        rect(x1 + half(-first), y1, x2 - half(-second), mid)
          .paint(color, s1, big_halves, at, device);
        rect(x1 + half(first + 1), mid, x2 - half(second + 1), y2)
          .paint(color, s2, halves, at, device);
      }
      BorderSide::Left => {
        let mid = middle(x1, x2);

        rect(x1, y1 + half(-first), mid, y2 - half(-second))
          .paint(color, s1, big_halves, at, device);
        rect(mid, y1 + half(first + 1), x2, y2 - half(second + 1))
          .paint(color, s2, halves, at, device);
      }
      BorderSide::Bottom => {
        let mid = middle(y1, y2);

        rect(x1 + half(first), y1, x2 - half(second), mid).paint(color, s2, big_halves, at, device);
        rect(x1 + half(-first + 1), mid, x2 - half(-second + 1), y2)
          .paint(color, s1, halves, at, device);
      }
      BorderSide::Right => {
        let mid = middle(x1, x2);

        rect(x1, y1 + half(first), mid, y2 - half(second)).paint(color, s2, big_halves, at, device);
        rect(mid, y1 + half(-first + 1), x2, y2 - half(-second + 1))
          .paint(color, s1, halves, at, device);
      }
    }
  }
}

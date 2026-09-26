//! A box's border, painted after Blink's
//! [`BoxBorderPainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_border_painter.cc).

use super::{FillShape, PaintDevice, StrokeStyle};
use crate::{
  geometry::{PathCommand, Point, Rect, Size},
  layout::border::{BorderPaint, BorderProperties, BorderSide, PaintedSide, SideBand},
  style::{Affine, BorderStyle, FillRule, Sides},
};

/// A border on a border box of `size`, ready to paint.
pub struct BoxBorderPainter<'b> {
  border: &'b BorderProperties,
  size: Size<f32>,
}

impl<'b> BoxBorderPainter<'b> {
  /// Prepares `border` for a border box of `size`.
  pub fn new(border: &'b BorderProperties, size: Size<f32>) -> Self {
    Self { border, size }
  }

  /// Paints the border with the border box's top-left at `origin`.
  pub fn paint<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    if !self.border.has_visible_sides() || self.paint_fast_path(origin, device) {
      return;
    }

    self.paint_sides(Affine::translation(origin.x, origin.y), device);
  }

  /// Paints a border whose visible sides share one colour and style in one pass, reporting
  /// whether it could. A uniform dashed or dotted border strokes the centerline so the pattern
  /// runs round the whole ring, and a double border fills two rings.
  pub fn paint_fast_path<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) -> bool {
    let border = self.border;
    let size = self.size;
    let at = Affine::translation(origin.x, origin.y);

    match border.paint() {
      BorderPaint::Sides => return false,
      // A transparent ring is a fill nobody sees, and painting it would only
      // lengthen the output.
      BorderPaint::Ring { color }
      | BorderPaint::Double { color, .. }
      | BorderPaint::Stroked { color, .. }
        if color.0[3] == 0 => {}
      BorderPaint::Ring { color } => {
        device.fill_shape(&FillShape::border_ring(border, size), color, at);
      }
      BorderPaint::Double { color, width } => {
        let third = width / 3.0;

        for inset in [0.0, third * 2.0] {
          let mut ring = *border;

          ring.expand_by(Rect {
            top: -inset,
            right: -inset,
            bottom: -inset,
            left: -inset,
          });
          ring.width = Sides([third; 4]).into();

          let ring_size = Size {
            width: (size.width - inset * 2.0).max(0.0),
            height: (size.height - inset * 2.0).max(0.0),
          };

          device.fill_shape(
            &FillShape::border_ring(&ring, ring_size),
            color,
            Affine::translation(origin.x + inset, origin.y + inset),
          );
        }
      }
      BorderPaint::Stroked {
        color,
        width,
        style,
      } => {
        let half = width / 2.0;
        let mut center = *border;

        center.expand_by(Rect {
          top: -half,
          right: -half,
          bottom: -half,
          left: -half,
        });

        let center_size = Size {
          width: (size.width - width).max(0.0),
          height: (size.height - width).max(0.0),
        };
        let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

        center.append_mask_commands(&mut commands, center_size, Point { x: half, y: half });

        let perimeter = center.approximate_rounded_rect_perimeter(center_size);
        let dash = style.dash_pattern(width, perimeter, true);

        device.stroke_shape(
          &FillShape::Path {
            commands,
            rule: FillRule::NonZero,
          },
          &StrokeStyle {
            color,
            width,
            dash: dash.map(|dash| dash.intervals),
            round_cap: dash.is_some_and(|dash| dash.round_cap),
          },
          at,
        );
      }
    }

    true
  }

  /// Paints each side on its own inside the ring: solid and 3D sides fill their corner-mitred
  /// polygon, dashed and dotted sides stroke their centerline.
  fn paint_sides<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    let mut sides = self.border.painted_sides().peekable();

    if sides.peek().is_none() {
      return;
    }

    // A collapsed border's sides are squared rectangles already inside the
    // ring, so the clip only adds an antialiased edge that leaks the
    // background where two cells meet. Patterned sides still need it to trim
    // their centerlines.
    let patterned = self
      .border
      .painted_sides()
      .any(|side| matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted));
    let ring =
      (!self.border.collapsed || patterned).then(|| FillShape::border_ring(self.border, self.size));

    device.save(ring.as_ref().map(|ring| (ring, at)));

    for side in sides {
      if matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted) {
        self.paint_side_pattern(side, at, device);
        continue;
      }

      for band in self.border.side_bands(side) {
        device.fill_shape(&self.band_polygon(side.side, &band), band.color, at);
      }
    }

    device.restore();
  }

  /// The polygon one band of `side` fills, mitred at the corners.
  fn band_polygon(&self, side: BorderSide, band: &SideBand) -> FillShape {
    let mut strip = *self.border;
    let mut commands = Vec::new();

    strip.width = band.width;
    strip.expand_by(band.inset.map(|value| -value));
    strip.append_side_clip_polygon_commands_at(
      side,
      &mut commands,
      self.size.inset(band.inset),
      band.inset.top_left(),
    );

    FillShape::Path {
      commands,
      rule: FillRule::NonZero,
    }
  }

  /// Strokes a dashed or dotted side along its centerline.
  fn paint_side_pattern<D: PaintDevice>(&self, side: PaintedSide, at: Affine, device: &mut D) {
    let [start, end] = self.centerline(side.side);
    let length = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
    let dash = side.style.dash_pattern(side.width, length, false);

    device.stroke_shape(
      &FillShape::Path {
        commands: vec![PathCommand::MoveTo(start), PathCommand::LineTo(end)],
        rule: FillRule::NonZero,
      },
      &StrokeStyle {
        color: side.color,
        width: side.width,
        dash: dash.map(|dash| dash.intervals),
        round_cap: dash.is_some_and(|dash| dash.round_cap),
      },
      at,
    );
  }

  /// The ends of the line through the middle of `side`, between the neighbouring sides' middles.
  fn centerline(&self, side: BorderSide) -> [Point<f32>; 2] {
    let half = self.border.width.map(|width| width / 2.0);
    let Size { width, height } = self.size;
    let point = |x, y| Point { x, y };

    match side {
      BorderSide::Top => [
        point(half.left, half.top),
        point(width - half.right, half.top),
      ],
      BorderSide::Right => [
        point(width - half.right, half.top),
        point(width - half.right, height - half.bottom),
      ],
      BorderSide::Bottom => [
        point(half.left, height - half.bottom),
        point(width - half.right, height - half.bottom),
      ],
      BorderSide::Left => [
        point(half.left, half.top),
        point(half.left, height - half.bottom),
      ],
    }
  }
}

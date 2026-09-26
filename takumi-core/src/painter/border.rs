//! A box's border, painted after Blink's
//! [`BoxBorderPainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_border_painter.cc).

use smallvec::SmallVec;

use super::{FillShape, PaintDevice, StrokeStyle};
use crate::{
  geometry::{PathCommand, Point, Size},
  layout::border::{BorderProperties, BorderSide, PaintedSide, SideBand},
  style::{Affine, BorderStyle, Color, FillRule},
};

/// How far a curved dashed side overstrokes its centerline, so the ring clip
/// rather than the stroke decides where each dash ends.
const CURVED_DASH_OVERSTROKE: f32 = 2.2;

/// A border on a border box of `size`, ready to paint.
pub struct BoxBorderPainter<'b> {
  border: &'b BorderProperties,
  size: Size<f32>,
}

/// How a border paints as a whole, before any per-side work.
enum BorderPaint {
  /// One even-odd fill of the whole ring.
  Ring(Color),
  /// Two concentric rings, a third of each side's width apiece.
  Double(Color),
  /// One dashed or dotted stroke round the rounded centerline.
  Stroked {
    color: Color,
    width: f32,
    style: BorderStyle,
  },
  /// Each side on its own.
  Sides,
}

/// The sides that fill one band in one colour, merged so adjacent sides share no seam.
struct SideFill {
  band: SideBand,
  area: Vec<PathCommand>,
}

impl<'b> BoxBorderPainter<'b> {
  /// Prepares `border` for a border box of `size`.
  pub fn new(border: &'b BorderProperties, size: Size<f32>) -> Self {
    Self { border, size }
  }

  /// Paints the border with the border box's top-left at `origin`.
  pub fn paint<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    if !self.border.has_visible_sides() {
      return;
    }

    let at = Affine::translation(origin.x, origin.y);

    match self.kind() {
      BorderPaint::Sides => self.paint_sides(at, device),
      // A transparent ring is a fill nobody sees, and painting it would only
      // lengthen the output.
      BorderPaint::Ring(color)
      | BorderPaint::Double(color)
      | BorderPaint::Stroked { color, .. }
        if color.0[3] == 0 => {}
      BorderPaint::Ring(color) => {
        device.fill_shape(&FillShape::border_ring(self.border, self.size), color, at);
      }
      BorderPaint::Double(color) => self.paint_double(color, at, device),
      BorderPaint::Stroked {
        color,
        width,
        style,
      } => {
        let (centerline, perimeter) = self.centerline_loop(self.border);

        device.stroke_shape(
          &centerline,
          &StrokeStyle::border(color, width, style.dash_pattern(width, perimeter, true)),
          at,
        );
      }
    }
  }

  /// Whether the border paints in one pass, as Blink's `PaintBorderFastPath` does for a solid or
  /// double border of one colour on all four sides. A rounded dashed or dotted border of one
  /// width strokes its whole centerline, which is what its per-side strokes add up to.
  fn kind(&self) -> BorderPaint {
    let border = self.border;
    let sides = border.sides();
    let Some(color) = border.has_uniform_visible_color() else {
      return BorderPaint::Sides;
    };
    let style = sides[0].style;
    let all_alike = sides
      .iter()
      .all(|side| side.is_visible() && side.style == style);

    match style {
      BorderStyle::Solid if all_alike => BorderPaint::Ring(color),
      BorderStyle::Double if all_alike => BorderPaint::Double(color),
      BorderStyle::Dashed | BorderStyle::Dotted
        if !border.is_zero() && border.is_uniform_all_sides_style(style) =>
      {
        BorderPaint::Stroked {
          color,
          width: border.width.top,
          style,
        }
      }
      _ => BorderPaint::Sides,
    }
  }

  /// Fills the outer and inner thirds of a double border as two rings.
  fn paint_double<D: PaintDevice>(&self, color: Color, at: Affine, device: &mut D) {
    let third = self.border.width.map(|width| width / 3.0);

    for inset in [third.map(|_| 0.0), third.map(|width| width * 2.0)] {
      let band = SideBand {
        inset,
        width: third,
        color,
      };

      device.fill_shape(&self.band_ring(self.border, &band), color, at);
    }
  }

  /// Paints each side on its own. Sides meet along the diagonal from the outer to the inner
  /// corner, and sides that fill a band in the same colour merge into one fill. A rounded border
  /// fills each band's ring clipped to its sides' regions.
  fn paint_sides<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    let mut border = *self.border;

    border.width = border.visible_side_widths();

    let rounded = !border.is_zero();
    let mut fills: SmallVec<[SideFill; 4]> = SmallVec::new();

    for side in border.painted_sides() {
      if matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted) {
        self.paint_side_pattern(&border, side, at, device);
        continue;
      }

      for band in border.side_bands(side) {
        let area = if rounded {
          self.side_region(&border, side.side)
        } else {
          self.band_region(&border, side.side, &band)
        };

        match fills.iter_mut().find(|fill| fill.band == band) {
          Some(fill) => fill.area.extend(area),
          None => fills.push(SideFill { band, area }),
        }
      }
    }

    for fill in fills {
      let area = FillShape::Path {
        commands: fill.area,
        rule: FillRule::NonZero,
      };

      if rounded {
        device.save(Some((&area, at)));
        device.fill_shape(&self.band_ring(&border, &fill.band), fill.band.color, at);
        device.restore();
      } else {
        device.fill_shape(&area, fill.band.color, at);
      }
    }
  }

  /// Strokes a dashed or dotted side along its centerline, clipped to its corner mitres when a
  /// neighbour has width, and to the ring when the border is rounded.
  fn paint_side_pattern<D: PaintDevice>(
    &self,
    border: &BorderProperties,
    side: PaintedSide,
    at: Affine,
    device: &mut D,
  ) {
    let rounded = !border.is_zero();
    let curved = rounded && border.inner_edge_arcs(side.side, self.size);
    let widest_neighbour = side
      .side
      .adjacent()
      .iter()
      .map(|adjacent| adjacent.of(border.width))
      .fold(0.0, f32::max);
    let ring = rounded.then(|| FillShape::border_ring(border, self.size));
    let mitre = (curved || widest_neighbour > 0.0).then(|| FillShape::Path {
      commands: self.side_region(border, side.side),
      rule: FillRule::NonZero,
    });
    let clips = [&ring, &mitre].into_iter().flatten();

    for clip in clips.clone() {
      device.save(Some((clip, at)));
    }

    let (line, length, closed) = if curved {
      let (centerline, perimeter) = self.centerline_loop(border);

      (centerline, perimeter, true)
    } else {
      let (line, length) = self.centerline(side);

      (line, length, false)
    };
    let dash = side.style.dash_pattern(side.width, length, closed);
    let width = if curved && side.style == BorderStyle::Dashed {
      side.width.max(widest_neighbour) * CURVED_DASH_OVERSTROKE
    } else {
      side.width
    };

    device.stroke_shape(&line, &StrokeStyle::border(side.color, width, dash), at);

    for _ in clips {
      device.restore();
    }
  }

  /// The ring one band of the border fills.
  fn band_ring(&self, border: &BorderProperties, band: &SideBand) -> FillShape {
    let mut strip = *border;
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    strip.width = band.width;
    strip.expand_by(band.inset.map(|value| -value));
    strip.append_border_ring_commands_at(
      &mut commands,
      self.size.inset(band.inset),
      band.inset.top_left(),
    );

    FillShape::Path {
      commands,
      rule: FillRule::EvenOdd,
    }
  }

  /// The part of one band of `side` between its corner mitres.
  fn band_region(
    &self,
    border: &BorderProperties,
    side: BorderSide,
    band: &SideBand,
  ) -> Vec<PathCommand> {
    let mut strip = *border;
    let mut commands = Vec::with_capacity(5);

    strip.width = band.width;
    strip.expand_by(band.inset.map(|value| -value));
    strip.append_side_clip_polygon_commands_at(
      side,
      &mut commands,
      self.size.inset(band.inset),
      band.inset.top_left(),
    );

    commands
  }

  /// The region `side` owns, from its outer corners to the padding edge.
  fn side_region(&self, border: &BorderProperties, side: BorderSide) -> Vec<PathCommand> {
    let mut commands = Vec::with_capacity(5);

    border.append_side_clip_polygon_commands_at(side, &mut commands, self.size, Point::ZERO);

    commands
  }

  /// The closed path through the middle of every side, and its length.
  fn centerline_loop(&self, border: &BorderProperties) -> (FillShape, f32) {
    let half = border.width.map(|width| width / 2.0);
    let mut center = *border;
    let size = self.size.inset(half);
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

    center.expand_by(half.map(|value| -value));
    center.append_mask_commands(&mut commands, size, half.top_left());

    let perimeter = center.approximate_rounded_rect_perimeter(size);

    (
      FillShape::Path {
        commands,
        rule: FillRule::NonZero,
      },
      perimeter,
    )
  }

  /// The straight line through the middle of `side` across the whole border box, and the length
  /// its dashes spread over. Dots stop half a dot short of each end, so the end dots stay inside.
  fn centerline(&self, side: PaintedSide) -> (FillShape, f32) {
    let Size { width, height } = self.size;
    let half = side.width / 2.0;
    let inset = if side.style == BorderStyle::Dotted {
      half
    } else {
      0.0
    };
    let point = |x, y| Point { x, y };
    let (start, end, length) = match side.side {
      BorderSide::Top => (point(inset, half), point(width - inset, half), width),
      BorderSide::Bottom => (
        point(inset, height - half),
        point(width - inset, height - half),
        width,
      ),
      BorderSide::Left => (point(half, inset), point(half, height - inset), height),
      BorderSide::Right => (
        point(width - half, inset),
        point(width - half, height - inset),
        height,
      ),
    };

    (
      FillShape::Path {
        commands: vec![PathCommand::MoveTo(start), PathCommand::LineTo(end)],
        rule: FillRule::NonZero,
      },
      length,
    )
  }
}

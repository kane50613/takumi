//! A box's border, painted after Blink's
//! [`BoxBorderPainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/box_border_painter.cc).

use std::cmp::Ordering;

use smallvec::SmallVec;

use super::{FillShape, PaintDevice, StrokeStyle};
use crate::{
  geometry::{PathCommand, Point, Size},
  layout::border::{BorderProperties, BorderSide, PaintedSide, SideBand},
  style::{Affine, BorderStyle, Color, FillRule},
};

// The curved dash overstroke and `StyledLine` follow Blink, under the notice in LICENSE-CHROMIUM.

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
  /// Every side alike in a style that fills its bands as whole rings: `solid` fills one, `double`
  /// two.
  Rings(PaintedSide),
  /// Every side alike and dashed or dotted round a rounded box: one stroke along the centerline.
  Stroked(PaintedSide),
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
      BorderPaint::Rings(side) | BorderPaint::Stroked(side) if side.color.0[3] == 0 => {}
      BorderPaint::Rings(side) => {
        for band in self.border.side_bands(side) {
          device.fill_shape(&self.band_ring(self.border, &band), band.color, at);
        }
      }
      BorderPaint::Stroked(side) => {
        let (centerline, perimeter) = self.centerline_loop(self.border);
        let dash = side.style.dash_pattern(side.width, perimeter, true);

        device.stroke_shape(
          &centerline,
          &StrokeStyle::border(side.color, side.width, dash),
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
    let [first, ..] = border.sides();

    if border.has_uniform_visible_color().is_none() {
      return BorderPaint::Sides;
    }

    let all_alike = border
      .sides()
      .iter()
      .all(|side| side.is_visible() && side.style == first.style);

    match first.style {
      BorderStyle::Solid | BorderStyle::Double if all_alike => BorderPaint::Rings(first),
      BorderStyle::Dashed | BorderStyle::Dotted
        if !border.is_zero() && border.is_uniform_all_sides_style(first.style) =>
      {
        BorderPaint::Stroked(first)
      }
      _ => BorderPaint::Sides,
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
        device.push_clip(&area, at);
        device.fill_shape(&self.band_ring(&border, &fill.band), fill.band.color, at);
        device.pop_clip();
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
      device.push_clip(clip, at);
    }

    if curved {
      let (centerline, perimeter) = self.centerline_loop(border);
      let dash = side.style.dash_pattern(side.width, perimeter, true);
      let width = if side.style == BorderStyle::Dashed {
        side.width.max(widest_neighbour) * CURVED_DASH_OVERSTROKE
      } else {
        side.width
      };

      device.stroke_shape(
        &centerline,
        &StrokeStyle::border(side.color, width, dash),
        at,
      );
    } else {
      let [start, end] = self.side_line(side);

      StyledLine::box_side(start, end, side.width, side.style, side.color).paint(at, device);
    }

    for _ in clips {
      device.pop_clip();
    }
  }

  /// The ring one band of the border fills.
  fn band_ring(&self, border: &BorderProperties, band: &SideBand) -> FillShape {
    let (strip, size, offset) = self.band_strip(border, band);
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    strip.append_border_ring_commands_at(&mut commands, size, offset);

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
    let (strip, size, offset) = self.band_strip(border, band);
    let mut commands = Vec::with_capacity(5);

    strip.append_side_clip_polygon_commands_at(side, &mut commands, size, offset);

    commands
  }

  /// `border` narrowed to one band, with the band's outer size and offset in the border box.
  fn band_strip(
    &self,
    border: &BorderProperties,
    band: &SideBand,
  ) -> (BorderProperties, Size<f32>, Point<f32>) {
    let mut strip = *border;

    strip.width = band.width;
    strip.expand_by(band.inset.map(|value| -value));

    (strip, self.size.inset(band.inset), band.inset.top_left())
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

  /// The ends of the line through the middle of `side`, across the whole border box.
  fn side_line(&self, side: PaintedSide) -> [Point<f32>; 2] {
    let Size { width, height } = self.size;
    let half = side.width / 2.0;
    let point = |x, y| Point { x, y };

    match side.side {
      BorderSide::Top => [point(0.0, half), point(width, half)],
      BorderSide::Bottom => [point(0.0, height - half), point(width, height - half)],
      BorderSide::Left => [point(half, 0.0), point(half, height)],
      BorderSide::Right => [point(width - half, 0.0), point(width - half, height)],
    }
  }
}

/// A dashed or dotted line along one axis, after Blink's `DrawLineWithStyle`: its dashes spread
/// over the whole line, and round dots stop half a dot short of each end so the end dots stay
/// inside.
pub(super) struct StyledLine {
  line: FillShape,
  stroke: StrokeStyle,
  /// End dots filled on their own, each a top-left and a size.
  dots: SmallVec<[(Point<f32>, Size<f32>); 2]>,
}

impl StyledLine {
  /// The line from `start` to `end` in `style`, `width` thick.
  pub(super) fn new(
    start: Point<f32>,
    end: Point<f32>,
    width: f32,
    style: BorderStyle,
    color: Color,
  ) -> Self {
    let length = (end.x - start.x).abs() + (end.y - start.y).abs();
    let dash = style.dash_pattern(width, length, false);
    let [start, end] = if dash.is_some_and(|dash| dash.round_cap) {
      let half = width / 2.0;
      let step = |from: f32, to: f32| match to.total_cmp(&from) {
        Ordering::Greater => half,
        Ordering::Less => -half,
        Ordering::Equal => 0.0,
      };
      let (dx, dy) = (step(start.x, end.x), step(start.y, end.y));

      [
        Point {
          x: start.x + dx,
          y: start.y + dy,
        },
        Point {
          x: end.x - dx,
          y: end.y - dy,
        },
      ]
    } else {
      [start, end]
    };

    Self {
      line: FillShape::Path {
        commands: vec![PathCommand::MoveTo(start), PathCommand::LineTo(end)],
        rule: FillRule::NonZero,
      },
      stroke: StrokeStyle::border(color, width, dash),
      dots: SmallVec::new(),
    }
  }

  /// A left-to-right or top-to-bottom box side, as Blink's `DrawLineWithStyle` draws it.
  pub(super) fn box_side(
    start: Point<f32>,
    end: Point<f32>,
    width: f32,
    style: BorderStyle,
    color: Color,
  ) -> Self {
    let dot = width.round();

    if style != BorderStyle::Dotted || dot > 3.0 || dot < 1.0 {
      return Self::new(start, end, width, style, color);
    }

    let vertical = start.x == end.x;
    let length = ((end.x - start.x) + (end.y - start.y)).round() as i32;
    let ends = EndDots::of(dot as i32, length);
    let mut line = Self::new(start, end, width, style, color);
    let along = |point: Point<f32>, by: f32| {
      if vertical {
        Point {
          x: point.x,
          y: point.y + by,
        }
      } else {
        Point {
          x: point.x + by,
          y: point.y,
        }
      }
    };
    let dot_rect = |from: Point<f32>, length: f32| {
      let half = dot / 2.0;

      if vertical {
        (
          Point {
            x: from.x - half,
            y: from.y,
          },
          Size {
            width: dot,
            height: length,
          },
        )
      } else {
        (
          Point {
            x: from.x,
            y: from.y - half,
          },
          Size {
            width: length,
            height: dot,
          },
        )
      }
    };
    let (mut start, mut end) = (start, end);

    if let Some(growth) = ends.start {
      line.dots.push(dot_rect(start, dot + growth as f32));
      start = along(start, 2.0 * dot + ends.start_offset as f32);
    }
    if let Some(growth) = ends.end {
      let size = dot + growth as f32;

      line.dots.push(dot_rect(along(end, -size), size));
      end = along(end, -(size + 1.0));
    }
    line.line = FillShape::Path {
      commands: vec![PathCommand::MoveTo(start), PathCommand::LineTo(end)],
      rule: FillRule::NonZero,
    };
    line
  }

  /// Fills the end dots and strokes the line under `at`.
  pub(super) fn paint<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    for &(origin, size) in &self.dots {
      device.fill_shape(
        &FillShape::Rect(size),
        self.stroke.color,
        at * Affine::translation(origin.x, origin.y),
      );
    }
    device.stroke_shape(&self.line, &self.stroke, at);
  }
}

/// Blink's `EnforceDotsAtEndpoints` for a `dot`-wide dotted line `length` long.
struct EndDots {
  /// The start dot's growth, when there is one.
  start: Option<i32>,
  /// How far the first gap after the start dot shrinks or grows.
  start_offset: i32,
  /// The end dot's growth, when there is one.
  end: Option<i32>,
}

impl EndDots {
  fn of(dot: i32, length: i32) -> Self {
    let (mod_4, mod_6) = (length.rem_euclid(4), length.rem_euclid(6));
    let mut ends = Self {
      start: None,
      start_offset: 0,
      end: None,
    };

    if (dot == 1 && length % 2 == 0) || (dot == 3 && mod_6 == 0) {
      ends.start = Some(1);
      ends.start_offset = 1;
    }
    if (dot == 2 && (mod_4 == 0 || mod_4 == 1)) || (dot == 3 && (mod_6 == 1 || mod_6 == 2)) {
      ends.start = Some(ends.start.unwrap_or(0));
      ends.start_offset = -1;
    }
    if (dot == 2 && mod_4 == 0) || (dot == 3 && mod_6 == 1) {
      ends.end = Some(0);
    }
    if (dot == 2 && mod_4 == 3) || (dot == 3 && (mod_6 == 4 || mod_6 == 5)) {
      ends.start = Some(ends.start.unwrap_or(0));
      ends.start_offset = 1;
    }
    if dot == 3 && mod_6 == 5 {
      ends.end = Some(0);
    } else if dot == 3 && mod_6 == 0 {
      ends.end = Some(1);
    }
    ends
  }
}

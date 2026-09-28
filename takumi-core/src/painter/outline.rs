//! A box's `outline`, painted after Blink's
//! [`OutlinePainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/outline_painter.cc).

use super::{
  BoxBorderPainter, BoxPainter, FillShape, LayerBounds, OpacityLayer, PaintDevice, PaintRole,
  StrokeStyle, border::StyledLine,
};
use crate::{
  geometry::{PathBuilder, PathCommand, Point, Size},
  layout::{
    border::BorderProperties,
    decoration::OutlineGeometry,
    inline::{OutlineIsland, RightAngleContour},
  },
  style::{Affine, BorderStyle, Color, FillRule, Sides, SpacePair},
};

/// A box's outline, held until the box's content has painted, as CSS 2 Appendix E orders them.
#[derive(Debug, Clone, Copy)]
pub struct PendingOutline {
  outline: OutlineGeometry,
  origin: Point<f32>,
}

impl PendingOutline {
  /// Paints the outline as a ring around the border box, grown by
  /// `outline-offset + outline-width`.
  pub fn paint(&self, device: &mut dyn PaintDevice) {
    let OutlineGeometry { border, size, grow } = self.outline;

    device.set_role(PaintRole::Outline);
    BoxBorderPainter::new(&border, size).paint(
      Point {
        x: self.origin.x - grow,
        y: self.origin.y - grow,
      },
      device,
    );
  }
}

impl BoxPainter<'_> {
  /// The outline of the box with its border box at `origin`, or `None` when it paints none.
  pub fn pending_outline(&self, origin: Point<f32>) -> Option<PendingOutline> {
    Some(PendingOutline {
      outline: self.outline()?,
      origin,
    })
  }
}

impl OutlineIsland {
  /// Paints the outline of the element that owns the island, with the block's border box at
  /// `origin`: a lone rect as a box border, as Blink's `PaintSingleRectOutline` does, and anything
  /// else as Blink's `ComplexOutlinePainter` does.
  pub fn paint(&self, origin: Point<f32>, device: &mut dyn PaintDevice) {
    let (outline, opacity) = self.outline();
    let (width, offset) = outline.painted();

    if opacity <= 0.0 || width <= 0.0 {
      return;
    }

    if let Some(rect) = self.lone_rect() {
      let [left, top, right, bottom] = rect.pixel_snapped();
      let geometry = OutlineGeometry::ring(
        Size {
          width: right - left,
          height: bottom - top,
        },
        offset,
        BorderProperties {
          width: Sides([width; 4]).into(),
          color: Sides([outline.color; 4]).into(),
          style: Sides([outline.style; 4]).into(),
          radius: rect.radius,
          ..BorderProperties::default()
        },
      );
      let origin = Point {
        x: origin.x + left,
        y: origin.y + top,
      };

      return device.with_opacity(opacity, None, |device| {
        PendingOutline {
          outline: geometry,
          origin,
        }
        .paint(device)
      });
    }

    let Some(outer) = self.right_angle_path(offset, width) else {
      return;
    };
    let radius = self.radius();
    let painter = ComplexOutline::new(outer, width, offset, outline.style, outline.color, radius);

    device.set_role(PaintRole::Outline);
    device.with_opacity(opacity, None, |device| {
      painter.paint(Affine::translation(origin.x, origin.y), device)
    });
  }
}

/// Blink's `ComplexOutlinePainter`, for an outline around several rects. Follows Blink under the
/// notice in LICENSE-CHROMIUM.
struct ComplexOutline {
  /// The contour of the outline's outer edge.
  outer: RightAngleContour,
  width: f32,
  offset: f32,
  style: BorderStyle,
  color: Color,
  /// The element's corner radii, when it has any.
  radius: Option<Sides<SpacePair<f32>>>,
}

impl ComplexOutline {
  fn new(
    outer: RightAngleContour,
    width: f32,
    offset: f32,
    style: BorderStyle,
    color: Color,
    radius: Sides<SpacePair<f32>>,
  ) -> Self {
    let (style, color) = match style {
      BorderStyle::Double if width <= 2.0 => (BorderStyle::Solid, color),
      BorderStyle::Groove | BorderStyle::Ridge if width == 1.0 => (
        BorderStyle::Solid,
        color.lerp_premultiplied(color.dark(), 0.5),
      ),
      style => (style, color),
    };
    let rounded = radius
      .0
      .iter()
      .any(|corner| corner.x > 0.0 && corner.y > 0.0);

    Self {
      outer,
      width,
      offset,
      style,
      color,
      radius: rounded.then_some(radius),
    }
  }

  /// Blink's `ComputeRadii`: the element's radii grown to `outset` past the outline's offset.
  fn radii(&self, radius: Sides<SpacePair<f32>>, outset: f32) -> Sides<SpacePair<f32>> {
    let mut border = BorderProperties {
      radius,
      ..BorderProperties::default()
    };

    border.expand_by(Sides::from(self.offset + outset).into());
    border.radius
  }

  /// `contour` as a path, its corners rounded, when the element has radii, by the radii grown to
  /// `convex` on its convex corners and to `concave` on its concave ones.
  fn path(&self, contour: &RightAngleContour, convex: f32, concave: f32) -> Vec<PathCommand> {
    match self.radius {
      Some(radius) => contour.rounded(self.radii(radius, convex), self.radii(radius, concave)),
      None => contour.path(),
    }
  }

  /// Blink's `MakeClipOutPath`: everything around `path` inside the outline's bounds, keeping the
  /// areas its crossing edges wind the other way.
  fn clip_out(&self, mut path: Vec<PathCommand>) -> FillShape {
    let (low, high) = self.outer.bounds();

    path.move_to((low.x, low.y));
    path.line_to((low.x, high.y));
    path.line_to((high.x, high.y));
    path.line_to((high.x, low.y));
    path.close();
    FillShape::Path {
      commands: path,
      rule: FillRule::NonZero,
    }
  }

  /// Fills the rect around the outline, which the open clips cut to shape.
  fn fill_bounds(&self, color: Color, at: Affine, device: &mut dyn PaintDevice) {
    let (low, high) = self.outer.bounds();

    device.fill_shape(
      &FillShape::Rect(Size {
        width: high.x - low.x,
        height: high.y - low.y,
      }),
      color,
      at * Affine::translation(low.x, low.y),
    );
  }

  /// Blink's `CenterPath`: the contour down the outline's middle, a pixel nearer the outer edge
  /// when `prefer_outer` and the width is odd.
  fn center(&self, prefer_outer: bool) -> (RightAngleContour, f32) {
    let width = self.width as i32;
    let from_inner = if prefer_outer {
      width / 2
    } else {
      (width + 1) / 2
    } as f32;

    (self.outer.shrunk(self.width - from_inner), from_inner)
  }

  /// The width a 3D or dashed edge strokes at: an odd width grows by two to fill the clip.
  fn stroke_width(&self, dashed: bool) -> f32 {
    if self.width as i32 % 2 == 1 && dashed {
      self.width + 2.0
    } else {
      self.width
    }
  }

  fn paint(&self, at: Affine, device: &mut dyn PaintDevice) {
    let mut color = self.color;
    let alpha_layer =
      color.0[3] < u8::MAX && !matches!(self.style, BorderStyle::Solid | BorderStyle::Double);

    if alpha_layer {
      let (low, high) = self.outer.bounds();

      device.begin_layer(
        f32::from(color.0[3]) / f32::from(u8::MAX),
        Some(LayerBounds {
          size: Size {
            width: high.x - low.x,
            height: high.y - low.y,
          },
          transform: at * Affine::translation(low.x, low.y),
        }),
      );
      color.0[3] = u8::MAX;
    }

    let inner = self.outer.shrunk(self.width);

    device.push_clip(
      &FillShape::Path {
        commands: self.path(&self.outer, self.width, 0.0),
        rule: FillRule::NonZero,
      },
      at,
    );
    device.push_clip(&self.clip_out(self.path(&inner, 0.0, self.width)), at);

    match self.style {
      BorderStyle::Double => {
        let third = (self.width / 3.0).round();
        let inner_third = self.outer.shrunk(self.width - third);
        let outer_third = self.outer.shrunk(third);

        device.fill_shape(
          &FillShape::Path {
            commands: self.path(&inner_third, third, self.width - third),
            rule: FillRule::NonZero,
          },
          color,
          at,
        );
        device.push_clip(
          &self.clip_out(self.path(&outer_third, self.width - third, third)),
          at,
        );
        self.fill_bounds(color, at, device);
        device.pop_clip();
      }
      BorderStyle::Dotted | BorderStyle::Dashed => self.paint_dotted_or_dashed(color, at, device),
      BorderStyle::Groove | BorderStyle::Ridge => {
        let groove = self.style == BorderStyle::Groove;
        let (center, from_inner) = self.center(false);

        self.paint_inset_or_outset(groove, color, at, device);
        device.push_clip(
          &FillShape::Path {
            commands: self.path(&center, from_inner, from_inner),
            rule: FillRule::NonZero,
          },
          at,
        );
        self.paint_top_left_or_bottom_right(!groove, color.dark(), at, device);

        let odd = self.width as i32 % 2 == 1;

        if odd {
          let (center, from_inner) = self.center(true);

          device.push_clip(
            &FillShape::Path {
              commands: self.path(&center, from_inner, from_inner),
              rule: FillRule::NonZero,
            },
            at,
          );
        }
        self.paint_top_left_or_bottom_right(groove, color, at, device);
        if odd {
          device.pop_clip();
        }
        device.pop_clip();
      }
      BorderStyle::Inset | BorderStyle::Outset => {
        self.paint_inset_or_outset(self.style == BorderStyle::Inset, color, at, device);
      }
      _ => self.fill_bounds(color, at, device),
    }

    device.pop_clip();
    device.pop_clip();
    if alpha_layer {
      device.end_layer();
    }
  }

  /// Blink's `PaintDottedOrDashedOutline`: a rounded outline strokes its whole centre path, and a
  /// square one each edge on its own so its corners take whole dots or dashes.
  fn paint_dotted_or_dashed(&self, color: Color, at: Affine, device: &mut dyn PaintDevice) {
    let dashed = self.style == BorderStyle::Dashed || self.width <= 3.0;
    let thickness = self.stroke_width(dashed);
    let (center, from_inner) = self.center(false);

    if self.radius.is_some() {
      let path = self.path(&center, from_inner, from_inner);
      let length = path_length(&path).trunc();

      device.stroke_shape(
        &FillShape::Path {
          commands: path,
          rule: FillRule::NonZero,
        },
        &StrokeStyle::border(
          color,
          thickness,
          self.style.dash_pattern(self.width, length, true),
        ),
        at,
      );
      return;
    }

    for (start, end) in center.lines() {
      self
        .straight_edge(start, end, thickness, self.style, color)
        .paint(at, device);
    }
  }

  /// Blink's `PaintInsetOrOutsetOutline`: the lit edges in the colour, the shaded ones darker.
  fn paint_inset_or_outset(
    &self,
    inset: bool,
    color: Color,
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    self.paint_top_left_or_bottom_right(!inset, color, at, device);
    self.paint_top_left_or_bottom_right(inset, color.dark(), at, device);
  }

  /// Blink's `PaintTopLeftOrBottomRight`: strokes the top and left edges, or the bottom and right
  /// ones, each clipped to its mitres.
  fn paint_top_left_or_bottom_right(
    &self,
    top_left: bool,
    color: Color,
    at: Affine,
    device: &mut dyn PaintDevice,
  ) {
    let thickness = self.stroke_width(true);
    let (center, from_inner) = self.center(false);
    let rounded_edges = self.radius.map(|radius| {
      let radii = self.radii(radius, from_inner);

      center.rounded_edges(radii, radii, ((self.width as i32 + 1) / 2) as f32)
    });
    let lines: Vec<_> = if rounded_edges.is_some() {
      self.outer.lines().collect()
    } else {
      center.lines().collect()
    };
    let count = lines.len();

    for (index, &(start, end)) in lines.iter().enumerate() {
      let is_top_or_left = start.x < end.x || start.y > end.y;

      if is_top_or_left != top_left {
        continue;
      }

      let previous = lines[(index + count - 1) % count].0;
      let next = lines[(index + 1) % count].1;

      device.push_aliased_clip(&self.miter_clip(previous, start, end, next), at);
      match &rounded_edges {
        Some(edges) => device.stroke_shape(
          &FillShape::Path {
            commands: edges[index].clone(),
            rule: FillRule::NonZero,
          },
          &StrokeStyle::border(color, thickness, None),
          at,
        ),
        None => {
          self
            .straight_edge(start, end, thickness, BorderStyle::Solid, color)
            .paint(at, device);
        }
      }
      device.pop_clip();
    }
  }

  /// Blink's `MiterClipPath`: the area between the 45° mitres at an edge's ends, across the
  /// outline's bounds.
  fn miter_clip(
    &self,
    previous: Point<f32>,
    start: Point<f32>,
    end: Point<f32>,
    next: Point<f32>,
  ) -> FillShape {
    let (low, high) = self.outer.bounds();
    let slope = |first: Point<f32>, corner: Point<f32>, third: Point<f32>| {
      let same = if first.x == corner.x {
        (third.x > corner.x) == (corner.y > first.y)
      } else {
        (third.y > corner.y) == (corner.x > first.x)
      };

      if same { 1.0 } else { -1.0 }
    };
    let (start_slope, end_slope) = (slope(previous, start, end), slope(start, end, next));
    let mut commands = Vec::with_capacity(10);

    commands.move_to((start.x + start_slope * (start.y - low.y), low.y));
    commands.line_to((end.x + end_slope * (end.y - low.y), low.y));
    commands.line_to((end.x - end_slope * (high.y - end.y), high.y));
    commands.line_to((start.x - start_slope * (high.y - start.y), high.y));
    commands.close();

    // Otherwise the quadrilateral crosses itself and the vertical edge lies outside it, so the
    // clip is everything else, as Skia's inverse winding makes it.
    if start_slope != end_slope && start.x == end.x {
      let reach = (high.x - low.x) + (high.y - low.y);

      commands.move_to((low.x - reach, low.y - reach));
      commands.line_to((high.x + reach, low.y - reach));
      commands.line_to((high.x + reach, high.y + reach));
      commands.line_to((low.x - reach, high.y + reach));
      commands.close();

      return FillShape::Path {
        commands,
        rule: FillRule::EvenOdd,
      };
    }
    FillShape::Path {
      commands,
      rule: FillRule::NonZero,
    }
  }

  /// Blink's `PaintStraightEdge`: the edge, run on at both ends to cover its corners, on whole
  /// pixels, as `DrawLineWithStyle` draws a box side.
  fn straight_edge(
    &self,
    start: Point<f32>,
    end: Point<f32>,
    thickness: f32,
    style: BorderStyle,
    color: Color,
  ) -> StyledLine {
    let (start, end) = if start.x > end.x || start.y > end.y {
      (end, start)
    } else {
      (start, end)
    };
    let joint = ((self.width as i32 + 1) / 2) as f32;
    let horizontal = start.y == end.y;
    let run_on = |point: Point<f32>, by: f32| Point {
      x: (point.x + if horizontal { by } else { 0.0 }).round(),
      y: (point.y + if horizontal { 0.0 } else { by }).round(),
    };
    let (start, end) = (run_on(start, -joint), run_on(end, joint));
    let half_pixel = if thickness.round() as i32 % 2 == 1 {
      0.5
    } else {
      0.0
    };
    let shift = |point: Point<f32>| Point {
      x: point.x + if horizontal { 0.0 } else { half_pixel },
      y: point.y + if horizontal { half_pixel } else { 0.0 },
    };

    StyledLine::box_side(shift(start), shift(end), thickness, style, color)
  }
}

/// The length of a path of lines and cubics, each cubic measured over sixteen chords.
pub(super) fn path_length(path: &[PathCommand]) -> f32 {
  let (mut length, mut current, mut start) = (0.0, Point::ZERO, Point::ZERO);
  let distance = |a: Point<f32>, b: Point<f32>| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();

  for command in path {
    match *command {
      PathCommand::MoveTo(point) => {
        current = point;
        start = point;
      }
      PathCommand::LineTo(point) => {
        length += distance(current, point);
        current = point;
      }
      PathCommand::QuadTo(control, point) => {
        let lift = |from: Point<f32>| Point {
          x: from.x + (control.x - from.x) * 2.0 / 3.0,
          y: from.y + (control.y - from.y) * 2.0 / 3.0,
        };

        length += bezier_length(current, lift(current), lift(point), point);
        current = point;
      }
      PathCommand::CubicTo(first, second, point) => {
        length += bezier_length(current, first, second, point);
        current = point;
      }
      PathCommand::Close => {
        length += distance(current, start);
        current = start;
      }
    }
  }
  length
}

fn bezier_length(from: Point<f32>, first: Point<f32>, second: Point<f32>, to: Point<f32>) -> f32 {
  const CHORDS: usize = 16;

  let point_at = |t: f32| {
    let inverse = 1.0 - t;
    let (a, b, c, d) = (
      inverse * inverse * inverse,
      3.0 * inverse * inverse * t,
      3.0 * inverse * t * t,
      t * t * t,
    );

    Point {
      x: a * from.x + b * first.x + c * second.x + d * to.x,
      y: a * from.y + b * first.y + c * second.y + d * to.y,
    }
  };
  let mut previous = from;

  (1..=CHORDS)
    .map(|step| {
      let point = point_at(step as f32 / CHORDS as f32);
      let chord = ((point.x - previous.x).powi(2) + (point.y - previous.y).powi(2)).sqrt();

      previous = point;
      chord
    })
    .sum()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_quadratic_measures_as_its_degree_elevated_cubic() {
    let path = [
      PathCommand::MoveTo(Point::ZERO),
      PathCommand::QuadTo(Point { x: 10.0, y: 10.0 }, Point { x: 20.0, y: 0.0 }),
    ];

    assert!((path_length(&path) - 22.956).abs() < 0.05);
  }
}

//! Corners of a rectangle whose corners follow `corner-shape`, after Blink's
//! [`ContouredRect`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/geometry/contoured_rect.h)
//! and the radii constraint in
//! [`contoured_border_geometry.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/contoured_border_geometry.cc).

// Follows Blink, under the notice in LICENSE-CHROMIUM.

use std::f32::consts::{FRAC_1_SQRT_2, SQRT_2};

use crate::{
  geometry::{PathBuilder, PathCommand, Point, Rect, Size},
  layout::border::append_shaped_corner,
  style::{Sides, SpacePair, Superellipse},
};

/// Blink's `CornerCurvature` values: `kNotch`, `kRound` and `kStraight`.
const NOTCH: f32 = 1.0 / STRAIGHT;
const ROUND: f32 = 2.0;
const STRAIGHT: f32 = 1000.0;

/// One corner: the curve from `start` to `end` inside the rectangle with `outer` and `center` as
/// its other two vertices, and the superellipse exponent it follows.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Corner {
  pub(crate) start: Point<f32>,
  pub(crate) outer: Point<f32>,
  pub(crate) end: Point<f32>,
  pub(crate) center: Point<f32>,
  pub(crate) curvature: f32,
}

impl Corner {
  /// The four corners of a `size` box at `origin` with `radii` and `curvatures`, clockwise from
  /// the top-right, as Blink's `TopRightCorner`, `BottomRightCorner`, `BottomLeftCorner` and
  /// `TopLeftCorner`.
  pub(crate) fn of_box(
    origin: Point<f32>,
    size: Size<f32>,
    radii: &Sides<SpacePair<f32>>,
    curvatures: [f32; 4],
  ) -> [Self; 4] {
    let [top_left, top_right, bottom_right, bottom_left] = radii.0;
    let point = |x, y| Point {
      x: origin.x + x,
      y: origin.y + y,
    };
    let Size { width, height } = size;
    let [
      top_left_curvature,
      top_right_curvature,
      bottom_right_curvature,
      bottom_left_curvature,
    ] = curvatures;

    [
      Self {
        start: point(width - top_right.x, 0.0),
        outer: point(width, 0.0),
        end: point(width, top_right.y),
        center: point(width - top_right.x, top_right.y),
        curvature: top_right_curvature,
      },
      Self {
        start: point(width, height - bottom_right.y),
        outer: point(width, height),
        end: point(width - bottom_right.x, height),
        center: point(width - bottom_right.x, height - bottom_right.y),
        curvature: bottom_right_curvature,
      },
      Self {
        start: point(bottom_left.x, height),
        outer: point(0.0, height),
        end: point(0.0, height - bottom_left.y),
        center: point(bottom_left.x, height - bottom_left.y),
        curvature: bottom_left_curvature,
      },
      Self {
        start: point(0.0, top_left.y),
        outer: point(0.0, 0.0),
        end: point(top_left.x, 0.0),
        center: point(top_left.x, top_left.y),
        curvature: top_left_curvature,
      },
    ]
  }

  /// Each corner's curvature, clockwise from the top-left: its shape's exponent, or `round` where
  /// the radius is empty, as Blink's `EffectiveCurvature`.
  pub(crate) fn curvatures(
    radii: &Sides<SpacePair<f32>>,
    shapes: &Sides<Superellipse>,
  ) -> [f32; 4] {
    let mut curvatures = [ROUND; 4];

    for (index, curvature) in curvatures.iter_mut().enumerate() {
      let radius = radii.0[index];

      if radius.x != 0.0 && radius.y != 0.0 {
        *curvature = shapes.0[index].exponent().clamp(NOTCH, STRAIGHT);
      }
    }

    curvatures
  }

  fn v1(&self) -> Point<f32> {
    self.outer - self.start
  }

  fn v2(&self) -> Point<f32> {
    self.end - self.outer
  }

  fn v3(&self) -> Point<f32> {
    self.center - self.end
  }

  /// Whether the corner has no extent along either edge, as Blink's `IsEmpty`.
  fn is_empty(&self) -> bool {
    length(self.v1()) == 0.0 || length(self.v2()) == 0.0
  }

  fn same_as(&self, other: &Self) -> bool {
    self.start == other.start
      && self.outer == other.outer
      && self.end == other.end
      && self.center == other.center
      && self.curvature == other.curvature
  }

  /// Blink's `AlignedToOrigin`: this corner of a rectangle inset from the one `origin` belongs to,
  /// reshaped so the band between them keeps one thickness along the curve.
  fn aligned_to_origin(&self, origin: &Self, edge_inset_start: f32, edge_inset_end: f32) -> Self {
    if origin.is_empty() || self.same_as(origin) {
      return *self;
    }

    let start_radius = length(origin.v2());
    let end_radius = length(origin.v3());
    // Blink keeps insets beyond the radius because `FloatRoundedRect` clamps the inset radii to
    // zero.
    let start_inset = if edge_inset_start >= 0.0 && length(self.v2()) == 0.0 {
      edge_inset_start
    } else {
      start_radius - length(self.v2())
    };
    let end_inset = if edge_inset_end >= 0.0 && length(self.v3()) == 0.0 {
      edge_inset_end
    } else {
      end_radius - length(self.v3())
    };
    let parameter = origin.curvature.log2();
    let exponent = parameter.abs().exp2();
    let convex_half_corner = 0.5f32.powf(1.0 / exponent);
    let half_corner = if parameter < 0.0 {
      1.0 - convex_half_corner
    } else {
      convex_half_corner
    };
    let control_point = (half_corner / (SQRT_2 - 1.0) - FRAC_1_SQRT_2).clamp(0.0, 1.0);
    let inset_difference = (end_inset - start_inset).clamp(-start_radius, end_radius);
    let (mut start_control, mut end_control) = (control_point, control_point);

    if inset_difference != 0.0 {
      let normal_delta = (start_radius * start_radius + end_radius * end_radius
        - inset_difference * inset_difference)
        .sqrt();
      let normal_x = end_radius * inset_difference + start_radius * normal_delta;
      let normal_y = -start_radius * inset_difference + end_radius * normal_delta;
      let bevel_control =
        start_radius * normal_y / (start_radius * normal_y + end_radius * normal_x);

      start_control = if parameter < 0.0 {
        bevel_control * (2.0 * control_point)
      } else {
        1.0 - (1.0 - bevel_control) * (2.0 * (1.0 - control_point))
      };
      end_control = 2.0 * control_point - start_control;
    }

    let unmapped_start = normalize(Point {
      x: (1.0 - start_control) * start_radius,
      y: start_control * end_radius,
    });
    let unmapped_end = normalize(Point {
      x: end_control * start_radius,
      y: (1.0 - end_control) * end_radius,
    });
    let v3 = normalize(origin.v3());
    let v2 = normalize(origin.v2());
    let start_normal = scale(v3, unmapped_start.x) + scale(v2, unmapped_start.y);
    let end_normal = scale(v3, unmapped_end.x) + scale(v2, unmapped_end.y);
    let original_outer = self.outer - scale(v3, end_inset) - scale(v2, start_inset);
    let mut start = original_outer + scale(v3, end_radius) + scale(start_normal, start_inset);
    let mut end = original_outer + scale(v2, start_radius) + scale(end_normal, end_inset);
    let start_tangent = Point {
      x: -start_normal.y,
      y: start_normal.x,
    };
    let end_tangent = Point {
      x: -end_normal.y,
      y: end_normal.x,
    };

    if parameter >= 0.0 && start_inset < 0.0 {
      start =
        intersection(start, start - start_tangent, self.outer + v3, self.outer).unwrap_or(start);
    }
    if parameter >= 0.0 && end_inset < 0.0 {
      end = intersection(end, end + end_tangent, self.outer + v2, self.outer).unwrap_or(end);
    }

    let height = dot(end - start, v2);
    let outer = end - scale(v2, height);
    let center = start + scale(v2, height);

    if parameter <= -1.0 || parameter >= 0.0 {
      return Self {
        start,
        outer,
        end,
        center,
        curvature: origin.curvature,
      };
    }

    let tangents =
      intersection(start, start - start_tangent, end, end + end_tangent).unwrap_or(start);

    Self {
      start,
      outer: tangents,
      end,
      center: start + (end - tangents),
      curvature: ROUND,
    }
  }

  /// Blink's `AddCurvedCorner`: a line to the corner's start, then its curve to the end.
  fn append_curve(&self, path: &mut Vec<PathCommand>) {
    path.line_to((self.start.x, self.start.y));

    if self.curvature >= STRAIGHT || self.is_empty() {
      path.line_to((self.outer.x, self.outer.y));
      path.line_to((self.end.x, self.end.y));
      return;
    }

    append_shaped_corner(
      path,
      Superellipse(self.curvature.log2()),
      self.center,
      self.v1(),
      self.start - self.center,
    );
  }

  /// The point halfway along the curve, as Blink's `HalfCorner`.
  fn half_corner(&self) -> Point<f32> {
    let half = 0.5f32.powf(1.0 / self.curvature);

    self.map(half, half)
  }

  /// Blink's `MapPoint`: `(x, y)` in the unit square whose origin is the center.
  fn map(&self, x: f32, y: f32) -> Point<f32> {
    let v1 = self.outer - self.start;
    let v4 = self.start - self.center;

    Point {
      x: self.center.x + v1.x * x + v4.x * y,
      y: self.center.y + v1.y * x + v4.y * y,
    }
  }

  /// The corner's bounding box, as `(min, max)`.
  fn bounding_box(&self) -> (Point<f32>, Point<f32>) {
    (
      Point {
        x: self.start.x.min(self.end.x),
        y: self.start.y.min(self.end.y),
      },
      Point {
        x: self.start.x.max(self.end.x),
        y: self.start.y.max(self.end.y),
      },
    )
  }

  /// Blink's `ComputeHullQuad`: the curve's hull, cut by the tangent at its half point.
  fn hull(&self) -> [Point<f32>; 4] {
    let half = self.half_corner();
    let normal = Point {
      x: self.outer.y - half.y,
      y: half.x - self.outer.x,
    };
    let tangent_end = half + normal;
    let first = intersection(half, tangent_end, self.start, self.center).unwrap_or(self.center);
    let second = intersection(half, tangent_end, self.end, self.center).unwrap_or(self.center);

    [self.start, first, second, self.end]
  }
}

/// How far to scale every radius so that no two opposite concave corners overlap, as Blink's
/// `RadiiConstraintFactorForOppositeCorners` decides for each diagonal.
pub(crate) fn opposite_corners_factor(corners: &[Corner; 4]) -> f32 {
  let [top_right, bottom_right, bottom_left, top_left] = corners;

  opposite_pair_factor(top_left, bottom_right)
    .min(opposite_pair_factor(bottom_left, top_right))
    .min(1.0)
}

fn opposite_pair_factor(a: &Corner, b: &Corner) -> f32 {
  /// Blink shrinks the hulls by this much before calling them touching, to ignore floating-point
  /// noise.
  const NEAR_TOUCHING_SCALE: f32 = 1.0 - 5.0 * f32::EPSILON;

  let (a_min, a_max) = a.bounding_box();
  let (b_min, b_max) = b.bounding_box();
  let boxes_intersect =
    a_min.x < b_max.x && b_min.x < a_max.x && a_min.y < b_max.y && b_min.y < a_max.y;

  if !boxes_intersect {
    return 1.0;
  }

  let (hull_a, hull_b) = (a.hull(), b.hull());

  if !quads_intersect(&hull_a, &hull_b)
    || !quads_intersect(
      &scale_quad(&hull_a, a.outer, NEAR_TOUCHING_SCALE),
      &scale_quad(&hull_b, b.outer, NEAR_TOUCHING_SCALE),
    )
  {
    return 1.0;
  }

  solve_hull_scale(&hull_a, a.outer, &hull_b, b.outer, 0.0, 1.0)
}

/// Blink's `SolveOptimalHullScale`: the largest scale, to within 0.05, at which the hulls scaled
/// about their outer corners stop intersecting.
fn solve_hull_scale(
  a: &[Point<f32>; 4],
  a_origin: Point<f32>,
  b: &[Point<f32>; 4],
  b_origin: Point<f32>,
  min: f32,
  max: f32,
) -> f32 {
  const EPSILON: f32 = 0.05;

  if max - min <= EPSILON {
    return min;
  }

  let check = (min + max) / 2.0;

  if quads_intersect(
    &scale_quad(a, a_origin, check),
    &scale_quad(b, b_origin, check),
  ) {
    solve_hull_scale(a, a_origin, b, b_origin, min, check)
  } else {
    solve_hull_scale(a, a_origin, b, b_origin, check, max)
  }
}

fn scale_quad(quad: &[Point<f32>; 4], origin: Point<f32>, scale: f32) -> [Point<f32>; 4] {
  quad.map(|point| Point {
    x: origin.x + (point.x - origin.x) * scale,
    y: origin.y + (point.y - origin.y) * scale,
  })
}

fn cross(u: Point<f32>, v: Point<f32>) -> f32 {
  u.x * v.y - u.y * v.x
}

/// Where the lines through `a`–`b` and `c`–`d` cross, as gfx's `LineF::IntersectionWith`.
fn intersection(a: Point<f32>, b: Point<f32>, c: Point<f32>, d: Point<f32>) -> Option<Point<f32>> {
  let (ab, cd) = (b - a, d - c);
  let denominator = cross(ab, cd);

  if denominator == 0.0 {
    return None;
  }

  let param = cross(c - a, cd) / denominator;

  Some(Point {
    x: a.x + ab.x * param,
    y: a.y + ab.y * param,
  })
}

/// gfx's `QuadF::IntersectsQuad` for convex quads.
fn quads_intersect(a: &[Point<f32>; 4], b: &[Point<f32>; 4]) -> bool {
  !fully_outside_one_edge(a, b) && !fully_outside_one_edge(b, a)
}

/// gfx's `QuadF::FullyOutsideOneEdge`: whether `other` lies left of or on one edge of `quad`.
fn fully_outside_one_edge(quad: &[Point<f32>; 4], other: &[Point<f32>; 4]) -> bool {
  let [p1, p2, p3, p4] = *quad;
  let edges = if counter_clockwise(quad) {
    [(p1, p4 - p1), (p2, p1 - p2), (p3, p2 - p3), (p4, p3 - p4)]
  } else {
    [(p1, p2 - p1), (p2, p3 - p2), (p3, p4 - p3), (p4, p1 - p4)]
  };

  edges
    .iter()
    .any(|&(base, edge)| other.iter().all(|&point| cross(edge, point - base) < 0.0))
}

/// gfx's `QuadF::IsCounterClockwise`, in screen coordinates with y pointing down.
fn counter_clockwise(quad: &[Point<f32>; 4]) -> bool {
  let [p1, p2, p3, p4] = *quad;
  let p24 = f64::from(p2.y - p4.y);
  let p31 = f64::from(p3.y - p1.y);

  f64::from(p1.x) * p24 + f64::from(p2.x) * p31 < f64::from(p3.x) * p24 + f64::from(p4.x) * p31
}

fn dot(u: Point<f32>, v: Point<f32>) -> f32 {
  u.x * v.x + u.y * v.y
}

fn length(v: Point<f32>) -> f32 {
  dot(v, v).sqrt()
}

fn scale(v: Point<f32>, factor: f32) -> Point<f32> {
  Point {
    x: v.x * factor,
    y: v.y * factor,
  }
}

fn normalize(v: Point<f32>) -> Point<f32> {
  let length = length(v);

  if length == 0.0 {
    v
  } else {
    scale(v, 1.0 / length)
  }
}

/// Whether a box with `radii` and `curvatures` is a plain rounded rectangle, as Blink's
/// `HasRoundCurvature`: every corner is round, or none is rounded at all.
pub(crate) fn has_round_curvature(radii: &Sides<SpacePair<f32>>, curvatures: [f32; 4]) -> bool {
  curvatures.iter().all(|&curvature| curvature == ROUND)
    || radii
      .0
      .iter()
      .all(|radius| radius.x == 0.0 && radius.y == 0.0)
}

/// One piece of a corner curve.
#[derive(Debug, Clone, Copy)]
enum Segment {
  Line(Point<f32>, Point<f32>),
  Cubic([Point<f32>; 4]),
}

impl Segment {
  fn start(&self) -> Point<f32> {
    match self {
      Self::Line(start, _) => *start,
      Self::Cubic([start, ..]) => *start,
    }
  }

  fn at(&self, t: f32) -> Point<f32> {
    match *self {
      Self::Line(start, end) => start + scale(end - start, t),
      Self::Cubic([p0, p1, p2, p3]) => {
        let u = 1.0 - t;

        scale(p0, u * u * u)
          + scale(p1, 3.0 * u * u * t)
          + scale(p2, 3.0 * u * t * t)
          + scale(p3, t * t * t)
      }
    }
  }

  /// The piece between `from` and `to`, found by de Casteljau subdivision.
  fn between(&self, from: f32, to: f32) -> Self {
    match *self {
      Self::Line(..) => Self::Line(self.at(from), self.at(to)),
      Self::Cubic(points) => {
        let tail = split_cubic(points, from).1;
        let local = if from < 1.0 {
          (to - from) / (1.0 - from)
        } else {
          0.0
        };

        Self::Cubic(split_cubic(tail, local).0)
      }
    }
  }

  fn append(&self, path: &mut Vec<PathCommand>) {
    match self {
      Self::Line(_, end) => path.line_to((end.x, end.y)),
      Self::Cubic([_, c1, c2, end]) => path.curve_to((c1.x, c1.y), (c2.x, c2.y), (end.x, end.y)),
    }
  }
}

/// A cubic split at `t` into its two halves.
fn split_cubic([p0, p1, p2, p3]: [Point<f32>; 4], t: f32) -> ([Point<f32>; 4], [Point<f32>; 4]) {
  let lerp = |a: Point<f32>, b: Point<f32>| a + scale(b - a, t);
  let (p01, p12, p23) = (lerp(p0, p1), lerp(p1, p2), lerp(p2, p3));
  let (p012, p123) = (lerp(p01, p12), lerp(p12, p23));
  let mid = lerp(p012, p123);

  ([p0, p01, p012, mid], [mid, p123, p23, p3])
}

/// Where along `segment` the increasing `measure` reaches zero: 0 when it starts at or above
/// zero, 1 when it never does.
fn crossing(segment: &Segment, measure: impl Fn(Point<f32>) -> f32) -> f32 {
  if measure(segment.start()) >= 0.0 {
    return 0.0;
  }
  if measure(segment.at(1.0)) < 0.0 {
    return 1.0;
  }

  let (mut low, mut high) = (0.0, 1.0);

  for _ in 0..32 {
    let middle = (low + high) / 2.0;

    if measure(segment.at(middle)) >= 0.0 {
      high = middle;
    } else {
      low = middle;
    }
  }

  high
}

impl Corner {
  /// The corner's curve as segments from start to end, as Blink's `AddCurvedCorner` draws it.
  fn segments(&self) -> Vec<Segment> {
    if self.curvature >= STRAIGHT || self.is_empty() {
      return vec![
        Segment::Line(self.start, self.outer),
        Segment::Line(self.outer, self.end),
      ];
    }

    let mut commands = vec![PathCommand::MoveTo(self.start)];

    self.append_curve(&mut commands);

    let mut current = self.start;

    commands
      .into_iter()
      .skip(1)
      .filter_map(|command| {
        let segment = match command {
          PathCommand::LineTo(end) if end != current => Segment::Line(current, end),
          PathCommand::CubicTo(c1, c2, end) => Segment::Cubic([current, c1, c2, end]),
          _ => return None,
        };

        current = match segment {
          Segment::Line(_, end) | Segment::Cubic([.., end]) => end,
        };
        Some(segment)
      })
      .collect()
  }
}

/// The corner `target` takes once it follows `origin`'s at the given edge insets, cut to the
/// target rectangle: the pieces of the aligned curve inside it, or `None` when the corner stays
/// square. This is what Blink's `AddContouredRect` leaves of the rectangle after subtracting each
/// [border-aligned corner clip-out path](https://drafts.csswg.org/css-borders-4/#border-aligned-corner-clip-out-path),
/// for a rectangle inset from its origin.
fn inset_corner(
  origin: &Corner,
  target: &Corner,
  edge_inset_start: f32,
  edge_inset_end: f32,
) -> Option<Vec<Segment>> {
  let start_radius = length(origin.v2());
  let end_radius = length(origin.v3());

  if start_radius == 0.0 || end_radius == 0.0 || origin.curvature >= STRAIGHT {
    return None;
  }

  let start_inset = if edge_inset_start >= 0.0 && length(target.v2()) == 0.0 {
    edge_inset_start
  } else {
    start_radius - length(target.v2())
  };
  let end_inset = if edge_inset_end >= 0.0 && length(target.v3()) == 0.0 {
    edge_inset_end
  } else {
    end_radius - length(target.v3())
  };
  let inset_difference = (end_inset - start_inset).clamp(-start_radius, end_radius);

  if origin.curvature <= 1.0
    && (inset_difference == -start_radius || inset_difference == end_radius)
  {
    return None;
  }

  let aligned = target.aligned_to_origin(origin, edge_inset_start, edge_inset_end);
  let along_start = normalize(origin.v1());
  let along_end = normalize(origin.v2());
  // Past the start edge while this grows through zero, and still inside the end edge while this
  // stays below zero.
  let past_start = |point: Point<f32>| dot(point - target.outer, along_end);
  let before_end = |point: Point<f32>| dot(point - target.outer, along_start);
  let segments = if origin.curvature <= NOTCH {
    vec![
      Segment::Line(aligned.start, aligned.center),
      Segment::Line(aligned.center, aligned.end),
    ]
  } else {
    aligned.segments()
  };
  let pieces: Vec<Segment> = segments
    .iter()
    .filter_map(|segment| {
      let from = crossing(segment, past_start);
      let to = crossing(segment, before_end);

      (from < to).then(|| segment.between(from, to))
    })
    .collect();

  (!pieces.is_empty()).then_some(pieces)
}

/// The rectangle at `target` inside the `origin` box, its corners following the origin's
/// `radii` and `curvatures` as Blink's `AddContouredRect` draws an inset contoured rectangle.
pub(crate) fn append_inset_contour(
  origin_size: Size<f32>,
  radii: &Sides<SpacePair<f32>>,
  curvatures: [f32; 4],
  target: Rect<f32>,
  path: &mut Vec<PathCommand>,
) {
  let origin_corners = Corner::of_box(Point::ZERO, origin_size, radii, curvatures);
  let insets = Rect {
    left: target.left,
    top: target.top,
    right: origin_size.width - target.right,
    bottom: origin_size.height - target.bottom,
  };
  let shrink = |radius: f32, inset: f32| {
    if radius > 0.0 {
      (radius - inset).max(0.0)
    } else {
      radius
    }
  };
  let [top_left, top_right, bottom_right, bottom_left] = radii.0;
  let target_radii = Sides([
    SpacePair {
      x: shrink(top_left.x, insets.left),
      y: shrink(top_left.y, insets.top),
    },
    SpacePair {
      x: shrink(top_right.x, insets.right),
      y: shrink(top_right.y, insets.top),
    },
    SpacePair {
      x: shrink(bottom_right.x, insets.right),
      y: shrink(bottom_right.y, insets.bottom),
    },
    SpacePair {
      x: shrink(bottom_left.x, insets.left),
      y: shrink(bottom_left.y, insets.bottom),
    },
  ]);
  let size = Size {
    width: (target.right - target.left).max(0.0),
    height: (target.bottom - target.top).max(0.0),
  };
  let target_corners = Corner::of_box(
    Point {
      x: target.left,
      y: target.top,
    },
    size,
    &target_radii,
    curvatures,
  );
  let edge_insets = [
    (insets.top, insets.right),
    (insets.right, insets.bottom),
    (insets.bottom, insets.left),
    (insets.left, insets.top),
  ];
  let mut started = false;
  let mut visit = |point: Point<f32>, path: &mut Vec<PathCommand>| {
    if started {
      path.line_to((point.x, point.y));
    } else {
      path.move_to((point.x, point.y));
      started = true;
    }
  };

  for ((origin, target), (start, end)) in
    origin_corners.iter().zip(&target_corners).zip(edge_insets)
  {
    match inset_corner(origin, target, start, end) {
      Some(pieces) => {
        visit(pieces[0].start(), path);
        for piece in &pieces {
          piece.append(path);
        }
      }
      None => visit(target.outer, path),
    }
  }

  path.close();
}

impl Corner {
  /// The same corner traversed from end to start, as Blink's `Reverse`.
  fn reverse(&self) -> Self {
    Self {
      start: self.end,
      outer: self.outer,
      end: self.start,
      center: self.center,
      curvature: self.curvature,
    }
  }

  /// The concave corner as the convex one mirrored across its chord, as Blink's `Inverse`.
  fn inverse(&self) -> Self {
    Self {
      start: self.start,
      outer: self.center,
      end: self.end,
      center: self.outer,
      curvature: 1.0 / self.curvature,
    }
  }

  /// Blink's `QuadraticControlPoint`: the control point of the quadratic closest to the curve.
  fn quadratic_control_point(&self) -> Point<f32> {
    if self.curvature < 1.0 {
      return self.inverse().quadratic_control_point();
    }
    if self.curvature >= ROUND {
      return self.outer;
    }

    let normalized = 2.0 * 0.5f32.powf(1.0 / self.curvature) - 0.5;

    self.map(normalized, normalized)
  }
}

/// A side's corner: the outer border edge's, the inner edge's aligned to it, and where the inner
/// edge's rectangle has its corner.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CornerInfo {
  pub(crate) outer: Corner,
  pub(crate) inner: Corner,
  pub(crate) unadjusted_inner_edge: Point<f32>,
}

/// One clip a side paints under.
pub(crate) struct SideClip {
  pub(crate) path: Vec<PathCommand>,
  /// Whether the clip keeps what lies outside the path.
  pub(crate) out: bool,
  pub(crate) antialias: bool,
}

/// The corners of `inner` inside the `origin_size` border box, aligned to the border box's, as
/// Blink's `ContouredRect` hands out the corners of a rectangle with an origin.
pub(crate) fn aligned_inset_corners(
  origin_size: Size<f32>,
  radii: &Sides<SpacePair<f32>>,
  curvatures: [f32; 4],
  inner: Rect<f32>,
) -> [Corner; 4] {
  let origin_corners = Corner::of_box(Point::ZERO, origin_size, radii, curvatures);
  let insets = Rect {
    left: inner.left,
    top: inner.top,
    right: origin_size.width - inner.right,
    bottom: origin_size.height - inner.bottom,
  };
  let shrink = |radius: f32, inset: f32| {
    if radius > 0.0 {
      (radius - inset).max(0.0)
    } else {
      radius
    }
  };
  let [top_left, top_right, bottom_right, bottom_left] = radii.0;
  let inner_radii = Sides([
    SpacePair {
      x: shrink(top_left.x, insets.left),
      y: shrink(top_left.y, insets.top),
    },
    SpacePair {
      x: shrink(top_right.x, insets.right),
      y: shrink(top_right.y, insets.top),
    },
    SpacePair {
      x: shrink(bottom_right.x, insets.right),
      y: shrink(bottom_right.y, insets.bottom),
    },
    SpacePair {
      x: shrink(bottom_left.x, insets.left),
      y: shrink(bottom_left.y, insets.bottom),
    },
  ]);
  let inner_corners = Corner::of_box(
    Point {
      x: inner.left,
      y: inner.top,
    },
    Size {
      width: (inner.right - inner.left).max(0.0),
      height: (inner.bottom - inner.top).max(0.0),
    },
    &inner_radii,
    curvatures,
  );
  let edge_insets = [
    (insets.top, insets.right),
    (insets.right, insets.bottom),
    (insets.bottom, insets.left),
    (insets.left, insets.top),
  ];
  let mut aligned = inner_corners;

  for (index, (start, end)) in edge_insets.into_iter().enumerate() {
    aligned[index] = inner_corners[index].aligned_to_origin(&origin_corners[index], start, end);
  }

  aligned
}

/// Blink's `ExtendInnerCornerToIncludePaddingEdgeIfNeeded`: grows the inner corner inward so it
/// reaches the padding edge when the border is wider than the radius.
fn extend_inner_corner(corner: &mut CornerInfo) {
  if corner.outer.start == corner.outer.end || corner.outer.curvature >= STRAIGHT {
    return;
  }

  let direction = normalize(corner.inner.v2());
  let unadjusted = corner.unadjusted_inner_edge - corner.outer.outer;
  let reach = length(Point {
    x: unadjusted.x * direction.x,
    y: unadjusted.y * direction.y,
  })
  .max(length(corner.inner.v2()));
  let adjusted = scale(direction, reach);
  let inner = corner.inner;

  corner.inner = Corner {
    start: inner.start,
    outer: inner.outer,
    end: inner.outer + adjusted,
    center: inner.start + adjusted,
    curvature: inner.curvature,
  };
}

/// The union of the inner corners' boxes and the rectangle between their unadjusted corners, as
/// Blink's `UnionInnerCornersAndEdge`, as `(min, max)`.
fn inner_corners_and_edge(a: &CornerInfo, b: &CornerInfo) -> (Point<f32>, Point<f32>) {
  let points = [
    a.inner.start,
    a.inner.end,
    b.inner.start,
    b.inner.end,
    a.unadjusted_inner_edge,
    b.unadjusted_inner_edge,
  ];
  let min = points.iter().fold(points[0], |low, point| Point {
    x: low.x.min(point.x),
    y: low.y.min(point.y),
  });
  let max = points.iter().fold(points[0], |high, point| Point {
    x: high.x.max(point.x),
    y: high.y.max(point.y),
  });

  (min, max)
}

fn box_contains((min, max): (Point<f32>, Point<f32>), point: Point<f32>) -> bool {
  min.x <= point.x && point.x < max.x && min.y <= point.y && point.y < max.y
}

fn boxes_intersect(
  (a_min, a_max): (Point<f32>, Point<f32>),
  (b_min, b_max): (Point<f32>, Point<f32>),
) -> bool {
  a_min.x < a_max.x
    && a_min.y < a_max.y
    && b_min.x < b_max.x
    && b_min.y < b_max.y
    && a_min.x < b_max.x
    && b_min.x < a_max.x
    && a_min.y < b_max.y
    && b_min.y < a_max.y
}

/// Blink's `ClipOutHalfCornerWithMiter`: cuts away the half of `corners[0]` the adjacent side
/// owns past the miter.
fn clip_out_half_corner(corners: [&CornerInfo; 4], antialias: bool) -> SideClip {
  let [slice, other, opposite, adjacent] = corners;
  let opposite = opposite.outer.outer;
  let adjacent = adjacent.outer.outer;
  let (miter_start, miter_end) = (slice.outer.outer, slice.unadjusted_inner_edge);
  let tangent_end = if other.inner.curvature < 1.0 {
    other.inner.quadratic_control_point()
  } else {
    other.inner.start
  };
  let tangent_hit = intersection(other.inner.end, tangent_end, miter_start, miter_end);
  let (other_min, other_max) = other.inner.bounding_box();
  let mut path = Vec::new();

  if let Some(hit) = tangent_hit
    && other_min.x <= hit.x
    && hit.x <= other_max.x
    && other_min.y <= hit.y
    && hit.y <= other_max.y
  {
    let across = intersection(
      other.inner.end,
      tangent_end,
      other.outer.outer,
      other.outer.start,
    )
    .unwrap_or(other.inner.center);

    path.move_to((slice.outer.outer.x, slice.outer.outer.y));
    path.line_to((hit.x, hit.y));
    path.line_to((across.x, across.y));
    path.line_to((opposite.x, opposite.y));
    path.line_to((adjacent.x, adjacent.y));
    path.line_to((slice.outer.start.x, slice.outer.start.y));
    path.close();

    return SideClip {
      path,
      out: true,
      antialias,
    };
  }

  let offset = slice.unadjusted_inner_edge - slice.outer.outer;
  let hypot =
    intersection(miter_start, miter_end, opposite + offset, adjacent + offset).unwrap_or(opposite);
  let from = slice.outer.outer - offset;
  let (near, far) = (adjacent + offset, adjacent - offset);

  path.move_to((from.x, from.y));
  path.line_to((hypot.x, hypot.y));
  path.line_to((near.x, near.y));
  path.line_to((far.x, far.y));
  path.close();

  SideClip {
    path,
    out: true,
    antialias,
  }
}

/// Blink's `ClipBorderSidePolygonFromCorners`: the clips a side between `corners[0]` and
/// `corners[1]` paints under, `corners[2]` and `corners[3]` being the opposite side's.
pub(crate) fn side_clips_from_corners(
  mut corners: [CornerInfo; 4],
  first_antialias: bool,
  second_antialias: bool,
  width_vector: Point<f32>,
  needs_miters: bool,
) -> Vec<SideClip> {
  let edge = inner_corners_and_edge(&corners[0], &corners[1]);
  let opposite = inner_corners_and_edge(&corners[2], &corners[3]);
  let mut clips = Vec::new();

  if boxes_intersect(edge, opposite)
    || box_contains(edge, corners[2].unadjusted_inner_edge)
    || box_contains(edge, corners[3].unadjusted_inner_edge)
    || box_contains(opposite, corners[0].unadjusted_inner_edge)
    || box_contains(opposite, corners[1].unadjusted_inner_edge)
  {
    let [first, second, ..] = &corners;
    let mut path = Vec::new();
    let point = |path: &mut Vec<PathCommand>, point: Point<f32>| path.line_to((point.x, point.y));

    path.move_to((first.outer.outer.x, first.outer.outer.y));
    point(&mut path, first.outer.start);
    first.inner.append_curve(&mut path);
    point(&mut path, first.outer.end + width_vector);
    point(&mut path, second.outer.start + width_vector);
    second.inner.append_curve(&mut path);
    point(&mut path, second.outer.end);
    point(&mut path, second.outer.outer);
    path.close();
    path.move_to((second.outer.outer.x, second.outer.outer.y));
    point(&mut path, first.outer.outer);
    point(&mut path, first.outer.outer + width_vector);
    point(&mut path, second.outer.outer + width_vector);
    path.close();

    clips.push(SideClip {
      path,
      out: false,
      antialias: true,
    });
  } else {
    let (min, max) = opposite;
    let mut path = Vec::new();

    path.move_to((min.x, min.y));
    path.line_to((max.x, min.y));
    path.line_to((max.x, max.y));
    path.line_to((min.x, max.y));
    path.close();
    clips.push(SideClip {
      path,
      out: true,
      antialias: false,
    });
  }

  if !needs_miters {
    return clips;
  }

  extend_inner_corner(&mut corners[0]);
  extend_inner_corner(&mut corners[1]);

  let second_reversed = CornerInfo {
    outer: corners[1].outer.reverse(),
    inner: corners[1].inner.reverse(),
    unadjusted_inner_edge: corners[1].unadjusted_inner_edge,
  };

  clips.push(clip_out_half_corner(
    [&corners[0], &second_reversed, &corners[2], &corners[3]],
    first_antialias,
  ));
  clips.push(clip_out_half_corner(
    [&second_reversed, &corners[0], &corners[3], &corners[2]],
    second_antialias,
  ));

  clips
}

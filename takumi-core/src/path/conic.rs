//! Skia's rational quadratics, `SkConic` in `src/core/SkGeometry.cpp`: the arcs strokes round their
//! caps and joins with, and their split into the quads a [`Path`](super::Path) holds.

use super::Point;
use crate::style::Affine;

/// `SK_ScalarNearlyZero`.
pub(crate) use crate::math::NEARLY_ZERO;

/// `SK_ScalarRoot2Over2`, the weight of a quarter circle.
pub(crate) const ROOT_2_OVER_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// `kMaxConicToQuadPOW2`.
const MAX_CONIC_TO_QUAD_POW2: u32 = 5;

/// The tolerance `SkEdgeBuilder` splits conics into quads to, a quarter pixel.
pub(crate) const CONIC_TOLERANCE: f32 = 0.25;

/// A conic through `points`, its middle point pulled by `weight`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Conic {
  pub(crate) points: [Point<f32>; 3],
  pub(crate) weight: f32,
}

/// `SkPointPriv::CanNormalize`.
pub(crate) fn can_normalize(dx: f32, dy: f32) -> bool {
  dx.is_finite() && dy.is_finite() && (dx != 0.0 || dy != 0.0)
}

/// `SkPoint::setLength`: `(x, y)` scaled to `length` through doubles, none where it has no
/// direction or leaves the finite range.
pub(crate) fn with_length(x: f32, y: f32, length: f32) -> Option<Point<f32>> {
  let magnitude = (f64::from(x) * f64::from(x) + f64::from(y) * f64::from(y)).sqrt();
  let scale = f64::from(length) / magnitude;
  let (x, y) = ((f64::from(x) * scale) as f32, (f64::from(y) * scale) as f32);

  (x.is_finite() && y.is_finite() && (x != 0.0 || y != 0.0)).then_some(Point::new(x, y))
}

/// `SkPointPriv::EqualsWithinTolerance` without a tolerance: no direction between them.
fn equals_within_tolerance(a: Point<f32>, b: Point<f32>) -> bool {
  !can_normalize(a.x - b.x, a.y - b.y)
}

/// `between` in `SkGeometry.cpp`.
fn between(a: f32, b: f32, c: f32) -> bool {
  (a - b) * (c - b) <= 0.0
}

impl Conic {
  /// `SkConic::chop`: the two halves at the middle.
  fn chop(&self) -> [Self; 2] {
    let scale = 1.0 / (1.0 + self.weight);
    let weight = (0.5 + self.weight * 0.5).sqrt();
    let [p0, p1, p2] = self.points;
    let weighted = Point::new(self.weight * p1.x, self.weight * p1.y);
    let mut middle = Point::new(
      (p0.x + (weighted.x + weighted.x) + p2.x) * scale * 0.5,
      (p0.y + (weighted.y + weighted.y) + p2.y) * scale * 0.5,
    );

    if !middle.is_finite() {
      let w = f64::from(self.weight);
      let scale_half = 1.0 / (1.0 + w) * 0.5;
      let axis = |a: f32, b: f32, c: f32| {
        ((f64::from(a) + w * 2.0 * f64::from(b) + f64::from(c)) * scale_half) as f32
      };

      middle = Point::new(axis(p0.x, p1.x, p2.x), axis(p0.y, p1.y, p2.y));
    }

    [
      Self {
        points: [
          p0,
          Point::new((p0.x + weighted.x) * scale, (p0.y + weighted.y) * scale),
          middle,
        ],
        weight,
      },
      Self {
        points: [
          middle,
          Point::new((weighted.x + p2.x) * scale, (weighted.y + p2.y) * scale),
          p2,
        ],
        weight,
      },
    ]
  }

  /// `SkConic::computeQuadPOW2`: how many halvings bring the conic within `tolerance` of quads.
  fn quad_pow2(&self, tolerance: f32) -> u32 {
    let bad_weight = self.weight < 0.0 || !self.weight.is_finite();

    if tolerance < 0.0
      || !tolerance.is_finite()
      || !self.points.iter().all(|point| point.is_finite())
      || bad_weight
    {
      return 0;
    }

    // `AS_QUAD_ERROR_SETUP`.
    let a = self.weight - 1.0;
    let k = a / (4.0 * (2.0 + a));
    let [p0, p1, p2] = self.points;
    let x = k * (p0.x - 2.0 * p1.x + p2.x);
    let y = k * (p0.y - 2.0 * p1.y + p2.y);
    let mut error = (x * x + y * y).sqrt();
    let mut pow2 = 0;

    while pow2 < MAX_CONIC_TO_QUAD_POW2 && error > tolerance {
      error *= 0.25;
      pow2 += 1;
    }

    pow2
  }

  /// `subdivide` in `SkGeometry.cpp`: the control and end points of `2^level` quads, keeping a
  /// y-monotonic conic's quads monotonic.
  fn subdivide(&self, level: u32, out: &mut Vec<Point<f32>>) {
    if level == 0 {
      out.extend_from_slice(&self.points[1..]);
      return;
    }

    let mut halves = self.chop();
    let start_y = self.points[0].y;
    let end_y = self.points[2].y;

    if between(start_y, self.points[1].y, end_y) {
      let middle_y = halves[0].points[2].y;

      if !between(start_y, middle_y, end_y) {
        let closer = if (middle_y - start_y).abs() < (middle_y - end_y).abs() {
          start_y
        } else {
          end_y
        };

        halves[0].points[2].y = closer;
        halves[1].points[0].y = closer;
      }

      if !between(start_y, halves[0].points[1].y, halves[0].points[2].y) {
        halves[0].points[1].y = start_y;
      }

      if !between(halves[1].points[0].y, halves[1].points[1].y, end_y) {
        halves[1].points[1].y = end_y;
      }
    }

    halves[0].subdivide(level - 1, out);
    halves[1].subdivide(level - 1, out);
  }

  /// `SkAutoConicToQuads::computeQuads`: the conic as quads within `tolerance`, their points after
  /// the start, two per quad.
  pub(crate) fn to_quads(self, tolerance: f32) -> Vec<Point<f32>> {
    let mut pow2 = self.quad_pow2(tolerance);
    let mut out = Vec::with_capacity(2 << pow2);

    if pow2 == MAX_CONIC_TO_QUAD_POW2 {
      let halves = self.chop();

      if equals_within_tolerance(halves[0].points[1], halves[0].points[2])
        && equals_within_tolerance(halves[1].points[0], halves[1].points[1])
      {
        let corner = halves[0].points[1];

        out.extend([corner, corner, corner, halves[1].points[2]]);
        pow2 = 1;
      }
    }

    if out.is_empty() {
      self.subdivide(pow2, &mut out);
    }

    if !out.iter().all(|point| point.is_finite()) {
      let count = out.len();

      out[..count - 1].fill(self.points[1]);
    }

    out
  }

  /// `SkConic::BuildUnitArc`: the conics of the unit arc from `start` to `stop`, clockwise unless
  /// `ccw`, mapped by scaling by `radius` and moving to `center`.
  pub(crate) fn unit_arc(
    start: Point<f32>,
    stop: Point<f32>,
    ccw: bool,
    radius: f32,
    center: Point<f32>,
  ) -> Vec<Self> {
    let x = start.x * stop.x + start.y * stop.y;
    let mut y = start.x * stop.y - start.y * stop.x;

    if y.abs() <= NEARLY_ZERO && x > 0.0 && ((y >= 0.0 && !ccw) || (y <= 0.0 && ccw)) {
      return Vec::new();
    }

    if ccw {
      y = -y;
    }

    let quadrant = if y == 0.0 {
      2
    } else if x == 0.0 {
      if y > 0.0 { 1 } else { 3 }
    } else {
      usize::from(y < 0.0) * 2 + usize::from((x < 0.0) != (y < 0.0))
    };
    let quadrant_points = [
      Point::new(1.0, 0.0),
      Point::new(1.0, 1.0),
      Point::new(0.0, 1.0),
      Point::new(-1.0, 1.0),
      Point::new(-1.0, 0.0),
      Point::new(-1.0, -1.0),
      Point::new(0.0, -1.0),
      Point::new(1.0, -1.0),
    ];
    let mut conics: Vec<Self> = (0..quadrant)
      .map(|index| Self {
        points: [
          quadrant_points[index * 2],
          quadrant_points[index * 2 + 1],
          quadrant_points[index * 2 + 2],
        ],
        weight: ROOT_2_OVER_2,
      })
      .collect();
    let last = quadrant_points[quadrant * 2];
    let dot = last.x * x + last.y * y;

    if dot.is_nan() {
      return Vec::new();
    }

    if dot < 1.0 {
      let cos_half = ((1.0 + dot) / 2.0).sqrt();
      let off_curve = with_length(last.x + x, last.y + y, 1.0 / cos_half).unwrap_or_default();

      if can_normalize(last.x - off_curve.x, last.y - off_curve.y) {
        conics.push(Self {
          points: [last, off_curve, Point::new(x, y)],
          weight: cos_half,
        });
      }
    }

    // `setSinCos(start.y, start.x)`, `preScale(1, -1)` when counter-clockwise, then the scale by
    // `radius` and move to `center` concatenated after it.
    let flip = if ccw { -1.0 } else { 1.0 };
    let matrix = Affine {
      a: radius * start.x,
      b: radius * start.y,
      c: radius * (-start.y * flip),
      d: radius * (start.x * flip),
      x: center.x,
      y: center.y,
    };

    for conic in &mut conics {
      conic.points = conic.points.map(|point| {
        let (x, y) = matrix.transform_point(point.x, point.y);

        Point::new(x, y)
      });
    }

    conics
  }
}

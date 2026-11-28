//! Skia's stroker, `SkStroke` and `SkPathStroker` in `src/core/SkStroke.cpp` with the caps and
//! joins of `src/core/SkStrokerPriv.cpp`: the outline a stroke of a path fills. Approximate: a
//! closed rect strokes as any other path, where Skia's `strokeRect` builds its outline directly.

use std::cmp::Ordering;

use super::{
  Path, PathBuilder, PathVerb, Point,
  conic::{CONIC_TOLERANCE, Conic, NEARLY_ZERO, ROOT_2_OVER_2, can_normalize, with_length},
  find_unit_quad_roots,
};
use crate::style::{Affine, LineCap, LineJoin};

/// The stroke of a path: its width, caps, joins, and miter limit.
#[derive(Clone, PartialEq, Debug)]
pub struct Stroke {
  /// The full width.
  pub width: f32,
  /// How far a miter join may reach, in widths.
  pub miter_limit: f32,
  /// How open ends end.
  pub line_cap: LineCap,
  /// How corners join.
  pub line_join: LineJoin,
}

impl Default for Stroke {
  fn default() -> Self {
    Self {
      width: 1.0,
      miter_limit: 4.0,
      line_cap: LineCap::Butt,
      line_join: LineJoin::Miter,
    }
  }
}

/// `SkMatrixPriv::ComputeResScaleForStroking`: how finely a stroke drawn under `transform` is
/// approximated, the longer of its columns.
pub fn resolution_scale(transform: Affine) -> f32 {
  // `SkPoint::Length`, through doubles only where the floats overflow.
  let length = |x: f32, y: f32| {
    let squared = x * x + y * y;

    if squared.is_finite() {
      squared.sqrt()
    } else {
      (f64::from(x) * f64::from(x) + f64::from(y) * f64::from(y)).sqrt() as f32
    }
  };
  let (sx, sy) = (
    length(transform.a, transform.b),
    length(transform.c, transform.d),
  );

  if sx.is_finite() && sy.is_finite() {
    let scale = sx.max(sy);

    if scale > 0.0 {
      return scale;
    }
  }

  1.0
}

impl Path {
  /// `SkStroke::strokePath`: the outline `stroke` draws around the path, approximated to
  /// `resolution_scale`. None where the stroke has no width or draws nothing.
  pub fn stroke(&self, stroke: &Stroke, resolution_scale: f32) -> Option<Path> {
    let radius = stroke.width / 2.0;

    // Also refuses a NaN width.
    if radius.partial_cmp(&0.0) != Some(Ordering::Greater) {
      return None;
    }

    let mut stroker = Stroker::new(self, radius, stroke, resolution_scale);
    let mut records = Records::new(self);
    let mut last_segment = PathVerb::Move;

    while let Some(record) = records.next() {
      match record {
        Record::Move(point) => stroker.move_to(point),
        Record::Line(_, to) => {
          stroker.line_to(to, Some(&records));
          last_segment = PathVerb::Line;
        }
        Record::Quad(_, control, to) => {
          stroker.quad_to(control, to);
          last_segment = PathVerb::Quad;
        }
        Record::Cubic(_, first, second, to) => {
          stroker.cubic_to(first, second, to);
          last_segment = PathVerb::Cubic;
        }
        Record::Close => {
          if stroke.line_cap != LineCap::Butt {
            // A move and a close, or zero-length segments and a close, draw as a zero-length line,
            // which has square and round caps.
            if stroker.segment_count == 0 {
              stroker.line_to(stroker.first_point, None);
              last_segment = PathVerb::Line;
              continue;
            }

            if stroker.is_current_contour_empty() {
              last_segment = PathVerb::Line;
              continue;
            }
          }

          stroker.finish_contour(true, last_segment == PathVerb::Line);
        }
      }
    }

    stroker.finish_contour(false, last_segment == PathVerb::Line);
    stroker.outer.finish()
  }
}

/// A segment as `SkPath::Iter` walks it without forcing contours closed: a close after a
/// contour that ends away from its start comes first as the line back.
#[derive(Copy, Clone)]
enum Record {
  Move(Point),
  Line(Point, Point),
  Quad(Point, Point, Point),
  Cubic(Point, Point, Point, Point),
  Close,
}

/// `SkPath::Iter` over a path.
#[derive(Clone)]
struct Records<'a> {
  path: &'a Path,
  verb: usize,
  point: usize,
  move_to: Point,
  last: Point,
}

impl<'a> Records<'a> {
  fn new(path: &'a Path) -> Self {
    Self {
      path,
      verb: 0,
      point: 0,
      move_to: Point::zero(),
      last: Point::zero(),
    }
  }

  fn take(&mut self, count: usize) -> &'a [Point] {
    let points = &self.path.points()[self.point..self.point + count];

    self.point += count;
    points
  }
}

impl Iterator for Records<'_> {
  type Item = Record;

  fn next(&mut self) -> Option<Record> {
    let verb = *self.path.verbs().get(self.verb)?;

    self.verb += 1;

    Some(match verb {
      PathVerb::Move => {
        let point = self.take(1)[0];

        (self.move_to, self.last) = (point, point);
        Record::Move(point)
      }
      PathVerb::Line => {
        let to = self.take(1)[0];
        let from = std::mem::replace(&mut self.last, to);

        Record::Line(from, to)
      }
      PathVerb::Quad => {
        let points = self.take(2);
        let from = std::mem::replace(&mut self.last, points[1]);

        Record::Quad(from, points[0], points[1])
      }
      PathVerb::Cubic => {
        let points = self.take(3);
        let from = std::mem::replace(&mut self.last, points[2]);

        Record::Cubic(from, points[0], points[1], points[2])
      }
      PathVerb::Close => {
        let (last, start) = (self.last, self.move_to);

        if last != start && !last.x.is_nan() && !last.y.is_nan() {
          // `autoClose`: the line back, with the close itself still to come.
          self.verb -= 1;
          self.last = start;
          Record::Line(last, start)
        } else {
          self.last = start;
          Record::Close
        }
      }
    })
  }
}

/// `has_valid_tangent`: whether a segment with a direction comes before the contour ends.
fn has_valid_tangent(records: &Records) -> bool {
  for record in records.clone() {
    match record {
      Record::Move(_) | Record::Close => return false,
      Record::Line(from, to) if from == to => {}
      Record::Quad(from, control, to) if from == control && from == to => {}
      Record::Cubic(from, first, second, to) if from == first && from == second && from == to => {}
      _ => return true,
    }
  }

  false
}

fn sub(a: Point, b: Point) -> Point {
  Point::from_xy(a.x - b.x, a.y - b.y)
}

fn add(a: Point, b: Point) -> Point {
  Point::from_xy(a.x + b.x, a.y + b.y)
}

fn scale(a: Point, factor: f32) -> Point {
  Point::from_xy(a.x * factor, a.y * factor)
}

fn negate(a: Point) -> Point {
  Point::from_xy(-a.x, -a.y)
}

fn dot(a: Point, b: Point) -> f32 {
  a.x * b.x + a.y * b.y
}

fn cross(a: Point, b: Point) -> f32 {
  a.x * b.y - a.y * b.x
}

fn length_squared(a: Point) -> f32 {
  a.x * a.x + a.y * a.y
}

fn distance_squared(a: Point, b: Point) -> f32 {
  length_squared(sub(a, b))
}

fn degenerate_vector(v: Point) -> bool {
  !can_normalize(v.x, v.y)
}

/// `SkScalarNearlyZero` with a tolerance.
fn nearly_zero(value: f32, tolerance: f32) -> bool {
  value.abs() <= tolerance
}

/// `set_normal_unitnormal`: the left-turned unit vector along `vector` and it scaled by `radius`.
fn normals(vector: Point, radius: f32) -> Option<(Point, Point)> {
  let unit = with_length(vector.x, vector.y, 1.0)?;
  // `SkPointPriv::RotateCCW`.
  let unit = Point::from_xy(unit.y, -unit.x);

  Some((scale(unit, radius), unit))
}

/// `SkQuadConstruct`: a quad stroke being fitted to a span of the original curve.
#[derive(Copy, Clone, Default)]
struct QuadConstruct {
  quad: [Point; 3],
  tangent_start: Point,
  tangent_end: Point,
  start_t: f32,
  mid_t: f32,
  end_t: f32,
  start_set: bool,
  end_set: bool,
  opposite_tangents: bool,
}

impl QuadConstruct {
  fn init(&mut self, start: f32, end: f32) -> bool {
    self.start_t = start;
    self.mid_t = (start + end) * 0.5;
    self.end_t = end;
    self.start_set = false;
    self.end_set = false;
    self.start_t < self.mid_t && self.mid_t < self.end_t
  }

  fn init_with_start(parent: &Self) -> Option<Self> {
    let mut half = Self::default();

    half.init(parent.start_t, parent.mid_t).then(|| {
      half.quad[0] = parent.quad[0];
      half.tangent_start = parent.tangent_start;
      half.start_set = true;
      half
    })
  }

  fn init_with_end(parent: &Self) -> Option<Self> {
    let mut half = Self::default();

    half.init(parent.mid_t, parent.end_t).then(|| {
      half.quad[2] = parent.quad[2];
      half.tangent_end = parent.tangent_end;
      half.end_set = true;
      half
    })
  }
}

#[derive(Copy, Clone, PartialEq)]
enum StrokeType {
  Outer,
  Inner,
}

#[derive(Copy, Clone, PartialEq)]
enum ResultType {
  Split,
  Degenerate,
  Quad,
}

#[derive(Copy, Clone, PartialEq, PartialOrd)]
enum Reduction {
  Point,
  Line,
  Quad,
  Degenerate,
  Degenerate2,
  Degenerate3,
}

#[derive(Copy, Clone, PartialEq)]
enum RayType {
  ControlPoint,
  Result,
}

/// `kRecursiveLimits` for the tangent search, cubics, and quads.
const TANGENT_RECURSIVE_LIMIT: u32 = 5 * 3;
const CUBIC_RECURSIVE_LIMIT: u32 = 24;
const QUAD_RECURSIVE_LIMIT: u32 = 16;

/// `kOneOverSqrt2` in `SkStrokerPriv.cpp`.
const ONE_OVER_SQRT_2: f32 = 0.707_106_77;

/// `SkPathStroker`.
struct Stroker {
  radius: f32,
  inv_miter_limit: f32,
  res_scale: f32,
  inv_res_scale: f32,
  inv_res_scale_squared: f32,
  /// How finely caps, joins, and cusps split their conics, a quarter of a device pixel.
  conic_tolerance: f32,
  first_normal: Point,
  prev_normal: Point,
  first_unit_normal: Point,
  prev_unit_normal: Point,
  first_point: Point,
  prev_point: Point,
  first_outer_point: Point,
  first_outer_point_index_in_contour: usize,
  segment_count: i32,
  prev_is_line: bool,
  cap: LineCap,
  join: LineJoin,
  inner: PathBuilder,
  outer: PathBuilder,
  cusper: PathBuilder,
  stroke_type: StrokeType,
  recursion_depth: u32,
  found_tangents: bool,
  join_completed: bool,
}

impl Stroker {
  fn new(source: &Path, radius: f32, stroke: &Stroke, res_scale: f32) -> Self {
    let mut join = stroke.line_join;
    let mut inv_miter_limit = 0.0;

    if join == LineJoin::Miter {
      if stroke.miter_limit <= 1.0 {
        join = LineJoin::Bevel;
      } else {
        inv_miter_limit = 1.0 / stroke.miter_limit;
      }
    }

    let inv_res_scale = 1.0 / (res_scale * 4.0);
    let mut outer = PathBuilder::new();
    let mut inner = PathBuilder::new();

    outer.points.reserve(source.points().len() * 3);
    inner.points.reserve(source.points().len());

    Self {
      radius,
      inv_miter_limit,
      res_scale,
      inv_res_scale,
      inv_res_scale_squared: inv_res_scale * inv_res_scale,
      conic_tolerance: CONIC_TOLERANCE / res_scale,
      first_normal: Point::zero(),
      prev_normal: Point::zero(),
      first_unit_normal: Point::zero(),
      prev_unit_normal: Point::zero(),
      first_point: Point::zero(),
      prev_point: Point::zero(),
      first_outer_point: Point::zero(),
      first_outer_point_index_in_contour: 0,
      segment_count: -1,
      prev_is_line: false,
      cap: stroke.line_cap,
      join,
      inner,
      outer,
      cusper: PathBuilder::new(),
      stroke_type: StrokeType::Outer,
      recursion_depth: 0,
      found_tangents: false,
      join_completed: false,
    }
  }

  fn is_current_contour_empty(&self) -> bool {
    is_zero_length_since_point(self.inner.points(), 0)
      && is_zero_length_since_point(self.outer.points(), self.first_outer_point_index_in_contour)
  }

  fn move_to(&mut self, point: Point) {
    if self.segment_count > 0 {
      self.finish_contour(false, false);
    }

    self.segment_count = 0;
    self.first_point = point;
    self.prev_point = point;
    self.join_completed = false;
  }

  fn pre_join_to(&mut self, point: Point, is_line: bool) -> Option<(Point, Point)> {
    let (normal, unit_normal) = match normals(
      scale(sub(point, self.prev_point), self.res_scale),
      self.radius,
    ) {
      Some(normals) => normals,
      // Square and round caps draw even a zero-length segment, upright.
      None if self.cap != LineCap::Butt => {
        (Point::from_xy(self.radius, 0.0), Point::from_xy(1.0, 0.0))
      }
      None => return None,
    };

    if self.segment_count == 0 {
      self.first_normal = normal;
      self.first_unit_normal = unit_normal;
      self.first_outer_point = add(self.prev_point, normal);
      self
        .outer
        .move_to(self.first_outer_point.x, self.first_outer_point.y);

      let inner = sub(self.prev_point, normal);

      self.inner.move_to(inner.x, inner.y);
    } else {
      self.join_to(
        self.join,
        self.prev_unit_normal,
        self.prev_point,
        unit_normal,
        self.prev_is_line,
        is_line,
      );
    }

    self.prev_is_line = is_line;
    Some((normal, unit_normal))
  }

  fn post_join_to(&mut self, point: Point, normal: Point, unit_normal: Point) {
    self.join_completed = true;
    self.prev_point = point;
    self.prev_unit_normal = unit_normal;
    self.prev_normal = normal;
    self.segment_count += 1;
  }

  fn finish_contour(&mut self, close: bool, is_line: bool) {
    if self.segment_count > 0 {
      if close {
        self.join_to(
          self.join,
          self.prev_unit_normal,
          self.prev_point,
          self.first_unit_normal,
          self.prev_is_line,
          is_line,
        );
        self.outer.close();

        if let Some(last) = self.inner.last_point() {
          self.outer.move_to(last.x, last.y);
          self.outer.push_reversed(&self.inner);
          self.outer.close();
        }
      } else if let Some(last) = self.inner.last_point() {
        self.cap_to(self.prev_point, self.prev_normal, last, is_line);
        self.outer.push_reversed(&self.inner);
        self.cap_to(
          self.first_point,
          negate(self.first_normal),
          self.first_outer_point,
          self.prev_is_line,
        );
        self.outer.close();
      }

      if !self.cusper.is_empty() {
        let cusper = std::mem::take(&mut self.cusper);

        self.outer.push_builder(&cusper);
      }
    }

    self.inner.clear();
    self.segment_count = -1;
    self.first_outer_point_index_in_contour = self.outer.points().len();
  }

  fn line_to_both(&mut self, point: Point, normal: Point) {
    let (outer, inner) = (add(point, normal), sub(point, normal));

    self.outer.line_to(outer.x, outer.y);
    self.inner.line_to(inner.x, inner.y);
  }

  fn line_to(&mut self, point: Point, records: Option<&Records>) {
    let tolerance = NEARLY_ZERO * self.inv_res_scale;
    let teeny = nearly_zero(self.prev_point.x - point.x, tolerance)
      && nearly_zero(self.prev_point.y - point.y, tolerance);

    if self.cap == LineCap::Butt && teeny {
      return;
    }

    if teeny && (self.join_completed || records.is_some_and(has_valid_tangent)) {
      return;
    }

    if let Some((normal, unit_normal)) = self.pre_join_to(point, true) {
      self.line_to_both(point, normal);
      self.post_join_to(point, normal, unit_normal);
    }
  }

  /// Draws `steps`, lines through the reduced points of a degenerate curve, joined round.
  fn degenerate_lines(&mut self, steps: &[Point]) {
    let join = std::mem::replace(&mut self.join, LineJoin::Round);

    for &step in steps {
      self.line_to(step, None);
    }

    self.join = join;
  }

  fn quad_to(&mut self, control: Point, to: Point) {
    let quad = [self.prev_point, control, to];
    let (reduction, reduced) = check_quad_linear(&quad);

    match reduction {
      Reduction::Point | Reduction::Line => {
        self.line_to(to, None);
        return;
      }
      Reduction::Degenerate => {
        self.line_to(reduced, None);
        self.degenerate_lines(&[to]);
        return;
      }
      _ => {}
    }

    let Some((normal_ab, unit_ab)) = self.pre_join_to(control, false) else {
      self.line_to(to, None);
      return;
    };
    let mut quad_points = QuadConstruct::default();

    self.init(StrokeType::Outer, &mut quad_points, 0.0, 1.0);
    self.quad_stroke(&quad, &mut quad_points);
    self.init(StrokeType::Inner, &mut quad_points, 0.0, 1.0);
    self.quad_stroke(&quad, &mut quad_points);

    // `setQuadEndNormal`.
    let (normal_bc, unit_bc) = normals(scale(sub(quad[2], quad[1]), self.res_scale), self.radius)
      .unwrap_or((normal_ab, unit_ab));

    self.post_join_to(to, normal_bc, unit_bc);
  }

  fn cubic_to(&mut self, first: Point, second: Point, to: Point) {
    let cubic = [self.prev_point, first, second, to];
    let (reduction, reduced, tangent_point) = check_cubic_linear(&cubic);

    match reduction {
      Reduction::Point | Reduction::Line => {
        self.line_to(to, None);
        return;
      }
      Reduction::Degenerate | Reduction::Degenerate2 | Reduction::Degenerate3 => {
        let count = match reduction {
          Reduction::Degenerate => 1,
          Reduction::Degenerate2 => 2,
          _ => 3,
        };

        self.line_to(reduced[0], None);

        let mut steps = reduced[1..count].to_vec();

        steps.push(to);
        self.degenerate_lines(&steps);
        return;
      }
      Reduction::Quad => {}
    }

    let Some((normal_ab, unit_ab)) = self.pre_join_to(tangent_point, false) else {
      self.line_to(to, None);
      return;
    };
    let (inflections, count) = find_cubic_inflections(&cubic);
    let mut last_t = 0.0;

    for next_t in inflections[..count].iter().copied().chain([1.0]) {
      let mut quad_points = QuadConstruct::default();

      self.init(StrokeType::Outer, &mut quad_points, last_t, next_t);
      self.cubic_stroke(&cubic, &mut quad_points);
      self.init(StrokeType::Inner, &mut quad_points, last_t, next_t);
      self.cubic_stroke(&cubic, &mut quad_points);
      last_t = next_t;
    }

    if let Some(cusp) = find_cubic_cusp(&cubic) {
      let at = eval_cubic(&cubic, cusp);

      self
        .cusper
        .push_circle(at, self.radius, self.conic_tolerance);
    }

    // `setCubicEndNormal`.
    let (normal_cd, unit_cd) =
      cubic_end_normals(&cubic, self.radius).unwrap_or((normal_ab, unit_ab));

    self.post_join_to(to, normal_cd, unit_cd);
  }

  fn init(
    &mut self,
    stroke_type: StrokeType,
    quad_points: &mut QuadConstruct,
    start: f32,
    end: f32,
  ) {
    self.stroke_type = stroke_type;
    self.found_tangents = false;
    self.recursion_depth = 0;
    quad_points.init(start, end);
  }

  fn sink(&mut self) -> &mut PathBuilder {
    match self.stroke_type {
      StrokeType::Outer => &mut self.outer,
      StrokeType::Inner => &mut self.inner,
    }
  }

  fn add_degenerate_line(&mut self, quad_points: &QuadConstruct) {
    let end = quad_points.quad[2];

    self.sink().line_to(end.x, end.y);
  }

  fn add_quad(&mut self, quad_points: &QuadConstruct) {
    let [_, control, end] = quad_points.quad;

    self.sink().quad_to(control.x, control.y, end.x, end.y);
  }

  /// `setRayPts`: the point `radius` off `point` across `direction`, on this stroke's side.
  fn ray_points(&self, point: Point, direction: Point) -> (Point, Point) {
    let direction = with_length(direction.x, direction.y, self.radius)
      .unwrap_or(Point::from_xy(self.radius, 0.0));
    let flip = match self.stroke_type {
      StrokeType::Outer => 1.0,
      StrokeType::Inner => -1.0,
    };

    (
      Point::from_xy(point.x + flip * direction.y, point.y - flip * direction.x),
      direction,
    )
  }

  /// `quadPerpRay`: the curve point at `t`, the stroke point beside it, and the tangent there.
  fn quad_perp_ray(&self, quad: &[Point; 3], t: f32) -> (Point, Point, Point) {
    let point = super::eval_quad_at(quad, t);
    let mut direction = eval_quad_tangent(quad, t);

    if direction == Point::zero() {
      direction = sub(quad[2], quad[0]);
    }

    let (on, tangent) = self.ray_points(point, direction);

    (point, on, tangent)
  }

  /// `cubicPerpRay`.
  fn cubic_perp_ray(&self, cubic: &[Point; 4], t: f32) -> (Point, Point, Point) {
    let point = eval_cubic(cubic, t);
    let mut direction = eval_cubic_tangent(cubic, t);

    if direction == Point::zero() {
      let mut points = *cubic;

      if nearly_zero(t, NEARLY_ZERO) {
        direction = sub(cubic[2], cubic[0]);
      } else if nearly_zero(1.0 - t, NEARLY_ZERO) {
        direction = sub(cubic[3], cubic[1]);
      } else {
        let chopped = super::chop_cubic_at(cubic, t);

        direction = sub(chopped[3], chopped[2]);

        if direction == Point::zero() {
          direction = sub(chopped[3], chopped[1]);
          points = [chopped[0], chopped[1], chopped[2], chopped[3]];
        }
      }

      if direction == Point::zero() {
        direction = sub(points[3], points[0]);
      }
    }

    let (on, tangent) = self.ray_points(point, direction);

    (point, on, tangent)
  }

  fn cubic_quad_ends(&self, cubic: &[Point; 4], quad_points: &mut QuadConstruct) {
    if !quad_points.start_set {
      let (_, on, tangent) = self.cubic_perp_ray(cubic, quad_points.start_t);

      quad_points.quad[0] = on;
      quad_points.tangent_start = tangent;
      quad_points.start_set = true;
    }

    if !quad_points.end_set {
      let (_, on, tangent) = self.cubic_perp_ray(cubic, quad_points.end_t);

      quad_points.quad[2] = on;
      quad_points.tangent_end = tangent;
      quad_points.end_set = true;
    }
  }

  /// `intersectRay`: where the tangents at the quad's ends meet, set as its control point when
  /// `ray_type` asks for it.
  fn intersect_ray(&self, quad_points: &mut QuadConstruct, ray_type: RayType) -> ResultType {
    let start = quad_points.quad[0];
    let end = quad_points.quad[2];
    let a = quad_points.tangent_start;
    let b = quad_points.tangent_end;
    let denom = cross(a, b);

    if denom == 0.0 || !denom.is_finite() {
      quad_points.opposite_tangents = dot(a, b) < 0.0;
      return ResultType::Degenerate;
    }

    quad_points.opposite_tangents = false;

    let ab0 = sub(start, end);
    let mut numer_a = cross(b, ab0);
    let numer_b = cross(a, ab0);

    if (numer_a >= 0.0) == (numer_b >= 0.0) {
      let dist1 = pt_to_tangent_line(start, end, quad_points.tangent_end);
      let dist2 = pt_to_tangent_line(end, start, quad_points.tangent_start);

      return if dist1.max(dist2) <= self.inv_res_scale_squared {
        ResultType::Degenerate
      } else {
        ResultType::Split
      };
    }

    numer_a /= denom;

    if numer_a > numer_a - 1.0 {
      if ray_type == RayType::ControlPoint {
        quad_points.quad[1] = add(start, scale(quad_points.tangent_start, numer_a));
      }

      return ResultType::Quad;
    }

    quad_points.opposite_tangents = dot(a, b) < 0.0;
    ResultType::Degenerate
  }

  fn pt_in_quad_bounds(&self, quad: &[Point; 3], point: Point) -> bool {
    let min_x = quad[0].x.min(quad[1].x).min(quad[2].x);
    let max_x = quad[0].x.max(quad[1].x).max(quad[2].x);
    let min_y = quad[0].y.min(quad[1].y).min(quad[2].y);
    let max_y = quad[0].y.max(quad[1].y).max(quad[2].y);

    !(point.x + self.inv_res_scale < min_x
      || point.x - self.inv_res_scale > max_x
      || point.y + self.inv_res_scale < min_y
      || point.y - self.inv_res_scale > max_y)
  }

  /// `strokeCloseEnough`: whether the quad stroke stays within the resolution of the ray from the
  /// curve's middle.
  fn stroke_close_enough(
    &self,
    stroke: &[Point; 3],
    ray: [Point; 2],
    quad_points: &QuadConstruct,
  ) -> ResultType {
    let stroke_mid = super::eval_quad_at(stroke, 0.5);

    if points_within_dist(ray[0], stroke_mid, self.inv_res_scale) {
      return if sharp_angle(&quad_points.quad) {
        ResultType::Split
      } else {
        ResultType::Quad
      };
    }

    if !self.pt_in_quad_bounds(stroke, ray[0]) {
      return ResultType::Split;
    }

    let (roots, count) = intersect_quad_ray(ray, stroke);

    if count != 1 {
      return ResultType::Split;
    }

    let quad_point = super::eval_quad_at(stroke, roots[0]);
    let error = self.inv_res_scale * (1.0 - (roots[0] - 0.5).abs() * 2.0);

    if points_within_dist(ray[0], quad_point, error) {
      return if sharp_angle(&quad_points.quad) {
        ResultType::Split
      } else {
        ResultType::Quad
      };
    }

    ResultType::Split
  }

  fn compare_quad_cubic(&self, cubic: &[Point; 4], quad_points: &mut QuadConstruct) -> ResultType {
    self.cubic_quad_ends(cubic, quad_points);

    let result = self.intersect_ray(quad_points, RayType::ControlPoint);

    if result != ResultType::Quad {
      return result;
    }

    let (point, on, _) = self.cubic_perp_ray(cubic, quad_points.mid_t);

    self.stroke_close_enough(&quad_points.quad, [on, point], quad_points)
  }

  fn compare_quad_quad(&self, quad: &[Point; 3], quad_points: &mut QuadConstruct) -> ResultType {
    if !quad_points.start_set {
      let (_, on, tangent) = self.quad_perp_ray(quad, quad_points.start_t);

      quad_points.quad[0] = on;
      quad_points.tangent_start = tangent;
      quad_points.start_set = true;
    }

    if !quad_points.end_set {
      let (_, on, tangent) = self.quad_perp_ray(quad, quad_points.end_t);

      quad_points.quad[2] = on;
      quad_points.tangent_end = tangent;
      quad_points.end_set = true;
    }

    let result = self.intersect_ray(quad_points, RayType::ControlPoint);

    if result != ResultType::Quad {
      return result;
    }

    let (point, on, _) = self.quad_perp_ray(quad, quad_points.mid_t);

    self.stroke_close_enough(&quad_points.quad, [on, point], quad_points)
  }

  fn cubic_mid_on_line(&self, cubic: &[Point; 4], quad_points: &QuadConstruct) -> bool {
    let (_, mid, _) = self.cubic_perp_ray(cubic, quad_points.mid_t);

    pt_to_line(mid, quad_points.quad[0], quad_points.quad[2]) < self.inv_res_scale_squared
  }

  /// Splits the span of `quad_points` in two and strokes each with `stroke_half`, false where one
  /// gives up. A span that cannot split or recurses past `limit` gets a line.
  fn split(
    &mut self,
    quad_points: &QuadConstruct,
    limit: u32,
    stroke_half: &mut dyn FnMut(&mut Self, &mut QuadConstruct) -> bool,
  ) -> bool {
    self.recursion_depth += 1;

    if self.recursion_depth > limit {
      self.add_degenerate_line(quad_points);
      self.recursion_depth -= 1;
      return true;
    }

    let Some(mut half) = QuadConstruct::init_with_start(quad_points) else {
      self.add_degenerate_line(quad_points);
      self.recursion_depth -= 1;
      return true;
    };

    if !stroke_half(self, &mut half) {
      return false;
    }

    let Some(mut half) = QuadConstruct::init_with_end(quad_points) else {
      self.add_degenerate_line(quad_points);
      self.recursion_depth -= 1;
      return true;
    };

    if !stroke_half(self, &mut half) {
      return false;
    }

    self.recursion_depth -= 1;
    true
  }

  fn cubic_stroke(&mut self, cubic: &[Point; 4], quad_points: &mut QuadConstruct) -> bool {
    if !self.found_tangents {
      self.cubic_quad_ends(cubic, quad_points);

      let result = self.intersect_ray(quad_points, RayType::Result);

      if result != ResultType::Quad {
        let near = points_within_dist(quad_points.quad[0], quad_points.quad[2], self.inv_res_scale);

        if (result == ResultType::Degenerate || near) && self.cubic_mid_on_line(cubic, quad_points)
        {
          self.add_degenerate_line(quad_points);
          return true;
        }
      } else {
        self.found_tangents = true;
      }
    }

    if self.found_tangents {
      match self.compare_quad_cubic(cubic, quad_points) {
        ResultType::Quad => {
          self.add_quad(quad_points);
          return true;
        }
        ResultType::Degenerate if !quad_points.opposite_tangents => {
          self.add_degenerate_line(quad_points);
          return true;
        }
        _ => {}
      }
    }

    if !quad_points.quad[2].is_finite() {
      return false;
    }

    let limit = if self.found_tangents {
      CUBIC_RECURSIVE_LIMIT
    } else {
      TANGENT_RECURSIVE_LIMIT
    };

    self.split(quad_points, limit, &mut |stroker, half| {
      stroker.cubic_stroke(cubic, half)
    })
  }

  fn quad_stroke(&mut self, quad: &[Point; 3], quad_points: &mut QuadConstruct) -> bool {
    match self.compare_quad_quad(quad, quad_points) {
      ResultType::Quad => {
        self.add_quad(quad_points);
        return true;
      }
      ResultType::Degenerate => {
        self.add_degenerate_line(quad_points);
        return true;
      }
      ResultType::Split => {}
    }

    if !quad_points.quad[2].is_finite() {
      return false;
    }

    self.split(quad_points, QUAD_RECURSIVE_LIMIT, &mut |stroker, half| {
      stroker.quad_stroke(quad, half)
    })
  }

  /// `SkStrokerPriv`'s cappers: ends the contour at `pivot` from its outer side `normal` to `stop`.
  fn cap_to(&mut self, pivot: Point, normal: Point, stop: Point, extend_last_point: bool) {
    match self.cap {
      LineCap::Butt => self.outer.line_to(stop.x, stop.y),
      LineCap::Round => {
        // `SkPointPriv::RotateCW`.
        let parallel = Point::from_xy(-normal.y, normal.x);
        let center = add(pivot, parallel);
        let (first, second) = (add(center, normal), sub(center, normal));

        let tolerance = self.conic_tolerance;

        self.outer.conic_to_within(
          first.x,
          first.y,
          center.x,
          center.y,
          ROOT_2_OVER_2,
          tolerance,
        );
        self
          .outer
          .conic_to_within(second.x, second.y, stop.x, stop.y, ROOT_2_OVER_2, tolerance);
      }
      LineCap::Square => {
        let parallel = Point::from_xy(-normal.y, normal.x);
        let near = add(add(pivot, normal), parallel);
        let far = add(sub(pivot, normal), parallel);

        if extend_last_point {
          self.outer.set_last_point(near);
          self.outer.line_to(far.x, far.y);
        } else {
          self.outer.line_to(near.x, near.y);
          self.outer.line_to(far.x, far.y);
          self.outer.line_to(stop.x, stop.y);
        }
      }
    }
  }

  /// `SkStrokerPriv`'s joiners: joins the segment before `pivot` to the one after it.
  fn join_to(
    &mut self,
    join: LineJoin,
    before_unit_normal: Point,
    pivot: Point,
    after_unit_normal: Point,
    prev_is_line: bool,
    is_line: bool,
  ) {
    match join {
      LineJoin::Bevel => self.blunt_join(before_unit_normal, pivot, after_unit_normal),
      LineJoin::Round => self.round_join(before_unit_normal, pivot, after_unit_normal),
      LineJoin::Miter => {
        self.miter_join(
          before_unit_normal,
          pivot,
          after_unit_normal,
          prev_is_line,
          is_line,
        );
      }
    }
  }

  /// The outer and inner builders, swapped when the turn is counter-clockwise.
  fn sides(&mut self, swap: bool) -> (&mut PathBuilder, &mut PathBuilder) {
    if swap {
      (&mut self.inner, &mut self.outer)
    } else {
      (&mut self.outer, &mut self.inner)
    }
  }

  /// `BluntJoiner`.
  fn blunt_join(&mut self, before_unit_normal: Point, pivot: Point, after_unit_normal: Point) {
    let mut after = scale(after_unit_normal, self.radius);
    let ccw = !is_clockwise(before_unit_normal, after_unit_normal);

    if ccw {
      after = negate(after);
    }

    let (outer, inner) = self.sides(ccw);

    outer.line_to(pivot.x + after.x, pivot.y + after.y);
    inner.handle_inner_join(pivot, after);
  }

  /// `RoundJoiner`.
  fn round_join(&mut self, before_unit_normal: Point, pivot: Point, after_unit_normal: Point) {
    if angle_type(dot(before_unit_normal, after_unit_normal)) == AngleType::NearlyLine {
      return;
    }

    let ccw = !is_clockwise(before_unit_normal, after_unit_normal);
    let (before, mut after) = if ccw {
      (negate(before_unit_normal), negate(after_unit_normal))
    } else {
      (before_unit_normal, after_unit_normal)
    };
    let radius = self.radius;
    let conics = Conic::unit_arc(before, after, ccw, radius, pivot);

    if conics.is_empty() {
      return;
    }

    after = scale(after, radius);

    let tolerance = self.conic_tolerance;
    let (outer, inner) = self.sides(ccw);

    for conic in conics {
      let [_, control, to] = conic.points;

      outer.conic_to_within(control.x, control.y, to.x, to.y, conic.weight, tolerance);
    }

    inner.handle_inner_join(pivot, after);
  }

  /// `MiterJoiner`.
  fn miter_join(
    &mut self,
    before_unit_normal: Point,
    pivot: Point,
    after_unit_normal: Point,
    prev_is_line: bool,
    mut is_line: bool,
  ) {
    let radius = self.radius;
    let inv_miter_limit = self.inv_miter_limit;
    let dot_product = dot(before_unit_normal, after_unit_normal);
    let angle = angle_type(dot_product);
    let mut before = before_unit_normal;
    let mut after = after_unit_normal;
    let mut ccw = false;

    if angle == AngleType::NearlyLine {
      return;
    }

    let mid = if angle == AngleType::Nearly180 {
      is_line = false;
      None
    } else {
      ccw = !is_clockwise(before, after);

      if ccw {
        before = negate(before);
        after = negate(after);
      }

      if dot_product == 0.0 && inv_miter_limit <= ONE_OVER_SQRT_2 {
        // An upright right angle, the common corner of a stroked rect.
        Some(scale(add(before, after), radius))
      } else {
        let sin_half_angle = ((1.0 + dot_product) / 2.0).sqrt();

        if sin_half_angle < inv_miter_limit {
          is_line = false;
          None
        } else {
          let mut mid = if angle == AngleType::Sharp {
            let mid = Point::from_xy(after.y - before.y, before.x - after.x);

            if ccw { negate(mid) } else { mid }
          } else {
            add(before, after)
          };

          if let Some(scaled) = with_length(mid.x, mid.y, radius / sin_half_angle) {
            mid = scaled;
          } else {
            mid = Point::zero();
          }

          Some(mid)
        }
      }
    };
    let (outer, inner) = self.sides(ccw);

    if let Some(mid) = mid {
      let miter = add(pivot, mid);

      if prev_is_line {
        outer.set_last_point(miter);
      } else {
        outer.line_to(miter.x, miter.y);
      }
    }

    let after = scale(after, radius);

    if !is_line {
      outer.line_to(pivot.x + after.x, pivot.y + after.y);
    }

    inner.handle_inner_join(pivot, after);
  }
}

/// `isZeroLengthSincePoint`.
fn is_zero_length_since_point(points: &[Point], start: usize) -> bool {
  let Some(points) = points.get(start..) else {
    return true;
  };

  points.len() < 2 || points[1..].iter().all(|&point| point == points[0])
}

/// `is_clockwise` in `SkStrokerPriv.cpp`.
fn is_clockwise(before: Point, after: Point) -> bool {
  before.x * after.y > before.y * after.x
}

#[derive(Copy, Clone, PartialEq)]
enum AngleType {
  Nearly180,
  Sharp,
  Shallow,
  NearlyLine,
}

/// `Dot2AngleType`.
fn angle_type(dot: f32) -> AngleType {
  if dot >= 0.0 {
    if nearly_zero(1.0 - dot, NEARLY_ZERO) {
      AngleType::NearlyLine
    } else {
      AngleType::Shallow
    }
  } else if nearly_zero(1.0 + dot, NEARLY_ZERO) {
    AngleType::Nearly180
  } else {
    AngleType::Sharp
  }
}

impl PathBuilder {
  /// `HandleInnerJoin`: goes through the pivot, so a radius wider than the segments does not show
  /// a diagonal across the inner side.
  fn handle_inner_join(&mut self, pivot: Point, after: Point) {
    self.line_to(pivot.x, pivot.y);
    self.line_to(pivot.x - after.x, pivot.y - after.y);
  }

  /// `SkPathBuilder::addCircle`: a clockwise circle of four conics from its right.
  fn push_circle(&mut self, center: Point, radius: f32, tolerance: f32) {
    let (left, top, right, bottom) = (
      center.x - radius,
      center.y - radius,
      center.x + radius,
      center.y + radius,
    );
    let (cx, cy) = (left * 0.5 + right * 0.5, top * 0.5 + bottom * 0.5);

    self.move_to(right, cy);
    self.conic_to_within(right, bottom, cx, bottom, ROOT_2_OVER_2, tolerance);
    self.conic_to_within(left, bottom, left, cy, ROOT_2_OVER_2, tolerance);
    self.conic_to_within(left, top, cx, top, ROOT_2_OVER_2, tolerance);
    self.conic_to_within(right, top, right, cy, ROOT_2_OVER_2, tolerance);
    self.close();
  }
}

/// `setCubicEndNormal`'s normals at the cubic's end, none where its end has no direction.
fn cubic_end_normals(cubic: &[Point; 4], radius: f32) -> Option<(Point, Point)> {
  let mut ab = sub(cubic[1], cubic[0]);
  let mut cd = sub(cubic[3], cubic[2]);
  let mut degenerate_ab = degenerate_vector(ab);
  let mut degenerate_cd = degenerate_vector(cd);

  if degenerate_ab && degenerate_cd {
    return None;
  }

  if degenerate_ab {
    ab = sub(cubic[2], cubic[0]);
    degenerate_ab = degenerate_vector(ab);
  }

  if degenerate_cd {
    cd = sub(cubic[3], cubic[1]);
    degenerate_cd = degenerate_vector(cd);
  }

  if degenerate_ab || degenerate_cd {
    return None;
  }

  normals(cd, radius)
}

fn pt_to_line(point: Point, start: Point, end: Point) -> f32 {
  let dxy = sub(end, start);
  let ab0 = sub(point, start);
  let t = dot(dxy, ab0) / dot(dxy, dxy);

  if (0.0..=1.0).contains(&t) {
    let hit = Point::from_xy(
      start.x * (1.0 - t) + end.x * t,
      start.y * (1.0 - t) + end.y * t,
    );

    distance_squared(hit, point)
  } else {
    distance_squared(point, start)
  }
}

fn pt_to_tangent_line(point: Point, start: Point, tangent: Point) -> f32 {
  let ab0 = sub(point, start);
  let t = dot(tangent, ab0) / dot(tangent, tangent);

  if (0.0..=1.0).contains(&t) {
    distance_squared(add(start, scale(tangent, t)), point)
  } else {
    distance_squared(point, start)
  }
}

fn points_within_dist(near: Point, far: Point, limit: f32) -> bool {
  distance_squared(near, far) <= limit * limit
}

fn sharp_angle(quad: &[Point; 3]) -> bool {
  let mut smaller = sub(quad[1], quad[0]);
  let mut larger = sub(quad[1], quad[2]);
  let smaller_length = length_squared(smaller);
  let mut larger_length = length_squared(larger);

  if smaller_length > larger_length {
    std::mem::swap(&mut smaller, &mut larger);
    larger_length = smaller_length;
  }

  let Some(smaller) = with_length(smaller.x, smaller.y, larger_length) else {
    return false;
  };

  dot(smaller, larger) > 0.0
}

fn intersect_quad_ray(line: [Point; 2], quad: &[Point; 3]) -> ([f32; 2], usize) {
  let vector = sub(line[1], line[0]);
  let r = quad.map(|point| cross(vector, sub(point, line[0])));
  let (mut a, mut b, c) = (r[2], r[1], r[0]);

  a += c - 2.0 * b;
  b -= c;
  find_unit_quad_roots(a, 2.0 * b, c)
}

/// `quad_in_line`: whether the quad's middle point lies close to the line between its outer ones.
fn quad_in_line(quad: &[Point; 3]) -> bool {
  let mut point_max = -1.0;
  let (mut outer1, mut outer2) = (0, 0);

  for index in 0..2 {
    for inner in index + 1..3 {
      let diff = sub(quad[inner], quad[index]);
      let test = diff.x.abs().max(diff.y.abs());

      if point_max < test {
        (outer1, outer2, point_max) = (index, inner, test);
      }
    }
  }

  let mid = outer1 ^ outer2 ^ 3;
  let slop = point_max * point_max * 0.000_005;

  pt_to_line(quad[mid], quad[outer1], quad[outer2]) <= slop
}

/// `cubic_in_line`.
fn cubic_in_line(cubic: &[Point; 4]) -> bool {
  let mut point_max = -1.0;
  let (mut outer1, mut outer2) = (0, 0);

  for index in 0..3 {
    for inner in index + 1..4 {
      let diff = sub(cubic[inner], cubic[index]);
      let test = diff.x.abs().max(diff.y.abs());

      if point_max < test {
        (outer1, outer2, point_max) = (index, inner, test);
      }
    }
  }

  let mid1 = (1 + (2 >> outer2)) >> outer1;
  let mid2 = outer1 ^ outer2 ^ mid1;
  let slop = point_max * point_max * 0.000_01;

  pt_to_line(cubic[mid1], cubic[outer1], cubic[outer2]) <= slop
    && pt_to_line(cubic[mid2], cubic[outer1], cubic[outer2]) <= slop
}

/// `CheckQuadLinear`: how far the quad reduces, with the point a degenerate one turns at.
fn check_quad_linear(quad: &[Point; 3]) -> (Reduction, Point) {
  let degenerate_ab = degenerate_vector(sub(quad[1], quad[0]));
  let degenerate_bc = degenerate_vector(sub(quad[2], quad[1]));

  if degenerate_ab && degenerate_bc {
    return (Reduction::Point, Point::zero());
  }

  if degenerate_ab || degenerate_bc {
    return (Reduction::Line, Point::zero());
  }

  if !quad_in_line(quad) {
    return (Reduction::Quad, Point::zero());
  }

  let t = find_quad_max_curvature(quad);

  if t == 0.0 || t == 1.0 {
    return (Reduction::Line, Point::zero());
  }

  (Reduction::Degenerate, super::eval_quad_at(quad, t))
}

/// `CheckCubicLinear`: how far the cubic reduces, the points a degenerate one turns at, and the
/// point its start tangent aims at.
fn check_cubic_linear(cubic: &[Point; 4]) -> (Reduction, [Point; 3], Point) {
  let degenerate_ab = degenerate_vector(sub(cubic[1], cubic[0]));
  let degenerate_bc = degenerate_vector(sub(cubic[2], cubic[1]));
  let degenerate_cd = degenerate_vector(sub(cubic[3], cubic[2]));
  let mut reduction = [Point::zero(); 3];

  if degenerate_ab && degenerate_bc && degenerate_cd {
    return (Reduction::Point, reduction, cubic[1]);
  }

  if usize::from(degenerate_ab) + usize::from(degenerate_bc) + usize::from(degenerate_cd) == 2 {
    return (Reduction::Line, reduction, cubic[1]);
  }

  if !cubic_in_line(cubic) {
    let tangent = if degenerate_ab { cubic[2] } else { cubic[1] };

    return (Reduction::Quad, reduction, tangent);
  }

  let (t_values, count) = find_cubic_max_curvature(cubic);
  let mut found = 0;

  for &t in &t_values[..count] {
    if 0.0 >= t || t >= 1.0 {
      continue;
    }

    let point = eval_cubic(cubic, t);

    if point != cubic[0] && point != cubic[3] {
      reduction[found] = point;
      found += 1;
    }
  }

  let kind = match found {
    0 => Reduction::Line,
    1 => Reduction::Degenerate,
    2 => Reduction::Degenerate2,
    _ => Reduction::Degenerate3,
  };

  (kind, reduction, cubic[1])
}

/// `SkEvalQuadTangentAt`.
fn eval_quad_tangent(quad: &[Point; 3], t: f32) -> Point {
  if (t == 0.0 && quad[0] == quad[1]) || (t == 1.0 && quad[1] == quad[2]) {
    return sub(quad[2], quad[0]);
  }

  let b = sub(quad[1], quad[0]);
  let a = sub(sub(quad[2], quad[1]), b);
  let tangent = add(scale(a, t), b);

  add(tangent, tangent)
}

/// `SkEvalCubicAt`'s location.
fn eval_cubic(cubic: &[Point; 4], t: f32) -> Point {
  super::eval_cubic_pos_at(cubic, t)
}

/// `eval_cubic_derivative`.
fn eval_cubic_derivative(cubic: &[Point; 4], t: f32) -> Point {
  let axis = |p0: f32, p1: f32, p2: f32, p3: f32| {
    let a = p3 + 3.0 * (p1 - p2) - p0;
    let inner = p2 - (p1 + p1) + p0;
    let b = inner + inner;
    let c = p1 - p0;

    (a * t + b) * t + c
  };

  Point::from_xy(
    axis(cubic[0].x, cubic[1].x, cubic[2].x, cubic[3].x),
    axis(cubic[0].y, cubic[1].y, cubic[2].y, cubic[3].y),
  )
}

/// `SkEvalCubicAt`'s tangent.
fn eval_cubic_tangent(cubic: &[Point; 4], t: f32) -> Point {
  if (t == 0.0 && cubic[0] == cubic[1]) || (t == 1.0 && cubic[2] == cubic[3]) {
    let tangent = if t == 0.0 {
      sub(cubic[2], cubic[0])
    } else {
      sub(cubic[3], cubic[1])
    };

    if tangent == Point::zero() {
      sub(cubic[3], cubic[0])
    } else {
      tangent
    }
  } else {
    eval_cubic_derivative(cubic, t)
  }
}

/// `SkFindQuadMaxCurvature`.
fn find_quad_max_curvature(quad: &[Point; 3]) -> f32 {
  let ax = quad[1].x - quad[0].x;
  let ay = quad[1].y - quad[0].y;
  let bx = quad[0].x - quad[1].x - quad[1].x + quad[2].x;
  let by = quad[0].y - quad[1].y - quad[1].y + quad[2].y;
  let mut numer = -(ax * bx + ay * by);
  let mut denom = bx * bx + by * by;

  if denom < 0.0 {
    numer = -numer;
    denom = -denom;
  }

  if numer <= 0.0 {
    return 0.0;
  }

  if numer >= denom {
    return 1.0;
  }

  numer / denom
}

/// `SkFindCubicInflections`.
fn find_cubic_inflections(cubic: &[Point; 4]) -> ([f32; 2], usize) {
  let ax = cubic[1].x - cubic[0].x;
  let ay = cubic[1].y - cubic[0].y;
  let bx = cubic[2].x - 2.0 * cubic[1].x + cubic[0].x;
  let by = cubic[2].y - 2.0 * cubic[1].y + cubic[0].y;
  let cx = cubic[3].x + 3.0 * (cubic[1].x - cubic[2].x) - cubic[0].x;
  let cy = cubic[3].y + 3.0 * (cubic[1].y - cubic[2].y) - cubic[0].y;

  find_unit_quad_roots(bx * cy - by * cx, ax * cy - ay * cx, ax * by - ay * bx)
}

/// `formulate_F1DotF2`.
fn f1_dot_f2(p0: f32, p1: f32, p2: f32, p3: f32) -> [f32; 4] {
  let a = p1 - p0;
  let b = p2 - 2.0 * p1 + p0;
  let c = p3 + 3.0 * (p1 - p2) - p0;

  [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

/// `SkTPin`: `value` within `low` to `high`, NaN pinned to `low`.
fn pin(value: f32, low: f32, high: f32) -> f32 {
  let below = if high < value { high } else { value };

  if low < below { below } else { low }
}

/// `solve_cubic_poly`: the roots of the cubic `coeff`, clamped to 0 to 1, ascending and distinct.
fn solve_cubic_poly(coeff: [f32; 4]) -> ([f32; 3], usize) {
  let mut values = [0.0; 3];

  if nearly_zero(coeff[0], NEARLY_ZERO) {
    let (roots, count) = find_unit_quad_roots(coeff[1], coeff[2], coeff[3]);

    values[..2].copy_from_slice(&roots);
    return (values, count);
  }

  let inverse = 1.0 / coeff[0];
  let (a, b, c) = (coeff[1] * inverse, coeff[2] * inverse, coeff[3] * inverse);
  let q = (a * a - b * 3.0) / 9.0;
  let r = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
  let q3 = q * q * q;
  let r2_minus_q3 = r * r - q3;
  let a_third = a / 3.0;

  if r2_minus_q3 < 0.0 {
    let theta = pin(r / q3.sqrt(), -1.0, 1.0).acos();
    let neg_2_root_q = -2.0 * q.sqrt();
    let pi = std::f32::consts::PI;

    values = [
      theta / 3.0,
      (theta + 2.0 * pi) / 3.0,
      (theta - 2.0 * pi) / 3.0,
    ]
    .map(|angle| pin(neg_2_root_q * angle.cos() - a_third, 0.0, 1.0));
    values.sort_by(f32::total_cmp);

    // `collaps_duplicates`.
    let mut count = 1;

    for index in 1..3 {
      if values[index] != values[count - 1] {
        values[count] = values[index];
        count += 1;
      }
    }

    return (values, count);
  }

  // `SkScalarCubeRoot`.
  let mut big_a = (r.abs() + r2_minus_q3.sqrt()).powf(0.333_333_3);

  if r > 0.0 {
    big_a = -big_a;
  }

  if big_a != 0.0 {
    big_a += q / big_a;
  }

  values[0] = pin(big_a - a_third, 0.0, 1.0);
  (values, 1)
}

/// `SkFindCubicMaxCurvature`.
fn find_cubic_max_curvature(cubic: &[Point; 4]) -> ([f32; 3], usize) {
  let x = f1_dot_f2(cubic[0].x, cubic[1].x, cubic[2].x, cubic[3].x);
  let y = f1_dot_f2(cubic[0].y, cubic[1].y, cubic[2].y, cubic[3].y);

  solve_cubic_poly([x[0] + y[0], x[1] + y[1], x[2] + y[2], x[3] + y[3]])
}

/// `on_same_side`.
fn on_same_side(cubic: &[Point; 4], test: usize, line: usize) -> bool {
  let origin = cubic[line];
  let direction = sub(cubic[line + 1], origin);
  let crosses = [0, 1].map(|index| cross(direction, sub(cubic[test + index], origin)));

  crosses[0] * crosses[1] >= 0.0
}

/// `SkFindCubicCusp`: where the cubic doubles back on itself, if it does.
fn find_cubic_cusp(cubic: &[Point; 4]) -> Option<f32> {
  if cubic[0] == cubic[1] || cubic[2] == cubic[3] {
    return None;
  }

  if on_same_side(cubic, 0, 2) || on_same_side(cubic, 2, 0) {
    return None;
  }

  let (values, count) = find_cubic_max_curvature(cubic);
  // `calc_cubic_precision`.
  let precision = (distance_squared(cubic[1], cubic[0])
    + distance_squared(cubic[2], cubic[1])
    + distance_squared(cubic[3], cubic[2]))
    * 1e-8;

  values[..count]
    .iter()
    .copied()
    .find(|&t| 0.0 < t && t < 1.0 && length_squared(eval_cubic_derivative(cubic, t)) < precision)
}

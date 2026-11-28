//! Path to edge-list conversion for the analytic scan converter: Skia's
//! `SkEdgeBuilder::build` for `SkAnalyticEdgeBuilder` (`src/core/SkEdgeBuilder.cpp`), with the
//! y-extrema chopping of `src/core/SkGeometry.cpp` and the convexity and rect tests of
//! `src/core/SkPathPriv.cpp` that pick the scan converter's fast paths.

use takumi_core::path::PathVerb;
use takumi_core::{
  geometry::{Point, Rect},
  path::Path,
};

use super::{
  edge::Edge,
  edge_clipper::{EdgeClipper, Segment},
};

/// An edge `SkEdgeBuilder` builds: an analytic edge, or an aliased fill's `SkEdge`.
pub(super) trait BuiltEdge: Sized {
  fn line(p0: Point<f32>, p1: Point<f32>) -> Option<Self>;
  fn quadratic(points: &[Point<f32>; 3]) -> Option<Self>;
  fn cubic(points: &[Point<f32>; 4]) -> Option<Self>;

  /// `combineVertical`: how this vertical line merges into the edge before it.
  fn combine_vertical(&self, _last: &mut Self) -> Combine {
    Combine::No
  }
}

impl BuiltEdge for Edge {
  fn line(p0: Point<f32>, p1: Point<f32>) -> Option<Self> {
    Edge::line(p0, p1)
  }

  fn quadratic(points: &[Point<f32>; 3]) -> Option<Self> {
    Edge::quadratic(points)
  }

  fn cubic(points: &[Point<f32>; 4]) -> Option<Self> {
    Edge::cubic(points)
  }

  fn combine_vertical(&self, last: &mut Self) -> Combine {
    if self.dx == 0 {
      combine_vertical(self, last)
    } else {
      Combine::No
    }
  }
}

/// `SkEdgeBuilder::buildEdges`, walking contours as `SkPathEdgeIter` does and clipping each
/// segment when `clipper` is given.
pub(super) fn build_edges<E: BuiltEdge>(path: &Path, clipper: Option<&EdgeClipper>) -> Vec<E> {
  let mut edges = Vec::with_capacity(path.points().len());
  let mut builder = Builder {
    edges: &mut edges,
    clipper,
    segments: Vec::new(),
  };
  let points = path.points();
  let mut index = 0;
  let mut move_to = None;
  let mut last = Point::ZERO;

  for verb in path.verbs() {
    match verb {
      PathVerb::Move => {
        builder.close_contour(&mut move_to, last);
        last = points[index];
        move_to = Some(last);
        index += 1;
      }
      PathVerb::Line => {
        builder.line(last, points[index]);
        last = points[index];
        index += 1;
      }
      PathVerb::Quad => {
        builder.quad([last, points[index], points[index + 1]]);
        last = points[index + 1];
        index += 2;
      }
      PathVerb::Cubic => {
        builder.cubic([last, points[index], points[index + 1], points[index + 2]]);
        last = points[index + 2];
        index += 3;
      }
      PathVerb::Close => {
        let start = move_to.unwrap_or(last);

        builder.close_contour(&mut move_to, last);
        move_to = Some(start);
        last = start;
      }
    }
  }
  builder.close_contour(&mut move_to, last);

  edges
}

/// The edge list being built and the clipper its segments pass through, `SkEdgeBuilder::build`.
struct Builder<'a, E> {
  edges: &'a mut Vec<E>,
  clipper: Option<&'a EdgeClipper>,
  segments: Vec<Segment>,
}

impl<E: BuiltEdge> Builder<'_, E> {
  /// The line `SkPathEdgeIter` closes every contour with, back to its start.
  fn close_contour(&mut self, move_to: &mut Option<Point<f32>>, last: Point<f32>) {
    if let Some(start) = move_to.take() {
      self.line(last, start);
    }
  }

  fn line(&mut self, p0: Point<f32>, p1: Point<f32>) {
    match self.clipper {
      Some(clipper) => {
        clipper.clip_line(p0, p1, &mut self.segments);
        self.add_segments();
      }
      None => add_line(self.edges, p0, p1),
    }
  }

  fn quad(&mut self, quad: [Point<f32>; 3]) {
    match self.clipper {
      Some(clipper) => {
        clipper.clip_quad(quad, &mut self.segments);
        self.add_segments();
      }
      None => {
        for mono in chop_quad_at_y_extrema(quad) {
          self.edges.extend(E::quadratic(&mono));
        }
      }
    }
  }

  fn cubic(&mut self, cubic: [Point<f32>; 4]) {
    match self.clipper {
      Some(clipper) => {
        clipper.clip_cubic(cubic, &mut self.segments);
        self.add_segments();
      }
      None => {
        for mono in chop_cubic_at_y_extrema(cubic) {
          self.edges.extend(E::cubic(&mono));
        }
      }
    }
  }

  /// Adds what the clipper produced, already monotonic in y.
  fn add_segments(&mut self) {
    for segment in self.segments.drain(..) {
      match segment {
        Segment::Line(p0, p1) => add_line(self.edges, p0, p1),
        Segment::Quad(quad) => self.edges.extend(E::quadratic(&quad)),
        Segment::Cubic(cubic) => self.edges.extend(E::cubic(&cubic)),
      }
    }
  }
}

/// `SkEdgeBuilder::addLine`, merging vertical lines with the previous edge.
fn add_line<E: BuiltEdge>(edges: &mut Vec<E>, p0: Point<f32>, p1: Point<f32>) {
  let Some(edge) = E::line(p0, p1) else {
    return;
  };

  let combine = match edges.last_mut() {
    Some(last) => edge.combine_vertical(last),
    None => Combine::No,
  };

  match combine {
    Combine::Total => {
      edges.pop();
    }
    Combine::Partial => {}
    Combine::No => edges.push(edge),
  }
}

/// `SkEdgeBuilder::Combine`.
pub(super) enum Combine {
  No,
  Partial,
  Total,
}

/// `SkAnalyticEdgeBuilder::combineVertical`.
fn combine_vertical(edge: &Edge, last: &mut Edge) -> Combine {
  let approximately_equal = |a: i32, b: i32| (a - b).abs() < 0x100;

  if !last.is_line || last.dx != 0 || edge.x != last.x {
    return Combine::No;
  }

  if edge.winding == last.winding {
    if edge.lower_y == last.upper_y {
      last.upper_y = edge.upper_y;
      last.y = last.upper_y;
      return Combine::Partial;
    }
    if approximately_equal(edge.upper_y, last.lower_y) {
      last.lower_y = edge.lower_y;
      return Combine::Partial;
    }
    return Combine::No;
  }

  if approximately_equal(edge.upper_y, last.upper_y) {
    if approximately_equal(edge.lower_y, last.lower_y) {
      return Combine::Total;
    }
    if edge.lower_y < last.lower_y {
      last.upper_y = edge.lower_y;
      last.y = last.upper_y;
      return Combine::Partial;
    }
    last.upper_y = last.lower_y;
    last.y = last.upper_y;
    last.lower_y = edge.lower_y;
    last.winding = edge.winding;
    return Combine::Partial;
  }

  if approximately_equal(edge.lower_y, last.lower_y) {
    if edge.upper_y > last.upper_y {
      last.lower_y = edge.upper_y;
      return Combine::Partial;
    }
    last.lower_y = last.upper_y;
    last.upper_y = edge.upper_y;
    last.y = last.upper_y;
    last.winding = edge.winding;
    return Combine::Partial;
  }

  Combine::No
}

/// `valid_unit_divide` in `SkGeometry.cpp`.
fn valid_unit_divide(mut numer: f32, mut denom: f32) -> Option<f32> {
  if numer < 0.0 {
    numer = -numer;
    denom = -denom;
  }

  if denom == 0.0 || numer == 0.0 || numer >= denom {
    return None;
  }

  let ratio = numer / denom;

  (!ratio.is_nan() && ratio != 0.0).then_some(ratio)
}

/// `is_not_monotonic` in `SkGeometry.cpp`.
fn is_not_monotonic(a: f32, b: f32, c: f32) -> bool {
  let ab = a - b;
  let bc = if ab < 0.0 { c - b } else { b - c };

  ab == 0.0 || bc < 0.0
}

fn lerp(a: Point<f32>, b: Point<f32>, t: f32) -> Point<f32> {
  Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// `SkChopQuadAt`: the two halves of `src` split at `t`, sharing their middle point.
pub(super) fn chop_quad_at(src: [Point<f32>; 3], t: f32) -> [Point<f32>; 5] {
  let p01 = lerp(src[0], src[1], t);
  let p12 = lerp(src[1], src[2], t);

  [src[0], p01, lerp(p01, p12, t), p12, src[2]]
}

/// `SkChopQuadAtYExtrema`: one or two y-monotonic quads.
pub(super) fn chop_quad_at_y_extrema(
  src: [Point<f32>; 3],
) -> impl Iterator<Item = [Point<f32>; 3]> {
  let [p0, p1, p2] = src;
  let (a, mut b, c) = (p0.y, p1.y, p2.y);

  if is_not_monotonic(a, b, c) {
    if let Some(t) = valid_unit_divide(a - b, a - b - b + c) {
      let [_, p01, mid, p12, _] = chop_quad_at(src, t);

      // `flatten_double_quad_extrema`.
      return [
        Some([p0, Point::new(p01.x, mid.y), mid]),
        Some([mid, Point::new(p12.x, mid.y), p2]),
      ]
      .into_iter()
      .flatten();
    }

    // The unit divide underflowed, so the control point is forced onto an end point.
    b = if (a - b).abs() < (b - c).abs() { a } else { c };
  }

  [Some([p0, Point::new(p1.x, b), p2]), None]
    .into_iter()
    .flatten()
}

pub(super) fn transpose(point: Point<f32>) -> Point<f32> {
  Point::new(point.y, point.x)
}

/// `SkChopQuadAtXExtrema`, through [`chop_quad_at_y_extrema`] on the transposed quad.
pub(super) fn chop_quad_at_x_extrema(
  src: [Point<f32>; 3],
) -> impl Iterator<Item = [Point<f32>; 3]> {
  chop_quad_at_y_extrema(src.map(transpose)).map(|quad| quad.map(transpose))
}

/// `SkChopCubicAtXExtrema`, through [`chop_cubic_at_y_extrema`] on the transposed cubic.
pub(super) fn chop_cubic_at_x_extrema(
  src: [Point<f32>; 4],
) -> impl Iterator<Item = [Point<f32>; 4]> {
  chop_cubic_at_y_extrema(src.map(transpose)).map(|cubic| cubic.map(transpose))
}

/// `SkFindUnitQuadRoots`.
pub(super) fn find_unit_quad_roots(a: f32, b: f32, c: f32) -> ([f32; 2], usize) {
  let mut roots = [0.0; 2];
  let mut count = 0;
  let mut push = |root: Option<f32>| {
    if let Some(root) = root {
      roots[count] = root;
      count += 1;
    }
  };

  if a == 0.0 {
    push(valid_unit_divide(-c, b));
    return (roots, count);
  }

  let discriminant = f64::from(b) * f64::from(b) - 4.0 * f64::from(a) * f64::from(c);

  if discriminant < 0.0 {
    return (roots, 0);
  }

  let r = discriminant.sqrt() as f32;

  if !r.is_finite() {
    return (roots, 0);
  }

  let q = if b < 0.0 {
    -(b - r) / 2.0
  } else {
    -(b + r) / 2.0
  };

  push(valid_unit_divide(q, a));
  push(valid_unit_divide(c, q));

  if count == 2 {
    if roots[0] > roots[1] {
      roots.swap(0, 1);
    } else if roots[0] == roots[1] {
      count = 1;
    }
  }

  (roots, count)
}

/// The control points `SkChopCubicAt` interpolates at `t`: `ab`, `bc`, `cd`, `abc`, `bcd`, `abcd`.
pub(super) fn cubic_split(src: &[Point<f32>; 4], t: f32) -> [Point<f32>; 6] {
  let ab = lerp(src[0], src[1], t);
  let bc = lerp(src[1], src[2], t);
  let cd = lerp(src[2], src[3], t);
  let abc = lerp(ab, bc, t);
  let bcd = lerp(bc, cd, t);

  [ab, bc, cd, abc, bcd, lerp(abc, bcd, t)]
}

/// `SkChopCubicAtYExtrema`: one to three y-monotonic cubics, sharing end points in `dst`.
pub(super) fn chop_cubic_at_y_extrema(
  src: [Point<f32>; 4],
) -> impl Iterator<Item = [Point<f32>; 4]> {
  // `SkFindCubicExtrema`.
  let [a, b, c, d] = src.map(|point| point.y);
  let (roots, count) = find_unit_quad_roots(d - a + 3.0 * (b - c), 2.0 * (a - b - b + c), b - a);
  let mut dst = [src[3]; 10];

  dst[..4].copy_from_slice(&src);

  match count {
    0 => {}
    // `SkChopCubicAt(src, dst, t)`.
    1 => {
      let [ab, _, cd, abc, bcd, abcd] = cubic_split(&src, roots[0]);

      dst[..7].copy_from_slice(&[src[0], ab, abc, abcd, bcd, cd, src[3]]);
    }
    // `SkChopCubicAt(src, dst, t0, t1)`, which chops the original cubic at both roots. The roots
    // lie in (0, 1), so its `t1 == 1` case never applies.
    _ => {
      let [t0, t1] = roots;
      let [ab0, _, _, abc0, bcd0, abcd0] = cubic_split(&src, t0);

      let [_, _, cd1, abc1, bcd1, abcd1] = cubic_split(&src, t1);

      dst = [
        src[0],
        ab0,
        abc0,
        abcd0,
        lerp(abc0, bcd0, t1),
        lerp(abc1, bcd1, t0),
        abcd1,
        bcd1,
        cd1,
        src[3],
      ];
    }
  }

  // `flatten_double_cubic_extrema` at each split point.
  for split in 0..count {
    let end = 3 * split + 3;

    dst[end - 1].y = dst[end].y;
    dst[end + 1].y = dst[end].y;
  }

  (0..=count).map(move |piece| {
    [
      dst[3 * piece],
      dst[3 * piece + 1],
      dst[3 * piece + 2],
      dst[3 * piece + 3],
    ]
  })
}

/// `DirChange` in `SkPathPriv.cpp`.
#[derive(Clone, Copy, PartialEq)]
enum DirChange {
  Unknown,
  Left,
  Right,
  Straight,
  Backwards,
  Invalid,
}

/// `Convexicator` in `SkPathPriv.cpp`, for a single contour.
struct Convexicator {
  first_point: Point<f32>,
  first_vector: Point<f32>,
  last_point: Point<f32>,
  last_vector: Point<f32>,
  expected: DirChange,
  first_direction_known: bool,
  reversals: u32,
}

impl Convexicator {
  fn new() -> Self {
    Self {
      first_point: Point::ZERO,
      first_vector: Point::ZERO,
      last_point: Point::ZERO,
      last_vector: Point::ZERO,
      expected: DirChange::Invalid,
      first_direction_known: false,
      reversals: 0,
    }
  }

  fn set_move_point(&mut self, point: Point<f32>) {
    self.first_point = point;
    self.last_point = point;
    self.expected = DirChange::Invalid;
  }

  fn add_point(&mut self, point: Point<f32>) -> bool {
    if self.last_point == point {
      return true;
    }

    let vector = point - self.last_point;

    if self.first_point == self.last_point
      && self.expected == DirChange::Invalid
      && self.last_vector == Point::ZERO
    {
      self.last_vector = vector;
      self.first_vector = vector;
    } else if !self.add_vector(vector) {
      return false;
    }
    self.last_point = point;

    true
  }

  fn close(&mut self) -> bool {
    self.add_point(self.first_point) && self.add_vector(self.first_vector)
  }

  fn direction_change(&self, vector: Point<f32>) -> DirChange {
    let cross = self.last_vector.x * vector.y - self.last_vector.y * vector.x;

    if !cross.is_finite() {
      return DirChange::Unknown;
    }
    if cross == 0.0 {
      let dot = self.last_vector.x * vector.x + self.last_vector.y * vector.y;

      return if dot < 0.0 {
        DirChange::Backwards
      } else {
        DirChange::Straight
      };
    }
    if cross > 0.0 {
      DirChange::Right
    } else {
      DirChange::Left
    }
  }

  fn add_vector(&mut self, vector: Point<f32>) -> bool {
    match self.direction_change(vector) {
      direction @ (DirChange::Left | DirChange::Right) => {
        if self.expected == DirChange::Invalid {
          self.expected = direction;
          self.first_direction_known = true;
        } else if direction != self.expected {
          self.first_direction_known = false;
          return false;
        }
        self.last_vector = vector;
      }
      DirChange::Straight => {}
      DirChange::Backwards => {
        self.last_vector = vector;
        self.reversals += 1;
        return self.reversals < 3;
      }
      DirChange::Unknown | DirChange::Invalid => return false,
    }

    true
  }
}

/// `Convexicator::IsConcaveBySign`: whether the points change sign in x or y more than three times.
fn is_concave_by_sign(points: &[Point<f32>]) -> bool {
  if points.len() <= 3 {
    return false;
  }

  let mut current = points[0];
  let mut dxes = 0;
  let mut dyes = 0;
  let mut last_sx = 2;
  let mut last_sy = 2;

  for point in points[1..].iter().chain([&points[0]]) {
    let vector = *point - current;

    if vector != Point::ZERO {
      if !vector.x.is_finite() || !vector.y.is_finite() {
        return true;
      }

      let sx = i32::from(vector.x < 0.0);
      let sy = i32::from(vector.y < 0.0);

      dxes += i32::from(sx != last_sx);
      dyes += i32::from(sy != last_sy);
      if dxes > 3 || dyes > 3 {
        return true;
      }
      last_sx = sx;
      last_sy = sy;
    }
    current = *point;
  }

  false
}

/// `SkPathPriv::ComputeConvexity` reduced to whether the path is convex.
pub(super) fn is_convex(path: &Path) -> bool {
  let mut verbs = path.verbs();
  let mut points = path.points();

  while let [rest @ .., PathVerb::Move] = verbs {
    verbs = rest;
    points = &points[..points.len() - 1];
  }

  if verbs.is_empty() {
    return true;
  }

  if is_concave_by_sign(points) {
    return false;
  }

  let mut contours = 0;
  let mut needs_close = false;
  let mut state = Convexicator::new();
  let mut index = 0;

  for verb in verbs {
    let count = match verb {
      PathVerb::Move | PathVerb::Line => 1,
      PathVerb::Quad => 2,
      PathVerb::Cubic => 3,
      PathVerb::Close => 0,
    };
    let segment = &points[index.min(points.len())..(index + count).min(points.len())];

    if contours == 0 {
      if *verb == PathVerb::Move {
        state.set_move_point(segment[0]);
      } else {
        contours += 1;
        needs_close = true;
      }
    }

    if contours == 1 {
      if matches!(verb, PathVerb::Close | PathVerb::Move) {
        if !state.close() {
          return false;
        }
        needs_close = false;
        contours += 1;
      } else if !segment.iter().all(|point| state.add_point(*point)) {
        return false;
      }
    } else if *verb != PathVerb::Move {
      return false;
    }

    index += count;
  }

  if needs_close && !state.close() {
    return false;
  }

  state.first_direction_known || state.reversals < 3
}

/// `trivial_rect` in `SkPathPriv.cpp`: the rect of a move, three lines and a close.
///
/// Approximate: `SkPathPriv::IsRectContour` also accepts rects with collinear or repeated points
/// and implicit closes, which fall back to the general scan here.
pub(super) fn trivial_rect(path: &Path) -> Option<Rect<f32>> {
  let (verbs, points) = (path.verbs(), path.points());

  if verbs
    != [
      PathVerb::Move,
      PathVerb::Line,
      PathVerb::Line,
      PathVerb::Line,
      PathVerb::Close,
    ]
    || points.len() != 4
  {
    return None;
  }

  let sides = [
    points[1] - points[0],
    points[2] - points[1],
    points[3] - points[2],
    points[0] - points[3],
  ];
  let axis_aligned = |v: Point<f32>| (v.x == 0.0) ^ (v.y == 0.0);
  let orthogonal =
    |a: Point<f32>, b: Point<f32>| ((a.x == 0.0) ^ (b.x == 0.0)) & ((a.y == 0.0) ^ (b.y == 0.0));

  if !(axis_aligned(sides[0])
    & orthogonal(sides[0], sides[1])
    & orthogonal(sides[1], sides[2])
    & orthogonal(sides[2], sides[3]))
  {
    return None;
  }

  Rect::from_ltrb(
    points[0].x.min(points[2].x),
    points[0].y.min(points[2].y),
    points[0].x.max(points[2].x),
    points[0].y.max(points[2].y),
  )
}

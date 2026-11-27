//! Skia's path geometry: `SkPath` and `SkPathBuilder` as the raster and PDF backends build and walk
//! paths, its float and integer rects, and the `SkGeometry` curve helpers that bound them.

mod geometry;
mod rect;

pub use geometry::{
  chop_quad_at, cubic_split, eval_cubic_pos_at, eval_quad_at, find_cubic_extrema,
  find_quad_extrema, find_unit_quad_roots, valid_unit_divide,
};
pub use rect::{IntRect, IntSize, Rect};

use crate::style::Affine;

/// A point, Skia's `SkPoint`.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct Point {
  /// The horizontal coordinate.
  pub x: f32,
  /// The vertical coordinate.
  pub y: f32,
}

impl Point {
  /// The point at `x`, `y`.
  pub const fn from_xy(x: f32, y: f32) -> Self {
    Self { x, y }
  }

  /// The origin.
  pub const fn zero() -> Self {
    Self { x: 0.0, y: 0.0 }
  }

  /// Whether both coordinates are finite.
  pub fn is_finite(self) -> bool {
    self.x.is_finite() && self.y.is_finite()
  }
}

impl std::ops::Sub for Point {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self::from_xy(self.x - other.x, self.y - other.y)
  }
}

/// A path instruction, Skia's `SkPathVerb` without conics.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum PathVerb {
  /// Starts a contour at the next point.
  Move,
  /// A line to the next point.
  Line,
  /// A quadratic through the next two points.
  Quad,
  /// A cubic through the next three points.
  Cubic,
  /// Closes the contour.
  Close,
}

/// A path instruction with its points.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PathSegment {
  /// Starts a contour.
  MoveTo(Point),
  /// A line to a point.
  LineTo(Point),
  /// A quadratic through a control point to a point.
  QuadTo(Point, Point),
  /// A cubic through two control points to a point.
  CubicTo(Point, Point, Point),
  /// Closes the contour.
  Close,
}

/// A finished, non-empty path with the bounds of its points.
#[derive(Clone, PartialEq, Debug)]
pub struct Path {
  verbs: Vec<PathVerb>,
  points: Vec<Point>,
  bounds: Rect,
}

impl Path {
  /// The number of verbs.
  pub fn len(&self) -> usize {
    self.verbs.len()
  }

  /// Whether the path has no verbs, which a finished path never is.
  pub fn is_empty(&self) -> bool {
    self.verbs.is_empty()
  }

  /// The bounds of every point, control points included.
  pub fn bounds(&self) -> Rect {
    self.bounds
  }

  /// The verbs.
  pub fn verbs(&self) -> &[PathVerb] {
    &self.verbs
  }

  /// The points the verbs consume, in order.
  pub fn points(&self) -> &[Point] {
    &self.points
  }

  /// The path with every point mapped by `transform`, none where a point leaves the finite range.
  pub fn transform(mut self, transform: Affine) -> Option<Self> {
    if transform == Affine::IDENTITY {
      return Some(self);
    }

    for point in &mut self.points {
      let (x, y) = transform.transform_point(point.x, point.y);

      *point = Point::from_xy(x, y);
    }

    self.bounds = Rect::from_points(&self.points)?;
    Some(self)
  }

  /// The segments, each with the points it consumes.
  pub fn segments(&self) -> impl Iterator<Item = PathSegment> + '_ {
    let mut points = self.points.iter().copied();

    self.verbs.iter().map(move |verb| {
      let mut next = || points.next().unwrap_or_default();

      match verb {
        PathVerb::Move => PathSegment::MoveTo(next()),
        PathVerb::Line => PathSegment::LineTo(next()),
        PathVerb::Quad => PathSegment::QuadTo(next(), next()),
        PathVerb::Cubic => PathSegment::CubicTo(next(), next(), next()),
        PathVerb::Close => PathSegment::Close,
      }
    })
  }

  /// `SkPath::computeTightBounds`: the bounds of the outline itself, its curves at their extrema.
  pub fn compute_tight_bounds(&self) -> Option<Rect> {
    let mut min = self.points[0];
    let mut max = self.points[0];
    let mut last = Point::zero();
    let mut extend = |point: Point| {
      min = Point::from_xy(min.x.min(point.x), min.y.min(point.y));
      max = Point::from_xy(max.x.max(point.x), max.y.max(point.y));
    };

    for segment in self.segments() {
      match segment {
        PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
          extend(point);
          last = point;
        }
        PathSegment::QuadTo(control, to) => {
          let quad = [last, control, to];

          for t in [
            find_quad_extrema(last.x, control.x, to.x),
            find_quad_extrema(last.y, control.y, to.y),
          ]
          .into_iter()
          .flatten()
          {
            extend(eval_quad_at(&quad, t));
          }

          extend(to);
          last = to;
        }
        PathSegment::CubicTo(first, second, to) => {
          let cubic = [last, first, second, to];
          let (xs, x_count) = find_cubic_extrema(last.x, first.x, second.x, to.x);
          let (ys, y_count) = find_cubic_extrema(last.y, first.y, second.y, to.y);

          for &t in xs[..x_count].iter().chain(&ys[..y_count]) {
            extend(eval_cubic_pos_at(&cubic, t));
          }

          extend(to);
          last = to;
        }
        PathSegment::Close => {}
      }
    }

    Rect::from_ltrb(min.x, min.y, max.x, max.y)
  }
}

/// Builds a [`Path`] as Skia's `SkPathBuilder` does: a line or curve with no contour open starts
/// one at the last contour's start, and a move after a move replaces it.
#[derive(Clone, Debug)]
pub struct PathBuilder {
  verbs: Vec<PathVerb>,
  points: Vec<Point>,
  last_move_to_index: usize,
  move_to_required: bool,
}

impl Default for PathBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl PathBuilder {
  /// An empty builder.
  pub fn new() -> Self {
    Self {
      verbs: Vec::new(),
      points: Vec::new(),
      last_move_to_index: 0,
      move_to_required: true,
    }
  }

  /// The closed outline of `rect`, clockwise from its top left.
  pub fn from_rect(rect: Rect) -> Path {
    Path {
      verbs: vec![
        PathVerb::Move,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Close,
      ],
      points: vec![
        Point::from_xy(rect.left(), rect.top()),
        Point::from_xy(rect.right(), rect.top()),
        Point::from_xy(rect.right(), rect.bottom()),
        Point::from_xy(rect.left(), rect.bottom()),
      ],
      bounds: rect,
    }
  }

  /// Whether nothing has been added.
  pub fn is_empty(&self) -> bool {
    self.verbs.is_empty()
  }

  /// The last point added.
  pub fn last_point(&self) -> Option<Point> {
    self.points.last().copied()
  }

  /// Starts a contour at `x`, `y`.
  pub fn move_to(&mut self, x: f32, y: f32) {
    if let Some(PathVerb::Move) = self.verbs.last() {
      if let Some(last) = self.points.last_mut() {
        *last = Point::from_xy(x, y);
      }
    } else {
      self.last_move_to_index = self.points.len();
      self.move_to_required = false;
      self.verbs.push(PathVerb::Move);
      self.points.push(Point::from_xy(x, y));
    }
  }

  fn inject_move_to_if_needed(&mut self) {
    if self.move_to_required {
      let start = self
        .points
        .get(self.last_move_to_index)
        .copied()
        .unwrap_or_default();

      self.move_to(start.x, start.y);
    }
  }

  /// Adds a line to `x`, `y`.
  pub fn line_to(&mut self, x: f32, y: f32) {
    self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Line);
    self.points.push(Point::from_xy(x, y));
  }

  /// Adds a quadratic through `x1`, `y1` to `x`, `y`.
  pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
    self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Quad);
    self.points.push(Point::from_xy(x1, y1));
    self.points.push(Point::from_xy(x, y));
  }

  /// Adds a cubic through `x1`, `y1` and `x2`, `y2` to `x`, `y`.
  pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
    self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Cubic);
    self.points.push(Point::from_xy(x1, y1));
    self.points.push(Point::from_xy(x2, y2));
    self.points.push(Point::from_xy(x, y));
  }

  /// Closes the open contour.
  pub fn close(&mut self) {
    if self
      .verbs
      .last()
      .is_some_and(|&verb| verb != PathVerb::Close)
    {
      self.verbs.push(PathVerb::Close);
    }

    self.move_to_required = true;
  }

  /// The path, none where it has fewer than two verbs or a point that is not finite.
  pub fn finish(self) -> Option<Path> {
    if self.verbs.len() < 2 {
      return None;
    }

    Some(Path {
      bounds: Rect::from_points(&self.points)?,
      verbs: self.verbs,
      points: self.points,
    })
  }
}

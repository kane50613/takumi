//! Skia's path geometry: `SkPath` and `SkPathBuilder` as the raster and PDF backends build and walk
//! paths, its float and integer rects, and the `SkGeometry` curve helpers that bound them.

mod conic;
mod dash;
mod geometry;
mod rect;
mod stroke;
#[cfg(test)]
mod tests;

pub use dash::StrokeDash;
pub use geometry::{
  chop_cubic_at, chop_quad_at, cubic_split, eval_cubic_pos_at, eval_quad_at, find_cubic_extrema,
  find_quad_extrema, find_unit_quad_roots, valid_unit_divide,
};
pub use stroke::{Stroke, resolution_scale};

use crate::{
  geometry::{PathCommand, Point, Rect},
  style::Affine,
};

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

/// A finished, non-empty path with the bounds of its points.
#[derive(Clone, PartialEq, Debug)]
pub struct Path {
  verbs: Vec<PathVerb>,
  points: Vec<Point<f32>>,
  bounds: Rect<f32>,
}

impl Path {
  /// The closed outline of `rect`, clockwise from its top left.
  pub fn from_rect(rect: Rect<f32>) -> Self {
    Path {
      verbs: vec![
        PathVerb::Move,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Close,
      ],
      points: vec![
        Point::new(rect.left, rect.top),
        Point::new(rect.right, rect.top),
        Point::new(rect.right, rect.bottom),
        Point::new(rect.left, rect.bottom),
      ],
      bounds: rect,
    }
  }

  /// The bounds of every point, control points included.
  pub fn bounds(&self) -> Rect<f32> {
    self.bounds
  }

  /// The verbs.
  pub fn verbs(&self) -> &[PathVerb] {
    &self.verbs
  }

  /// The points the verbs consume, in order.
  pub fn points(&self) -> &[Point<f32>] {
    &self.points
  }

  /// The path with every point mapped by `transform`, none where a point leaves the finite range.
  pub fn transform(mut self, transform: Affine) -> Option<Self> {
    if transform == Affine::IDENTITY {
      return Some(self);
    }

    for point in &mut self.points {
      let (x, y) = transform.transform_point(point.x, point.y);

      *point = Point::new(x, y);
    }

    self.bounds = Rect::from_points(&self.points)?;
    Some(self)
  }

  /// The segments, each with the points it consumes.
  pub fn segments(&self) -> impl Iterator<Item = PathCommand> + '_ {
    let mut points = self.points.iter().copied();

    self.verbs.iter().map(move |verb| {
      let mut next = || points.next().unwrap_or_default();

      match verb {
        PathVerb::Move => PathCommand::MoveTo(next()),
        PathVerb::Line => PathCommand::LineTo(next()),
        PathVerb::Quad => PathCommand::QuadTo(next(), next()),
        PathVerb::Cubic => PathCommand::CubicTo(next(), next(), next()),
        PathVerb::Close => PathCommand::Close,
      }
    })
  }

  /// `SkPath::computeTightBounds`: the bounds of the outline itself, its curves at their extrema.
  pub fn compute_tight_bounds(&self) -> Option<Rect<f32>> {
    let mut min = self.points[0];
    let mut max = self.points[0];
    let mut last = Point::ZERO;
    let mut extend = |point: Point<f32>| {
      min = Point::new(min.x.min(point.x), min.y.min(point.y));
      max = Point::new(max.x.max(point.x), max.y.max(point.y));
    };

    for segment in self.segments() {
      match segment {
        PathCommand::MoveTo(point) | PathCommand::LineTo(point) => {
          extend(point);
          last = point;
        }
        PathCommand::QuadTo(control, to) => {
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
        PathCommand::CubicTo(first, second, to) => {
          let cubic = [last, first, second, to];
          let (xs, x_count) = find_cubic_extrema(last.x, first.x, second.x, to.x);
          let (ys, y_count) = find_cubic_extrema(last.y, first.y, second.y, to.y);

          for &t in xs[..x_count].iter().chain(&ys[..y_count]) {
            extend(eval_cubic_pos_at(&cubic, t));
          }

          extend(to);
          last = to;
        }
        PathCommand::Close => {}
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
  points: Vec<Point<f32>>,
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

  /// Whether nothing has been added.
  pub fn is_empty(&self) -> bool {
    self.verbs.is_empty()
  }

  /// The last point added.
  pub fn last_point(&self) -> Option<Point<f32>> {
    self.points.last().copied()
  }

  /// Starts a contour at `x`, `y`.
  pub fn move_to(&mut self, x: f32, y: f32) {
    if let Some(PathVerb::Move) = self.verbs.last() {
      if let Some(last) = self.points.last_mut() {
        *last = Point::new(x, y);
      }
    } else {
      self.last_move_to_index = self.points.len();
      self.move_to_required = false;
      self.verbs.push(PathVerb::Move);
      self.points.push(Point::new(x, y));
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
    self.points.push(Point::new(x, y));
  }

  /// Adds a quadratic through `x1`, `y1` to `x`, `y`.
  pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
    self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Quad);
    self.points.push(Point::new(x1, y1));
    self.points.push(Point::new(x, y));
  }

  /// Adds a cubic through `x1`, `y1` and `x2`, `y2` to `x`, `y`.
  pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
    self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Cubic);
    self.points.push(Point::new(x1, y1));
    self.points.push(Point::new(x2, y2));
    self.points.push(Point::new(x, y));
  }

  /// Adds a conic through `x1`, `y1` to `x`, `y` pulled by `weight`, as `SkPathBuilder::conicTo`
  /// takes it. Approximate: the conic is split into quads here, within a quarter of a unit, where
  /// `SkEdgeBuilder` splits it in device space.
  pub fn conic_to(&mut self, x1: f32, y1: f32, x: f32, y: f32, weight: f32) {
    self.conic_to_within(x1, y1, x, y, weight, conic::CONIC_TOLERANCE);
  }

  /// [`Self::conic_to`], split into quads within `tolerance`.
  fn conic_to_within(&mut self, x1: f32, y1: f32, x: f32, y: f32, weight: f32, tolerance: f32) {
    self.inject_move_to_if_needed();

    if weight <= 0.0 {
      self.line_to(x, y);
    } else if weight == 1.0 {
      self.quad_to(x1, y1, x, y);
    } else if !weight.is_finite() {
      self.line_to(x1, y1);
      self.line_to(x, y);
    } else {
      let start = self.last_point().unwrap_or_default();
      let conic = conic::Conic {
        points: [start, Point::new(x1, y1), Point::new(x, y)],
        weight,
      };

      for quad in conic.to_quads(tolerance).as_chunks::<2>().0 {
        self.quad_to(quad[0].x, quad[0].y, quad[1].x, quad[1].y);
      }
    }
  }

  /// Moves the last point to `point`, or starts a contour there when there is none.
  pub fn set_last_point(&mut self, point: Point<f32>) {
    match self.points.last_mut() {
      Some(last) => *last = point,
      None => self.move_to(point.x, point.y),
    }
  }

  /// The points added so far.
  pub fn points(&self) -> &[Point<f32>] {
    &self.points
  }

  /// Empties the builder.
  pub fn clear(&mut self) {
    self.verbs.clear();
    self.points.clear();
    self.last_move_to_index = 0;
    self.move_to_required = true;
  }

  /// Appends `other`'s verbs and points, `SkPathBuilder::addPath` in append mode.
  pub fn push_builder(&mut self, other: &Self) {
    if other.is_empty() {
      return;
    }

    // A pending move gives way to the one `other` starts with.
    if self.verbs.last() == Some(&PathVerb::Move) {
      self.verbs.pop();
      self.points.pop();
    }

    self.last_move_to_index = self.points.len() + other.last_move_to_index;
    self.move_to_required = other.move_to_required;
    self.verbs.extend_from_slice(&other.verbs);
    self.points.extend_from_slice(&other.points);
  }

  /// `SkPathBuilder::privateReversePathTo`: continues with `other`'s last contour walked backwards
  /// to its start.
  pub fn push_reversed(&mut self, other: &Self) {
    let mut end = other.points.len();

    for verb in other.verbs.iter().rev() {
      match verb {
        PathVerb::Move => return,
        PathVerb::Line => {
          end -= 1;

          let to = other.points[end - 1];

          self.line_to(to.x, to.y);
        }
        PathVerb::Quad => {
          end -= 2;

          let [to, control] = [other.points[end - 1], other.points[end]];

          self.quad_to(control.x, control.y, to.x, to.y);
        }
        PathVerb::Cubic => {
          end -= 3;

          let [to, second, first] = [
            other.points[end - 1],
            other.points[end],
            other.points[end + 1],
          ];

          self.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
        }
        PathVerb::Close => {}
      }
    }
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

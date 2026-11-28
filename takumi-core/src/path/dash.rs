//! Skia's dashing, `SkDashPath` in `src/utils/SkDashPath.cpp`, walking contours as
//! `SkContourMeasure` in `src/core/SkContourMeasure.cpp` measures them. Approximate: a lone line
//! dashes through the general walk, where Skia's `SpecialLineRec` emits the dashes as rects. No
//! cull rect trims a line or rect to the clip before the dash limit as `cull_path` does with one,
//! and a path past the limit strokes nothing, where `skpathutils::FillPathWithPaint` strokes it
//! undashed.

use std::cmp::Ordering;

use super::{
  Path, PathBuilder, PathVerb, Point, chop_cubic_at, chop_quad_at, conic::NEARLY_ZERO,
  eval_cubic_pos_at, eval_quad_at,
};

/// `kMaxDashCount`: past this many dashes a path is not dashed.
const MAX_DASH_COUNT: f32 = 1_000_000.0;

/// `kMaxTValue`: a curve parameter in 30-bit fixed point.
const MAX_T_VALUE: u32 = 0x3FFF_FFFF;

/// `CHEAP_DIST_LIMIT`.
const CHEAP_DIST_LIMIT: f32 = 0.5;

/// `kMaxRecursionDepth`.
const MAX_RECURSION_DEPTH: u32 = 8;

/// A dash pattern: alternating on and off lengths, and how far into them the path starts.
#[derive(Clone, PartialEq, Debug)]
pub struct StrokeDash {
  intervals: Vec<f32>,
  initial_dash_length: f32,
  initial_dash_index: usize,
  interval_length: f32,
}

impl StrokeDash {
  /// The pattern of `intervals` started `phase` along, none where Skia's `ValidDashPath` rejects
  /// it: fewer than two or an odd number of intervals, a negative one, or no length.
  pub fn new(intervals: Vec<f32>, phase: f32) -> Option<Self> {
    if intervals.len() < 2
      || !intervals.len().is_multiple_of(2)
      || intervals.iter().any(|&interval| interval < 0.0)
    {
      return None;
    }

    let length: f32 = intervals.iter().sum();

    if !(length > 0.0 && length.is_finite() && phase.is_finite()) {
      return None;
    }

    // `CalcDashParameters`.
    let mut phase = phase;

    if phase < 0.0 {
      phase = -phase;

      if phase > length {
        phase %= length;
      }

      phase = length - phase;

      if phase == length {
        phase = 0.0;
      }
    } else if phase >= length {
      phase %= length;
    }

    let (initial_dash_length, initial_dash_index) = first_interval(&intervals, phase);

    Some(Self {
      intervals,
      initial_dash_length,
      initial_dash_index,
      interval_length: length,
    })
  }
}

/// `find_first_interval`: the length left of the interval `phase` falls in, and its index.
fn first_interval(intervals: &[f32], mut phase: f32) -> (f32, usize) {
  for (index, &gap) in intervals.iter().enumerate() {
    if phase > gap || (phase == gap && gap != 0.0) {
      phase -= gap;
    } else {
      return (gap - phase, index);
    }
  }

  (intervals[0], 0)
}

impl Path {
  /// `SkDashPath::InternalFilter`: the dashes `dash` cuts the path into, measured to
  /// `resolution_scale`. None where it has more than a million dashes or none at all.
  pub fn dash(&self, dash: &StrokeDash, resolution_scale: f32) -> Option<Path> {
    let count = dash.intervals.len();
    let mut builder = PathBuilder::new();
    let mut dash_count = 0.0;
    let bumped = self.bump_zero_length_line();
    let mut contours = ContourMeasures::new(bumped.as_ref().unwrap_or(self), resolution_scale);

    while let Some(contour) = contours.next() {
      let mut skip_first_segment = contour.closed;
      let mut added_segment = false;
      let length = contour.length;
      let mut index = dash.initial_dash_index;

      dash_count += length * (count >> 1) as f32 / dash.interval_length;

      if dash_count > MAX_DASH_COUNT {
        return None;
      }

      let mut distance = 0.0_f64;
      let mut dash_length = f64::from(dash.initial_dash_length);

      while distance < f64::from(length) {
        added_segment = false;

        if index.is_multiple_of(2) && !skip_first_segment {
          added_segment = true;
          contour.segment(
            distance as f32,
            (distance + dash_length) as f32,
            &mut builder,
            true,
          );
        }

        distance += dash_length;
        skip_first_segment = false;
        index += 1;

        if index == count {
          index = 0;
        }

        dash_length = f64::from(dash.intervals[index]);
      }

      if contour.closed
        && dash.initial_dash_index.is_multiple_of(2)
        && dash.initial_dash_length >= 0.0
      {
        contour.segment(0.0, dash.initial_dash_length, &mut builder, !added_segment);
      }
    }

    builder.finish()
  }

  /// `cull_path` without a cull rect: a lone line of no length with its end moved out by
  /// `adjust_zero_length_line`, so its caps draw.
  fn bump_zero_length_line(&self) -> Option<Path> {
    let ([PathVerb::Move, PathVerb::Line], [start, end]) = (self.verbs(), self.points()) else {
      return None;
    };

    if start != end {
      return None;
    }

    let mut builder = PathBuilder::new();

    builder.move_to(start.x, start.y);
    builder.line_to(end.x + end.x.max(1.001) * NEARLY_ZERO, end.y);
    builder.finish()
  }
}

#[derive(Copy, Clone, PartialEq)]
enum SegmentType {
  Line,
  Quad,
  Cubic,
}

/// `SkContourMeasure::Segment`: where a piece of the contour ends along it.
#[derive(Copy, Clone)]
struct Segment {
  distance: f32,
  point_index: usize,
  t_value: u32,
  kind: SegmentType,
}

impl Segment {
  fn scalar_t(&self) -> f32 {
    self.t_value as f32 * (1.0 / MAX_T_VALUE as f32)
  }
}

/// `SkContourMeasure`: one contour measured into segments.
struct ContourMeasure {
  segments: Vec<Segment>,
  points: Vec<Point>,
  length: f32,
  closed: bool,
}

/// `SkContourMeasureIter`: the contours of a path, measured one at a time.
struct ContourMeasures<'a> {
  path: &'a Path,
  verb: usize,
  point: usize,
  tolerance: f32,
}

/// `SkPoint::Distance`.
fn point_distance(a: Point, b: Point) -> f32 {
  let (dx, dy) = (a.x - b.x, a.y - b.y);
  let squared = dx * dx + dy * dy;

  if squared.is_finite() {
    squared.sqrt()
  } else {
    (f64::from(dx) * f64::from(dx) + f64::from(dy) * f64::from(dy)).sqrt() as f32
  }
}

/// `SkScalarInterp`.
fn interp(a: f32, b: f32, t: f32) -> f32 {
  a + (b - a) * t
}

/// `tspan_big_enough`.
fn tspan_big_enough(span: u32) -> bool {
  span >> 10 != 0
}

fn quad_too_curvy(points: &[Point; 3], tolerance: f32) -> bool {
  let dx = (points[1].x / 2.0) - (((points[0].x + points[2].x) / 2.0) / 2.0);
  let dy = (points[1].y / 2.0) - (((points[0].y + points[2].y) / 2.0) / 2.0);

  dx.abs().max(dy.abs()) > tolerance
}

fn cheap_dist_exceeds_limit(point: Point, x: f32, y: f32, tolerance: f32) -> bool {
  (x - point.x).abs().max((y - point.y).abs()) > tolerance
}

fn cubic_too_curvy(points: &[Point; 4], tolerance: f32) -> bool {
  let at = |t: f32| {
    (
      interp(points[0].x, points[3].x, t),
      interp(points[0].y, points[3].y, t),
    )
  };
  let (x1, y1) = at(1.0 / 3.0);
  let (x2, y2) = at(2.0 / 3.0);

  cheap_dist_exceeds_limit(points[1], x1, y1, tolerance)
    || cheap_dist_exceeds_limit(points[2], x2, y2, tolerance)
}

impl<'a> ContourMeasures<'a> {
  fn new(path: &'a Path, resolution_scale: f32) -> Self {
    Self {
      path,
      verb: 0,
      point: 0,
      tolerance: CHEAP_DIST_LIMIT * (1.0 / resolution_scale),
    }
  }

  fn next(&mut self) -> Option<ContourMeasure> {
    while self.verb < self.path.verbs().len() {
      if let Some(contour) = self.build_segments() {
        return Some(contour);
      }
    }

    None
  }

  /// `buildSegments`: measures the next contour, none where it has no length.
  fn build_segments(&mut self) -> Option<ContourMeasure> {
    let mut contour = ContourMeasure {
      segments: Vec::new(),
      points: Vec::new(),
      length: 0.0,
      closed: false,
    };
    let mut point_index: isize = -1;
    let mut distance = 0.0_f32;
    let mut seen_move = false;
    let verbs = self.path.verbs();
    let points = self.path.points();
    let mut last = Point::zero();

    while self.verb < verbs.len() {
      let verb = verbs[self.verb];

      if seen_move && verb == PathVerb::Move {
        break;
      }

      let index = point_index.max(0) as usize;

      match verb {
        PathVerb::Move => {
          last = points[self.point];
          self.point += 1;
          point_index += 1;
          contour.points.push(last);
          seen_move = true;
        }
        PathVerb::Line => {
          let to = points[self.point];
          let previous = distance;

          self.point += 1;
          distance = contour.line_segment(last, to, distance, index);

          if distance > previous {
            contour.points.push(to);
            point_index += 1;
          }

          last = to;
        }
        PathVerb::Quad => {
          let quad = [last, points[self.point], points[self.point + 1]];
          let previous = distance;

          self.point += 2;
          distance =
            contour.quad_segments(&quad, distance, 0, MAX_T_VALUE, index, 0, self.tolerance);

          if distance > previous {
            contour.points.extend_from_slice(&quad[1..]);
            point_index += 2;
          }

          last = quad[2];
        }
        PathVerb::Cubic => {
          let cubic = [
            last,
            points[self.point],
            points[self.point + 1],
            points[self.point + 2],
          ];
          let previous = distance;

          self.point += 3;
          distance =
            contour.cubic_segments(&cubic, distance, 0, MAX_T_VALUE, index, 0, self.tolerance);

          if distance > previous {
            contour.points.extend_from_slice(&cubic[1..]);
            point_index += 3;
          }

          last = cubic[3];
        }
        PathVerb::Close => contour.closed = true,
      }

      self.verb += 1;
    }

    if !distance.is_finite() || contour.segments.is_empty() {
      return None;
    }

    if contour.closed {
      let index = point_index.max(0) as usize;
      let previous = distance;
      let first = contour.points[0];

      distance = contour.line_segment(contour.points[index], first, distance, index);

      if distance > previous {
        contour.points.push(first);
      }
    }

    contour.length = distance;
    Some(contour)
  }
}

impl ContourMeasure {
  fn push(&mut self, distance: f32, point_index: usize, kind: SegmentType, t_value: u32) {
    self.segments.push(Segment {
      distance,
      point_index,
      t_value,
      kind,
    });
  }

  /// `compute_line_seg`.
  fn line_segment(&mut self, from: Point, to: Point, distance: f32, point_index: usize) -> f32 {
    let next = distance + point_distance(from, to);

    if next > distance {
      self.push(next, point_index, SegmentType::Line, MAX_T_VALUE);
    }

    next
  }

  /// `compute_quad_segs`.
  #[allow(clippy::too_many_arguments)]
  fn quad_segments(
    &mut self,
    quad: &[Point; 3],
    distance: f32,
    min_t: u32,
    max_t: u32,
    point_index: usize,
    depth: u32,
    tolerance: f32,
  ) -> f32 {
    if depth < MAX_RECURSION_DEPTH
      && tspan_big_enough(max_t - min_t)
      && quad_too_curvy(quad, tolerance)
    {
      let halves = chop_quad_at(*quad, 0.5);
      let half_t = (min_t + max_t) >> 1;
      let first = [halves[0], halves[1], halves[2]];
      let second = [halves[2], halves[3], halves[4]];
      let distance = self.quad_segments(
        &first,
        distance,
        min_t,
        half_t,
        point_index,
        depth + 1,
        tolerance,
      );

      return self.quad_segments(
        &second,
        distance,
        half_t,
        max_t,
        point_index,
        depth + 1,
        tolerance,
      );
    }

    let next = distance + point_distance(quad[0], quad[2]);

    if next > distance {
      self.push(next, point_index, SegmentType::Quad, max_t);
    }

    next
  }

  /// `compute_cubic_segs`.
  #[allow(clippy::too_many_arguments)]
  fn cubic_segments(
    &mut self,
    cubic: &[Point; 4],
    distance: f32,
    min_t: u32,
    max_t: u32,
    point_index: usize,
    depth: u32,
    tolerance: f32,
  ) -> f32 {
    if depth < MAX_RECURSION_DEPTH
      && tspan_big_enough(max_t - min_t)
      && cubic_too_curvy(cubic, tolerance)
    {
      let halves = chop_cubic_at(cubic, 0.5);
      let half_t = (min_t + max_t) >> 1;
      let first = [halves[0], halves[1], halves[2], halves[3]];
      let second = [halves[3], halves[4], halves[5], halves[6]];
      let distance = self.cubic_segments(
        &first,
        distance,
        min_t,
        half_t,
        point_index,
        depth + 1,
        tolerance,
      );

      return self.cubic_segments(
        &second,
        distance,
        half_t,
        max_t,
        point_index,
        depth + 1,
        tolerance,
      );
    }

    let next = distance + point_distance(cubic[0], cubic[3]);

    if next > distance {
      self.push(next, point_index, SegmentType::Cubic, max_t);
    }

    next
  }

  /// `distanceToSegment`: the segment `distance` falls in and the curve parameter there.
  fn distance_to_segment(&self, distance: f32) -> (usize, f32) {
    // `SkTKSearch`, folded to the insertion index where the distance is not found.
    let index = self
      .segments
      .partition_point(|segment| segment.distance < distance);
    let index = index.min(self.segments.len() - 1);
    let segment = &self.segments[index];
    let (mut start_t, mut start_d) = (0.0, 0.0);

    if index > 0 {
      let previous = &self.segments[index - 1];

      start_d = previous.distance;

      if previous.point_index == segment.point_index {
        start_t = previous.scalar_t();
      }
    }

    let t = start_t
      + (segment.scalar_t() - start_t) * (distance - start_d) / (segment.distance - start_d);

    (index, t)
  }

  /// `SkContourMeasure::getSegment`: the stretch from `start` to `stop` along the contour, added
  /// to `builder`.
  fn segment(
    &self,
    start: f32,
    stop: f32,
    builder: &mut PathBuilder,
    start_with_move_to: bool,
  ) -> bool {
    let start = start.max(0.0);
    let stop = stop.min(self.length);

    // Also refuses a NaN distance.
    if !matches!(
      start.partial_cmp(&stop),
      Some(Ordering::Less | Ordering::Equal)
    ) || self.segments.is_empty()
    {
      return false;
    }

    let (mut segment, start_t) = self.distance_to_segment(start);

    if !start_t.is_finite() {
      return false;
    }

    let (stop_segment, stop_t) = self.distance_to_segment(stop);

    if !stop_t.is_finite() {
      return false;
    }

    if start_with_move_to {
      let point = self.position(segment, start_t);

      builder.move_to(point.x, point.y);
    }

    let stop_index = self.segments[stop_segment].point_index;

    if self.segments[segment].point_index == stop_index {
      self.segment_to(segment, start_t, stop_t, builder);
    } else {
      let mut start_t = start_t;

      loop {
        self.segment_to(segment, start_t, 1.0, builder);
        segment = self.next_segment(segment);
        start_t = 0.0;

        if self.segments[segment].point_index >= stop_index {
          break;
        }
      }

      self.segment_to(segment, 0.0, stop_t, builder);
    }

    true
  }

  /// `SkContourMeasure::Segment::Next`: the first segment past the curve `index` belongs to.
  fn next_segment(&self, index: usize) -> usize {
    let point_index = self.segments[index].point_index;
    let mut next = index;

    while self.segments[next].point_index == point_index {
      next += 1;
    }

    next
  }

  /// The points of the curve segment `index` is a piece of, from its start.
  fn curve(&self, index: usize) -> &[Point] {
    &self.points[self.segments[index].point_index..]
  }

  /// `compute_pos_tan`'s position.
  fn position(&self, index: usize, t: f32) -> Point {
    let points = self.curve(index);

    match self.segments[index].kind {
      SegmentType::Line => Point::from_xy(
        interp(points[0].x, points[1].x, t),
        interp(points[0].y, points[1].y, t),
      ),
      SegmentType::Quad => eval_quad_at(&[points[0], points[1], points[2]], t),
      SegmentType::Cubic => eval_cubic_pos_at(&[points[0], points[1], points[2], points[3]], t),
    }
  }

  /// `SkContourMeasure_segTo`: the curve of segment `index` from `start_t` to `stop_t`.
  fn segment_to(&self, index: usize, start_t: f32, stop_t: f32, builder: &mut PathBuilder) {
    if start_t == stop_t {
      // A zero-length dash adds a zero-length line, which the stroker caps.
      if let Some(last) = builder.last_point() {
        builder.line_to(last.x, last.y);
      }

      return;
    }

    let points = self.curve(index);

    match self.segments[index].kind {
      SegmentType::Line => {
        if stop_t == 1.0 {
          builder.line_to(points[1].x, points[1].y);
        } else {
          builder.line_to(
            interp(points[0].x, points[1].x, stop_t),
            interp(points[0].y, points[1].y, stop_t),
          );
        }
      }
      SegmentType::Quad => {
        let quad = [points[0], points[1], points[2]];
        let [control, to] = if start_t == 0.0 {
          if stop_t == 1.0 {
            [quad[1], quad[2]]
          } else {
            let chopped = chop_quad_at(quad, stop_t);

            [chopped[1], chopped[2]]
          }
        } else {
          let chopped = chop_quad_at(quad, start_t);

          if stop_t == 1.0 {
            [chopped[3], chopped[4]]
          } else {
            let rest = chop_quad_at(
              [chopped[2], chopped[3], chopped[4]],
              (stop_t - start_t) / (1.0 - start_t),
            );

            [rest[1], rest[2]]
          }
        };

        builder.quad_to(control.x, control.y, to.x, to.y);
      }
      SegmentType::Cubic => {
        let cubic = [points[0], points[1], points[2], points[3]];
        let [first, second, to] = if start_t == 0.0 {
          if stop_t == 1.0 {
            [cubic[1], cubic[2], cubic[3]]
          } else {
            let chopped = chop_cubic_at(&cubic, stop_t);

            [chopped[1], chopped[2], chopped[3]]
          }
        } else {
          let chopped = chop_cubic_at(&cubic, start_t);

          if stop_t == 1.0 {
            [chopped[4], chopped[5], chopped[6]]
          } else {
            let rest = chop_cubic_at(
              &[chopped[3], chopped[4], chopped[5], chopped[6]],
              (stop_t - start_t) / (1.0 - start_t),
            );

            [rest[1], rest[2], rest[3]]
          }
        };

        builder.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
      }
    }
  }
}

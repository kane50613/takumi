//! Clipping of path segments to a rect before they become edges, a port of Skia's `SkEdgeClipper`
//! (`src/core/SkEdgeClipper.cpp`) and `SkLineClipper::ClipLine` (`src/core/SkLineClipper.cpp`).
//! Parts left or right of the clip become vertical lines on its edge, so the winding of what stays
//! inside is unchanged.

use takumi_core::{
  geometry::{Point, Rect},
  path::{chop_quad_at, cubic_split, find_unit_quad_roots},
};

use super::edge_builder::{
  chop_cubic_at_x_extrema, chop_cubic_at_y_extrema, chop_quad_at_x_extrema, chop_quad_at_y_extrema,
  transpose,
};

/// A clipped segment, as `SkEdgeClipper::next` returns it.
pub(super) enum Segment {
  Line(Point<f32>, Point<f32>),
  Quad([Point<f32>; 3]),
  Cubic([Point<f32>; 4]),
}

/// `SkEdgeClipper`: clips segments to `clip`, dropping what lies right of it when the fill
/// allows.
pub(super) struct EdgeClipper {
  clip: Rect<f32>,
  can_cull_to_the_right: bool,
}

/// `pin_unsorted` in `SkLineClipper.cpp`.
fn pin_unsorted(value: f64, limit0: f64, limit1: f64) -> f64 {
  value.clamp(limit0.min(limit1), limit0.max(limit1))
}

/// `SkScalarNearlyZero` with its default tolerance, `SK_ScalarNearlyZero`.
fn nearly_zero(value: f32) -> bool {
  value.abs() <= 1.0 / (1 << 12) as f32
}

/// `sect_with_horizontal`: the x where `p0`-`p1` crosses `y`.
fn sect_with_horizontal(p0: Point<f32>, p1: Point<f32>, y: f32) -> f32 {
  if nearly_zero(p1.y - p0.y) {
    return (p0.x + p1.x) * 0.5;
  }

  let (x0, y0, x1, y1) = (
    f64::from(p0.x),
    f64::from(p0.y),
    f64::from(p1.x),
    f64::from(p1.y),
  );

  pin_unsorted(x0 + (f64::from(y) - y0) * (x1 - x0) / (y1 - y0), x0, x1) as f32
}

/// `sect_with_vertical`: the y where `p0`-`p1` crosses `x`.
fn sect_with_vertical(p0: Point<f32>, p1: Point<f32>, x: f32) -> f32 {
  if nearly_zero(p1.x - p0.x) {
    return (p0.y + p1.y) * 0.5;
  }

  let (x0, y0, x1, y1) = (
    f64::from(p0.x),
    f64::from(p0.y),
    f64::from(p1.x),
    f64::from(p1.y),
  );

  (y0 + (f64::from(x) - x0) * (y1 - y0) / (x1 - x0)) as f32
}

/// `sect_clamp_with_vertical`.
fn sect_clamp_with_vertical(p0: Point<f32>, p1: Point<f32>, x: f32) -> f32 {
  pin_unsorted(
    f64::from(sect_with_vertical(p0, p1, x)),
    f64::from(p0.y),
    f64::from(p1.y),
  ) as f32
}

/// The segment flipped to run upwards when `reverse`.
fn oriented<const N: usize>(mut points: [Point<f32>; N], reverse: bool) -> [Point<f32>; N] {
  if reverse {
    points.reverse();
  }

  points
}

/// `chopMonoQuadAt`: where a monotonic quad reaches `target` on the axis of `c0`, `c1`, `c2`.
fn chop_mono_quad_at(c0: f32, c1: f32, c2: f32, target: f32) -> Option<f32> {
  let (roots, count) = find_unit_quad_roots(c0 - c1 - c1 + c2, 2.0 * (c1 - c0), c0 - target);

  (count > 0).then_some(roots[0])
}

/// `mono_cubic_closestT`, the fallback when the exact chop finds no root.
fn mono_cubic_closest_t(coords: [f32; 4], x: f32) -> f32 {
  let mut t = 0.5f32;
  let mut best_t = t;
  let mut step = 0.25f32;
  let d = coords[0];
  let a = coords[3] + 3.0 * (coords[1] - coords[2]) - d;
  let b = 3.0 * (coords[2] - coords[1] - coords[1] + d);
  let c = 3.0 * (coords[1] - d);
  let x = x - d;
  let mut closest = f32::MAX;

  loop {
    let location = ((a * t + b) * t + c) * t;
    let distance = (location - x).abs();

    if closest > distance {
      closest = distance;
      best_t = t;
    }

    let last_t = t;

    t += if location < x { step } else { -step };
    step *= 0.5;

    if closest <= 0.25 || last_t == t {
      return best_t;
    }
  }
}

/// `SkChopMonoCubicAtY` falling back to `mono_cubic_closestT`, as `chop_mono_cubic_at_y` does:
/// the cubic split where it reaches `y`, as seven points.
///
/// Approximate: Skia finds the root with `SkCubics::RootsValidT` and subdivides in doubles. This
/// bisects the monotonic polynomial in doubles instead, which can pick a split point a float bit
/// away.
fn chop_mono_cubic_at_y(src: [Point<f32>; 4], y: f32) -> [Point<f32>; 7] {
  let coords = src.map(|point| f64::from(point.y));
  let eval = |t: f64| {
    let u = 1.0 - t;

    u * u * u * coords[0]
      + 3.0 * u * u * t * coords[1]
      + 3.0 * u * t * t * coords[2]
      + t * t * t * coords[3]
  };
  let target = f64::from(y);
  let increasing = coords[3] >= coords[0];
  let in_range = (coords[0].min(coords[3])..=coords[0].max(coords[3])).contains(&target);

  if in_range {
    let (mut low, mut high) = (0.0f64, 1.0f64);

    for _ in 0..64 {
      let mid = (low + high) * 0.5;

      if (eval(mid) < target) == increasing {
        low = mid;
      } else {
        high = mid;
      }
    }

    let t = (low + high) * 0.5;
    let points = src.map(|point| (f64::from(point.x), f64::from(point.y)));
    let mix = |a: (f64, f64), b: (f64, f64)| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let ab = mix(points[0], points[1]);
    let bc = mix(points[1], points[2]);
    let cd = mix(points[2], points[3]);
    let abc = mix(ab, bc);
    let bcd = mix(bc, cd);
    let abcd = mix(abc, bcd);

    return [points[0], ab, abc, abcd, bcd, cd, points[3]]
      .map(|(x, y)| Point::new(x as f32, y as f32));
  }

  let t = mono_cubic_closest_t(src.map(|point| point.y), y);
  let [ab, _, cd, abc, bcd, abcd] = cubic_split(&src, t);

  [src[0], ab, abc, abcd, bcd, cd, src[3]]
}

/// `chop_mono_cubic_at_x`, through [`chop_mono_cubic_at_y`] on the transposed cubic.
fn chop_mono_cubic_at_x(src: [Point<f32>; 4], x: f32) -> [Point<f32>; 7] {
  chop_mono_cubic_at_y(src.map(transpose), x).map(transpose)
}

impl EdgeClipper {
  pub(super) fn new(clip: Rect<f32>, can_cull_to_the_right: bool) -> Self {
    Self {
      clip,
      can_cull_to_the_right,
    }
  }

  /// `appendVLine`.
  fn vertical_line(x: f32, y0: f32, y1: f32, reverse: bool, out: &mut Vec<Segment>) {
    let (y0, y1) = if reverse { (y1, y0) } else { (y0, y1) };

    out.push(Segment::Line(Point::new(x, y0), Point::new(x, y1)));
  }

  /// `SkLineClipper::ClipLine`, keeping the line's direction.
  pub(super) fn clip_line(&self, p0: Point<f32>, p1: Point<f32>, out: &mut Vec<Segment>) {
    let clip = self.clip;
    let (top_index, bottom_index) = if p0.y < p1.y { (0, 1) } else { (1, 0) };
    let points = [p0, p1];

    if points[bottom_index].y <= clip.top || points[top_index].y >= clip.bottom {
      return;
    }

    let mut chopped = points;

    if points[top_index].y < clip.top {
      chopped[top_index] = Point::new(sect_with_horizontal(p0, p1, clip.top), clip.top);
    }
    if chopped[bottom_index].y > clip.bottom {
      chopped[bottom_index] = Point::new(sect_with_horizontal(p0, p1, clip.bottom), clip.bottom);
    }

    let (left_index, right_index, reverse) = if p0.x < p1.x {
      (0, 1, false)
    } else {
      (1, 0, true)
    };
    let mut result: Vec<Point<f32>> = Vec::with_capacity(4);
    let mut reverse = reverse;

    if chopped[right_index].x <= clip.left {
      result.extend([
        Point::new(clip.left, chopped[0].y),
        Point::new(clip.left, chopped[1].y),
      ]);
      reverse = false;
    } else if chopped[left_index].x >= clip.right {
      if self.can_cull_to_the_right {
        return;
      }
      result.extend([
        Point::new(clip.right, chopped[0].y),
        Point::new(clip.right, chopped[1].y),
      ]);
      reverse = false;
    } else {
      let (left, right) = (chopped[left_index], chopped[right_index]);

      if left.x < clip.left {
        result.push(Point::new(clip.left, left.y));
        result.push(Point::new(
          clip.left,
          sect_clamp_with_vertical(chopped[0], chopped[1], clip.left),
        ));
      } else {
        result.push(left);
      }
      if right.x > clip.right {
        result.push(Point::new(
          clip.right,
          sect_clamp_with_vertical(chopped[0], chopped[1], clip.right),
        ));
        result.push(Point::new(clip.right, right.y));
      } else {
        result.push(right);
      }
    }

    if reverse {
      result.reverse();
    }
    for pair in result.windows(2) {
      out.push(Segment::Line(pair[0], pair[1]));
    }
  }

  /// `SkEdgeClipper::clipQuad`.
  pub(super) fn clip_quad(&self, src: [Point<f32>; 3], out: &mut Vec<Segment>) {
    let top = src
      .iter()
      .map(|point| point.y)
      .fold(f32::INFINITY, f32::min);
    let bottom = src
      .iter()
      .map(|point| point.y)
      .fold(f32::NEG_INFINITY, f32::max);

    if top >= self.clip.bottom || bottom <= self.clip.top {
      return;
    }

    for mono_y in chop_quad_at_y_extrema(src) {
      for mono in chop_quad_at_x_extrema(mono_y) {
        self.clip_mono_quad(mono, out);
      }
    }
  }

  /// `SkEdgeClipper::clipMonoQuad`.
  fn clip_mono_quad(&self, src: [Point<f32>; 3], out: &mut Vec<Segment>) {
    let clip = self.clip;
    let mut reverse = src[0].y > src[2].y;
    let mut pts = oriented(src, reverse);

    if pts[2].y <= clip.top || pts[0].y >= clip.bottom {
      return;
    }

    // `chop_quad_in_Y`.
    if pts[0].y < clip.top {
      match chop_mono_quad_at(pts[0].y, pts[1].y, pts[2].y, clip.top) {
        Some(t) => {
          let mut tmp = chop_quad_at(pts, t);

          tmp[2].y = clip.top;
          tmp[3].y = tmp[3].y.max(clip.top);
          pts[0] = tmp[2];
          pts[1] = tmp[3];
        }
        None => {
          for point in &mut pts {
            point.y = point.y.max(clip.top);
          }
        }
      }
    }
    if pts[2].y > clip.bottom {
      match chop_mono_quad_at(pts[0].y, pts[1].y, pts[2].y, clip.bottom) {
        Some(t) => {
          let mut tmp = chop_quad_at(pts, t);

          tmp[1].y = tmp[1].y.min(clip.bottom);
          tmp[2].y = clip.bottom;
          pts[1] = tmp[1];
          pts[2] = tmp[2];
        }
        None => {
          for point in &mut pts {
            point.y = point.y.min(clip.bottom);
          }
        }
      }
    }

    if pts[0].x > pts[2].x {
      pts.swap(0, 2);
      reverse = !reverse;
    }

    if pts[2].x <= clip.left {
      return Self::vertical_line(clip.left, pts[0].y, pts[2].y, reverse, out);
    }
    if pts[0].x >= clip.right {
      if !self.can_cull_to_the_right {
        Self::vertical_line(clip.right, pts[0].y, pts[2].y, reverse, out);
      }
      return;
    }

    if pts[0].x < clip.left {
      let Some(t) = chop_mono_quad_at(pts[0].x, pts[1].x, pts[2].x, clip.left) else {
        return Self::vertical_line(clip.left, pts[0].y, pts[2].y, reverse, out);
      };
      let mut tmp = chop_quad_at(pts, t);

      Self::vertical_line(clip.left, tmp[0].y, tmp[2].y, reverse, out);
      tmp[2].x = clip.left;
      tmp[3].x = tmp[3].x.max(clip.left);
      pts[0] = tmp[2];
      pts[1] = tmp[3];
    }

    if pts[2].x > clip.right {
      match chop_mono_quad_at(pts[0].x, pts[1].x, pts[2].x, clip.right) {
        Some(t) => {
          let mut tmp = chop_quad_at(pts, t);

          tmp[1].x = tmp[1].x.min(clip.right);
          tmp[2].x = clip.right;
          out.push(Segment::Quad(oriented([tmp[0], tmp[1], tmp[2]], reverse)));
          Self::vertical_line(clip.right, tmp[2].y, tmp[4].y, reverse, out);
        }
        None => {
          pts[1].x = pts[1].x.min(clip.right);
          pts[2].x = pts[2].x.min(clip.right);
          out.push(Segment::Quad(oriented(pts, reverse)));
        }
      }
    } else {
      out.push(Segment::Quad(oriented(pts, reverse)));
    }
  }

  /// `SkEdgeClipper::clipCubic`.
  pub(super) fn clip_cubic(&self, src: [Point<f32>; 4], out: &mut Vec<Segment>) {
    let top = src
      .iter()
      .map(|point| point.y)
      .fold(f32::INFINITY, f32::min);
    let bottom = src
      .iter()
      .map(|point| point.y)
      .fold(f32::NEG_INFINITY, f32::max);

    if !(bottom > self.clip.top && top < self.clip.bottom) {
      return;
    }

    // `too_big_for_reliable_float_math`: such a cubic is clipped as its chord.
    const LIMIT: f32 = (1 << 22) as f32;

    if src
      .iter()
      .any(|point| point.x.abs() > LIMIT || point.y.abs() > LIMIT)
    {
      return self.clip_line(src[0], src[3], out);
    }

    for mono_y in chop_cubic_at_y_extrema(src) {
      for mono in chop_cubic_at_x_extrema(mono_y) {
        self.clip_mono_cubic(mono, out);
      }
    }
  }

  /// `SkEdgeClipper::clipMonoCubic`.
  fn clip_mono_cubic(&self, src: [Point<f32>; 4], out: &mut Vec<Segment>) {
    let clip = self.clip;
    let mut reverse = src[0].y > src[3].y;
    let mut pts = oriented(src, reverse);

    if pts[3].y <= clip.top || pts[0].y >= clip.bottom {
      return;
    }

    // `chop_cubic_in_Y`.
    if pts[0].y < clip.top {
      let mut tmp = chop_mono_cubic_at_y(pts, clip.top);

      // A poor chop of a large cubic can leave its lower part above the clip, so it is chopped
      // again.
      if tmp[3].y < clip.top && tmp[4].y < clip.top && tmp[5].y < clip.top {
        tmp = chop_mono_cubic_at_y([tmp[3], tmp[4], tmp[5], tmp[6]], clip.top);
      }
      tmp[3].y = clip.top;
      tmp[4].y = tmp[4].y.max(clip.top);
      pts[0] = tmp[3];
      pts[1] = tmp[4];
      pts[2] = tmp[5];
    }
    if pts[3].y > clip.bottom {
      let mut tmp = chop_mono_cubic_at_y(pts, clip.bottom);

      tmp[3].y = clip.bottom;
      tmp[2].y = tmp[2].y.min(clip.bottom);
      pts[1] = tmp[1];
      pts[2] = tmp[2];
      pts[3] = tmp[3];
    }

    if pts[0].x > pts[3].x {
      pts.reverse();
      reverse = !reverse;
    }

    if pts[3].x <= clip.left {
      return Self::vertical_line(clip.left, pts[0].y, pts[3].y, reverse, out);
    }
    if pts[0].x >= clip.right {
      if !self.can_cull_to_the_right {
        Self::vertical_line(clip.right, pts[0].y, pts[3].y, reverse, out);
      }
      return;
    }

    if pts[0].x < clip.left {
      let mut tmp = chop_mono_cubic_at_x(pts, clip.left);

      Self::vertical_line(clip.left, tmp[0].y, tmp[3].y, reverse, out);
      tmp[3].x = clip.left;
      tmp[4].x = tmp[4].x.max(clip.left);
      pts[0] = tmp[3];
      pts[1] = tmp[4];
      pts[2] = tmp[5];
    }

    if pts[3].x > clip.right {
      let mut tmp = chop_mono_cubic_at_x(pts, clip.right);

      tmp[3].x = clip.right;
      tmp[2].x = tmp[2].x.min(clip.right);
      out.push(Segment::Cubic(oriented(
        [tmp[0], tmp[1], tmp[2], tmp[3]],
        reverse,
      )));
      Self::vertical_line(clip.right, tmp[3].y, tmp[6].y, reverse, out);
    } else {
      out.push(Segment::Cubic(oriented(pts, reverse)));
    }
  }
}

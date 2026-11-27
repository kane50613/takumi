//! Aliased path fills: Skia's `SkScan::FillPath` (`src/core/SkScan_Path.cpp`), walking the
//! `SkEdge`, `SkQuadraticEdge`, and `SkCubicEdge` of `src/core/SkEdge.cpp`. A convex path walks
//! through `walk_edges` too, where Skia takes `walk_simple_edges`, which fills the same spans.
//! Vertical lines also skip `SkBasicEdgeBuilder::combineVertical`, and the unmerged edges walk to
//! the same spans.

use std::{iter, mem};

use crate::style::FillRule;
use takumi_core::{
  geometry::{Point, Rect},
  path::Path,
};

use super::{
  blitter::IntBounds,
  edge::{
    FDot6, Fixed, MAX_COEFF_SHIFT, cubic_delta_from_line, diff_to_shift, fdot6_div, fdot6_round,
    fdot6_to_fixed, fixed_mul, fixed_to_fdot6, left_shift, round_to_int, scalar_to_fdot6,
  },
  edge_builder::{BuiltEdge, build_edges, is_convex},
  edge_clipper::EdgeClipper,
};

/// `kConservativeRoundBias` in `SkScan_Path.cpp`.
const CONSERVATIVE_ROUND_BIAS: f64 = 0.5 + 1.5 / 64.0;

/// `clip_to_limit`: edges are 16.16 fixed point, so the clip stays within this many pixels.
const CLIP_LIMIT: i32 = 32767 >> 1;

/// Fills `path` by `fill_rule` into `mask`, `bounds`' width wide, setting covered pixels to 255.
pub(super) fn fill_path(path: &Path, fill_rule: FillRule, bounds: IntBounds, mask: &mut [u8]) {
  let clip = bounds.intersect(IntBounds {
    left: -CLIP_LIMIT,
    top: -CLIP_LIMIT,
    right: CLIP_LIMIT,
    bottom: CLIP_LIMIT,
  });
  let ir = conservative_round_out(path.bounds());

  if ir.width() <= 0 || ir.height() <= 0 || clip.width() <= 0 || clip.height() <= 0 {
    return;
  }

  let contained = clip.contains(ir);
  let clipper = if contained {
    None
  } else {
    clip
      .to_rect()
      .map(|rect| EdgeClipper::new(rect, !is_convex(path)))
  };
  let mut edges: Vec<Edge> = build_edges(path, clipper.as_ref());

  if edges.is_empty() {
    return;
  }

  edges.sort_by(|a, b| {
    a.rows
      .first_y
      .cmp(&b.rows.first_y)
      .then(a.rows.x.cmp(&b.rows.x))
  });

  let (start_y, stop_y) = if contained {
    (ir.top, ir.bottom)
  } else {
    (ir.top.max(clip.top), ir.bottom.min(clip.bottom))
  };
  let mut span = |left: i32, y: i32, right: i32| {
    let (left, right) = (left.max(clip.left), right.min(clip.right));

    if left < right {
      let row = (y - bounds.top) as usize * bounds.width() as usize;

      mask[row + (left - bounds.left) as usize..row + (right - bounds.left) as usize].fill(255);
    }
  };

  walk_edges(edges, fill_rule, start_y, stop_y, clip.right, &mut span);
}

/// `conservative_round_to_int`: `bounds` rounded out a little past `SkRect::round`.
fn conservative_round_out(bounds: Rect<f32>) -> IntBounds {
  let down = |value: f32| (f64::from(value) - CONSERVATIVE_ROUND_BIAS).ceil() as i32;
  let up = |value: f32| (f64::from(value) + CONSERVATIVE_ROUND_BIAS).floor() as i32;

  IntBounds {
    left: down(bounds.left),
    top: down(bounds.top),
    right: up(bounds.right),
    bottom: up(bounds.bottom),
  }
}

/// `walk_edges`: the spans between `edges` on each row from `start_y` to `stop_y`, handed to
/// `span` as `left, y, right`, with an interval left open at `right_clip` closed there.
fn walk_edges(
  edges: Vec<Edge>,
  fill_rule: FillRule,
  start_y: i32,
  stop_y: i32,
  right_clip: i32,
  span: &mut impl FnMut(i32, i32, i32),
) {
  let winding_mask = if fill_rule == FillRule::EvenOdd {
    1
  } else {
    -1
  };
  let mut pending = edges.into_iter().peekable();
  let mut active: Vec<Edge> = Vec::new();
  let mut y = start_y;

  while y < stop_y {
    // The list stays sorted by x, ties in the order they arrived, as Skia's `insert_new_edges` and
    // `backward_insert_edge_based_on_x` keep it: the edges that crossed move back, then the
    // arrivals merge in.
    for index in 1..active.len() {
      let mut at = index;

      while at > 0 && active[at - 1].rows.x > active[at].rows.x {
        active.swap(at - 1, at);
        at -= 1;
      }
    }

    let mut arrivals: Vec<Edge> =
      iter::from_fn(|| pending.next_if(|edge| edge.rows.first_y <= y)).collect();

    if !arrivals.is_empty() {
      arrivals.sort_by_key(|edge| edge.rows.x);
      active = merge_by_x(mem::take(&mut active), arrivals);
    }

    let mut winding = 0;
    let mut left = 0;

    for edge in &active {
      let x = round_to_int(edge.rows.x);

      if winding & winding_mask == 0 {
        left = x;
      }

      winding += edge.winding;

      if winding & winding_mask == 0 && x > left {
        span(left, y, x);
      }
    }

    if winding & winding_mask != 0 {
      span(left, y, right_clip);
    }

    active.retain_mut(|edge| edge.advance(y));
    y += 1;
  }
}

/// `survivors` and `arrivals`, each sorted by x, merged into one list sorted by x with a survivor
/// ahead of an arrival at the same x.
fn merge_by_x(survivors: Vec<Edge>, arrivals: Vec<Edge>) -> Vec<Edge> {
  let mut merged = Vec::with_capacity(survivors.len() + arrivals.len());
  let mut arrivals = arrivals.into_iter().peekable();

  for survivor in survivors {
    while let Some(arrival) = arrivals.next_if(|arrival| arrival.rows.x < survivor.rows.x) {
      merged.push(arrival);
    }

    merged.push(survivor);
  }

  merged.extend(arrivals);

  merged
}

/// `SkEdge`: a monotonic piece of the outline stepped one row at a time.
pub(super) struct Edge {
  rows: Rows,
  winding: i32,
  curve: Curve,
}

/// The line an edge steps along now: its x at the center of its first row, its slope, and its
/// rows.
#[derive(Default)]
struct Rows {
  x: Fixed,
  dx: Fixed,
  first_y: i32,
  last_y: i32,
}

/// What an edge steps along in lines: a line, or `SkQuadraticEdge`'s or `SkCubicEdge`'s forward
/// differences.
enum Curve {
  Line,
  Quad(Quad),
  Cubic(Cubic),
}

struct Quad {
  segments: u8,
  shift: i32,
  x: Fixed,
  y: Fixed,
  dx: Fixed,
  dy: Fixed,
  ddx: Fixed,
  ddy: Fixed,
  last_x: Fixed,
  last_y: Fixed,
}

struct Cubic {
  segments: u8,
  step_exponent: i32,
  to_fixed_shift: i32,
  x: Fixed,
  y: Fixed,
  dx: Fixed,
  dy: Fixed,
  ddx: Fixed,
  ddy: Fixed,
  dddx: Fixed,
  dddy: Fixed,
  last_x: Fixed,
  last_y: Fixed,
}

impl Rows {
  /// The line through `(x0, y0)` and `(x1, y1)`, with `y0 <= y1`, as `SkEdge::setLine` steps it.
  /// None where it crosses no row center.
  fn new(x0: FDot6, y0: FDot6, x1: FDot6, y1: FDot6) -> Option<Self> {
    let top = fdot6_round(y0);
    let bottom = fdot6_round(y1);

    if top == bottom {
      return None;
    }

    let slope = fdot6_div(x1 - x0, y1 - y0);
    // `SkEdge_Compute_DY`.
    let dy = left_shift(top, 6) + 32 - y0;

    Some(Self {
      x: fdot6_to_fixed(x0.wrapping_add(fixed_mul(slope, dy))),
      dx: slope,
      first_y: top,
      last_y: bottom - 1,
    })
  }

  /// `SkEdge::updateLine`: moves onto the line from `(x0, y0)` to `(x1, y1)` in 16.16, false
  /// where it crosses no row center.
  fn update(&mut self, x0: Fixed, y0: Fixed, x1: Fixed, y1: Fixed) -> bool {
    match Self::new(
      fixed_to_fdot6(x0),
      fixed_to_fdot6(y0),
      fixed_to_fdot6(x1),
      fixed_to_fdot6(y1),
    ) {
      Some(rows) => {
        *self = rows;
        true
      }
      None => false,
    }
  }
}

impl Edge {
  /// An edge along `curve` in the direction `winding` gives, moved onto its first line.
  fn curve(winding: i32, mut curve: Curve) -> Option<Self> {
    let mut rows = Rows::default();

    curve.next_segment(&mut rows).then_some(Self {
      rows,
      winding,
      curve,
    })
  }

  /// Steps past row `y`, false once the edge ends there.
  fn advance(&mut self, y: i32) -> bool {
    if self.rows.last_y != y {
      self.rows.x = self.rows.x.wrapping_add(self.rows.dx);
      return true;
    }

    self.curve.next_segment(&mut self.rows)
  }
}

impl BuiltEdge for Edge {
  /// `SkEdge::setLine`.
  fn line(p0: Point<f32>, p1: Point<f32>) -> Option<Self> {
    let (mut x0, mut y0) = (scalar_to_fdot6(p0.x, 0), scalar_to_fdot6(p0.y, 0));
    let (mut x1, mut y1) = (scalar_to_fdot6(p1.x, 0), scalar_to_fdot6(p1.y, 0));
    let mut winding = 1;

    if y0 > y1 {
      (x0, x1, y0, y1) = (x1, x0, y1, y0);
      winding = -1;
    }

    Some(Self {
      rows: Rows::new(x0, y0, x1, y1)?,
      winding,
      curve: Curve::Line,
    })
  }

  /// `SkQuadraticEdge::setQuadratic`.
  fn quadratic(points: &[Point<f32>; 3]) -> Option<Self> {
    let [mut x0, x1, mut x2] = points.map(|point| scalar_to_fdot6(point.x, 0));
    let [mut y0, y1, mut y2] = points.map(|point| scalar_to_fdot6(point.y, 0));
    let mut winding = 1;

    if y0 > y2 {
      (x0, x2, y0, y2) = (x2, x0, y2, y0);
      winding = -1;
    }

    if fdot6_round(y0) == fdot6_round(y2) {
      return None;
    }

    let shift =
      diff_to_shift((2 * x1 - x0 - x2) >> 2, (2 * y1 - y0 - y2) >> 2, 0).clamp(1, MAX_COEFF_SHIFT);
    // `SkFDot6ToFixedDiv2`: A and B at half their value, so they fit in 16.16.
    let half = |value: FDot6| left_shift(value, 16 - 6 - 1);
    let (a_x, b_x) = (half(x0 - x1 - x1 + x2), fdot6_to_fixed(x1 - x0));
    let (a_y, b_y) = (half(y0 - y1 - y1 + y2), fdot6_to_fixed(y1 - y0));

    Self::curve(
      winding,
      Curve::Quad(Quad {
        segments: 1 << shift,
        shift: shift - 1,
        x: fdot6_to_fixed(x0),
        y: fdot6_to_fixed(y0),
        dx: b_x + (a_x >> shift),
        dy: b_y + (a_y >> shift),
        ddx: a_x >> (shift - 1),
        ddy: a_y >> (shift - 1),
        last_x: fdot6_to_fixed(x2),
        last_y: fdot6_to_fixed(y2),
      }),
    )
  }

  /// `SkCubicEdge::setCubic`.
  fn cubic(points: &[Point<f32>; 4]) -> Option<Self> {
    let [mut x0, mut x1, mut x2, mut x3] = points.map(|point| scalar_to_fdot6(point.x, 0));
    let [mut y0, mut y1, mut y2, mut y3] = points.map(|point| scalar_to_fdot6(point.y, 0));
    let mut winding = 1;

    if y0 > y3 {
      (x0, x1, x2, x3) = (x3, x2, x1, x0);
      (y0, y1, y2, y3) = (y3, y2, y1, y0);
      winding = -1;
    }

    if fdot6_round(y0) == fdot6_round(y3) {
      return None;
    }

    let step_exponent = (diff_to_shift(
      cubic_delta_from_line(x0, x1, x2, x3),
      cubic_delta_from_line(y0, y1, y2, y3),
      2,
    ) + 1)
      .min(MAX_COEFF_SHIFT);
    let mut up_scale = 6;
    let mut to_fixed_shift = step_exponent + up_scale - 10;

    if to_fixed_shift < 0 {
      to_fixed_shift = 0;
      up_scale = 10 - step_exponent;
    }

    // The forward differences of `A t³ + B t² + C t`, scaled up by `up_scale` bits.
    let differences = |p0: FDot6, p1: FDot6, p2: FDot6, p3: FDot6| {
      let a = left_shift(p3 + 3 * (p1 - p2) - p0, up_scale as u32);
      let b = left_shift(3 * (p0 - 2 * p1 + p2), up_scale as u32);
      let c = left_shift(3 * (p1 - p0), up_scale as u32);
      let a3 = a.wrapping_mul(3) >> (step_exponent - 1);

      (
        (a >> (2 * step_exponent))
          .wrapping_add(b >> step_exponent)
          .wrapping_add(c),
        a3.wrapping_add(b.wrapping_mul(2)),
        a3,
      )
    };
    let (dx, ddx, dddx) = differences(x0, x1, x2, x3);
    let (dy, ddy, dddy) = differences(y0, y1, y2, y3);

    Self::curve(
      winding,
      Curve::Cubic(Cubic {
        segments: 1 << step_exponent,
        step_exponent,
        to_fixed_shift,
        x: fdot6_to_fixed(x0),
        y: fdot6_to_fixed(y0),
        dx,
        dy,
        ddx,
        ddy,
        dddx,
        dddy,
        last_x: fdot6_to_fixed(x3),
        last_y: fdot6_to_fixed(y3),
      }),
    )
  }
}

impl Curve {
  /// `nextSegment`: moves `rows` onto the curve's next line that crosses a row center, false where
  /// none is left.
  fn next_segment(&mut self, rows: &mut Rows) -> bool {
    match self {
      Self::Line => false,
      Self::Quad(quad) => quad.next_segment(rows),
      Self::Cubic(cubic) => cubic.next_segment(rows),
    }
  }
}

impl Quad {
  /// `SkQuadraticEdge::nextSegment`.
  fn next_segment(&mut self, rows: &mut Rows) -> bool {
    let (mut x, mut y) = (self.x, self.y);
    let mut success = false;

    while self.segments > 0 && !success {
      self.segments -= 1;

      let (next_x, next_y) = if self.segments > 0 {
        let next = (
          x.wrapping_add(self.dx >> self.shift),
          y.wrapping_add(self.dy >> self.shift),
        );

        self.dx = self.dx.wrapping_add(self.ddx);
        self.dy = self.dy.wrapping_add(self.ddy);
        next
      } else {
        (self.last_x, self.last_y)
      };

      success = rows.update(x, y, next_x, next_y);
      (x, y) = (next_x, next_y);
    }

    (self.x, self.y) = (x, y);
    success
  }
}

impl Cubic {
  /// `SkCubicEdge::nextSegment`.
  fn next_segment(&mut self, rows: &mut Rows) -> bool {
    let (mut x, mut y) = (self.x, self.y);
    let mut success = false;

    while self.segments > 0 && !success {
      self.segments -= 1;

      let (next_x, next_y) = if self.segments > 0 {
        let next = (
          x.wrapping_add(self.dx >> self.to_fixed_shift),
          y.wrapping_add(self.dy >> self.to_fixed_shift),
        );

        self.dx = self.dx.wrapping_add(self.ddx >> self.step_exponent);
        self.ddx = self.ddx.wrapping_add(self.dddx);
        self.dy = self.dy.wrapping_add(self.ddy >> self.step_exponent);
        self.ddy = self.ddy.wrapping_add(self.dddy);
        next
      } else {
        (self.last_x, self.last_y)
      };
      let next_y = next_y.max(y);

      success = rows.update(x, y, next_x, next_y);
      (x, y) = (next_x, next_y);
    }

    (self.x, self.y) = (x, y);
    success
  }
}

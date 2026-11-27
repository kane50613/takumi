//! Line, quadratic and cubic edges walked by the analytic scan converter, a port of Skia's
//! `SkAnalyticEdge` (`src/core/SkAnalyticEdge.{h,cpp}`) over the 16.16 `SkFixed` and 26.6
//! `SkFDot6` arithmetic of `SkFixed.h` and `SkFDot6.h`.

use takumi_core::geometry::Point;

pub(super) type Fixed = i32;
pub(super) type FDot6 = i32;

pub(super) const FIXED_ONE: Fixed = 1 << 16;
/// `SK_MaxS32`.
pub(super) const MAX_S32: i32 = i32::MAX;
/// `SK_MinS32`, which Skia defines as `-SK_MaxS32`.
pub(super) const MIN_S32: i32 = -i32::MAX;

/// `SkAnalyticEdge::kDefaultAccuracy`: y snaps to a quarter pixel.
pub(super) const ACCURACY: u32 = 2;
/// `MAX_COEFF_SHIFT` in `SkAnalyticEdge.cpp`.
const MAX_COEFF_SHIFT: i32 = 6;

/// `SkLeftShift`.
pub(super) const fn left_shift(value: i32, shift: u32) -> i32 {
  ((value as u32) << shift) as i32
}

/// `SkFixedMul`.
pub(super) const fn fixed_mul(a: Fixed, b: Fixed) -> Fixed {
  ((a as i64 * b as i64) >> 16) as i32
}

/// `SkFixedDiv`.
const fn fixed_div(numer: i32, denom: i32) -> Fixed {
  let quotient = ((numer as i64) << 16) / denom as i64;

  if quotient > MAX_S32 as i64 {
    MAX_S32
  } else if quotient < MIN_S32 as i64 {
    MIN_S32
  } else {
    quotient as i32
  }
}

/// `SkFDot6Div`.
pub(super) const fn fdot6_div(a: FDot6, b: FDot6) -> Fixed {
  if a == a as i16 as i32 {
    left_shift(a, 16) / b
  } else {
    fixed_div(a, b)
  }
}

/// `SkFDot6ToFixed`.
pub(super) const fn fdot6_to_fixed(value: FDot6) -> Fixed {
  left_shift(value, 10)
}

/// `SkFixedToFDot6`.
pub(super) const fn fixed_to_fdot6(value: Fixed) -> FDot6 {
  value >> 10
}

/// `SkFDot6Round`.
pub(super) const fn fdot6_round(value: FDot6) -> i32 {
  (value + 32) >> 6
}

/// `SkIntToFixed`.
pub(super) const fn int_to_fixed(value: i32) -> Fixed {
  left_shift(value, 16)
}

/// `SkFixedFloorToInt`.
pub(super) const fn floor_to_int(value: Fixed) -> i32 {
  value >> 16
}

/// `SkFixedCeilToInt`.
pub(super) const fn ceil_to_int(value: Fixed) -> i32 {
  value.wrapping_add(FIXED_ONE - 1) >> 16
}

/// `SkFixedRoundToInt`.
pub(super) const fn round_to_int(value: Fixed) -> i32 {
  value.wrapping_add(FIXED_ONE >> 1) >> 16
}

/// `SkFixedFloorToFixed`.
pub(super) const fn floor_to_fixed(value: Fixed) -> Fixed {
  value & !(FIXED_ONE - 1)
}

/// `SkFixedCeilToFixed`.
pub(super) const fn ceil_to_fixed(value: Fixed) -> Fixed {
  value.wrapping_add(FIXED_ONE - 1) & !(FIXED_ONE - 1)
}

/// `SkFixedRoundToFixed`.
const fn round_to_fixed(value: Fixed) -> Fixed {
  value.wrapping_add(FIXED_ONE >> 1) & !(FIXED_ONE - 1)
}

/// `SkAnalyticEdge::SnapY`.
pub(super) const fn snap_y(y: Fixed) -> Fixed {
  let shift = 16 - ACCURACY;

  ((((y as u32).wrapping_add((FIXED_ONE >> (ACCURACY + 1)) as u32)) >> shift) << shift) as i32
}

/// `quick_inverse` in `SkAnalyticEdge.cpp`, whose table holds `(1 << 22) / x` truncated.
const fn quick_inverse(x: FDot6) -> Fixed {
  if x == 0 { 0 } else { (1 << 22) / x }
}

/// `kInverseTableSize` in `SkAnalyticEdge.cpp`.
const INVERSE_TABLE_SIZE: i32 = 1024;

/// `quick_div` in `SkAnalyticEdge.cpp`.
const fn quick_div(a: FDot6, b: FDot6) -> Fixed {
  const MIN_BITS: u32 = 3;
  const MAX_ABS_A: i32 = 1 << (31 - (22 - MIN_BITS));

  let abs_b = b.abs();

  if abs_b >= (1 << MIN_BITS) && abs_b < INVERSE_TABLE_SIZE && a.abs() < MAX_ABS_A {
    return (a * quick_inverse(b)) >> 6;
  }

  fdot6_div(a, b)
}

/// `fDY` of `SkAnalyticEdge::setLine` and `updateLine`.
const fn inverse_slope(dx: FDot6, dy: FDot6, slope: Fixed, abs_slope: i32) -> Fixed {
  if dx == 0 || slope == 0 {
    MAX_S32
  } else if abs_slope < INVERSE_TABLE_SIZE {
    quick_inverse(abs_slope)
  } else {
    quick_div(dy, dx).abs()
  }
}

/// `SkScalarToFDot6` of `value` scaled by `1 << shift`.
pub(super) fn scalar_to_fdot6(value: f32, shift: u32) -> FDot6 {
  (value * (1 << (shift + 6)) as f32) as i32
}

/// `cheap_distance` in `SkAnalyticEdge.cpp`.
const fn cheap_distance(dx: FDot6, dy: FDot6) -> FDot6 {
  let (dx, dy) = (dx.abs(), dy.abs());

  if dx > dy {
    dx + (dy >> 1)
  } else {
    dy + (dx >> 1)
  }
}

/// `diff_to_shift` in `SkAnalyticEdge.cpp`.
const fn diff_to_shift(dx: FDot6, dy: FDot6, shift_aa: u32) -> i32 {
  let dist = (cheap_distance(dx, dy) + (1 << (2 + shift_aa))) >> (3 + shift_aa);

  (32 - dist.leading_zeros() as i32) >> 1
}

/// `cubic_delta_from_line` in `SkAnalyticEdge.cpp`.
const fn cubic_delta_from_line(a: FDot6, b: FDot6, c: FDot6, d: FDot6) -> FDot6 {
  let one_third = ((a * 8 - b * 15 + 6 * c + d) * 19) >> 9;
  let two_third = ((a + 6 * b - c * 15 + d * 8) * 19) >> 9;

  let (one_third, two_third) = (one_third.abs(), two_third.abs());

  if one_third > two_third {
    one_third
  } else {
    two_third
  }
}

/// `SkAnalyticEdge::Type`, carrying the forward-differencing state of curve edges.
#[derive(Clone, Copy)]
pub(super) enum Curve {
  Line,
  Quad(QuadState),
  Cubic(CubicState),
}

/// The `SkAnalyticQuadraticEdge` fields.
#[derive(Clone, Copy, Default)]
pub(super) struct QuadState {
  qx: Fixed,
  qy: Fixed,
  pub(super) qdx: Fixed,
  pub(super) qdy: Fixed,
  pub(super) qddx: Fixed,
  pub(super) qddy: Fixed,
  qlast_x: Fixed,
  qlast_y: Fixed,
  snapped_x: Fixed,
  snapped_y: Fixed,
}

/// The `SkAnalyticCubicEdge` fields.
#[derive(Clone, Copy, Default)]
pub(super) struct CubicState {
  cx: Fixed,
  cy: Fixed,
  pub(super) cdx: Fixed,
  pub(super) cdy: Fixed,
  pub(super) cddx: Fixed,
  pub(super) cddy: Fixed,
  cdddx: Fixed,
  cdddy: Fixed,
  clast_x: Fixed,
  clast_y: Fixed,
  snapped_y: Fixed,
  pub(super) to_fixed_shift: u8,
}

/// `SkAnalyticEdge`, linked by index into the edge list of [`super::scan`].
#[derive(Clone, Copy)]
pub(super) struct Edge {
  pub(super) next: u32,
  pub(super) prev: u32,
  pub(super) x: Fixed,
  pub(super) dx: Fixed,
  pub(super) upper_x: Fixed,
  pub(super) y: Fixed,
  pub(super) upper_y: Fixed,
  pub(super) lower_y: Fixed,
  pub(super) dy: Fixed,
  /// `fCurveCount`: positive for quads, negative for cubics, zero once a line.
  pub(super) curve_count: i8,
  pub(super) curve_shift: u8,
  pub(super) winding: i8,
  /// Whether the edge started as a line, `fEdgeType == kLine`.
  pub(super) is_line: bool,
  pub(super) curve: Curve,
}

impl Edge {
  /// A head or tail sentinel of the edge list.
  pub(super) const fn sentinel(y: Fixed, x: Fixed) -> Self {
    Self {
      next: u32::MAX,
      prev: u32::MAX,
      x,
      dx: 0,
      upper_x: x,
      y: 0,
      upper_y: y,
      lower_y: y,
      dy: MAX_S32,
      curve_count: 0,
      curve_shift: 0,
      winding: 1,
      is_line: true,
      curve: Curve::Line,
    }
  }

  /// `SkAnalyticEdge::setLine`.
  pub(super) fn line(p0: Point<f32>, p1: Point<f32>) -> Option<Self> {
    let multiplier = (1 << ACCURACY) as f32;
    let mut x0 = fdot6_to_fixed(scalar_to_fdot6(p0.x * multiplier, 0)) >> ACCURACY;
    let mut y0 = snap_y(fdot6_to_fixed(scalar_to_fdot6(p0.y * multiplier, 0)) >> ACCURACY);
    let mut x1 = fdot6_to_fixed(scalar_to_fdot6(p1.x * multiplier, 0)) >> ACCURACY;
    let mut y1 = snap_y(fdot6_to_fixed(scalar_to_fdot6(p1.y * multiplier, 0)) >> ACCURACY);

    let mut winding = 1;

    if y0 > y1 {
      (x0, x1) = (x1, x0);
      (y0, y1) = (y1, y0);
      winding = -1;
    }

    let dy = fixed_to_fdot6(y1 - y0);

    if dy == 0 {
      return None;
    }

    let dx = fixed_to_fdot6(x1 - x0);
    let slope = quick_div(dx, dy);

    Some(Self {
      next: u32::MAX,
      prev: u32::MAX,
      x: x0,
      dx: slope,
      upper_x: x0,
      y: y0,
      upper_y: y0,
      lower_y: y1,
      dy: inverse_slope(dx, dy, slope, slope.abs()),
      curve_count: 0,
      curve_shift: 0,
      winding,
      is_line: true,
      curve: Curve::Line,
    })
  }

  /// `SkAnalyticEdge::updateLine`.
  fn update_line(
    &mut self,
    mut x0: Fixed,
    mut y0: Fixed,
    mut x1: Fixed,
    mut y1: Fixed,
    slope: Fixed,
  ) -> bool {
    if y0 > y1 {
      (x0, x1) = (x1, x0);
      (y0, y1) = (y1, y0);
      self.winding = -self.winding;
    }

    let dx = fixed_to_fdot6(x1.wrapping_sub(x0));
    let dy = fixed_to_fdot6(y1 - y0);

    if dy == 0 {
      return false;
    }

    self.x = x0;
    self.dx = slope;
    self.upper_x = x0;
    self.y = y0;
    self.upper_y = y0;
    self.lower_y = y1;
    self.dy = inverse_slope(dx, dy, slope, fixed_to_fdot6(slope).abs());

    true
  }

  /// `SkAnalyticEdge::goY(SkFixed)`.
  pub(super) fn go_y(&mut self, y: Fixed) {
    if y == self.y.wrapping_add(FIXED_ONE) {
      self.x = self.x.wrapping_add(self.dx);
      self.y = y;
    } else if y != self.y {
      self.x = self
        .upper_x
        .wrapping_add(fixed_mul(self.dx, y.wrapping_sub(self.upper_y)));
      self.y = y;
    }
  }

  /// `SkAnalyticEdge::goY(SkFixed, int)`.
  pub(super) fn go_y_shift(&mut self, y: Fixed, y_shift: u32) {
    self.y = y;
    self.x = self.x.wrapping_add(self.dx >> y_shift);
  }

  /// `SkAnalyticEdge::update`: steps a curve edge to its next line, false once it ends.
  pub(super) fn update(&mut self) -> bool {
    match self.curve_count.signum() {
      -1 => self.update_cubic(),
      1 => self.update_quadratic(),
      _ => false,
    }
  }

  /// `keepContinuous` of `SkAnalyticQuadraticEdge` and `SkAnalyticCubicEdge`.
  pub(super) fn keep_continuous(&mut self) {
    match &mut self.curve {
      Curve::Line => {}
      Curve::Quad(quad) => {
        quad.snapped_x = self.x;
        quad.snapped_y = self.y;
      }
      Curve::Cubic(cubic) => {
        cubic.cx = self.x;
        cubic.snapped_y = self.y;
      }
    }
  }

  /// `SkAnalyticQuadraticEdge::setQuadratic` for a y-monotonic quad.
  pub(super) fn quadratic(points: &[Point<f32>; 3]) -> Option<Self> {
    let shift = ACCURACY;
    let mut x0 = scalar_to_fdot6(points[0].x, shift);
    let mut y0 = scalar_to_fdot6(points[0].y, shift);
    let x1 = scalar_to_fdot6(points[1].x, shift);
    let y1 = scalar_to_fdot6(points[1].y, shift);
    let mut x2 = scalar_to_fdot6(points[2].x, shift);
    let mut y2 = scalar_to_fdot6(points[2].y, shift);

    let mut winding = 1;

    if y0 > y2 {
      (x0, x2) = (x2, x0);
      (y0, y2) = (y2, y0);
      winding = -1;
    }

    if fdot6_round(y0) == fdot6_round(y2) {
      return None;
    }

    let dx = (left_shift(x1, 1) - x0 - x2) >> 2;
    let dy = (left_shift(y1, 1) - y0 - y2) >> 2;
    let curve_shift = diff_to_shift(dx, dy, shift).clamp(1, MAX_COEFF_SHIFT) as u32;

    // A and B are stored at half their value, so `fCurveShift` is one less than the shift.
    let a = left_shift(x0 - x1 - x1 + x2, 16 - 6 - 1);
    let b = fdot6_to_fixed(x1 - x0);
    let qx = fdot6_to_fixed(x0);
    let qdx = b + (a >> curve_shift);
    let qddx = a >> (curve_shift - 1);

    let a = left_shift(y0 - y1 - y1 + y2, 16 - 6 - 1);
    let b = fdot6_to_fixed(y1 - y0);
    let qy = fdot6_to_fixed(y0);
    let qdy = b + (a >> curve_shift);
    let qddy = a >> (curve_shift - 1);

    let qy = snap_y(qy >> ACCURACY);
    let qx = qx >> ACCURACY;

    let quad = QuadState {
      qx,
      qy,
      qdx: qdx >> ACCURACY,
      qdy: qdy >> ACCURACY,
      qddx: qddx >> ACCURACY,
      qddy: qddy >> ACCURACY,
      qlast_x: fdot6_to_fixed(x2) >> ACCURACY,
      qlast_y: snap_y(fdot6_to_fixed(y2) >> ACCURACY),
      snapped_x: qx,
      snapped_y: qy,
    };

    let mut edge = Self {
      curve_count: (1 << curve_shift) as i8,
      curve_shift: (curve_shift - 1) as u8,
      winding,
      is_line: false,
      curve: Curve::Quad(quad),
      ..Self::sentinel(0, 0)
    };

    edge.update_quadratic().then_some(edge)
  }

  /// `SkAnalyticQuadraticEdge::updateQuadratic`.
  pub(super) fn update_quadratic(&mut self) -> bool {
    let Curve::Quad(mut quad) = self.curve else {
      return false;
    };

    let mut success = false;
    let mut count = self.curve_count as i32;
    let mut old_x = quad.qx;
    let mut old_y = quad.qy;
    let mut dx = quad.qdx;
    let mut dy = quad.qdy;
    let shift = self.curve_shift as u32;
    let (mut new_x, mut new_y, mut new_snapped_x, mut new_snapped_y);

    loop {
      let slope;
      count -= 1;

      if count > 0 {
        new_x = old_x + (dx >> shift);
        new_y = old_y + (dy >> shift);

        if (dy >> shift).abs() >= FIXED_ONE * 2 && (i64::from(dy.abs()) << 6) > i64::from(dx.abs())
        {
          let diff_y = fixed_to_fdot6(new_y - quad.snapped_y);

          slope = if diff_y != 0 {
            quick_div(fixed_to_fdot6(new_x - quad.snapped_x), diff_y)
          } else {
            MAX_S32
          };
          new_snapped_y = quad.qlast_y.min(round_to_fixed(new_y));
          new_snapped_x = new_x - fixed_mul(slope, new_y - new_snapped_y);
        } else {
          new_snapped_y = quad.qlast_y.min(snap_y(new_y));
          new_snapped_x = new_x;

          let diff_y = fixed_to_fdot6(new_snapped_y - quad.snapped_y);

          slope = if diff_y != 0 {
            quick_div(fixed_to_fdot6(new_x - quad.snapped_x), diff_y)
          } else {
            MAX_S32
          };
        }
        dx += quad.qddx;
        dy += quad.qddy;
      } else {
        new_x = quad.qlast_x;
        new_y = quad.qlast_y;
        new_snapped_y = new_y;
        new_snapped_x = new_x;

        let diff_y = fixed_to_fdot6(new_y - quad.snapped_y);

        slope = if diff_y != 0 {
          quick_div(fixed_to_fdot6(new_x - quad.snapped_x), diff_y)
        } else {
          MAX_S32
        };
      }

      if slope < MAX_S32 {
        success = self.update_line(
          quad.snapped_x,
          quad.snapped_y,
          new_snapped_x,
          new_snapped_y,
          slope,
        );
      }
      old_x = new_x;
      old_y = new_y;

      if count <= 0 || success {
        break;
      }
    }

    quad.qx = new_x;
    quad.qy = new_y;
    quad.qdx = dx;
    quad.qdy = dy;
    quad.snapped_x = new_snapped_x;
    quad.snapped_y = new_snapped_y;
    self.curve = Curve::Quad(quad);
    self.curve_count = count as i8;

    success
  }

  /// `SkAnalyticCubicEdge::setCubic` for a y-monotonic cubic.
  pub(super) fn cubic(points: &[Point<f32>; 4]) -> Option<Self> {
    let shift = ACCURACY;
    let mut x0 = scalar_to_fdot6(points[0].x, shift);
    let mut y0 = scalar_to_fdot6(points[0].y, shift);
    let mut x1 = scalar_to_fdot6(points[1].x, shift);
    let mut y1 = scalar_to_fdot6(points[1].y, shift);
    let mut x2 = scalar_to_fdot6(points[2].x, shift);
    let mut y2 = scalar_to_fdot6(points[2].y, shift);
    let mut x3 = scalar_to_fdot6(points[3].x, shift);
    let mut y3 = scalar_to_fdot6(points[3].y, shift);

    let mut winding = 1;

    if y0 > y3 {
      (x0, x3) = (x3, x0);
      (x1, x2) = (x2, x1);
      (y0, y3) = (y3, y0);
      (y1, y2) = (y2, y1);
      winding = -1;
    }

    if fdot6_round(y0) == fdot6_round(y3) {
      return None;
    }

    let dx = cubic_delta_from_line(x0, x1, x2, x3);
    let dy = cubic_delta_from_line(y0, y1, y2, y3);
    let curve_shift = (diff_to_shift(dx, dy, 2) + 1).min(MAX_COEFF_SHIFT) as u32;

    // The input is shifted down by 10 (8 with anti-aliasing) and the coefficients carry a 3×, so
    // the safest upshift is 6.
    let mut up_shift = 6;
    let mut down_shift = curve_shift as i32 + up_shift - 10;

    if down_shift < 0 {
      down_shift = 0;
      up_shift = 10 - curve_shift as i32;
    }

    let up_shift = up_shift as u32;
    let coefficients = |p0: i32, p1: i32, p2: i32, p3: i32| {
      let b = left_shift(3 * (p1 - p0), up_shift);
      let c = left_shift(3 * (p0 - p1 - p1 + p2), up_shift);
      let d = left_shift(p3 + 3 * (p1 - p2) - p0, up_shift);

      let first = b + (c >> curve_shift) + (d >> (2 * curve_shift));
      let second = (2 * i64::from(c) + ((3 * i64::from(d)) >> (curve_shift - 1))) as i32;
      let third = ((3 * i64::from(d)) >> (curve_shift - 1)) as i32;

      (fdot6_to_fixed(p0), first, second, third)
    };

    let (cx, cdx, cddx, cdddx) = coefficients(x0, x1, x2, x3);
    let (cy, cdy, cddy, cdddy) = coefficients(y0, y1, y2, y3);
    let cy = snap_y(cy >> ACCURACY);

    let cubic = CubicState {
      cx: cx >> ACCURACY,
      cy,
      cdx: cdx >> ACCURACY,
      cdy: cdy >> ACCURACY,
      cddx: cddx >> ACCURACY,
      cddy: cddy >> ACCURACY,
      cdddx: cdddx >> ACCURACY,
      cdddy: cdddy >> ACCURACY,
      clast_x: fdot6_to_fixed(x3) >> ACCURACY,
      clast_y: snap_y(fdot6_to_fixed(y3) >> ACCURACY),
      snapped_y: cy,
      to_fixed_shift: down_shift as u8,
    };

    let mut edge = Self {
      curve_count: left_shift(-1, curve_shift) as i8,
      curve_shift: curve_shift as u8,
      winding,
      is_line: false,
      curve: Curve::Cubic(cubic),
      ..Self::sentinel(0, 0)
    };

    edge.update_cubic().then_some(edge)
  }

  /// `SkAnalyticCubicEdge::updateCubic`.
  pub(super) fn update_cubic(&mut self) -> bool {
    let Curve::Cubic(mut cubic) = self.curve else {
      return false;
    };

    let mut success;
    let mut count = self.curve_count as i32;
    let mut old_x = cubic.cx;
    let mut old_y = cubic.cy;
    let dd_shift = self.curve_shift as u32;
    let d_shift = cubic.to_fixed_shift as u32;
    let (mut new_x, mut new_y);

    loop {
      count += 1;

      if count < 0 {
        new_x = old_x + (cubic.cdx >> d_shift);
        cubic.cdx += cubic.cddx >> dd_shift;
        cubic.cddx += cubic.cdddx;

        new_y = old_y + (cubic.cdy >> d_shift);
        cubic.cdy += cubic.cddy >> dd_shift;
        cubic.cddy += cubic.cdddy;
      } else {
        new_x = cubic.clast_x;
        new_y = cubic.clast_y;
      }

      // Finite fixed point does not always keep y increasing, so it is pinned here.
      if new_y < old_y {
        new_y = old_y;
      }

      let mut new_snapped_y = snap_y(new_y);

      if cubic.clast_y < new_snapped_y {
        new_snapped_y = cubic.clast_y;
        count = 0;
      }

      let slope = if fixed_to_fdot6(new_snapped_y - cubic.snapped_y) == 0 {
        MAX_S32
      } else {
        fdot6_div(
          fixed_to_fdot6(new_x - old_x),
          fixed_to_fdot6(new_snapped_y - cubic.snapped_y),
        )
      };

      success = self.update_line(old_x, cubic.snapped_y, new_x, new_snapped_y, slope);

      old_x = new_x;
      old_y = new_y;
      cubic.snapped_y = new_snapped_y;

      if count >= 0 || success {
        break;
      }
    }

    cubic.cx = new_x;
    cubic.cy = new_y;
    self.curve = Curve::Cubic(cubic);
    self.curve_count = count as i8;

    success
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn quick_inverse_matches_skia_table() {
    // Entries of `quick_inverse`'s table in `SkAnalyticEdge.cpp`, indexed by `x + 1024`.
    for (x, expected) in [
      (-1024, -4096),
      (-1009, -4156),
      (-1008, -4161),
      (-1, -4194304),
      (0, 0),
    ] {
      assert_eq!(quick_inverse(x), expected);
      assert_eq!(quick_inverse(-x), -expected);
    }
  }

  #[test]
  fn snap_y_rounds_to_quarter_pixels() {
    assert_eq!(snap_y(FIXED_ONE / 8 - 1), 0);
    assert_eq!(snap_y(FIXED_ONE / 8), FIXED_ONE / 4);
  }
}

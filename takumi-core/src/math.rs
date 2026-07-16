//! Deterministic float math for painting. `std` trigonometry routes to the
//! platform's libm, whose results differ across OSes by a ulp and shift golden
//! pixels. These implementations use only IEEE-exact operations (add, mul,
//! sqrt, round), so every platform renders identical output. Polynomial
//! coefficients follow Cephes `sinf`/`cosf`/`atanf`; worst-case error is well
//! under 1e-6, far below 8-bit color resolution.

use std::f32::consts::FRAC_PI_2;

// π/2 split into exactly-representable high and low parts for accurate
// argument reduction.
const PIO2_HI: f32 = 1.570_796_2;
const PIO2_LO: f32 = 7.549_79e-8;

fn sin_poly(r: f32) -> f32 {
  let z = r * r;
  r + r * z * (-1.666_665_5e-1 + z * (8.332_161e-3 + z * -1.951_529_6e-4))
}

fn cos_poly(r: f32) -> f32 {
  let z = r * r;
  1.0 - 0.5 * z + z * z * (4.166_664_6e-2 + z * (-1.388_731_6e-3 + z * 2.443_315_7e-5))
}

fn sin_cos(x: f32) -> (f32, f32) {
  if !x.is_finite() {
    return (f32::NAN, f32::NAN);
  }

  let quadrants = (x * (1.0 / FRAC_PI_2)).round();
  let r = x - quadrants * PIO2_HI - quadrants * PIO2_LO;
  let (s, c) = (sin_poly(r), cos_poly(r));

  match (quadrants as i64).rem_euclid(4) {
    0 => (s, c),
    1 => (c, -s),
    2 => (-s, -c),
    _ => (-c, s),
  }
}

/// `f32::sin` with platform-independent results.
pub fn sin(x: f32) -> f32 {
  sin_cos(x).0
}

/// `f32::cos` with platform-independent results.
pub fn cos(x: f32) -> f32 {
  sin_cos(x).1
}

/// `f32::tan` with platform-independent results.
pub fn tan(x: f32) -> f32 {
  let (s, c) = sin_cos(x);
  s / c
}

fn atan_poly(x: f32) -> f32 {
  let z = x * x;
  x + x * z * (-3.333_295e-1 + z * (1.997_771e-1 + z * (-1.387_768_6e-1 + z * 8.053_744_5e-2)))
}

fn atan(x: f32) -> f32 {
  use std::f32::consts::FRAC_PI_4;

  let negative = x < 0.0;
  let x = x.abs();
  // tan(3π/8) and tan(π/8): reduce into the poly's accurate range [0, tan(π/8)].
  let (offset, reduced) = if x > 2.414_213_5 {
    (FRAC_PI_2, -1.0 / x)
  } else if x > 0.414_213_56 {
    (FRAC_PI_4, (x - 1.0) / (x + 1.0))
  } else {
    (0.0, x)
  };

  let result = offset + atan_poly(reduced);
  if negative { -result } else { result }
}

/// `f32::atan2` with platform-independent results.
pub fn atan2(y: f32, x: f32) -> f32 {
  use std::f32::consts::PI;

  if x == 0.0 && y == 0.0 {
    return 0.0;
  }
  if x == 0.0 {
    return if y > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
  }

  let base = atan(y / x);
  if x > 0.0 {
    base
  } else if y >= 0.0 {
    base + PI
  } else {
    base - PI
  }
}

/// `f64::atan2` with platform-independent results, at `f32` precision.
pub fn atan2_f64(y: f64, x: f64) -> f64 {
  atan2(y as f32, x as f32) as f64
}

/// Angle of `(x, y)` as a fraction of a full turn in `[0, 1)`:
/// `atan2(y, x) / τ` with negative angles wrapped into the upper half. This is
/// Skia's `xy_to_unit_angle` sweep-gradient polynomial (fpminimax of
/// `atan(t)/2π` over one octant), so it is both deterministic and cheaper than
/// a libm `atan2` per pixel.
pub fn xy_to_unit_angle(x: f32, y: f32) -> f32 {
  let x_abs = x.abs();
  let y_abs = y.abs();
  let max = x_abs.max(y_abs);
  if max == 0.0 {
    return 0.0;
  }

  let slope = x_abs.min(y_abs) / max;
  let s = slope * slope;
  let mut phi =
    slope * (1.591_211_7e-1 + s * (-5.185_397e-2 + s * (2.476_102e-2 + s * -7.054_738e-3)));

  if x_abs < y_abs {
    phi = 0.25 - phi;
  }
  if x < 0.0 {
    phi = 0.5 - phi;
  }
  if y < 0.0 {
    phi = 1.0 - phi;
  }
  if phi >= 1.0 { 0.0 } else { phi }
}

/// `f32::hypot` with platform-independent results. Plain
/// `sqrt(x * x + y * y)`: painting operates on pixel-scale magnitudes, so the
/// overflow guards of a real `hypot` are unnecessary.
pub fn hypot(x: f32, y: f32) -> f32 {
  (x * x + y * y).sqrt()
}

#[cfg(test)]
mod tests {
  use std::f32::consts::{PI, TAU};

  fn assert_close(actual: f32, expected: f32) {
    assert!(
      (actual - expected).abs() <= 2e-6,
      "expected {expected}, got {actual}"
    );
  }

  #[test]
  fn trig_matches_std_within_tolerance() {
    let mut x = -2.0 * TAU;
    while x <= 2.0 * TAU {
      assert_close(super::sin(x), x.sin());
      assert_close(super::cos(x), x.cos());
      x += 1e-3;
    }
  }

  #[test]
  fn tan_matches_std_away_from_poles() {
    let mut x = -1.4;
    while x <= 1.4 {
      assert!((super::tan(x) - x.tan()).abs() <= 1e-4 * (1.0 + x.tan().abs()));
      x += 1e-3;
    }
  }

  #[test]
  fn atan2_matches_std_within_tolerance() {
    let values = [-10.0, -2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, 10.0];
    for &y in &values {
      for &x in &values {
        if x == 0.0 && y == 0.0 {
          continue;
        }
        assert_close(super::atan2(y, x), y.atan2(x));
      }
    }
  }

  #[test]
  fn atan2_quadrant_boundaries() {
    assert_close(super::atan2(0.0, 1.0), 0.0);
    assert_close(super::atan2(1.0, 0.0), PI / 2.0);
    assert_close(super::atan2(0.0, -1.0), PI);
    assert_close(super::atan2(-1.0, 0.0), -PI / 2.0);
  }

  #[test]
  fn unit_angle_matches_atan2() {
    let values = [-10.0_f32, -2.0, -1.0, -0.5, -0.1, 0.1, 0.5, 1.0, 2.0, 10.0];
    for &y in &values {
      for &x in &values {
        let expected = y.atan2(x).rem_euclid(TAU) / TAU;
        let actual = super::xy_to_unit_angle(x, y);
        let distance = (actual - expected).abs();
        let distance = distance.min(1.0 - distance);
        assert!(
          distance <= 5e-5,
          "({x}, {y}): expected {expected}, got {actual}"
        );
      }
    }
    assert_eq!(super::xy_to_unit_angle(0.0, 0.0), 0.0);
    assert_eq!(super::xy_to_unit_angle(1.0, 0.0), 0.0);
  }

  #[test]
  fn hypot_matches_std() {
    assert_close(super::hypot(3.0, 4.0), 5.0);
    assert_close(super::hypot(-3.0, 4.0), 5.0);
  }
}

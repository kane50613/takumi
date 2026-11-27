//! Float math that renders the same on every platform, unlike libm, for finite pixel-scale inputs.

use std::hash::{Hash, Hasher};

/// `SK_ScalarNearlyZero`: the tolerance Skia treats a float as zero within.
pub(crate) const NEARLY_ZERO: f32 = 1.0 / 4096.0;

/// Hashes `value` so that floats `==` holds equal hash equally, `-0.0` as `0.0`.
pub(crate) fn hash_f32<H: Hasher>(value: f32, state: &mut H) {
  (value + 0.0).to_bits().hash(state);
}

/// The angle of `(x, y)` as a turn in `[0, 1)`, Skia's `xy_to_unit_angle` polynomial.
#[inline(always)]
pub(crate) fn xy_to_unit_angle(x: f32, y: f32) -> f32 {
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

/// Float comparison within four ulps, as strict-num's `approx_eq_ulps` and usvg compare floats.
pub trait ApproxEq {
  /// Whether `self` and `other` are equal or within four ulps of each other with the same sign.
  fn approx_eq(self, other: Self) -> bool;
}

impl ApproxEq for f32 {
  fn approx_eq(self, other: Self) -> bool {
    self == other
      || (self.is_sign_negative() == other.is_sign_negative()
        && self.to_bits().abs_diff(other.to_bits()) <= 4)
  }
}

#[cfg(test)]
mod tests {
  use std::f32::consts::TAU;

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
}

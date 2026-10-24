//! Float comparison by ulps, as usvg does it.

/// strict-num's `approx_eq_ulps` with four ulps, which usvg compares floats by.
pub(in crate::svg) trait ApproxEq {
  fn approx_eq(self, other: Self) -> bool;
}

impl ApproxEq for f32 {
  fn approx_eq(self, other: Self) -> bool {
    self == other
      || (self.is_sign_negative() == other.is_sign_negative()
        && self.to_bits().abs_diff(other.to_bits()) <= 4)
  }
}

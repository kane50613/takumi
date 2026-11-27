//! Small numeric helpers shared across style resolution and paint backends.

/// `v / 255` rounded to nearest for `v` up to `255 * 255`, the product of two bytes, by shifts
/// alone: Skia's `div255` on NEON, `vrshrq_n_u16(vrsraq_n_u16(v, v, 8), 8)`.
#[inline(always)]
pub fn div_255(v: u16) -> u8 {
  debug_assert!(v <= 255 * 255, "{v} is past the product of two bytes");

  ((v + ((v + 128) >> 8) + 128) >> 8) as u8
}

/// [`div_255`] of `a * b`, in 16-bit lanes so a loop over bytes vectorizes twice as wide.
#[inline(always)]
pub fn mul_div_255(a: u8, b: u8) -> u8 {
  div_255(u16::from(a) * u16::from(b))
}

pub(crate) fn lerp(lhs: f32, rhs: f32, progress: f32) -> f32 {
  lhs + (rhs - lhs) * progress
}

fn gcd(mut lhs: usize, mut rhs: usize) -> usize {
  while rhs != 0 {
    let remainder = lhs % rhs;
    lhs = rhs;
    rhs = remainder;
  }
  lhs
}

pub(crate) fn lcm(lhs: usize, rhs: usize) -> usize {
  if lhs == 0 || rhs == 0 {
    return 0;
  }
  lhs / gcd(lhs, rhs) * rhs
}

#[cfg(test)]
mod tests {
  use super::{div_255, mul_div_255};

  /// `v / 255` rounded to nearest, which no tie can reach since 255 is odd.
  fn rounded_div_255(v: u32) -> u8 {
    ((v + 127) / 255) as u8
  }

  #[test]
  fn divides_every_product_of_two_bytes_rounded_to_nearest() {
    for v in 0..=255 * 255 {
      assert_eq!(div_255(v), rounded_div_255(u32::from(v)));
    }
    for a in 0..=u8::MAX {
      for b in 0..=u8::MAX {
        assert_eq!(
          mul_div_255(a, b),
          rounded_div_255(u32::from(a) * u32::from(b))
        );
      }
    }
  }
}

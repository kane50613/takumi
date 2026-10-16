//! Small numeric helpers shared across style resolution and paint backends.

/// `v / 255` for `v` up to `255 * 255`, the product of two bytes, by shifts alone. It rounds to
/// nearest but for 126 of those inputs, where it lands one above.
#[inline(always)]
pub fn div_255(v: u16) -> u8 {
  debug_assert!(v <= 255 * 255, "{v} is past the product of two bytes");

  ((v + 128 + (v >> 8)) >> 8) as u8
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

  /// The 32-bit form takumi divided by 255 with before, saturating at 255.
  fn wide_div_255(v: u32) -> u8 {
    ((v + 128 + (v >> 8)) >> 8).min(255) as u8
  }

  #[test]
  fn sixteen_bit_lanes_divide_like_the_wide_form() {
    for v in 0..=255 * 255 {
      assert_eq!(div_255(v), wide_div_255(u32::from(v)));
    }
    for a in 0..=u8::MAX {
      for b in 0..=u8::MAX {
        assert_eq!(mul_div_255(a, b), wide_div_255(u32::from(a) * u32::from(b)));
      }
    }
  }

  #[test]
  fn a_sum_clamped_to_two_bytes_divides_like_the_wide_form() {
    for v in 0..=2 * 255 * 255 {
      assert_eq!(div_255(v.min(255 * 255) as u16), wide_div_255(v));
    }
  }
}

//! Small numeric helpers shared across style resolution and paint backends.

/// Fast division by 255, returning a `u8`.
#[inline(always)]
pub fn fast_div_255(v: u32) -> u8 {
  fast_div_255_u32(v) as u8
}

/// Fast division by 255 by approximating `v / 255` using bitwise operations.
#[inline(always)]
pub fn fast_div_255_u32(v: u32) -> u32 {
  ((v.wrapping_add(128).wrapping_add(v >> 8)) >> 8).min(255)
}

/// [`fast_div_255`] of `a * b` in 16-bit lanes, which the product of two bytes never overflows,
/// so a loop over bytes vectorizes twice as wide.
#[inline(always)]
pub fn mul_div_255(a: u8, b: u8) -> u8 {
  let product = u16::from(a) * u16::from(b);

  ((product + 128 + (product >> 8)) >> 8) as u8
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
  use super::{fast_div_255, mul_div_255};

  #[test]
  fn sixteen_bit_lanes_divide_like_fast_div_255() {
    for a in 0..=u8::MAX {
      for b in 0..=u8::MAX {
        assert_eq!(mul_div_255(a, b), fast_div_255(u32::from(a) * u32::from(b)));
      }
    }
  }
}

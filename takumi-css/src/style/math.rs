#[inline(always)]
pub fn fast_div_255(v: u32) -> u8 {
  fast_div_255_u32(v) as u8
}

/// Fast division by 255 by approximating `v / 255` using bitwise operations.
#[inline(always)]
pub fn fast_div_255_u32(v: u32) -> u32 {
  ((v.wrapping_add(128).wrapping_add(v >> 8)) >> 8).min(255)
}

pub(crate) fn lerp(lhs: f32, rhs: f32, progress: f32) -> f32 {
  lhs + (rhs - lhs) * progress
}

fn gcd(lhs: usize, rhs: usize) -> usize {
  let mut lhs = lhs;
  let mut rhs = rhs;
  while rhs != 0 {
    let remainder = lhs % rhs;
    lhs = rhs;
    rhs = remainder;
  }
  lhs
}

pub(crate) fn lcm(lhs: usize, rhs: usize) -> usize {
  lhs / gcd(lhs, rhs) * rhs
}

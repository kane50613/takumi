/// Pixel buffers above this pixel count are refused rather than allocated. At 4
/// bytes per pixel the largest accepted buffer is 1 GiB, which still fits the
/// 32-bit `usize` of the wasm target.
const MAX_BUFFER_PIXELS: u64 = 1 << 28;

/// Byte length of a `width * height` pixel buffer, or `None` when it is empty or
/// over [`MAX_BUFFER_PIXELS`]. Guards the `u32` products that a huge blur radius
/// or node size would otherwise wrap.
#[inline]
pub(crate) fn checked_area(width: u32, height: u32, bytes_per_pixel: u32) -> Option<usize> {
  let pixels = u64::from(width).saturating_mul(u64::from(height));

  (pixels > 0 && pixels <= MAX_BUFFER_PIXELS)
    .then(|| (pixels * u64::from(bytes_per_pixel)) as usize)
}

/// Uninitialized scratch buffer for callers that overwrite every byte before
/// reading; skips the memset a zeroed allocation would pay.
#[allow(clippy::uninit_vec)]
pub(crate) fn uninit_buffer(len: usize) -> Vec<u8> {
  let mut buf = Vec::with_capacity(len);

  unsafe {
    buf.set_len(len);
  }
  buf
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn checked_area_accepts_sane_dimensions() {
    assert_eq!(checked_area(1024, 1024, 1), Some(1024 * 1024));
    assert_eq!(checked_area(1024, 1024, 4), Some(4 * 1024 * 1024));
    assert_eq!(checked_area(16384, 16384, 1), Some(1 << 28));
  }

  /// The ceiling counts pixels, so a node far past the canvas budget still
  /// paints its 4-byte-per-pixel buffers.
  #[test]
  fn checked_area_accepts_a_node_larger_than_the_canvas() {
    assert_eq!(checked_area(9000, 9000, 4), Some(9000 * 9000 * 4));
    assert_eq!(checked_area(16384, 16384, 4), Some(1 << 30));
  }

  #[test]
  fn checked_area_rejects_empty() {
    assert_eq!(checked_area(0, 1024, 4), None);
    assert_eq!(checked_area(1024, 0, 4), None);
  }

  #[test]
  fn checked_area_rejects_u32_wrapping_products() {
    // `box-shadow: 0 0 100000px` on a large node produces ~1.5M px dimensions,
    // and `width: 100000px; height: 100000px` reaches the same range directly.
    assert_eq!(checked_area(1_500_000, 1_500_000, 1), None);
    assert_eq!(checked_area(100_000, 100_000, 4), None);
    assert_eq!(checked_area(u32::MAX, u32::MAX, 4), None);
    // Wraps to zero in `u32`, to a short buffer the caller would then overrun.
    assert_eq!(checked_area(1 << 16, 1 << 16, 4), None);
  }
}

use super::BYTE_TO_FLOAT;

/// The arithmetic operator of Blink's `FEComposite` as Skia's `sk_arithmetic_blend` runs it, with
/// the premultiplied clamp Blink asks for. `src1` is `in` and `src2` is `in2`.
///
/// All pixels are premultiplied and the slices have the same length.
pub(super) fn arithmetic(
  [k1, k2, k3, k4]: [f32; 4],
  src1: &[[u8; 4]],
  src2: &[[u8; 4]],
  dest: &mut [[u8; 4]],
) {
  for ((top, bottom), out) in src1.iter().zip(src2).zip(dest) {
    let mut color: [f32; 4] = std::array::from_fn(|channel| {
      let (top, bottom) = (
        f32::from(top[channel]) * BYTE_TO_FLOAT,
        f32::from(bottom[channel]) * BYTE_TO_FLOAT,
      );

      (k1 * top * bottom + k2 * top + k3 * bottom + k4).clamp(0.0, 1.0)
    });

    for channel in 0..3 {
      color[channel] = color[channel].min(color[3]);
    }

    *out = color.map(|channel| (channel * 255.0 + 0.5) as u8);
  }
}

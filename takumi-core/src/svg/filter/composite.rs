// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::svg::approx_eq::ApproxEq;

/// The arithmetic operator of Blink's `FEComposite` in `fe_composite.cc`, which Skia runs as
/// `SkImageFilters::Arithmetic`. Approximate: the clamping follows resvg's port of the formula.
///
/// All pixels are premultiplied and the slices have the same length.
pub(super) fn arithmetic(
  [k1, k2, k3, k4]: [f32; 4],
  src1: &[[u8; 4]],
  src2: &[[u8; 4]],
  dest: &mut [[u8; 4]],
) {
  let calc = |i1: u8, i2: u8, max: f32| {
    let i1 = f32::from(i1) / 255.0;
    let i2 = f32::from(i2) / 255.0;

    (k1 * i1 * i2 + k2 * i1 + k3 * i2 + k4).max(0.0).min(max)
  };

  for ((c1, c2), out) in src1.iter().zip(src2).zip(dest) {
    let a = calc(c1[3], c2[3], 1.0);

    if a.approx_eq(0.0) {
      continue;
    }

    *out = [
      (calc(c1[0], c2[0], a) * 255.0) as u8,
      (calc(c1[1], c2[1], a) * 255.0) as u8,
      (calc(c1[2], c2[2], a) * 255.0) as u8,
      (a * 255.0) as u8,
    ];
  }
}

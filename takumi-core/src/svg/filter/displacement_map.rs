// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ColorChannel, DisplacementMap};

impl DisplacementMap {
  /// Blink's `FEDisplacementMap` in `fe_displacement_map.cc`, which Skia runs as
  /// `SkImageFilters::DisplacementMap`. Approximate: pixels move to the nearest pixel, as in resvg.
  ///
  /// `map` pixels are straight alpha, `src` any alpha, and `sx` and `sy` the scale of the canvas.
  /// The slices are `width` pixels wide and have the same length.
  pub(super) fn apply(
    &self,
    [sx, sy]: [f32; 2],
    width: usize,
    src: &[[u8; 4]],
    map: &[[u8; 4]],
    dest: &mut [[u8; 4]],
  ) {
    let height = src.len() / width;
    let offset = |pixel: &[u8; 4], channel| {
      let index = match channel {
        ColorChannel::R => 0,
        ColorChannel::G => 1,
        ColorChannel::B => 2,
        ColorChannel::A => 3,
      };

      f32::from(pixel[index]) / 255.0 - 0.5
    };

    for (index, pixel) in map.iter().enumerate() {
      let (x, y) = (index % width, index / width);
      let ox = (x as f32 + offset(pixel, self.x_channel) * sx * self.scale).round() as i32;
      let oy = (y as f32 + offset(pixel, self.y_channel) * sy * self.scale).round() as i32;

      if (0..width as i32).contains(&ox) && (0..height as i32).contains(&oy) {
        dest[index] = src[oy as usize * width + ox as usize];
      }
    }
  }
}

// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ConvolveMatrix, EdgeMode};

impl ConvolveMatrix {
  /// Blink's `FEConvolveMatrix` in `fe_convolve_matrix.cc`, which Skia runs as
  /// `SkImageFilters::MatrixConvolution`. Approximate: the edge modes and rounding follow resvg.
  ///
  /// Pixels are premultiplied unless `preserve_alpha`, which wants straight alpha. The slice is
  /// `width` pixels wide.
  pub(super) fn apply(&self, pixels: &mut [[u8; 4]], width: usize) {
    let height = pixels.len() / width;
    let (width_max, height_max) = (width as i32 - 1, height as i32 - 1);
    let kernel = &self.kernel;
    let source = pixels.to_vec();

    for (index, (out, in_p)) in pixels.iter_mut().zip(&source).enumerate() {
      let (x, y) = ((index % width) as i32, (index / width) as i32);
      let mut sum = [0.0; 4];

      for oy in 0..kernel.rows {
        for ox in 0..kernel.columns {
          let mut tx = x - kernel.target_x as i32 + ox as i32;
          let mut ty = y - kernel.target_y as i32 + oy as i32;

          match self.edge_mode {
            EdgeMode::None => {
              if tx < 0 || tx > width_max || ty < 0 || ty > height_max {
                continue;
              }
            }
            EdgeMode::Duplicate => {
              tx = tx.clamp(0, width_max);
              ty = ty.clamp(0, height_max);
            }
            EdgeMode::Wrap => {
              tx = tx.rem_euclid(width as i32);
              ty = ty.rem_euclid(height as i32);
            }
          }

          let k = kernel.get(kernel.columns - ox - 1, kernel.rows - oy - 1);
          let p = source[ty as usize * width + tx as usize];

          for (sum, channel) in sum
            .iter_mut()
            .zip(p)
            .take(if self.preserve_alpha { 3 } else { 4 })
          {
            *sum += f32::from(channel) / 255.0 * k;
          }
        }
      }

      let alpha = if self.preserve_alpha {
        f32::from(in_p[3]) / 255.0
      } else {
        sum[3] / self.divisor + self.bias
      };
      let bounded_alpha = alpha.clamp(0.0, 1.0);
      let channel = |sum: f32| {
        let value = sum / self.divisor + self.bias * alpha;
        let value = if self.preserve_alpha {
          value.clamp(0.0, 1.0) * bounded_alpha
        } else {
          value.max(0.0).min(bounded_alpha)
        };

        (value * 255.0 + 0.5) as u8
      };

      *out = [
        channel(sum[0]),
        channel(sum[1]),
        channel(sum[2]),
        (bounded_alpha * 255.0 + 0.5) as u8,
      ];
    }
  }
}

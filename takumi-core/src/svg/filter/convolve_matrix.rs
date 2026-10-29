use super::{BYTE_TO_FLOAT, ConvolveMatrix, EdgeMode};
use crate::geometry::Rect;

/// Kernels with fewer cells keep their entries whole, Skia's `kMaxUniformKernelSize`. Larger ones
/// go through an 8-bit texture.
const MAX_UNIFORM_CELLS: usize = 28;

impl ConvolveMatrix {
  /// Blink's `FEConvolveMatrix` as Skia's `SkMatrixConvolutionImageFilter` runs it.
  pub(super) fn apply(
    &self,
    source: &[[u8; 4]],
    width: usize,
    window: Rect<usize>,
    out: &mut [[u8; 4]],
  ) {
    let span = window.right.saturating_sub(window.left);

    if span == 0 || window.bottom <= window.top {
      return;
    }

    let kernel = self.skia_kernel();
    let (columns, rows) = (self.kernel.columns as usize, self.kernel.rows as usize);
    let (target_x, target_y) = (self.kernel.target_x as isize, self.kernel.target_y as isize);
    let gain = 1.0 / self.divisor;
    let padded_width = span + columns - 1;
    let mut padded = vec![vec![[0.0_f32; 4]; padded_width]; rows];
    let mut padded_rows = vec![None; rows];
    let mut sum = vec![[0.0_f32; 4]; span];

    for y in window.top..window.bottom {
      sum.fill([0.0; 4]);

      for ky in 0..rows {
        let input_row = y as isize + ky as isize - target_y;
        let slot = input_row.rem_euclid(rows as isize) as usize;

        if padded_rows[slot] != Some(input_row) {
          self.pad_row(
            source,
            width,
            window,
            input_row,
            target_x,
            &mut padded[slot],
          );
          padded_rows[slot] = Some(input_row);
        }

        for kx in 0..columns {
          let entry = kernel[ky * columns + kx];

          for (sum, sample) in sum.iter_mut().zip(&padded[slot][kx..]) {
            for channel in 0..4 {
              sum[channel] += sample[channel] * entry;
            }
          }
        }
      }

      let row = y * width;

      for (x, sum) in (window.left..window.right).zip(&sum) {
        let mut color = sum.map(|channel| channel * gain + self.bias);

        if self.preserve_alpha {
          let alpha = f32::from(source[row + x][3]) * BYTE_TO_FLOAT;

          color = [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha];
        } else {
          // A huge gain can make alpha NaN, which saturates to zero as Skia's does in practice and
          // must not reach the `clamp` bounds below.
          color[3] = if color[3].is_nan() {
            0.0
          } else {
            color[3].clamp(0.0, 1.0)
          };
        }

        out[row + x] = [
          color[0].clamp(0.0, color[3]),
          color[1].clamp(0.0, color[3]),
          color[2].clamp(0.0, color[3]),
          color[3],
        ]
        .map(|channel| (channel * 255.0 + 0.5) as u8);
      }
    }
  }

  /// The entries as Skia reads them: reversed, as Blink hands them over, and through Skia's 8-bit
  /// texture when there are too many for uniforms.
  fn skia_kernel(&self) -> Vec<f32> {
    let mut kernel: Vec<f32> = self.kernel.data.iter().rev().copied().collect();

    if kernel.len() < MAX_UNIFORM_CELLS {
      return kernel;
    }

    // `create_kernel_bitmap` in `SkMatrixConvolutionImageFilter.cpp`.
    let min = kernel.iter().copied().fold(f32::INFINITY, f32::min);
    let max = kernel.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut inner_gain = max - min;

    if inner_gain.abs() <= 1.0 / 4096.0 {
      inner_gain = 1.0;
    }

    for entry in &mut kernel {
      let quantized = (255.0 * (*entry - min) / inner_gain + 0.5).floor();

      *entry = quantized * BYTE_TO_FLOAT * inner_gain + min;
    }

    kernel
  }

  /// Fills `padded` with the source row `input_row` as the kernel reads it, from `target_x` left of
  /// the window on: straight colors when it preserves alpha, and the edge mode past the window.
  fn pad_row(
    &self,
    source: &[[u8; 4]],
    width: usize,
    window: Rect<usize>,
    input_row: isize,
    target_x: isize,
    padded: &mut [[f32; 4]],
  ) {
    let height = source.len() / width;
    let tile = |position: isize, start: usize, end: usize| -> Option<usize> {
      let (start, end) = (start as isize, end as isize);

      match self.edge_mode {
        EdgeMode::None => None,
        EdgeMode::Duplicate => Some(position.clamp(start, end - 1) as usize),
        EdgeMode::Wrap => Some((start + (position - start).rem_euclid(end - start)) as usize),
      }
    };
    let inside = |position: isize, start: usize, end: usize| {
      (start as isize..end as isize).contains(&position)
    };
    let y = if inside(input_row, window.top, window.bottom) {
      Some(input_row as usize)
    } else if self.edge_mode == EdgeMode::None {
      inside(input_row, 0, height).then_some(input_row as usize)
    } else {
      tile(input_row, window.top, window.bottom)
    };

    for (offset, sample) in padded.iter_mut().enumerate() {
      let position = window.left as isize + offset as isize - target_x;
      let x = if inside(position, window.left, window.right) {
        Some(position as usize)
      } else if self.edge_mode == EdgeMode::None {
        inside(position, 0, width).then_some(position as usize)
      } else {
        tile(position, window.left, window.right)
      };
      let pixel = match (x, y) {
        (Some(x), Some(y)) => source[y * width + x],
        _ => [0; 4],
      };
      let color = pixel.map(|channel| f32::from(channel) * BYTE_TO_FLOAT);

      *sample = if self.preserve_alpha {
        let alpha = color[3].max(0.0001);

        [
          color[0] / alpha,
          color[1] / alpha,
          color[2] / alpha,
          color[3],
        ]
      } else {
        color
      };
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::svg::filter::{Input, Kernel};

  #[test]
  fn a_gain_past_floats_clears_transparent_pixels() {
    let convolve = ConvolveMatrix {
      input: Input::SourceGraphic,
      kernel: Kernel::new(1, 1, 3, 3, vec![1.0; 9]).expect("a 3x3 kernel"),
      divisor: 1e-40,
      bias: 0.0,
      edge_mode: EdgeMode::None,
      preserve_alpha: false,
    };
    let source = vec![[0; 4]; 16];
    let mut out = vec![[9; 4]; 16];

    convolve.apply(
      &source,
      4,
      Rect {
        left: 0,
        right: 4,
        top: 0,
        bottom: 4,
      },
      &mut out,
    );

    assert_eq!(out, vec![[0; 4]; 16]);
  }
}

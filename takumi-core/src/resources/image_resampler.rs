//! A resampler fed one source row at a time, matching `image::imageops::resize`.

use std::collections::VecDeque;

use crate::{resources::image_buffer::ImageBuffer, style::ImageScalingAlgorithm};

fn sinc(t: f32) -> f32 {
  let a = t * std::f32::consts::PI;
  if t == 0.0 { 1.0 } else { a.sin() / a }
}

fn lanczos3_kernel(x: f32) -> f32 {
  if x.abs() < 3.0 {
    sinc(x) * sinc(x / 3.0)
  } else {
    0.0
  }
}

/// Catmull-Rom spline: `bc_cubic_spline(x, 0.0, 0.5)`.
fn catmullrom_kernel(x: f32) -> f32 {
  let a = x.abs();
  let k = if a < 1.0 {
    9.0 * a.powi(3) - 15.0 * a.powi(2) + 6.0
  } else if a < 2.0 {
    -3.0 * a.powi(3) + 15.0 * a.powi(2) - 24.0 * a + 12.0
  } else {
    0.0
  };
  k / 6.0
}

fn box_kernel(_x: f32) -> f32 {
  1.0
}

fn filter(algorithm: ImageScalingAlgorithm) -> (fn(f32) -> f32, f32) {
  match algorithm {
    ImageScalingAlgorithm::Smooth => (lanczos3_kernel, 3.0),
    ImageScalingAlgorithm::Pixelated => (box_kernel, 0.0),
    _ => (catmullrom_kernel, 2.0),
  }
}

/// Source window `[left, right)` and normalized kernel weights for one output
/// index, matching `imageops`' clamping and normalization.
fn sample_window(
  out: u32,
  ratio: f32,
  sratio: f32,
  support: f32,
  source_len: u32,
  kernel: fn(f32) -> f32,
  weights: &mut Vec<f32>,
) -> (u32, u32) {
  let input = (out as f32 + 0.5) * ratio;
  let src_support = support * sratio;

  let left = ((input - src_support).floor() as i64).clamp(0, source_len as i64 - 1) as u32;
  let right =
    ((input + src_support).ceil() as i64).clamp(left as i64 + 1, source_len as i64) as u32;
  let center = input - 0.5;

  weights.clear();
  let mut sum = 0.0;
  for i in left..right {
    let weight = kernel((i as f32 - center) / sratio);
    weights.push(weight);
    sum += weight;
  }
  for weight in weights.iter_mut() {
    *weight /= sum;
  }

  (left, right)
}

struct HorizontalWindow {
  left: u32,
  weights: Box<[f32]>,
}

/// Takes premultiplied source rows in order and yields each target row once its window is full.
pub(crate) struct StreamResampler {
  native_width: u32,
  native_height: u32,
  target_width: u32,
  target_height: u32,
  kernel: fn(f32) -> f32,
  support: f32,
  vertical_ratio: f32,
  vertical_sratio: f32,
  horizontal: Box<[HorizontalWindow]>,
  ring: VecDeque<Box<[u8]>>,
  spare: Vec<Box<[u8]>>,
  ring_start: u32,
  rows_pushed: u32,
  next_target_row: u32,
  intermediate: Box<[f32]>,
  weights: Vec<f32>,
  output: Vec<u8>,
}

impl StreamResampler {
  pub(crate) fn new(
    (native_width, native_height): (u32, u32),
    (target_width, target_height): (u32, u32),
    algorithm: ImageScalingAlgorithm,
  ) -> Self {
    let (kernel, support) = filter(algorithm);

    let horizontal_ratio = native_width as f32 / target_width as f32;
    let horizontal_sratio = horizontal_ratio.max(1.0);
    let mut weights = Vec::new();
    let horizontal = (0..target_width)
      .map(|out_x| {
        let (left, _) = sample_window(
          out_x,
          horizontal_ratio,
          horizontal_sratio,
          support,
          native_width,
          kernel,
          &mut weights,
        );
        HorizontalWindow {
          left,
          weights: weights.as_slice().into(),
        }
      })
      .collect();

    let vertical_ratio = native_height as f32 / target_height as f32;

    Self {
      native_width,
      native_height,
      target_width,
      target_height,
      kernel,
      support,
      vertical_ratio,
      vertical_sratio: vertical_ratio.max(1.0),
      horizontal,
      ring: VecDeque::new(),
      spare: Vec::new(),
      ring_start: 0,
      rows_pushed: 0,
      next_target_row: 0,
      intermediate: vec![0.0; native_width as usize * 4].into(),
      weights,
      output: vec![0; target_width as usize * target_height as usize * 4],
    }
  }

  fn vertical_window(&mut self, out_y: u32) -> (u32, u32) {
    let mut weights = std::mem::take(&mut self.weights);
    let window = sample_window(
      out_y,
      self.vertical_ratio,
      self.vertical_sratio,
      self.support,
      self.native_height,
      self.kernel,
      &mut weights,
    );
    self.weights = weights;
    window
  }

  pub(crate) fn push_row(&mut self, row: &[u8]) {
    let mut stored = self
      .spare
      .pop()
      .unwrap_or_else(|| vec![0; self.native_width as usize * 4].into());
    stored.copy_from_slice(row);
    self.ring.push_back(stored);
    self.rows_pushed += 1;

    while self.next_target_row < self.target_height {
      let (left, right) = self.vertical_window(self.next_target_row);
      if right > self.rows_pushed {
        break;
      }

      self.emit(left);
      self.next_target_row += 1;

      if self.next_target_row < self.target_height {
        let (next_left, _) = self.vertical_window(self.next_target_row);
        while self.ring_start < next_left {
          let Some(row) = self.ring.pop_front() else {
            break;
          };
          self.spare.push(row);
          self.ring_start += 1;
        }
      }
    }
  }

  /// Resamples the current target row vertically, then horizontally, rounding as `imageops` does.
  fn emit(&mut self, left: u32) {
    self.intermediate.fill(0.0);
    for (offset, weight) in self.weights.iter().enumerate() {
      let row = &self.ring[(left - self.ring_start) as usize + offset];
      for (accumulator, value) in self.intermediate.iter_mut().zip(row.iter()) {
        *accumulator += *value as f32 * weight;
      }
    }

    let out_row = self.next_target_row as usize * self.target_width as usize * 4;
    for (out_x, window) in self.horizontal.iter().enumerate() {
      let mut t = [0.0_f32; 4];
      for (offset, weight) in window.weights.iter().enumerate() {
        let src = (window.left as usize + offset) * 4;
        for (accumulator, value) in t.iter_mut().zip(&self.intermediate[src..src + 4]) {
          *accumulator += value * weight;
        }
      }
      let dst = out_row + out_x * 4;
      for (out, value) in self.output[dst..dst + 4].iter_mut().zip(t) {
        *out = value.clamp(0.0, 255.0).round() as u8;
      }
    }
  }

  /// The resampled image, or `None` if the stream ended before every target
  /// row's window was filled.
  pub(crate) fn finish(self) -> Option<ImageBuffer> {
    if self.next_target_row != self.target_height {
      return None;
    }
    ImageBuffer::from_premultiplied_rgba(self.output, self.target_width, self.target_height)
  }
}

/// Resamples a whole premultiplied RGBA buffer in one call.
pub(crate) fn resample_premultiplied(
  data: &[u8],
  source: (u32, u32),
  target: (u32, u32),
  algorithm: ImageScalingAlgorithm,
) -> Option<ImageBuffer> {
  let mut resampler = StreamResampler::new(source, target, algorithm);
  for row in data.chunks_exact(source.0 as usize * 4) {
    resampler.push_row(row);
  }
  resampler.finish()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn gradient(width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
      for x in 0..width {
        data.extend_from_slice(&[
          (x * 7 % 256) as u8,
          (y * 11 % 256) as u8,
          ((x + y) * 3 % 256) as u8,
          255,
        ]);
      }
    }
    data
  }

  /// FNV-1a over `bytes`.
  fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
      (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
  }

  /// Pins the streamed output, first checked byte for byte against `image::imageops::resize`.
  #[test]
  fn streamed_output_stays_pinned() {
    let (width, height) = (64, 48);
    let data = gradient(width, height);
    let mut fingerprints = Vec::new();

    for algorithm in [
      ImageScalingAlgorithm::Auto,
      ImageScalingAlgorithm::Smooth,
      ImageScalingAlgorithm::Pixelated,
    ] {
      for target in [(17, 13), (32, 24), (63, 47), (1, 1)] {
        let mut resampler = StreamResampler::new((width, height), target, algorithm);

        for row in data.chunks_exact(width as usize * 4) {
          resampler.push_row(row);
        }

        fingerprints.push(fingerprint(resampler.finish().unwrap().data()));
      }
    }

    assert_eq!(fingerprints, PINNED);
  }

  /// Each algorithm's fingerprints, at each target size, in test order.
  const PINNED: [u64; 12] = [
    0x46cf8d0dd5f2ccec,
    0xcb96458ae29411d4,
    0x2e800b8c48a1a2ff,
    0xb80df1ef85123f69,
    0xd6099883e7e9aeae,
    0x3e994d31a05db8f6,
    0xfcddbd7534efe99f,
    0xb85bf1ef855460ba,
    0xd9b3505cd1fdfc20,
    0xbe022d5adf637da5,
    0x5099c597a29001dc,
    0x28184b5566eb268,
  ];

  #[test]
  fn truncated_stream_returns_none() {
    let (width, height) = (16, 16);
    let data = gradient(width, height);

    let mut resampler = StreamResampler::new((width, height), (8, 8), ImageScalingAlgorithm::Auto);
    for row in data.chunks_exact(width as usize * 4).take(4) {
      resampler.push_row(row);
    }

    assert!(resampler.finish().is_none());
  }
}

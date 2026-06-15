use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Output-stage dithering algorithms for static image exports and raw buffers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DitheringAlgorithm {
  /// Disable output dithering.
  #[default]
  None,
  /// Apply an ordered 8x8 Bayer pattern.
  OrderedBayer,
  /// Apply Floyd-Steinberg error diffusion with a reduced virtual color lattice.
  FloydSteinberg,
}

const BAYER_MATRIX_8X8: [[f32; 8]; 8] = [
  [
    -0.5, 0.0, -0.375, 0.125, -0.46875, 0.03125, -0.34375, 0.15625,
  ],
  [
    0.25, -0.25, 0.375, -0.125, 0.28125, -0.21875, 0.40625, -0.09375,
  ],
  [
    -0.3125, 0.1875, -0.4375, 0.0625, -0.28125, 0.21875, -0.40625, 0.09375,
  ],
  [
    0.4375, -0.0625, 0.3125, -0.1875, 0.46875, -0.03125, 0.34375, -0.15625,
  ],
  [
    -0.453125, 0.046875, -0.328125, 0.171875, -0.484375, 0.015625, -0.359375, 0.140625,
  ],
  [
    0.296875, -0.203125, 0.421875, -0.078125, 0.265625, -0.234375, 0.390625, -0.109375,
  ],
  [
    -0.265625, 0.234375, -0.390625, 0.109375, -0.296875, 0.203125, -0.421875, 0.078125,
  ],
  [
    0.484375, -0.015625, 0.359375, -0.140625, 0.453125, -0.046875, 0.328125, -0.171875,
  ],
];

const FLOYD_STEINBERG_LEVELS: f32 = 128.0;

/// Applies output dithering in-place to an RGBA image buffer.
pub fn apply_dithering(image: &mut RgbaImage, algorithm: DitheringAlgorithm) {
  match algorithm {
    DitheringAlgorithm::None => {}
    DitheringAlgorithm::OrderedBayer => apply_ordered_bayer(image),
    DitheringAlgorithm::FloydSteinberg => apply_floyd_steinberg(image),
  }
}

fn apply_ordered_bayer(image: &mut RgbaImage) {
  let width = image.width() as usize;

  for (pixel_index, pixel) in image.as_mut().chunks_exact_mut(4).enumerate() {
    if pixel[3] == 0 {
      continue;
    }

    let x = pixel_index % width;
    let y = pixel_index / width;
    let threshold = BAYER_MATRIX_8X8[y & 7][x & 7] + 0.5;

    for channel in &mut pixel[..3] {
      *channel = quantize_with_threshold(*channel as f32, threshold) as u8;
    }
  }
}

fn apply_floyd_steinberg(image: &mut RgbaImage) {
  let width = image.width() as usize;
  let mut current_errors = vec![[0.0; 3]; width + 2];
  let mut next_errors = vec![[0.0; 3]; width + 2];

  for row in image.as_mut().chunks_exact_mut(width * 4) {
    for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
      if pixel[3] == 0 {
        continue;
      }

      let error_index = x + 1;

      for channel_index in 0..3 {
        let value = pixel[channel_index] as f32 + current_errors[error_index][channel_index];
        let quantized = quantize_to_reduced_levels(value);
        let error = value - quantized;

        pixel[channel_index] = quantized as u8;
        current_errors[error_index + 1][channel_index] += error * (7.0 / 16.0);
        next_errors[error_index - 1][channel_index] += error * (3.0 / 16.0);
        next_errors[error_index][channel_index] += error * (5.0 / 16.0);
        next_errors[error_index + 1][channel_index] += error * (1.0 / 16.0);
      }
    }

    current_errors.fill([0.0; 3]);
    std::mem::swap(&mut current_errors, &mut next_errors);
  }
}

#[inline(always)]
fn quantize_to_reduced_levels(value: f32) -> f32 {
  let value = value.clamp(0.0, 255.0);
  let scaled = value / 255.0 * (FLOYD_STEINBERG_LEVELS - 1.0);
  let quantized = scaled.round() / (FLOYD_STEINBERG_LEVELS - 1.0) * 255.0;
  quantized.clamp(0.0, 255.0)
}

#[inline(always)]
fn quantize_with_threshold(value: f32, threshold: f32) -> f32 {
  let value = value.clamp(0.0, 255.0);
  let scaled = value / 255.0 * (FLOYD_STEINBERG_LEVELS - 1.0);
  let lower = scaled.floor();
  let fraction = scaled - lower;
  let quantized = if fraction > threshold {
    lower + 1.0
  } else {
    lower
  };

  (quantized / (FLOYD_STEINBERG_LEVELS - 1.0) * 255.0).clamp(0.0, 255.0)
}

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};

  use super::{DitheringAlgorithm, apply_dithering};

  fn sample_gradient_image() -> RgbaImage {
    let mut image = RgbaImage::new(16, 16);

    for y in 0..image.height() {
      for x in 0..image.width() {
        let value = (x * 8 + y * 4) as u8;
        image.put_pixel(x, y, Rgba([value, value, value, 255]));
      }
    }

    image
  }

  #[test]
  fn ordered_bayer_changes_rgb_but_preserves_alpha() {
    let mut image = sample_gradient_image();
    let before = image.clone();

    apply_dithering(&mut image, DitheringAlgorithm::OrderedBayer);

    assert_ne!(image, before);
    assert!(image.pixels().all(|pixel| pixel[3] == 255));
  }

  #[test]
  fn floyd_steinberg_changes_rgb_but_preserves_alpha() {
    let mut image = sample_gradient_image();
    let before = image.clone();

    apply_dithering(&mut image, DitheringAlgorithm::FloydSteinberg);

    assert_ne!(image, before);
    assert!(image.pixels().all(|pixel| pixel[3] == 255));
  }

  #[test]
  fn dithering_skips_fully_transparent_pixels() {
    let mut image = RgbaImage::from_pixel(4, 4, Rgba([100, 120, 140, 0]));
    let before = image.clone();

    apply_dithering(&mut image, DitheringAlgorithm::OrderedBayer);
    apply_dithering(&mut image, DitheringAlgorithm::FloydSteinberg);

    assert_eq!(image, before);
  }
}

// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::array;

use super::MorphologyOperator;

/// Blink's `FEMorphology` in `fe_morphology.cc`, which Skia runs as `SkImageFilters::Erode` and
/// `Dilate`. Approximate: the window follows resvg's, `ceil(radius) * 2` pixels wide.
///
/// Pixels are premultiplied and the slice is `width` pixels wide.
pub(super) fn apply(
  operator: MorphologyOperator,
  rx: f32,
  ry: f32,
  pixels: &mut [[u8; 4]],
  width: usize,
) {
  let height = pixels.len() / width;
  // No point in making matrix larger than image.
  let columns = ((rx.ceil() as usize).saturating_mul(2)).min(width);
  let rows = ((ry.ceil() as usize).saturating_mul(2)).min(height);
  let mut buf = vec![[0; 4]; pixels.len()];

  // The extremum over a rectangle is the extremum over its rows' extrema, so the window sweeps
  // each row, then each column.
  sweep(
    operator,
    pixels,
    &mut buf,
    columns,
    height,
    width,
    |row, x| row * width + x,
  );
  sweep(operator, &buf, pixels, rows, width, height, |column, y| {
    y * width + column
  });
}

/// Writes to `dst` the extremum of `src` along each of `lines` lines of `length` pixels, over a
/// window of `size` pixels that starts `size / 2` before each pixel and is clipped to the line.
fn sweep(
  operator: MorphologyOperator,
  src: &[[u8; 4]],
  dst: &mut [[u8; 4]],
  size: usize,
  lines: usize,
  length: usize,
  index: impl Fn(usize, usize) -> usize,
) {
  let before = size / 2;

  for line in 0..lines {
    for position in 0..length {
      let start = position.saturating_sub(before);
      let end = std::cmp::min((position + size).saturating_sub(before), length);

      dst[index(line, position)] = (start..end)
        .map(|offset| src[index(line, offset)])
        .reduce(|extremum, pixel| combine(operator, extremum, pixel))
        .unwrap_or_default();
    }
  }
}

fn combine(operator: MorphologyOperator, a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
  let pick = match operator {
    MorphologyOperator::Erode => u8::min,
    MorphologyOperator::Dilate => u8::max,
  };

  array::from_fn(|channel| pick(a[channel], b[channel]))
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The extremum over the whole clipped rectangle around each pixel, pixel by pixel.
  fn naive(
    operator: MorphologyOperator,
    columns: usize,
    rows: usize,
    width: usize,
    src: &[[u8; 4]],
  ) -> Vec<[u8; 4]> {
    let height = src.len() / width;
    let mut out = Vec::new();

    for y in 0..height {
      for x in 0..width {
        let xs = x.saturating_sub(columns / 2)..(x + columns - columns / 2).min(width);
        let ys = y.saturating_sub(rows / 2)..(y + rows - rows / 2).min(height);
        let pixels = ys.flat_map(|ty| xs.clone().map(move |tx| ty * width + tx));

        out.push(
          pixels
            .map(|index| src[index])
            .reduce(|a, b| combine(operator, a, b))
            .unwrap_or_default(),
        );
      }
    }

    out
  }

  #[test]
  fn sweeps_match_the_whole_rectangle() {
    let (width, height) = (9, 7);
    let mut seed = 0x2545_f491_u32;
    let pixels: Vec<[u8; 4]> = (0..width * height)
      .map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;

        seed.to_le_bytes()
      })
      .collect();

    for operator in [MorphologyOperator::Erode, MorphologyOperator::Dilate] {
      for (rx, ry) in [
        (0.5_f32, 0.5_f32),
        (1.0, 2.0),
        (2.5, 1.0),
        (3.0, 3.0),
        (20.0, 20.0),
      ] {
        let mut data = pixels.clone();
        let columns = ((rx.ceil() as usize) * 2).min(width);
        let rows = ((ry.ceil() as usize) * 2).min(height);
        let expected = naive(operator, columns, rows, width, &data);

        apply(operator, rx, ry, &mut data, width);

        assert_eq!(data, expected, "{operator:?} {rx} {ry}");
      }
    }
  }
}

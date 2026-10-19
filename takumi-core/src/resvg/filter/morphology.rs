// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::ImageRefMut;
use super::RGBA8;
use crate::resvg::usvg::filter::MorphologyOperator;

/// Applies a morphology filter.
///
/// `src` pixels should have a **premultiplied alpha**.
///
/// # Allocations
///
/// This method will allocate a copy of the `src` image as a back buffer.
pub fn apply(operator: MorphologyOperator, rx: f32, ry: f32, src: ImageRefMut) {
  // No point in making matrix larger than image.
  let columns = std::cmp::min((rx.ceil() as u32).saturating_mul(2), src.width) as usize;
  let rows = std::cmp::min((ry.ceil() as u32).saturating_mul(2), src.height) as usize;
  let width = src.width as usize;
  let height = src.height as usize;
  let mut buf = vec![RGBA8::default(); src.data.len()];

  // The extremum over a rectangle is the extremum over its rows' extrema, so the window sweeps
  // each row, then each column.
  sweep(
    operator,
    src.data,
    &mut buf,
    columns,
    height,
    width,
    |row, x| row * width + x,
  );
  sweep(
    operator,
    &buf,
    src.data,
    rows,
    width,
    height,
    |column, y| y * width + column,
  );
}

/// Writes to `dst` the extremum of `src` along each of `lines` lines of `length` pixels, over a
/// window of `size` pixels that starts `size / 2` before each pixel and is clipped to the line.
fn sweep(
  operator: MorphologyOperator,
  src: &[RGBA8],
  dst: &mut [RGBA8],
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

fn combine(operator: MorphologyOperator, a: RGBA8, b: RGBA8) -> RGBA8 {
  let pick = match operator {
    MorphologyOperator::Erode => std::cmp::min::<u8>,
    MorphologyOperator::Dilate => std::cmp::max::<u8>,
  };

  RGBA8 {
    r: pick(a.r, b.r),
    g: pick(a.g, b.g),
    b: pick(a.b, b.b),
    a: pick(a.a, b.a),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The extremum over the whole clipped rectangle around each pixel, pixel by pixel.
  fn naive(
    operator: MorphologyOperator,
    columns: usize,
    rows: usize,
    src: &ImageRefMut,
  ) -> Vec<RGBA8> {
    let (width, height) = (src.width as usize, src.height as usize);
    let mut out = Vec::new();

    for y in 0..height {
      for x in 0..width {
        let xs = x.saturating_sub(columns / 2)..(x + columns - columns / 2).min(width);
        let ys = y.saturating_sub(rows / 2)..(y + rows - rows / 2).min(height);
        let pixels = ys.flat_map(|ty| xs.clone().map(move |tx| ty * width + tx));

        out.push(
          pixels
            .map(|index| src.data[index])
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
    let pixels: Vec<RGBA8> = (0..width * height)
      .map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;

        let [r, g, b, a] = seed.to_le_bytes();

        RGBA8 { r, g, b, a }
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
        let columns = ((rx.ceil() as usize) * 2).min(width as usize);
        let rows = ((ry.ceil() as usize) * 2).min(height as usize);
        let expected = naive(
          operator,
          columns,
          rows,
          &ImageRefMut::new(width, height, &mut data.clone()),
        );

        apply(operator, rx, ry, ImageRefMut::new(width, height, &mut data));

        assert_eq!(data, expected, "{operator:?} {rx} {ry}");
      }
    }
  }
}

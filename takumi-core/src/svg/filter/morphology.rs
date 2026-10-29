use super::MorphologyOperator;

/// Skia's cap on a morphology radius, in `SkMorphologyImageFilter::radii` (crbug.com/1123035).
const MAX_RADIUS: f32 = 256.0;

/// Pixels a column strip spans, which keeps each strip's running extrema in cache.
const STRIP: usize = 64;

impl MorphologyOperator {
  /// Blink's `FEMorphology` as Skia's `SkMorphologyImageFilter` runs it.
  pub(super) fn apply(self, rx: f32, ry: f32, pixels: &mut [[u8; 4]], width: usize) {
    let radius = |radius: f32| radius.round().clamp(0.0, MAX_RADIUS) as usize;

    match self {
      MorphologyOperator::Erode => morph(u8::min, radius(rx), radius(ry), pixels, width),
      MorphologyOperator::Dilate => morph(u8::max, radius(rx), radius(ry), pixels, width),
    }
  }
}

fn morph(
  pick: impl Fn(u8, u8) -> u8 + Copy,
  rx: usize,
  ry: usize,
  pixels: &mut [[u8; 4]],
  width: usize,
) {
  let height = pixels.len() / width;
  let mut swept = vec![[0; 4]; pixels.len()];

  if ry > 0 {
    sweep_columns(pick, pixels, &mut swept, width, ry);
    pixels.copy_from_slice(&swept);
  }

  if rx > 0 {
    let mut transposed = vec![[0; 4]; pixels.len()];

    transpose(pixels, &mut transposed, width, height);
    sweep_columns(pick, &transposed, &mut swept, height, rx);
    transpose(&swept, pixels, height, width);
  }
}

/// The extremum of `src` over the rows around each row, by the van Herk/Gil-Werman algorithm.
fn sweep_columns(
  pick: impl Fn(u8, u8) -> u8 + Copy,
  src: &[[u8; 4]],
  dst: &mut [[u8; 4]],
  width: usize,
  radius: usize,
) {
  let height = src.len() / width;
  let window = 2 * radius + 1;
  let padded = height + 2 * radius;
  let strip_bytes = STRIP * 4;
  let mut prefix = vec![0_u8; padded * strip_bytes];
  let mut suffix = vec![0_u8; padded * strip_bytes];
  let blank = vec![0_u8; strip_bytes];
  let pick_into = |target: &mut [u8], a: &[u8], b: &[u8]| {
    for ((target, &a), &b) in target.iter_mut().zip(a).zip(b) {
      *target = pick(a, b);
    }
  };

  for left in (0..width).step_by(STRIP) {
    let bytes = (width - left).min(STRIP) * 4;
    let row = |padded_row: usize| -> &[u8] {
      match padded_row.checked_sub(radius) {
        Some(row) if row < height => src[row * width + left..][..bytes / 4].as_flattened(),
        _ => &blank[..bytes],
      }
    };

    for padded_row in 0..padded {
      let (before, rest) = prefix.split_at_mut(padded_row * strip_bytes);
      let target = &mut rest[..bytes];

      if padded_row % window == 0 {
        target.copy_from_slice(row(padded_row));
      } else {
        pick_into(
          target,
          &before[before.len() - strip_bytes..][..bytes],
          row(padded_row),
        );
      }
    }

    for padded_row in (0..padded).rev() {
      let (head, after) = suffix.split_at_mut((padded_row + 1) * strip_bytes);
      let target = &mut head[padded_row * strip_bytes..][..bytes];

      if padded_row % window == window - 1 || padded_row == padded - 1 {
        target.copy_from_slice(row(padded_row));
      } else {
        pick_into(target, &after[..bytes], row(padded_row));
      }
    }

    for row in 0..height {
      let target = dst[row * width + left..][..bytes / 4].as_flattened_mut();

      pick_into(
        target,
        &suffix[row * strip_bytes..][..bytes],
        &prefix[(row + window - 1) * strip_bytes..][..bytes],
      );
    }
  }
}

/// Writes the `width` by `height` pixels of `src` to `dst` with rows and columns swapped.
fn transpose(src: &[[u8; 4]], dst: &mut [[u8; 4]], width: usize, height: usize) {
  const TILE: usize = 32;

  for top in (0..height).step_by(TILE) {
    for left in (0..width).step_by(TILE) {
      for y in top..(top + TILE).min(height) {
        for x in left..(left + TILE).min(width) {
          dst[x * height + y] = src[y * width + x];
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The extremum over the window around each pixel, transparent beyond the edges, pixel by pixel.
  fn naive(
    operator: MorphologyOperator,
    rx: usize,
    ry: usize,
    width: usize,
    src: &[[u8; 4]],
  ) -> Vec<[u8; 4]> {
    let pick = match operator {
      MorphologyOperator::Erode => u8::min,
      MorphologyOperator::Dilate => u8::max,
    };
    let height = src.len() / width;
    let mut out = Vec::new();

    for y in 0..height as isize {
      for x in 0..width as isize {
        let mut extremum = src[y as usize * width + x as usize];

        for ty in y - ry as isize..=y + ry as isize {
          for tx in x - rx as isize..=x + rx as isize {
            let inside = (0..height as isize).contains(&ty) && (0..width as isize).contains(&tx);
            let sample = if inside {
              src[ty as usize * width + tx as usize]
            } else {
              [0; 4]
            };

            extremum = [0, 1, 2, 3].map(|channel| pick(extremum[channel], sample[channel]));
          }
        }

        out.push(extremum);
      }
    }

    out
  }

  #[test]
  fn sweeps_match_the_whole_window() {
    let (width, height) = (70, 9);
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
        (0.4_f32, 0.6_f32),
        (1.0, 2.0),
        (2.5, 0.0),
        (0.0, 3.0),
        (3.0, 3.0),
        (20.0, 40.0),
      ] {
        let mut data = pixels.clone();
        let expected = naive(
          operator,
          rx.round() as usize,
          ry.round() as usize,
          width,
          &data,
        );

        operator.apply(rx, ry, &mut data, width);

        assert_eq!(data, expected, "{operator:?} {rx} {ry}");
      }
    }
  }
}

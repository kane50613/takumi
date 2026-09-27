//! Gaussian blur of premultiplied RGBA or alpha pixels, as Skia's raster `SkBlurEngine` runs it
//! for Chrome's `filter`, `backdrop-filter` and `text-shadow`: a true Gaussian kernel below a
//! sigma of 2 (`GaussianPass`), and above it three box passes run as one (`ThreeBoxApproxPass`
//! for RGBA, `A8Pass` for alpha). Follows Skia under the notice in LICENSE-CHROMIUM.

use std::f32::consts::PI;

/// Largest sigma a three-box pass sums without overflowing a `u32`, Skia's `kMaxSigma`.
pub const MAX_SIGMA: f32 = 135.0;

/// Blurs a `width` by `height` premultiplied RGBA image in place by `sigma` on each axis.
///
/// Pixels past the edges count as transparent, so the caller pads the image by as far as the
/// blur reaches to keep what spreads out.
// Inlined, like `blur_alpha`, so the per-pixel loops compile in the calling crate at its opt-level
// rather than at takumi-core's size-optimized one.
#[inline]
pub fn blur_rgba(pixels: &mut [[u8; 4]], width: usize, height: usize, sigma: f32) {
  blur(pixels, width, height, sigma, Rounding::Rgba);
}

/// Blurs a `width` by `height` alpha mask in place by `sigma` on each axis, as [`blur_rgba`] does.
#[inline]
pub fn blur_alpha(alpha: &mut [u8], width: usize, height: usize, sigma: f32) {
  blur(
    alpha.as_chunks_mut::<1>().0,
    width,
    height,
    sigma,
    Rounding::Alpha,
  );
}

fn blur<const N: usize>(
  pixels: &mut [[u8; N]],
  width: usize,
  height: usize,
  sigma: f32,
  rounding: Rounding,
) {
  if width == 0 || height == 0 || pixels.len() != width * height {
    return;
  }
  let Some(pass) = Pass::new(sigma, rounding) else {
    return;
  };
  // Rows blur as the columns of a strip of them turned on its side, so both axes run the column
  // pass, which advances a row of lanes at once, and a strip stays small enough to stay cached.
  let mut strip = vec![[0u8; N]; ROW_STRIP * width];

  for rows in pixels.chunks_mut(ROW_STRIP * width) {
    let lanes = rows.len() / width;
    let strip = &mut strip[..rows.len()];

    for (lane, row) in rows.chunks_exact(width).enumerate() {
      for (column, pixel) in strip.chunks_exact_mut(lanes).zip(row) {
        column[lane] = *pixel;
      }
    }
    pass.blur_columns(strip, lanes, width);
    for (lane, row) in rows.chunks_exact_mut(width).enumerate() {
      for (column, pixel) in strip.chunks_exact(lanes).zip(row) {
        *pixel = column[lane];
      }
    }
  }
  pass.blur_columns(pixels, width, height);
}

/// Rows the horizontal pass turns on their side at once.
const ROW_STRIP: usize = 16;

/// How a three-box pass rounds its sum to a channel: Skia's RGBA pass seeds the sum with half the
/// divisor, its alpha pass adds half after scaling.
#[derive(Clone, Copy)]
enum Rounding {
  Rgba,
  Alpha,
}

/// One axis of the blur.
enum Pass {
  Gaussian(GaussianPass),
  ThreeBox(ThreeBoxPass),
}

impl Pass {
  /// The pass for `sigma`, or `None` when the blur leaves the pixels as they are.
  fn new(sigma: f32, rounding: Rounding) -> Option<Self> {
    // Skia's `SkBlurEngine::IsEffectivelyIdentity`.
    if !sigma.is_finite() || sigma <= 0.03 {
      return None;
    }

    if sigma < 2.0 {
      return Some(Self::Gaussian(GaussianPass::new(sigma)));
    }
    // TODO: past `MAX_SIGMA`, rescale the image as Skia's `FilterResult::rescale` does rather
    // than clamping the sigma.
    ThreeBoxPass::new(sigma.min(MAX_SIGMA), rounding).map(Self::ThreeBox)
  }

  /// Blurs every column of the `width` by `height` image in place.
  fn blur_columns<const N: usize>(&self, pixels: &mut [[u8; N]], width: usize, height: usize) {
    match self {
      Self::Gaussian(pass) => pass.blur_columns(pixels, width, height),
      Self::ThreeBox(pass) => pass.blur_columns(pixels, width, height),
    }
  }
}

/// Walks the rows of a column pass whose results land `border` rows behind the row read. Runs
/// `step` with each row read, zeros past the bottom, and the row written, or `None` while the
/// kernel has yet to reach the first one.
fn for_each_row<const N: usize>(
  pixels: &mut [[u8; N]],
  width: usize,
  height: usize,
  border: usize,
  mut step: impl FnMut(&[[u8; N]], Option<&mut [[u8; N]]>),
) {
  let zeros = vec![[0u8; N]; width];

  for index in 0..height + border {
    // The border is at least a pixel, so the row written lies wholly before the one read.
    let (written, unread) = pixels.split_at_mut((index * width).min(pixels.len()));
    let entering = unread.get(..width).unwrap_or(&zeros);
    let target = index
      .checked_sub(border)
      .map(|target| &mut written[target * width..(target + 1) * width]);

    step(entering, target);
  }
}

/// Skia's `GaussianPass`: a normalised Gaussian kernel `ceil(3 * sigma)` pixels each way.
struct GaussianPass {
  kernel: Vec<f32>,
}

impl GaussianPass {
  fn new(sigma: f32) -> Self {
    let radius = (3.0 * sigma).ceil() as usize;
    let denominator = 1.0 / (2.0 * sigma * sigma);
    let mut kernel: Vec<f32> = (0..=2 * radius)
      .map(|index| {
        let offset = index as f32 - radius as f32;

        (-(offset * offset * denominator)).exp()
      })
      .collect();
    let scale = 1.0 / kernel.iter().sum::<f32>();

    kernel.iter_mut().for_each(|weight| *weight *= scale);
    Self { kernel }
  }

  fn blur_columns<const N: usize>(&self, pixels: &mut [[u8; N]], width: usize, height: usize) {
    let window = self.kernel.len();
    // The channels of a row side by side, as the three-box pass lays them out. The ring holds the
    // last `window` rows read as fractions of full; `next` is the oldest, overwritten next.
    let channels = width * N;
    let mut ring = vec![0.0f32; window * channels];
    let mut sums = vec![0.0f32; channels];
    let mut next = 0;

    for_each_row(pixels, width, height, window / 2, |entering, target| {
      for (cell, channel) in ring[next * channels..][..channels]
        .iter_mut()
        .zip(entering.as_flattened())
      {
        *cell = f32::from(*channel) * (1.0 / 255.0);
      }
      next = if next + 1 == window { 0 } else { next + 1 };

      let Some(target) = target else {
        return;
      };

      sums.fill(0.0);
      for (offset, weight) in self.kernel.iter().enumerate() {
        let slot = (next + offset) % window;

        for (sum, sample) in sums.iter_mut().zip(&ring[slot * channels..][..channels]) {
          *sum += sample * weight;
        }
      }
      for (channel, sum) in target.as_flattened_mut().iter_mut().zip(&sums) {
        *channel = (sum * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
      }
    });
  }
}

/// Skia's `ThreeBoxApproxPass` and `A8Pass`: three box filters of `window` pixels summed at once,
/// the last one a pixel wider when the window is even.
struct ThreeBoxPass {
  window: usize,
  border: usize,
  factor: u32,
  seed: u32,
  bias: u64,
}

impl ThreeBoxPass {
  fn new(sigma: f32, rounding: Rounding) -> Option<Self> {
    let window = box_window(sigma);

    if window <= 1 {
      return None;
    }

    let odd = !window.is_multiple_of(2);
    let border = if odd {
      3 * ((window - 1) / 2)
    } else {
      3 * (window / 2) - 1
    };
    let cube = window * window * window;
    let divisor = if odd { cube } else { cube + window * window };
    let factor = ((1.0 / divisor as f64) * (1u64 << 32) as f64).round() as u32;
    let (seed, bias) = match rounding {
      Rounding::Rgba => (divisor.div_ceil(2) as u32, 0),
      Rounding::Alpha => (0, 1u64 << 31),
    };

    Some(Self {
      window,
      border,
      factor,
      seed,
      bias,
    })
  }

  fn blur_columns<const N: usize>(&self, pixels: &mut [[u8; N]], width: usize, height: usize) {
    let pass = self.window - 1;
    let last = if self.window.is_multiple_of(2) {
      pass + 1
    } else {
      pass
    };
    // The channels of a row, side by side: every channel is a lane of its own. Each box keeps a
    // running sum per lane and the trailing edge it drops `pass`, `pass` and `last` rows back.
    let channels = width * N;
    let mut sum0 = vec![0u32; channels];
    let mut sum1 = vec![0u32; channels];
    let mut sum2 = vec![self.seed; channels];
    let mut first = vec![0u32; pass * channels];
    let mut second = vec![0u32; pass * channels];
    let mut third = vec![0u32; last * channels];
    let mut blurred = vec![0u8; channels];
    let (mut slot, mut last_slot) = (0, 0);

    for_each_row(pixels, width, height, self.border, |entering, target| {
      let entering = &entering.as_flattened()[..channels];
      let (sum0, sum1, sum2, blurred) = (
        &mut sum0[..channels],
        &mut sum1[..channels],
        &mut sum2[..channels],
        &mut blurred[..channels],
      );
      let first = &mut first[slot * channels..][..channels];
      let second = &mut second[slot * channels..][..channels];
      let third = &mut third[last_slot * channels..][..channels];

      for lane in 0..channels {
        let leading = u32::from(entering[lane]);

        sum0[lane] += leading;
        sum1[lane] += sum0[lane];
        sum2[lane] += sum1[lane];
        blurred[lane] = ((u64::from(sum2[lane]) * u64::from(self.factor) + self.bias) >> 32) as u8;
        sum2[lane] -= third[lane];
        third[lane] = sum1[lane];
        sum1[lane] -= second[lane];
        second[lane] = sum0[lane];
        sum0[lane] -= first[lane];
        first[lane] = leading;
      }
      if let Some(target) = target {
        target.as_flattened_mut().copy_from_slice(blurred);
      }

      slot = if slot + 1 == pass { 0 } else { slot + 1 };
      last_slot = if last_slot + 1 == last {
        0
      } else {
        last_slot + 1
      };
    });
  }
}

/// Skia's `SkBlurEngine::BoxBlurWindow`: the box width three passes of which approximate a
/// Gaussian of `sigma`.
fn box_window(sigma: f32) -> usize {
  ((sigma * 3.0 * (2.0 * PI).sqrt() / 4.0 + 0.5).floor() as usize).max(1)
}

#[cfg(test)]
mod tests {
  use super::{MAX_SIGMA, blur_alpha, blur_rgba, box_window};

  #[test]
  fn the_box_window_follows_skia() {
    assert_eq!(box_window(2.0), 4);
    assert_eq!(box_window(5.0), 9);
    assert!(box_window(MAX_SIGMA) < 255);
  }

  #[test]
  fn a_blur_keeps_the_ink_it_spreads() {
    let (width, height) = (61, 61);
    let mut alpha = vec![0u8; width * height];

    for row in 25..36 {
      for column in 25..36 {
        alpha[row * width + column] = 255;
      }
    }
    let before: u32 = alpha.iter().map(|&value| u32::from(value)).sum();

    blur_alpha(&mut alpha, width, height, 4.0);

    let after: u32 = alpha.iter().map(|&value| u32::from(value)).sum();

    assert!(before.abs_diff(after) * 100 < before);
    assert!(alpha[30 * width + 30] < 255);
    assert!(alpha[30 * width + 30] > alpha[30 * width + 40]);
  }

  #[test]
  fn a_small_sigma_uses_a_symmetric_kernel() {
    let (width, height) = (9, 1);
    let mut pixels = vec![[0u8; 4]; width * height];

    pixels[4] = [255; 4];
    blur_rgba(&mut pixels, width, height, 1.0);

    assert_eq!(pixels[3], pixels[5]);
    assert_eq!(pixels[2], pixels[6]);
    assert!(pixels[4][3] > pixels[3][3]);
  }
}

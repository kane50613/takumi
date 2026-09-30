//! Gaussian blur of premultiplied RGBA or alpha pixels, as Skia's raster `SkBlurEngine` runs it
//! for Chrome's `filter`, `backdrop-filter` and `text-shadow`: a true Gaussian kernel below a
//! sigma of 2 (`GaussianPass`), and above it three box passes run as one (`ThreeBoxApproxPass`
//! for RGBA, `A8Pass` for alpha). Follows Skia under the notice in LICENSE-CHROMIUM.

use std::{array, f32::consts::PI};

use crate::geometry::Point;

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
  if sigma > MAX_SIGMA {
    return blur_rescaled(pixels, width, height, sigma, rounding);
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

/// Blurs past `MAX_SIGMA` as Skia's `FilterResult::Builder::blur` does: the image shrinks by
/// `MAX_SIGMA / sigma` in `FilterResult::rescale`'s halving steps, blurs there, and scales back up.
///
/// Approximate: every resampling filters bilinearly in `f32`, where Skia's raster pipeline may
/// filter at eight-bit precision, so a channel can land one step apart.
fn blur_rescaled<const N: usize>(
  pixels: &mut [[u8; N]],
  width: usize,
  height: usize,
  sigma: f32,
  rounding: Rounding,
) {
  let scale = MAX_SIGMA / sigma;
  let source = Bounds {
    left: 0.0,
    top: 0.0,
    right: width as f32,
    bottom: height as f32,
  };
  let mut bounds = source;
  let mut image = Raster {
    left: 0,
    top: 0,
    width,
    height,
    pixels: pixels.to_vec(),
  };

  for step in (0..downscale_step_count(scale)).rev() {
    let (factor_x, factor_y) = if step > 0 {
      (0.5, 0.5)
    } else {
      (
        source.width() * scale / bounds.width(),
        source.height() * scale / bounds.height(),
      )
    };
    let target = scale_about_center(bounds, factor_x, factor_y);

    image = image.resampled(bounds, target);
    bounds = target;
  }

  let low_sigma = (sigma * bounds.width() / source.width()).min(MAX_SIGMA);

  blur(
    &mut image.pixels,
    image.width,
    image.height,
    low_sigma,
    rounding,
  );

  for (index, pixel) in pixels.iter_mut().enumerate() {
    let center = Point {
      x: (index % width) as f32 + 0.5,
      y: (index / width) as f32 + 0.5,
    };

    *pixel = image.sample(map_between(center, source, bounds));
  }
}

/// Skia's `downscale_step_count`: how many resampling steps take an image to `scale`, all halving
/// but the last, which is dropped when it would barely shrink.
fn downscale_step_count(scale: f32) -> u32 {
  let mut steps = (1.0 / scale).ceil().log2().ceil() as u32;

  if steps > 0 {
    let last = scale * (1 << (steps - 1)) as f32;
    let limit = if steps == 1 { 1.0 - 1e-3 } else { 0.9 };

    if last >= limit {
      steps -= 1;
    }
  }
  steps
}

/// `rect` scaled by `factor_x` and `factor_y` about its centre, Skia's `scale_about_center`.
fn scale_about_center(rect: Bounds, factor_x: f32, factor_y: f32) -> Bounds {
  let center_x = if factor_x == 1.0 {
    0.0
  } else {
    0.5 * rect.left + 0.5 * rect.right
  };
  let center_y = if factor_y == 1.0 {
    0.0
  } else {
    0.5 * rect.top + 0.5 * rect.bottom
  };

  Bounds {
    left: center_x + factor_x * (rect.left - center_x),
    top: center_y + factor_y * (rect.top - center_y),
    right: center_x + factor_x * (rect.right - center_x),
    bottom: center_y + factor_y * (rect.bottom - center_y),
  }
}

/// `point` in `from` carried to the same place in `to`.
fn map_between(point: Point<f32>, from: Bounds, to: Bounds) -> Point<f32> {
  Point {
    x: to.left + (point.x - from.left) * to.width() / from.width(),
    y: to.top + (point.y - from.top) * to.height() / from.height(),
  }
}

/// A rectangle by its edges, in pixels that need not be whole.
#[derive(Clone, Copy)]
struct Bounds {
  left: f32,
  top: f32,
  right: f32,
  bottom: f32,
}

impl Bounds {
  fn width(self) -> f32 {
    self.right - self.left
  }

  fn height(self) -> f32 {
    self.bottom - self.top
  }
}

/// An image placed on the pixel grid, its top-left pixel at `left`, `top`.
struct Raster<const N: usize> {
  left: i64,
  top: i64,
  width: usize,
  height: usize,
  pixels: Vec<[u8; N]>,
}

impl<const N: usize> Raster<N> {
  /// The image drawn from `from` onto `to` with bilinear filtering, onto the whole pixels `to`
  /// touches, transparent past its edges.
  fn resampled(&self, from: Bounds, to: Bounds) -> Self {
    let (left, top) = (to.left.floor() as i64, to.top.floor() as i64);
    let width = (to.right.ceil() as i64 - left).max(0) as usize;
    let height = (to.bottom.ceil() as i64 - top).max(0) as usize;
    let pixels = (0..width * height)
      .map(|index| {
        let center = Point {
          x: (left + (index % width) as i64) as f32 + 0.5,
          y: (top + (index / width) as i64) as f32 + 0.5,
        };

        self.sample(map_between(center, to, from))
      })
      .collect();

    Self {
      left,
      top,
      width,
      height,
      pixels,
    }
  }

  /// The image filtered bilinearly at `point`, transparent past its edges.
  fn sample(&self, point: Point<f32>) -> [u8; N] {
    let x = point.x - self.left as f32 - 0.5;
    let y = point.y - self.top as f32 - 0.5;
    let (column, row) = (x.floor(), y.floor());
    let (weight_x, weight_y) = (x - column, y - row);
    let texel = |column: f32, row: f32| -> [f32; N] {
      if column < 0.0 || row < 0.0 || column >= self.width as f32 || row >= self.height as f32 {
        return [0.0; N];
      }
      self.pixels[row as usize * self.width + column as usize].map(f32::from)
    };
    let (top_left, top_right) = (texel(column, row), texel(column + 1.0, row));
    let (bottom_left, bottom_right) = (texel(column, row + 1.0), texel(column + 1.0, row + 1.0));

    array::from_fn(|channel| {
      let top = top_left[channel] + (top_right[channel] - top_left[channel]) * weight_x;
      let bottom = bottom_left[channel] + (bottom_right[channel] - bottom_left[channel]) * weight_x;

      (top + (bottom - top) * weight_y + 0.5).clamp(0.0, 255.0) as u8
    })
  }
}

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
  use super::{MAX_SIGMA, blur_alpha, blur_rgba, box_window, downscale_step_count};

  #[test]
  fn the_box_window_follows_skia() {
    assert_eq!(box_window(2.0), 4);
    assert_eq!(box_window(5.0), 9);
    assert!(box_window(MAX_SIGMA) < 255);
  }

  #[test]
  fn a_rescale_halves_until_the_last_step() {
    assert_eq!(downscale_step_count(1.0), 0);
    assert_eq!(downscale_step_count(0.9995), 0);
    assert_eq!(downscale_step_count(135.0 / 160.0), 1);
    assert_eq!(downscale_step_count(0.25), 2);
    assert_eq!(downscale_step_count(0.46), 1);
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

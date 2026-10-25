//! Gaussian blur as Skia's raster `SkBlurEngine` runs it for Chrome's filters and text shadows.
//! Follows Skia under the notice in LICENSE-CHROMIUM.

use std::{array, f32::consts::PI, ops::Range};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::geometry::{Placement, Point, Size};

/// Smallest sigma that blurs, Skia's `SkBlurEngine::IsEffectivelyIdentity`.
pub(crate) const MIN_SIGMA: f32 = 0.03;

/// Largest sigma a three-box pass sums without overflowing a `u32`, Skia's `kMaxSigma`.
pub(crate) const MAX_SIGMA: f32 = 135.0;

/// Blurs premultiplied RGBA pixels in place, treating pixels past the edges as transparent.
// Inlined so the loops compile at the caller's opt-level, not takumi-core's "z".
#[inline]
pub fn blur_rgba(pixels: &mut [[u8; 4]], width: usize, height: usize, sigma: f32) {
  blur_rgba_axes(pixels, width, height, Point { x: sigma, y: sigma });
}

/// Blurs premultiplied RGBA pixels in place with a sigma for each axis, as `feGaussianBlur` has.
#[inline]
pub(crate) fn blur_rgba_axes(
  pixels: &mut [[u8; 4]],
  width: usize,
  height: usize,
  sigma: Point<f32>,
) {
  blur(pixels, width, height, sigma, Rounding::Rgba);
}

/// Blurs an alpha mask in place, as [`blur_rgba`] does.
#[inline]
pub fn blur_alpha(alpha: &mut [u8], width: usize, height: usize, sigma: f32) {
  blur(
    alpha.as_chunks_mut::<1>().0,
    width,
    height,
    Point { x: sigma, y: sigma },
    Rounding::Alpha,
  );
}

/// Blurs `mask`, placed at `mask_at` in an `image` of transparent pixels, and returns the pixels
/// `region` covers as [`blur_alpha`] leaves them, without allocating or blurring the rest.
///
/// `mask_at` and `region` lie in the image, with the top-left pixel at the origin.
pub fn blur_alpha_region(
  mask: &[u8],
  mask_at: Placement,
  image: Size<u32>,
  region: Placement,
  sigma: f32,
) -> Vec<u8> {
  let sigma = if sigma.is_finite() { sigma } else { 0.0 };
  let region = Window::of(region);
  let mask_window = Window::of(mask_at);

  if sigma > MAX_SIGMA {
    let pixels = mask.as_chunks::<1>().0.to_vec();
    let source = Raster {
      left: mask_window.columns.start,
      top: mask_window.rows.start,
      width: mask_window.width(),
      height: mask_window.height(),
      pixels,
    };
    let size = (image.width as usize, image.height as usize);

    return rescaled(
      source,
      size,
      &region,
      Point { x: sigma, y: sigma },
      Rounding::Alpha,
    )
    .as_flattened()
    .to_vec();
  }

  // Every pass reads a pixel's neighbours no further than three sigma away, plus a rounding margin.
  let reach = (3.0 * sigma).ceil() as i64 + 4;
  let image_window = Window::of(Placement {
    left: 0,
    top: 0,
    width: image.width,
    height: image.height,
  });
  let crop = region.grown(reach).intersection(&image_window);
  let mut pixels = vec![[0u8; 1]; crop.width() * crop.height()];
  let overlap = mask_window.intersection(&crop);

  if !overlap.is_empty() {
    for row in overlap.rows.clone() {
      let source = (row - mask_window.rows.start) * mask_window.width() as i64
        + (overlap.columns.start - mask_window.columns.start);
      let target = (row - crop.rows.start) * crop.width() as i64
        + (overlap.columns.start - crop.columns.start);
      let source = &mask[source as usize..][..overlap.width()];

      for (pixel, &alpha) in pixels[target as usize..][..overlap.width()]
        .iter_mut()
        .zip(source)
      {
        *pixel = [alpha];
      }
    }
  }

  blur(
    &mut pixels,
    crop.width(),
    crop.height(),
    Point { x: sigma, y: sigma },
    Rounding::Alpha,
  );

  let inside = region.intersection(&crop);
  let mut blurred = Vec::with_capacity(inside.width() * inside.height());

  for row in inside.rows.clone() {
    let start =
      (row - crop.rows.start) * crop.width() as i64 + (inside.columns.start - crop.columns.start);

    blurred.extend(
      pixels[start as usize..][..inside.width()]
        .iter()
        .map(|pixel| pixel[0]),
    );
  }
  blurred
}

fn blur<const N: usize>(
  pixels: &mut [[u8; N]],
  width: usize,
  height: usize,
  sigma: Point<f32>,
  rounding: Rounding,
) {
  if width == 0 || height == 0 || pixels.len() != width * height {
    return;
  }

  let sigma = Point {
    x: if sigma.x.is_finite() { sigma.x } else { 0.0 },
    y: if sigma.y.is_finite() { sigma.y } else { 0.0 },
  };

  if sigma.x.max(sigma.y) > MAX_SIGMA {
    return blur_rescaled(pixels, width, height, sigma, rounding);
  }

  let parallel = pixels.len() * N >= PARALLEL_CHANNELS;

  // Rows blur as the columns of a transposed strip, so both axes run the vectorized column pass.
  if let Some(pass) = Pass::new(sigma.x, rounding) {
    for_each_with(
      pixels.chunks_mut(ROW_STRIP * width).collect(),
      parallel,
      || vec![[0u8; N]; ROW_STRIP * width],
      |strip, rows| {
        let lanes = rows.len() / width;
        let strip = &mut strip[..rows.len()];

        for (lane, row) in rows.chunks_exact(width).enumerate() {
          for (column, pixel) in strip.chunks_exact_mut(lanes).zip(row) {
            column[lane] = *pixel;
          }
        }
        pass.blur_columns(strip, lanes, false);
        for (lane, row) in rows.chunks_exact_mut(width).enumerate() {
          for (column, pixel) in strip.chunks_exact(lanes).zip(row) {
            *pixel = column[lane];
          }
        }
      },
    );
  }
  if let Some(pass) = Pass::new(sigma.y, rounding) {
    pass.blur_columns(pixels, width, parallel);
  }
}

/// Rows the horizontal pass transposes at once.
const ROW_STRIP: usize = 16;

/// Smallest image, in channels, worth spreading across threads; a glyph's shadow mask stays on one.
const PARALLEL_CHANNELS: usize = 1 << 18;

/// Runs `work` on each item with scratch state from `init`, across threads when `parallel` and
/// built with `rayon`.
fn for_each_with<T: Send, S>(
  items: Vec<T>,
  #[cfg_attr(not(feature = "rayon"), allow(unused_variables))] parallel: bool,
  init: impl Fn() -> S + Send + Sync,
  work: impl Fn(&mut S, T) + Send + Sync,
) {
  #[cfg(feature = "rayon")]
  if parallel {
    items.into_par_iter().for_each_init(init, work);
    return;
  }

  let mut state = init();

  items.into_iter().for_each(|item| work(&mut state, item));
}

/// Shrinks, blurs and scales back past `MAX_SIGMA`, as Skia's `FilterResult::Builder::blur` does.
fn blur_rescaled<const N: usize>(
  pixels: &mut [[u8; N]],
  width: usize,
  height: usize,
  sigma: Point<f32>,
  rounding: Rounding,
) {
  let source = Raster {
    left: 0,
    top: 0,
    width,
    height,
    pixels: pixels.to_vec(),
  };
  let whole = Window {
    columns: 0..width as i64,
    rows: 0..height as i64,
  };

  pixels.copy_from_slice(&rescaled(source, (width, height), &whole, sigma, rounding));
}

/// The pixels of `region` that `source`, in an image of `size` otherwise transparent, blurs to
/// past `MAX_SIGMA`.
///
/// Approximate: resamples in `f32`, where Skia's raster pipeline may filter at eight bits.
fn rescaled<const N: usize>(
  source: Raster<N>,
  (width, height): (usize, usize),
  region: &Window,
  sigma: Point<f32>,
  rounding: Rounding,
) -> Vec<[u8; N]> {
  let scale = Point {
    x: (MAX_SIGMA / sigma.x).min(1.0),
    y: (MAX_SIGMA / sigma.y).min(1.0),
  };
  let steps = Point {
    x: downscale_step_count(scale.x),
    y: downscale_step_count(scale.y),
  };
  let image_bounds = Bounds {
    left: 0.0,
    top: 0.0,
    right: width as f32,
    bottom: height as f32,
  };
  let mut bounds = image_bounds;
  let mut image = source;

  for step in (0..steps.x.max(steps.y)).rev() {
    let factor = |steps: u32, scale: f32, source: f32, bounds: f32| {
      if step >= steps {
        1.0
      } else if step > 0 {
        0.5
      } else {
        source * scale / bounds
      }
    };
    let target = scale_about_center(
      bounds,
      factor(steps.x, scale.x, image_bounds.width(), bounds.width()),
      factor(steps.y, scale.y, image_bounds.height(), bounds.height()),
    );

    image = image.resampled(bounds, target);
    bounds = target;
  }

  let mut image = image.within(Window::covering(bounds));
  let low_sigma = Point {
    x: (sigma.x * bounds.width() / image_bounds.width()).min(MAX_SIGMA),
    y: (sigma.y * bounds.height() / image_bounds.height()).min(MAX_SIGMA),
  };

  blur(
    &mut image.pixels,
    image.width,
    image.height,
    low_sigma,
    rounding,
  );

  region
    .pixels()
    .map(|(column, row)| {
      let center = Point {
        x: column as f32 + 0.5,
        y: row as f32 + 0.5,
      };

      image.sample(map_between(center, image_bounds, bounds))
    })
    .collect()
}

/// Skia's `downscale_step_count`.
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

/// Skia's `scale_about_center`.
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

/// `point` carried from `from` to `to`.
fn map_between(point: Point<f32>, from: Bounds, to: Bounds) -> Point<f32> {
  Point {
    x: to.left + (point.x - from.left) * to.width() / from.width(),
    y: to.top + (point.y - from.top) * to.height() / from.height(),
  }
}

/// A rectangle by its fractional edges.
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

/// A rectangle of pixels by its columns and rows.
#[derive(Clone)]
struct Window {
  columns: Range<i64>,
  rows: Range<i64>,
}

impl Window {
  fn of(placement: Placement) -> Self {
    let (left, top) = (i64::from(placement.left), i64::from(placement.top));

    Self {
      columns: left..left + i64::from(placement.width),
      rows: top..top + i64::from(placement.height),
    }
  }

  /// The pixels `bounds` touches.
  fn covering(bounds: Bounds) -> Self {
    let (left, top) = (bounds.left.floor() as i64, bounds.top.floor() as i64);

    Self {
      columns: left..(bounds.right.ceil() as i64).max(left),
      rows: top..(bounds.bottom.ceil() as i64).max(top),
    }
  }

  fn width(&self) -> usize {
    (self.columns.end - self.columns.start).max(0) as usize
  }

  fn height(&self) -> usize {
    (self.rows.end - self.rows.start).max(0) as usize
  }

  fn is_empty(&self) -> bool {
    self.width() == 0 || self.height() == 0
  }

  fn grown(&self, by: i64) -> Self {
    Self {
      columns: self.columns.start - by..self.columns.end + by,
      rows: self.rows.start - by..self.rows.end + by,
    }
  }

  fn intersection(&self, other: &Self) -> Self {
    Self {
      columns: self.columns.start.max(other.columns.start)..self.columns.end.min(other.columns.end),
      rows: self.rows.start.max(other.rows.start)..self.rows.end.min(other.rows.end),
    }
  }

  /// Each pixel's column and row, row by row.
  fn pixels(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
    self
      .rows
      .clone()
      .flat_map(|row| self.columns.clone().map(move |column| (column, row)))
  }
}

/// The pixels along one axis of `target` whose taps can land on the `source` pixels from its
/// origin, when `from` (the edge and extent) maps onto `to`.
fn reaching(
  (origin, size): (i64, usize),
  (from_edge, from_extent): (f32, f32),
  (to_edge, to_extent): (f32, f32),
  target: &Range<i64>,
) -> Range<i64> {
  let map = |position: f32| to_edge + (position - from_edge) * to_extent / from_extent;
  let (low, high) = (
    map(origin as f32 - 1.0),
    map((origin + size as i64) as f32 + 1.0),
  );

  if !low.is_finite() || !high.is_finite() {
    return target.clone();
  }

  let (low, high) = (low.min(high), low.max(high));

  (low.floor() as i64 - 2).max(target.start)..(high.ceil() as i64 + 2).min(target.end)
}

/// An image whose top-left pixel sits at `left`, `top`.
struct Raster<const N: usize> {
  left: i64,
  top: i64,
  width: usize,
  height: usize,
  pixels: Vec<[u8; N]>,
}

impl<const N: usize> Raster<N> {
  /// The image mapped from `from` onto the pixels `to` touches, filtered bilinearly, as far as
  /// its taps reach this image and no further.
  fn resampled(&self, from: Bounds, to: Bounds) -> Self {
    let touched = Window::covering(to);
    let window = Window {
      columns: reaching(
        (self.left, self.width),
        (from.left, from.width()),
        (to.left, to.width()),
        &touched.columns,
      ),
      rows: reaching(
        (self.top, self.height),
        (from.top, from.height()),
        (to.top, to.height()),
        &touched.rows,
      ),
    };
    let pixels = window
      .pixels()
      .map(|(column, row)| {
        let center = Point {
          x: column as f32 + 0.5,
          y: row as f32 + 0.5,
        };

        self.sample(map_between(center, to, from))
      })
      .collect();

    Self {
      left: window.columns.start,
      top: window.rows.start,
      width: window.width(),
      height: window.height(),
      pixels,
    }
  }

  /// The image on a transparent raster of `window`, which holds it.
  fn within(self, window: Window) -> Self {
    let (width, height) = (window.width(), window.height());
    let mut pixels = vec![[0u8; N]; width * height];
    let columns =
      self.left.max(window.columns.start)..(self.left + self.width as i64).min(window.columns.end);

    if !columns.is_empty() {
      let length = (columns.end - columns.start) as usize;

      for row in 0..self.height {
        let y = self.top + row as i64 - window.rows.start;

        if y < 0 || y >= height as i64 {
          continue;
        }

        let from = row * self.width + (columns.start - self.left) as usize;
        let to = y as usize * width + (columns.start - window.columns.start) as usize;

        pixels[to..to + length].copy_from_slice(&self.pixels[from..from + length]);
      }
    }

    Self {
      left: window.columns.start,
      top: window.rows.start,
      width,
      height,
      pixels,
    }
  }

  /// The image filtered bilinearly at `point`, transparent outside.
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

/// Where a three-box pass adds its rounding half: Skia's RGBA pass to the sum, its alpha pass after.
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
    if !sigma.is_finite() || sigma <= MIN_SIGMA {
      return None;
    }

    if sigma < 2.0 {
      return Some(Self::Gaussian(GaussianPass::new(sigma)));
    }
    ThreeBoxPass::new(sigma.min(MAX_SIGMA), rounding).map(Self::ThreeBox)
  }

  fn blur_columns<const N: usize>(&self, pixels: &mut [[u8; N]], width: usize, parallel: bool) {
    match self {
      Self::Gaussian(pass) => pass.blur_columns(pixels, width, parallel),
      Self::ThreeBox(pass) => pass.blur_columns(pixels, width, parallel),
    }
  }
}

/// Runs `step` down a band's `rows`: the row entering, zeros past the bottom, and the row
/// `border` behind it, if any.
fn for_each_row<const N: usize>(
  rows: &mut [&mut [[u8; N]]],
  border: usize,
  mut step: impl FnMut(&[[u8; N]], Option<&mut [[u8; N]]>),
) {
  let zeros = vec![[0u8; N]; rows.first().map_or(0, |row| row.len())];

  for index in 0..rows.len() + border {
    // The border is at least a pixel, so the row written lies wholly before the one read.
    let (written, unread) = rows.split_at_mut(index.min(rows.len()));
    let entering = unread.first().map_or(&zeros[..], |row| &**row);
    let target = index
      .checked_sub(border)
      .map(|target| &mut *written[target]);

    step(entering, target);
  }
}

/// The rows of each band of columns a column pass runs down, each at most `BAND_LANES` channels
/// wide, so bands blur independently.
fn column_bands<const N: usize>(pixels: &mut [[u8; N]], width: usize) -> Vec<Vec<&mut [[u8; N]]>> {
  let band = band_columns::<N>(width);
  let mut bands: Vec<Vec<_>> = (0..width.div_ceil(band))
    .map(|_| Vec::with_capacity(pixels.len() / width))
    .collect();

  for row in pixels.chunks_exact_mut(width) {
    for (rows, piece) in bands.iter_mut().zip(row.chunks_mut(band)) {
      rows.push(piece);
    }
  }
  bands
}

/// Columns in the widest band of an image `width` pixels wide.
fn band_columns<const N: usize>(width: usize) -> usize {
  width.min(BAND_LANES / N)
}

/// Channels a pass blurs at once, which bounds its rings by the band rather than the image width.
const BAND_LANES: usize = 256;

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

  fn blur_columns<const N: usize>(&self, pixels: &mut [[u8; N]], width: usize, parallel: bool) {
    let window = self.kernel.len();
    // Every channel of a band's row is a lane; `next` is the oldest row in the ring.
    let lanes = band_columns::<N>(width) * N;

    for_each_with(
      column_bands(pixels, width),
      parallel,
      || (vec![0.0f32; window * lanes], vec![0.0f32; lanes]),
      |(ring, sums), mut rows| {
        let channels = rows[0].len() * N;
        let mut next = 0;

        ring.fill(0.0);
        for_each_row(&mut rows, window / 2, |entering, target| {
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
          let sums = &mut sums[..channels];

          sums.fill(0.0);
          for (offset, weight) in self.kernel.iter().enumerate() {
            let slot = (next + offset) % window;

            for (sum, sample) in sums.iter_mut().zip(&ring[slot * channels..][..channels]) {
              *sum += sample * weight;
            }
          }
          for (channel, sum) in target.as_flattened_mut().iter_mut().zip(sums.iter()) {
            *channel = (sum * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
          }
        });
      },
    );
  }
}

/// Skia's `ThreeBoxApproxPass` and `A8Pass`: three `window`-wide boxes summed in one pass.
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

  fn blur_columns<const N: usize>(&self, pixels: &mut [[u8; N]], width: usize, parallel: bool) {
    let pass = self.window - 1;
    let last = if self.window.is_multiple_of(2) {
      pass + 1
    } else {
      pass
    };
    // Every channel of a band's row is a lane; each box drops its trailing edge `pass`, `pass`
    // and `last` rows back.
    let lanes = band_columns::<N>(width) * N;

    for_each_with(
      column_bands(pixels, width),
      parallel,
      || BoxRings::new(lanes, pass, last),
      |rings, mut rows| {
        let channels = rows[0].len() * N;
        let (mut slot, mut last_slot) = (0, 0);

        rings.reset(self.seed);

        let BoxRings {
          sum0,
          sum1,
          sum2,
          first,
          second,
          third,
          blurred,
        } = rings;

        for_each_row(&mut rows, self.border, |entering, target| {
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
            blurred[lane] =
              ((u64::from(sum2[lane]) * u64::from(self.factor) + self.bias) >> 32) as u8;
            sum2[lane] -= third[lane];
            third[lane] = sum1[lane];
            sum1[lane] -= u32::from(second[lane]);
            second[lane] = sum0[lane] as u16;
            sum0[lane] -= u32::from(first[lane]);
            first[lane] = entering[lane];
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
      },
    );
  }
}

/// A three-box pass's running sums and the rings of what each box drops, one lane per channel.
struct BoxRings {
  sum0: Vec<u32>,
  sum1: Vec<u32>,
  sum2: Vec<u32>,
  // A pixel, then a sum of at most `window` of them, which fits a `u16` since `MAX_SIGMA` keeps
  // `window` under 255.
  first: Vec<u8>,
  second: Vec<u16>,
  third: Vec<u32>,
  blurred: Vec<u8>,
}

impl BoxRings {
  fn new(lanes: usize, pass: usize, last: usize) -> Self {
    Self {
      sum0: vec![0; lanes],
      sum1: vec![0; lanes],
      sum2: vec![0; lanes],
      first: vec![0; pass * lanes],
      second: vec![0; pass * lanes],
      third: vec![0; last * lanes],
      blurred: vec![0; lanes],
    }
  }

  /// Empties the rings for a new band, the last sum starting at `seed`.
  fn reset(&mut self, seed: u32) {
    self.sum0.fill(0);
    self.sum1.fill(0);
    self.sum2.fill(seed);
    self.first.fill(0);
    self.second.fill(0);
    self.third.fill(0);
  }
}

/// Skia's `SkBlurEngine::BoxBlurWindow`.
fn box_window(sigma: f32) -> usize {
  ((sigma * 3.0 * (2.0 * PI).sqrt() / 4.0 + 0.5).floor() as usize).max(1)
}

#[cfg(test)]
mod tests {
  use super::{
    BAND_LANES, MAX_SIGMA, Placement, Point, Size, blur_alpha, blur_alpha_region, blur_rgba,
    blur_rgba_axes, box_window, downscale_step_count,
  };

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
  fn a_band_edge_leaves_no_seam() {
    let (width, height) = (BAND_LANES * 2 + 40, 48);

    for sigma in [1.0, 4.0] {
      // Ink along the bottom edge leaves a band's rings full when it ends.
      let mut alpha = vec![0u8; width * height];

      alpha[(height - 8) * width..].fill(255);
      blur_alpha(&mut alpha, width, height, sigma);

      for row in alpha.chunks_exact(width) {
        for column in [BAND_LANES - 1, BAND_LANES, BAND_LANES * 2] {
          assert_eq!(row[column], row[width / 2]);
        }
      }
    }
  }

  #[test]
  fn a_sigma_of_zero_on_one_axis_leaves_that_axis_alone() {
    let (width, height) = (15, 7);
    let mut pixels = vec![[0u8; 4]; width * height];

    pixels[3 * width + 7] = [255; 4];
    blur_rgba_axes(&mut pixels, width, height, Point { x: 2.0, y: 0.0 });

    assert!(pixels[3 * width + 5][3] > 0);
    assert!(pixels[2 * width + 7].iter().all(|channel| *channel == 0));
    assert!(pixels[4 * width + 7].iter().all(|channel| *channel == 0));

    let mut pixels = vec![[0u8; 4]; width * height];

    pixels[3 * width + 7] = [255; 4];
    blur_rgba_axes(&mut pixels, width, height, Point { x: 0.0, y: 2.0 });

    assert!(pixels[width + 7][3] > 0);
    assert!(pixels[3 * width + 6].iter().all(|channel| *channel == 0));
  }

  #[test]
  fn a_sigma_past_the_limit_on_one_axis_still_blurs_the_other() {
    let (width, height) = (32, 32);
    let mut pixels = vec![[0u8; 4]; width * height];

    pixels.iter_mut().for_each(|pixel| *pixel = [255; 4]);
    blur_rgba_axes(
      &mut pixels,
      width,
      height,
      Point {
        x: MAX_SIGMA * 2.0,
        y: 1.0,
      },
    );

    assert!(pixels[16 * width + 16][3] > 0);
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

  /// A region blurs to the pixels the whole image blurs to, below the sigma limit and past it.
  #[test]
  fn a_region_blurs_as_the_whole_image_does() {
    let image = Size {
      width: 360,
      height: 280,
    };
    let mask_at = Placement {
      left: 90,
      top: 70,
      width: 113,
      height: 61,
    };
    let mask: Vec<u8> = (0..mask_at.width * mask_at.height)
      .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
      .collect();
    let regions = [
      Placement {
        left: 0,
        top: 0,
        width: 360,
        height: 280,
      },
      Placement {
        left: 17,
        top: 203,
        width: 150,
        height: 77,
      },
      Placement {
        left: 330,
        top: 3,
        width: 30,
        height: 40,
      },
    ];

    for sigma in [
      0.0,
      0.5,
      1.7,
      3.0,
      11.0,
      40.0,
      MAX_SIGMA - 1.0,
      MAX_SIGMA + 0.5,
      190.0,
      400.0,
    ] {
      let mut whole = vec![0u8; (image.width * image.height) as usize];

      for (row, source) in mask.chunks_exact(mask_at.width as usize).enumerate() {
        let start = (mask_at.top as usize + row) * image.width as usize + mask_at.left as usize;

        whole[start..start + source.len()].copy_from_slice(source);
      }
      blur_alpha(
        &mut whole,
        image.width as usize,
        image.height as usize,
        sigma,
      );

      for region in regions {
        let expected: Vec<u8> = (region.top..region.bottom())
          .flat_map(|row| {
            let start = row as usize * image.width as usize + region.left as usize;

            whole[start..start + region.width as usize].iter().copied()
          })
          .collect();

        assert_eq!(
          blur_alpha_region(&mask, mask_at, image, region, sigma),
          expected,
          "sigma {sigma} region {region:?}"
        );
      }
    }
  }
}

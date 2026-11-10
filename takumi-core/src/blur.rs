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

/// The pixels of `region` that [`blur_alpha`] leaves of `mask` at `mask_at`, blurring nothing else.
///
/// A box's shadow is a mask whose middle rows are all alike, and its middle columns too. The blur
/// of such lines is one line repeated, so the mask blurs without them: see [`Fold`].
pub fn blur_alpha_region(
  mask: &[u8],
  mask_at: Placement,
  image: Size<u32>,
  region: Placement,
  sigma: f32,
) -> Vec<u8> {
  let sigma = if sigma.is_finite() { sigma } else { 0.0 };
  let region = Window::from(region);
  let mask_window = Window::from(mask_at);

  if mask.len() != mask_window.width() * mask_window.height() {
    return vec![0; region.width() * region.height()];
  }

  let mask = mask.as_chunks::<1>().0;

  if sigma > MAX_SIGMA {
    let size = (image.width as usize, image.height as usize);
    let source = Raster {
      left: mask_window.columns.start,
      top: mask_window.rows.start,
      width: mask_window.width(),
      height: mask_window.height(),
      pixels: mask.to_vec(),
    };

    return rescaled(
      source,
      size,
      &region,
      Point { x: sigma, y: sigma },
      Rounding::Alpha,
    )
    .into_flattened();
  }

  // Every pass reads a pixel's neighbours no further than three sigma away, plus a rounding margin.
  let reach = (3.0 * sigma).ceil() as i64 + 4;
  let image_window = Window::from(Placement {
    left: 0,
    top: 0,
    width: image.width,
    height: image.height,
  });
  let crop = region.grown(reach).intersection(&image_window);
  let cropped = Raster::part(mask, &mask_window, crop.clone());
  let kept = region.intersection(&crop);
  let folds = Folds::of(&cropped, reach as usize);
  let mut folded = match &folds {
    Some(folds) => folds.fold(&cropped),
    None => cropped,
  };

  blur(
    &mut folded.pixels,
    folded.width,
    folded.height,
    Point { x: sigma, y: sigma },
    Rounding::Alpha,
  );

  match folds {
    Some(folds) => folds.unfold(&folded, &crop, &kept),
    None => folded.within(kept).pixels.into_flattened(),
  }
}

/// Fewest lines a [`Fold`] takes out; under it the blur saves less than the fold costs.
const MIN_FOLD: usize = 16;

/// The middle of a run of equal lines, rows or columns, longer than a blur reaches both ways. A
/// pixel's blur reads no further than `reach` lines away, so every line that far inside the run
/// blurs to the same pixels. A raster without all but one of them blurs to the pixels the whole
/// one does, with that one line standing for the rest.
#[derive(Clone, Copy)]
struct Fold {
  /// The first line taken out. The line before it stands for them all.
  start: usize,
  /// How many lines are taken out.
  len: usize,
}

impl Fold {
  /// The fold of the longest run among `lines` lines, where `equals_previous` tells whether a line
  /// equals the one before it.
  fn of(lines: usize, reach: usize, equals_previous: impl Fn(usize) -> bool) -> Option<Self> {
    let keep = 2 * reach + 1;
    let mut longest = 0..0;
    let mut run_start = 0;

    for line in 1..=lines {
      if line == lines || !equals_previous(line) {
        if line - run_start > longest.len() {
          longest = run_start..line;
        }
        run_start = line;
      }
    }

    (longest.len() >= keep + MIN_FOLD).then(|| Self {
      start: longest.start + reach + 1,
      len: longest.len() - keep,
    })
  }

  /// Where the folded raster keeps what line `line` blurs to.
  fn folded(self, line: usize) -> usize {
    if line < self.start {
      line
    } else if line < self.start + self.len {
      self.start - 1
    } else {
      line - self.len
    }
  }

  /// A fold that takes nothing out of `lines` lines.
  fn none(lines: usize) -> Self {
    Self {
      start: lines,
      len: 0,
    }
  }
}

/// The rows and columns an alpha raster blurs without.
struct Folds {
  rows: Fold,
  columns: Fold,
}

impl Folds {
  /// The folds of `raster` for a blur reaching `reach` pixels, when it has a run to fold.
  fn of(raster: &Raster<1>, reach: usize) -> Option<Self> {
    let (width, height) = (raster.width, raster.height);
    let keep = 2 * reach + 1;

    if width.max(height) < keep + MIN_FOLD || width == 0 {
      return None;
    }

    let row = |row: usize| &raster.pixels[row * width..(row + 1) * width];
    let rows = Fold::of(height, reach, |line| row(line) == row(line - 1));
    let columns = (width >= keep + MIN_FOLD)
      .then(|| {
        // Whether each column differs from the one before it in any row.
        let mut differs = vec![0u8; width];

        for row in raster.pixels.chunks_exact(width) {
          for (differs, pair) in differs[1..].iter_mut().zip(row.windows(2)) {
            *differs |= u8::from(pair[0] != pair[1]);
          }
        }
        Fold::of(width, reach, |line| differs[line] == 0)
      })
      .flatten();

    (rows.is_some() || columns.is_some()).then(|| Self {
      rows: rows.unwrap_or(Fold::none(height)),
      columns: columns.unwrap_or(Fold::none(width)),
    })
  }

  /// `raster` without the folded rows and columns.
  fn fold(&self, raster: &Raster<1>) -> Raster<1> {
    let (width, height) = (
      raster.width - self.columns.len,
      raster.height - self.rows.len,
    );
    let mut pixels = Vec::with_capacity(width * height);
    let after = self.columns.start + self.columns.len;

    for (index, row) in raster.pixels.chunks_exact(raster.width).enumerate() {
      if (self.rows.start..self.rows.start + self.rows.len).contains(&index) {
        continue;
      }
      pixels.extend_from_slice(&row[..self.columns.start]);
      pixels.extend_from_slice(&row[after..]);
    }

    Raster {
      left: raster.left,
      top: raster.top,
      width,
      height,
      pixels,
    }
  }

  /// The pixels of `kept`, a window inside `crop`, that a raster of `crop` blurs to, read from
  /// `blurred`, the blur of its fold.
  fn unfold(&self, blurred: &Raster<1>, crop: &Window, kept: &Window) -> Vec<u8> {
    let (width, height) = (kept.width(), kept.height());
    let mut pixels = vec![0u8; width * height];

    if width == 0 || height == 0 {
      return pixels;
    }

    let left = (kept.columns.start - crop.columns.start) as usize;
    let top = (kept.rows.start - crop.rows.start) as usize;
    let source = blurred.pixels.as_flattened();
    // Where the folded columns begin and end in a row of `kept`.
    let fold_start = self.columns.start.clamp(left, left + width) - left;
    let fold_end = (self.columns.start + self.columns.len).clamp(left, left + width) - left;

    for (index, row) in pixels.chunks_exact_mut(width).enumerate() {
      let folded_row = self.rows.folded(top + index);
      let from = &source[folded_row * blurred.width..(folded_row + 1) * blurred.width];

      row[..fold_start].copy_from_slice(&from[left..left + fold_start]);
      if fold_start < fold_end {
        row[fold_start..fold_end].fill(from[self.columns.start - 1]);
      }
      row[fold_end..].copy_from_slice(
        &from[left + fold_end - self.columns.len..left + width - self.columns.len],
      );
    }

    pixels
  }
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

  let parallel = pixels.len() * N >= PARALLEL_CHANNELS && crate::parallel::spreads();

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

/// The pixels of `region` that `source` blurs to past `MAX_SIGMA`. Approximate: resamples in `f32`.
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

impl From<Placement> for Window {
  fn from(placement: Placement) -> Self {
    let (left, top) = (i64::from(placement.left), i64::from(placement.top));

    Self {
      columns: left..left + i64::from(placement.width),
      rows: top..top + i64::from(placement.height),
    }
  }
}

impl Window {
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

  /// The part of the image `window` covers, on a transparent raster of `window`.
  fn within(self, window: Window) -> Self {
    let at = Window {
      columns: self.left..self.left + self.width as i64,
      rows: self.top..self.top + self.height as i64,
    };

    Self::part(&self.pixels, &at, window)
  }

  /// The part of `pixels`, an image filling `at`, that `window` covers, on a transparent raster of
  /// `window`.
  fn part(pixels: &[[u8; N]], at: &Window, window: Window) -> Self {
    let (width, height) = (window.width(), window.height());
    let (source_width, source_height) = (at.width(), at.height());
    let mut part = vec![[0u8; N]; width * height];
    let columns =
      at.columns.start.max(window.columns.start)..at.columns.end.min(window.columns.end);

    if !columns.is_empty() {
      let length = (columns.end - columns.start) as usize;

      for row in 0..source_height {
        let y = at.rows.start + row as i64 - window.rows.start;

        if y < 0 || y >= height as i64 {
          continue;
        }

        let from = row * source_width + (columns.start - at.columns.start) as usize;
        let to = y as usize * width + (columns.start - window.columns.start) as usize;

        part[to..to + length].copy_from_slice(&pixels[from..from + length]);
      }
    }

    Self {
      left: window.columns.start,
      top: window.rows.start,
      width,
      height,
      pixels: part,
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
    BAND_LANES, Folds, MAX_SIGMA, Placement, Point, Raster, Size, Window, blur_alpha,
    blur_alpha_region, blur_rgba, blur_rgba_axes, box_window, downscale_step_count,
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

  /// A mask that does not fill its placement blurs to a transparent region, as `blur` leaves
  /// pixels of the wrong length alone.
  #[test]
  fn a_short_mask_blurs_to_a_transparent_region() {
    let mask_at = Placement {
      left: 2,
      top: 2,
      width: 4,
      height: 4,
    };
    let region = Placement {
      left: 0,
      top: 0,
      width: 8,
      height: 8,
    };
    let image = Size {
      width: 8,
      height: 8,
    };

    for sigma in [2.0, MAX_SIGMA + 1.0] {
      assert_eq!(
        blur_alpha_region(&[255; 3], mask_at, image, region, sigma),
        [0; 64]
      );
    }
  }

  /// The coverage of a box with rounded corners, antialiased, as a shadow's mask is.
  fn rounded_box(width: u32, height: u32, radius: f32) -> Vec<u8> {
    (0..height)
      .flat_map(|row| (0..width).map(move |column| (column, row)))
      .map(|(column, row)| {
        let (x, y) = (column as f32 + 0.5, row as f32 + 0.5);
        let inside_x = (radius - x).max(x - (width as f32 - radius)).max(0.0);
        let inside_y = (radius - y).max(y - (height as f32 - radius)).max(0.0);
        let distance = (inside_x * inside_x + inside_y * inside_y).sqrt();

        ((radius - distance + 0.5).clamp(0.0, 1.0) * 255.0) as u8
      })
      .collect()
  }

  /// A box's mask blurs through its folds to the pixels the whole image blurs to, whichever of its
  /// rows and columns the region reaches.
  #[test]
  fn a_folded_box_blurs_as_the_whole_image_does() {
    let mut folded_both = 0;

    for (width, height, radius) in [
      (300, 220, 0.0),
      (300, 220, 24.0),
      (411, 97, 40.0),
      (64, 400, 9.0),
      (260, 260, 130.0),
      (173, 181, 3.5),
    ] {
      let mask = rounded_box(width, height, radius);

      for sigma in [0.6, 2.0, 7.5, 12.5, 30.0] {
        let reach = (3.0_f32 * sigma).ceil() as u32 + 4;
        let image = Size {
          width: width + 2 * reach + 11,
          height: height + 2 * reach + 9,
        };
        let mask_at = Placement {
          left: reach as i32 + 6,
          top: reach as i32 + 4,
          width,
          height,
        };
        let mut whole = vec![0u8; (image.width * image.height) as usize];

        for (row, source) in mask.chunks_exact(width as usize).enumerate() {
          let start = (mask_at.top as usize + row) * image.width as usize + mask_at.left as usize;

          whole[start..start + source.len()].copy_from_slice(source);
        }
        blur_alpha(
          &mut whole,
          image.width as usize,
          image.height as usize,
          sigma,
        );

        let regions = [
          // The whole image, its left part, a window inside the box, and strips across each fold.
          (0, 0, image.width, image.height),
          (0, 0, image.width / 2 + 3, image.height),
          (
            mask_at.left as u32 + width / 3,
            mask_at.top as u32 + height / 3,
            width / 3,
            height / 3,
          ),
          (0, image.height / 2 - 2, image.width, 5),
          (image.width / 2 - 2, 0, 5, image.height),
          (image.width - 40, image.height - 30, 40, 30),
        ];

        for (left, top, region_width, region_height) in regions {
          let region = Placement {
            left: left as i32,
            top: top as i32,
            width: region_width,
            height: region_height,
          };
          let expected: Vec<u8> = (region.top..region.bottom())
            .flat_map(|row| {
              let start = row as usize * image.width as usize + region.left as usize;

              whole[start..start + region.width as usize].iter().copied()
            })
            .collect();

          assert_eq!(
            blur_alpha_region(&mask, mask_at, image, region, sigma),
            expected,
            "box {width}x{height} radius {radius} sigma {sigma} region {region:?}"
          );
        }

        let window = Window::from(Placement {
          left: 0,
          top: 0,
          width: image.width,
          height: image.height,
        });
        let raster = Raster::part(mask.as_chunks::<1>().0, &Window::from(mask_at), window);

        if let Some(folds) = Folds::of(&raster, reach as usize)
          && folds.rows.len > 0
          && folds.columns.len > 0
        {
          folded_both += 1;
        }
      }
    }

    // The grid is only worth its name if most of it folds both ways.
    assert!(
      folded_both >= 12,
      "{folded_both} cases folded rows and columns"
    );
  }

  /// A mask of noise has no run to fold, and one with equal lines along one axis folds that axis.
  #[test]
  fn only_equal_lines_fold() {
    let window = |width: u32, height: u32| {
      Window::from(Placement {
        left: 0,
        top: 0,
        width,
        height,
      })
    };
    let noise: Vec<[u8; 1]> = (0..200u32 * 120)
      .map(|index| [(index.wrapping_mul(2_654_435_761) >> 13) as u8])
      .collect();
    let raster = Raster::part(&noise, &window(200, 120), window(200, 120));

    assert!(Folds::of(&raster, 5).is_none());

    // Every row alike: columns of noise, so only the rows fold.
    let striped: Vec<[u8; 1]> = (0..120).flat_map(|_| noise[..200].to_vec()).collect();
    let raster = Raster::part(&striped, &window(200, 120), window(200, 120));
    let folds = Folds::of(&raster, 5).unwrap();

    assert_eq!((folds.rows.start, folds.rows.len), (6, 120 - 11));
    assert_eq!(folds.columns.len, 0);
    assert_eq!(folds.fold(&raster).height, 11);
    // A run only as long as the blur reaches both ways, and a little more, stays.
    assert!(Folds::of(&raster, 52).is_none());
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

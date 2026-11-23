//! Gaussian blur by decimation, as filtra's `filtra_core::blur` plans it and its
//! `filtra_kernels::blur` runs it, under the notice in LICENSE-FILTRA. Each axis halves with
//! `[1, 3, 3, 1] / 8` until a variance of at most 4 is left, blurs that rest with a small Gaussian
//! and doubles back with `[1, 3] / 4`, so a pixel costs the same at any sigma. Approximate: Chrome
//! blurs with Skia's three-box passes, which this differs from by a few levels of 255, lying
//! closer to the exact Gaussian.

use std::{array, mem};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::geometry::{Placement, Point, Size};

/// Smallest sigma that blurs, Skia's `SkBlurEngine::IsEffectivelyIdentity`.
pub(crate) const MIN_SIGMA: f32 = 0.03;

/// How many pixels away a blur of `sigma` can carry a pixel's ink: an upper bound, linear so a
/// transform's scale carries it, past the three deviations Chrome's blur reaches.
pub(crate) fn reach(sigma: f32) -> f32 {
  if !sigma.is_finite() || sigma <= MIN_SIGMA {
    return 0.0;
  }

  REACH_PER_SIGMA * sigma + 1.0
}

/// The decimated support grows at most 4.9 pixels per unit of sigma, past one pixel.
const REACH_PER_SIGMA: f32 = 5.0;

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
  blur_in_place(pixels, width, height, sigma);
}

/// Blurs an alpha mask in place, as [`blur_rgba`] does.
#[inline]
pub fn blur_alpha(alpha: &mut [u8], width: usize, height: usize, sigma: f32) {
  blur_in_place(
    alpha.as_chunks_mut::<1>().0,
    width,
    height,
    Point { x: sigma, y: sigma },
  );
}

/// The pixels of `region` that [`blur_alpha`] leaves of `mask` at `mask_at`, blurring nothing else.
#[inline]
pub fn blur_alpha_region(
  mask: &[u8],
  mask_at: Placement,
  image: Size<u32>,
  region: Placement,
  sigma: f32,
) -> Vec<u8> {
  let mut pixels = vec![[0u8; 1]; region.width as usize * region.height as usize];

  if mask.len() != mask_at.width as usize * mask_at.height as usize {
    return pixels.into_flattened();
  }

  let image = spans(Placement {
    left: 0,
    top: 0,
    width: image.width,
    height: image.height,
  });
  let source = Image {
    pixels: mask.as_chunks::<1>().0,
    spans: spans(mask_at),
  };
  // The image clips the mask, as it would once drawn into it.
  let shown = intersect(source.spans, image);
  let clipped;
  let source = if shown == source.spans {
    source
  } else {
    clipped = source.part(shown);
    Image {
      pixels: &clipped,
      spans: shown,
    }
  };
  let region = spans(region);
  let blur = Blur::new(
    Point { x: sigma, y: sigma },
    source.spans,
    intersect(region, image),
  );

  blur.run(source, &mut pixels, region);
  pixels.into_flattened()
}

/// Blurs `width` × `height` pixels of `N` channels in place with a sigma for each axis.
#[inline(always)]
fn blur_in_place<const N: usize>(
  pixels: &mut [[u8; N]],
  width: usize,
  height: usize,
  sigma: Point<f32>,
) {
  if width == 0 || height == 0 || pixels.len() != width * height {
    return;
  }

  let rect = [Span(0, width as i64), Span(0, height as i64)];
  let blur = Blur::new(sigma, rect, rect);

  if blur == Blur::Unchanged {
    return;
  }

  let source = pixels.to_vec();

  blur.run(
    Image {
      pixels: &source,
      spans: rect,
    },
    pixels,
    rect,
  );
}

/// Pixels of `N` channels on a rectangle, row by row.
#[derive(Clone, Copy)]
struct Image<'a, const N: usize> {
  pixels: &'a [[u8; N]],
  spans: [Span; 2],
}

impl<'a, const N: usize> Image<'a, N> {
  /// Row `y`, if the image has it.
  fn row(&self, y: i64) -> Option<&'a [[u8; N]]> {
    let [x, rows] = self.spans;

    rows.contains(y).then(|| {
      let start = (y - rows.0) as usize * x.len();

      &self.pixels[start..start + x.len()]
    })
  }

  /// The pixels of `rect`, which lies inside the image.
  fn part(&self, rect: [Span; 2]) -> Vec<[u8; N]> {
    let left = (rect[0].0 - self.spans[0].0) as usize;

    (rect[1].0..rect[1].1)
      .filter_map(|y| self.row(y))
      .flat_map(|row| row[left..left + rect[0].len()].iter().copied())
      .collect()
  }

  /// Writes the image to `target`, the pixels of `rect`, transparent where it has none.
  fn copy_to(&self, target: &mut [[u8; N]], rect: [Span; 2]) {
    let overlap = rect[0].intersect(self.spans[0]);

    target.fill([0; N]);
    if overlap.is_empty() {
      return;
    }

    let columns = |x: Span| (overlap.0 - x.0) as usize..(overlap.1 - x.0) as usize;

    for (y, out) in (rect[1].0..).zip(target.chunks_exact_mut(rect[0].len())) {
      if let Some(row) = self.row(y) {
        out[columns(rect[0])].copy_from_slice(&row[columns(self.spans[0])]);
      }
    }
  }
}

/// A Gaussian blur as the passes that apply it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Blur {
  /// The input unchanged.
  Unchanged,
  /// A transparent result.
  Transparent,
  /// The input decimated.
  Decimated {
    /// The passes in execution order.
    passes: Vec<Pass>,
    /// The weights of the Gaussian along x and y from the centre outwards, in units of 2^-15.
    weights: [Vec<u16>; 2],
  },
}

impl Blur {
  /// The blur of `source` into the pixels of `target`, with a sigma for each axis.
  fn new(sigma: Point<f32>, source: [Span; 2], target: [Span; 2]) -> Self {
    let axes = [sigma.x, sigma.y].map(Axis::new);

    if axes.iter().all(|axis| axis.weights.is_empty()) {
      return Self::Unchanged;
    }

    // Halving and doubling each add a variance of 0.75 in the finer grid, and a variance in the
    // coarser grid is four times as large in the finer one, so the passes add up to the requested
    // variance. Each level has half the pixels of the one before along every halved axis.
    let levels = axes[0].halvings.max(axes[1].halvings);
    let layout: [Layout; 2] =
      array::from_fn(|a| Layout::new(&axes[a], levels, source[a], target[a]));

    if layout.iter().any(|axis| axis.up[0].is_empty()) {
      return Self::Transparent;
    }

    let resampling = |level: usize, op: Op| {
      [0, 1].map(|a| {
        if level < axes[a].halvings {
          op
        } else {
          Op::Copy
        }
      })
    };
    let gauss = |axis: &Axis| {
      if axis.weights.is_empty() {
        Op::Copy
      } else {
        Op::Gauss
      }
    };
    let down = |level: usize| [layout[0].down[level], layout[1].down[level]];
    let up = |level: usize| [layout[0].up[level], layout[1].up[level]];
    let passes = (0..levels)
      .map(|level| (resampling(level, Op::Halve), down(level + 1)))
      .chain([(axes.each_ref().map(gauss), up(levels))])
      .chain(
        (0..levels)
          .rev()
          .map(|level| (resampling(level, Op::Double), up(level))),
      )
      .map(|(ops, spans)| Pass { ops, spans })
      .collect();

    Self::Decimated {
      passes,
      weights: axes.map(|axis| axis.weights),
    }
  }

  /// Blurs `source` into `target`, the pixels of `rect`.
  fn run<const N: usize>(&self, source: Image<'_, N>, target: &mut [[u8; N]], rect: [Span; 2]) {
    let (passes, weights) = match self {
      Self::Unchanged => return source.copy_to(target, rect),
      Self::Transparent => return target.fill([0; N]),
      Self::Decimated { passes, weights } => (passes, weights.each_ref().map(Vec::as_slice)),
    };
    let Some((last, passes)) = passes.split_last() else {
      return;
    };
    let mut current: Option<(Vec<[u8; N]>, [Span; 2])> = None;
    // Only the image a pass reads and the one it writes are alive at a time, so the memory of the
    // image before goes to the next one.
    let mut spare = Vec::new();

    for pass in passes {
      let mut plane = mem::take(&mut spare);

      plane.clear();
      plane.resize(pass.spans[0].len() * pass.spans[1].len(), [0; N]);

      let input = current.as_ref().map_or(source, |(pixels, spans)| Image {
        pixels,
        spans: *spans,
      });

      pass.fill(input, weights, &mut plane, pass.spans);
      if let Some((previous, _)) = current.replace((plane, pass.spans)) {
        spare = previous;
      }
    }

    let input = current.as_ref().map_or(source, |(pixels, spans)| Image {
      pixels,
      spans: *spans,
    });

    last.fill(input, weights, target, rect);
  }
}

/// One pass of a blur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pass {
  /// The operations along x and y.
  ops: [Op; 2],
  /// The pixels the pass computes along x and y.
  spans: [Span; 2],
}

impl Pass {
  /// Writes the pass over `input` to `out`, the pixels of `rect`, transparent outside its spans.
  fn fill<const N: usize>(
    &self,
    input: Image<'_, N>,
    weights: [&[u16]; 2],
    out: &mut [[u8; N]],
    rect: [Span; 2],
  ) {
    let [x, y] = self.spans;
    let width = rect[0].len();

    if width == 0 {
      return;
    }

    let columns = (x.0 - rect[0].0) as usize..(x.1 - rect[0].0) as usize;
    let parallel = out.len() * N >= PARALLEL_CHANNELS;

    for_each_with(
      out.chunks_mut(BAND_ROWS * width).enumerate().collect(),
      parallel,
      || Rows::new(input, self.ops, weights, x),
      |rows, (band, pixels)| {
        let top = rect[1].0 + (band * BAND_ROWS) as i64;

        for (row_y, row) in (top..).zip(pixels.chunks_exact_mut(width)) {
          if x.is_empty() || !y.contains(row_y) {
            row.fill([0; N]);
            continue;
          }

          row[..columns.start].fill([0; N]);
          rows.row(row_y, &mut row[columns.clone()]);
          row[columns.end..].fill([0; N]);
        }
      },
    );
  }
}

/// Rows of a pass a thread computes at once.
const BAND_ROWS: usize = 64;

/// Smallest pass, in channels, worth spreading across threads; a glyph's shadow mask stays on one.
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

/// The operation of a pass along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
  /// The pixels unchanged.
  Copy,
  /// [1, 3, 3, 1] / 8 to every other pixel.
  Halve,
  /// [1, 3] / 4 and [3, 1] / 4 between the pixels.
  Double,
  /// The Gaussian with the weights of the axis.
  Gauss,
}

impl Op {
  /// The fractional bits of the horizontal result.
  fn fraction(self) -> u32 {
    match self {
      Op::Copy => 0,
      Op::Halve => 3,
      Op::Double => 2,
      Op::Gauss => GAUSS_FRACTION,
    }
  }

  /// The first input row and the number of rows that output row `y` reads.
  fn rows(self, y: i64, weights: &[u16]) -> (i64, usize) {
    match self {
      Op::Copy => (y, 1),
      Op::Halve => (2 * y - 1, 4),
      Op::Double => (y.div_euclid(2) - 1, 3),
      Op::Gauss => (y - (weights.len() as i64 - 1), 2 * weights.len() - 1),
    }
  }

  /// The input columns that the outputs from `x0` to `x1` read, both even when doubling.
  fn reads(self, x0: i64, x1: i64, weights: &[u16]) -> Span {
    match self {
      Op::Copy => Span(x0, x1),
      Op::Halve => Span(2 * x0 - 1, 2 * x1 + 1),
      Op::Double => Span(x0 / 2 - 1, x1 / 2 + 1),
      Op::Gauss => {
        let radius = weights.len() as i64 - 1;

        Span(x0 - radius, x1 + radius)
      }
    }
  }
}

/// The fractional bits of a horizontal Gaussian, which leave room for the sum of two of them.
const GAUSS_FRACTION: u32 = 6;

/// The most rows a vertical step reads: a Gaussian of variance 4 reaches 6 pixels each way.
const MAX_TAPS: usize = 13;

/// A pass's rows: a horizontal step of each input row into 16-bit rows, then a vertical step that
/// combines them.
struct Rows<'a, const N: usize> {
  input: Image<'a, N>,
  ops: [Op; 2],
  weights: [&'a [u16]; 2],
  /// The channels before the first output in a horizontal result, which starts at an even column
  /// when doubling, as doubling makes two pixels per input pixel.
  skip: usize,
  /// The input columns that the horizontal step reads.
  reads: Span,
  /// An input row over `reads`, transparent outside the input.
  padded: Vec<[u8; N]>,
  /// Horizontal results in a ring, with the input row each one holds.
  ring: Vec<Vec<u16>>,
  held: Vec<Option<i64>>,
  split: Split<N>,
}

impl<'a, const N: usize> Rows<'a, N> {
  fn new(input: Image<'a, N>, ops: [Op; 2], weights: [&'a [u16]; 2], x: Span) -> Self {
    let first = match ops[0] {
      Op::Double => x.0.div_euclid(2) * 2,
      _ => x.0,
    };
    let end = match ops[0] {
      Op::Double => first + ((x.1 - first) as usize).div_ceil(2) as i64 * 2,
      _ => x.1,
    };
    let reads = ops[0].reads(first, end, weights[0]);
    let ring = ops[1].rows(0, weights[1]).1;

    Self {
      input,
      ops,
      weights,
      skip: (x.0 - first) as usize * N,
      reads,
      padded: vec![[0; N]; reads.len()],
      ring: vec![vec![0; (end - first) as usize * N]; ring],
      held: vec![None; ring],
      split: Split::default(),
    }
  }

  /// Computes output row `y` into `out`.
  fn row(&mut self, y: i64, out: &mut [[u8; N]]) {
    let (top, count) = self.ops[1].rows(y, self.weights[1]);
    let mut slots = [0; MAX_TAPS];

    for (offset, slot) in slots.iter_mut().take(count).enumerate() {
      *slot = self.horizontal(top + offset as i64);
    }

    let rows: [&[u16]; MAX_TAPS] = array::from_fn(|index| {
      if index < count {
        &self.ring[slots[index]][self.skip..][..out.len() * N]
      } else {
        &[]
      }
    });

    vertical(
      self.ops[1],
      self.weights[1],
      self.ops[0].fraction(),
      &rows[..count],
      y.rem_euclid(2) == 1,
      out.as_flattened_mut(),
    );
  }

  /// The ring slot that holds the horizontal result of input row `y`.
  fn horizontal(&mut self, y: i64) -> usize {
    let slot = y.rem_euclid(self.ring.len() as i64) as usize;

    if self.held[slot] == Some(y) {
      return slot;
    }

    let Self {
      input,
      ops,
      weights,
      reads,
      padded,
      ring,
      held,
      split,
      ..
    } = self;
    let out = &mut ring[slot];

    match input.row(y) {
      None => out.fill(0),
      Some(row) => {
        let x = input.spans[0];
        let row: &[[u8; N]] = if x.0 <= reads.0 && reads.1 <= x.1 {
          &row[(reads.0 - x.0) as usize..(reads.1 - x.0) as usize]
        } else {
          let overlap = reads.intersect(x);

          padded.fill([0; N]);
          if !overlap.is_empty() {
            padded[(overlap.0 - reads.0) as usize..(overlap.1 - reads.0) as usize]
              .copy_from_slice(&row[(overlap.0 - x.0) as usize..(overlap.1 - x.0) as usize]);
          }
          padded
        };

        horizontal(ops[0], weights[0], row, out, split);
      }
    }

    held[slot] = Some(y);
    slot
  }
}

/// The horizontal step of `row`, the pixels the outputs read, into the pixels of `out`.
#[inline(always)]
fn horizontal<const N: usize>(
  op: Op,
  weights: &[u16],
  row: &[[u8; N]],
  out: &mut [u16],
  split: &mut Split<N>,
) {
  match op {
    Op::Copy => {
      for (out, channel) in out.iter_mut().zip(row.as_flattened()) {
        *out = u16::from(*channel);
      }
    }
    Op::Halve => {
      // The even and odd pixels apart, so the taps of every output lie at the same offsets.
      let Split { even, odd, .. } = split;
      let pairs = row.as_chunks::<2>().0;

      even.resize(pairs.len(), [0; N]);
      odd.resize(pairs.len(), [0; N]);
      for ((even, odd), pair) in even.iter_mut().zip(odd.iter_mut()).zip(pairs) {
        *even = pair[0];
        *odd = pair[1];
      }

      let (even, odd) = (even.as_flattened(), odd.as_flattened());
      let taps = even.iter().zip(&odd[N..]).zip(odd.iter().zip(&even[N..]));

      for (out, ((a, d), (b, c))) in out.iter_mut().zip(taps) {
        *out = u16::from(*a) + u16::from(*d) + times3(u16::from(*b) + u16::from(*c));
      }
    }
    Op::Double => {
      // The even and odd outputs apart, then interleaved.
      let Split { halves, .. } = split;
      let row = row.as_flattened();
      let len = out.len() / 2;
      let tap = |offset: usize| &row[offset * N..][..len];

      for (half, outer) in halves.iter_mut().zip([tap(0), tap(2)]) {
        half.resize(len, 0);
        for (half, (outer, centre)) in half.iter_mut().zip(outer.iter().zip(tap(1))) {
          *half = u16::from(*outer) + times3(u16::from(*centre));
        }
      }

      let [even, odd] = halves.each_ref().map(|half| half.as_chunks::<N>().0);

      let pairs = out.as_chunks_mut::<N>().0.as_chunks_mut::<2>().0;

      for ([out_even, out_odd], (even, odd)) in pairs.iter_mut().zip(even.iter().zip(odd)) {
        *out_even = *even;
        *out_odd = *odd;
      }
    }
    Op::Gauss => {
      // The taps run outermost over contiguous channels, so each is one vectorized sweep.
      let radius = weights.len() - 1;
      let row = row.as_flattened();
      let len = out.len();
      let tap = |offset: usize| &row[offset * N..][..len];

      for (out, centre) in out.iter_mut().zip(tap(radius)) {
        *out = mul_q15(u16::from(*centre) << GAUSS_FRACTION, weights[0]);
      }
      for (distance, &weight) in weights.iter().enumerate().skip(1) {
        let pairs = tap(radius + distance).iter().zip(tap(radius - distance));

        for (out, (a, b)) in out.iter_mut().zip(pairs) {
          *out += mul_q15((u16::from(*a) + u16::from(*b)) << GAUSS_FRACTION, weight);
        }
      }
    }
  }
}

/// Scratch rows that resampling splits a row into.
#[derive(Default)]
struct Split<const N: usize> {
  even: Vec<[u8; N]>,
  odd: Vec<[u8; N]>,
  halves: [Vec<u16>; 2],
}

/// The vertical step into `out` from horizontal results with `fraction` fractional bits.
#[inline(always)]
fn vertical(op: Op, weights: &[u16], fraction: u32, rows: &[&[u16]], odd: bool, out: &mut [u8]) {
  match op {
    Op::Copy => {
      for (out, a) in out.iter_mut().zip(rows[0]) {
        *out = round(*a, fraction);
      }
    }
    Op::Halve => {
      let taps = rows[0].iter().zip(rows[1]).zip(rows[2].iter().zip(rows[3]));

      for (out, ((a, b), (c, d))) in out.iter_mut().zip(taps) {
        *out = round(a + d + times3(b + c), fraction + 3);
      }
    }
    Op::Double => {
      // An even row is a quarter of the row above and three quarters of the one below it, an odd
      // row three quarters of that one and a quarter of the next.
      let outer = if odd { rows[2] } else { rows[0] };

      for (out, (a, b)) in out.iter_mut().zip(outer.iter().zip(rows[1])) {
        *out = round(a + times3(*b), fraction + 2);
      }
    }
    Op::Gauss => {
      let radius = weights.len() - 1;
      let scale = GAUSS_FRACTION - fraction;

      // A chunk of sums at a time, so they stay in registers across the taps.
      for (start, out) in (0..).step_by(GAUSS_LANES).zip(out.chunks_mut(GAUSS_LANES)) {
        let tap = |row: usize| &rows[row][start..][..out.len()];
        let mut sums = [0u16; GAUSS_LANES];
        let sums = &mut sums[..out.len()];

        for (sum, centre) in sums.iter_mut().zip(tap(radius)) {
          *sum = mul_q15(centre << scale, weights[0]);
        }
        for (distance, &weight) in weights.iter().enumerate().skip(1) {
          let pairs = tap(radius - distance).iter().zip(tap(radius + distance));

          for (sum, (above, below)) in sums.iter_mut().zip(pairs) {
            *sum += mul_q15((above + below) << scale, weight);
          }
        }
        for (out, sum) in out.iter_mut().zip(sums.iter()) {
          *out = round(*sum, GAUSS_FRACTION);
        }
      }
    }
  }
}

/// Channels a vertical Gaussian sums at once.
const GAUSS_LANES: usize = 64;

/// `value` times the Q15 `weight`, rounded, as Arm's `vqrdmulh` and x86's `pmulhrsw` compute it.
#[inline(always)]
fn mul_q15(value: u16, weight: u16) -> u16 {
  ((u32::from(value) * u32::from(weight) + (1 << 14)) >> 15) as u16
}

#[inline(always)]
fn times3(value: u16) -> u16 {
  value + (value << 1)
}

#[inline(always)]
fn round(value: u16, bits: u32) -> u8 {
  ((value + ((1 << bits) >> 1)) >> bits).min(255) as u8
}

/// Columns or rows from `.0` inclusive to `.1` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span(i64, i64);

impl Span {
  const EMPTY: Span = Span(0, 0);

  fn new(start: i64, end: i64) -> Span {
    if start < end {
      Span(start, end)
    } else {
      Self::EMPTY
    }
  }

  fn len(self) -> usize {
    (self.1 - self.0) as usize
  }

  fn is_empty(self) -> bool {
    self.0 >= self.1
  }

  fn contains(self, i: i64) -> bool {
    (self.0..self.1).contains(&i)
  }

  fn intersect(self, other: Span) -> Span {
    Span::new(self.0.max(other.0), self.1.min(other.1))
  }

  /// The span extended by `by` pixels on both sides.
  fn grow(self, by: i64) -> Span {
    self.map(|a, b| (a - by, b + by))
  }

  /// The pixels that halving this span makes non-zero.
  fn halved(self) -> Span {
    self.map(|a, b| ((a - 1).div_euclid(2), b.div_euclid(2) + 1))
  }

  /// The pixels that doubling this span makes non-zero, and that halving reads for it.
  fn doubled(self) -> Span {
    self.map(|a, b| (2 * a - 1, 2 * b + 1))
  }

  /// The coarser pixels that doubling into this span reads.
  fn doubling_reads(self) -> Span {
    self.map(|a, b| (a.div_euclid(2) - 1, (b - 1).div_euclid(2) + 2))
  }

  fn map(self, f: impl FnOnce(i64, i64) -> (i64, i64)) -> Span {
    if self.is_empty() {
      return Self::EMPTY;
    }

    let (a, b) = f(self.0, self.1);

    Span::new(a, b)
  }
}

/// The columns and rows of `placement`.
fn spans(placement: Placement) -> [Span; 2] {
  [
    Span::new(placement.left.into(), placement.right().into()),
    Span::new(placement.top.into(), placement.bottom().into()),
  ]
}

fn intersect(a: [Span; 2], b: [Span; 2]) -> [Span; 2] {
  [a[0].intersect(b[0]), a[1].intersect(b[1])]
}

/// One axis of a blur.
struct Axis {
  halvings: usize,
  /// The weight of the centre followed by those of the distances 1, 2, …, summing to 2^15 over
  /// both sides; empty for an axis that isn't blurred.
  weights: Vec<u16>,
}

impl Axis {
  fn new(sigma: f32) -> Self {
    if !sigma.is_finite() || sigma <= MIN_SIGMA {
      return Self {
        halvings: 0,
        weights: Vec::new(),
      };
    }

    let sigma = f64::from(sigma);
    let mut variance = sigma * sigma;
    let mut halvings = 0;

    while variance > 4.0 {
      variance = (variance - 1.5) / 4.0;
      halvings += 1;
    }

    let radius = (3.0 * variance.sqrt()).ceil() as usize;
    let exact: Vec<f64> = (0..=radius)
      .map(|i| (-((i * i) as f64) / (2.0 * variance)).exp())
      .collect();
    let total = exact[0] + 2.0 * exact[1..].iter().sum::<f64>();
    let mut weights: Vec<u16> = exact
      .iter()
      .map(|weight| (weight / total * 32768.0).round() as u16)
      .collect();
    let sum = u32::from(weights[0])
      + 2
        * weights[1..]
          .iter()
          .map(|&weight| u32::from(weight))
          .sum::<u32>();
    // The rounding error goes to the centre. A lone centre weight of 2^15 doesn't fit the 15-bit
    // range, and 2^15 - 1 leaves every 8-bit value unchanged.
    weights[0] = (u32::from(weights[0]) + 32768 - sum).min(32767) as u16;

    Self { halvings, weights }
  }
}

/// The pixels each pass computes along one axis.
struct Layout {
  /// The halved levels, indexed by level from 1.
  down: Vec<Span>,
  /// The Gaussian on the coarsest level and the levels doubled from it, indexed by level.
  up: Vec<Span>,
}

impl Layout {
  fn new(axis: &Axis, levels: usize, source: Span, target: Span) -> Self {
    let resampled = |level: usize, span: Span, resample: fn(Span) -> Span| {
      if level < axis.halvings {
        resample(span)
      } else {
        span
      }
    };
    let mut data = vec![source];
    let mut needed = vec![target];

    for level in 0..levels {
      data.push(resampled(level, data[level], Span::halved));
      needed.push(resampled(level, needed[level], Span::doubling_reads));
    }

    let radius = axis.weights.len().saturating_sub(1) as i64;
    let gauss = needed[levels].intersect(data[levels].grow(radius));
    let mut down = vec![Span::EMPTY; levels + 1];

    if levels > 0 {
      down[levels] = gauss.grow(radius).intersect(data[levels]);
    }

    for level in (1..levels).rev() {
      let read = resampled(level, down[level + 1], Span::doubled);

      down[level] = read.intersect(data[level]);
    }

    let mut up = vec![gauss; levels + 1];

    for level in (0..levels).rev() {
      let spread = resampled(level, up[level + 1], Span::doubled);

      up[level] = needed[level].intersect(spread);
    }

    Self { down, up }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    Axis, Layout, Placement, Point, Size, Span, blur_alpha, blur_alpha_region, blur_rgba,
    blur_rgba_axes, reach,
  };

  #[test]
  fn a_blur_keeps_the_ink_it_spreads() {
    let (width, height) = (61, 61);

    for sigma in [2.0, 4.0] {
      let mut alpha = vec![0u8; width * height];

      for row in 25..36 {
        alpha[row * width + 25..row * width + 36].fill(255);
      }
      let before: u32 = alpha.iter().map(|&value| u32::from(value)).sum();

      blur_alpha(&mut alpha, width, height, sigma);

      let after: u32 = alpha.iter().map(|&value| u32::from(value)).sum();

      assert!(before.abs_diff(after) * 100 < before, "sigma {sigma}");
      assert!(alpha[30 * width + 30] < 255);
      assert!(alpha[30 * width + 30] > alpha[30 * width + 40]);
    }
  }

  /// A band blurs across into a profile with its own variance plus that of the sigma asked for.
  #[test]
  fn a_band_spreads_by_the_sigma() {
    let (width, height, band) = (8, 1001, 40);

    for sigma in [0.6f64, 1.5, 3.0, 7.0, 20.0, 60.0] {
      let mut pixels = vec![[0u8; 4]; width * height];

      pixels[(height - band) / 2 * width..(height + band) / 2 * width].fill([255; 4]);
      blur_rgba_axes(
        &mut pixels,
        width,
        height,
        Point {
          x: 0.0,
          y: sigma as f32,
        },
      );

      let profile: Vec<f64> = pixels
        .chunks_exact(width)
        .map(|row| f64::from(row[width / 2][3]))
        .collect();
      let total: f64 = profile.iter().sum();
      let mean = profile
        .iter()
        .enumerate()
        .map(|(y, value)| y as f64 * value)
        .sum::<f64>()
        / total;
      let variance = profile
        .iter()
        .enumerate()
        .map(|(y, value)| (y as f64 - mean).powi(2) * value)
        .sum::<f64>()
        / total;

      let spread = (variance - (band * band - 1) as f64 / 12.0).sqrt();

      assert!(
        (spread - sigma).abs() < 0.05 * sigma + 0.15,
        "sigma {sigma} spread {spread}"
      );
    }
  }

  /// No pixel spreads past [`reach`], so a source cut that far out blurs as the whole one does.
  #[test]
  fn the_reach_covers_the_support() {
    let mut sigma = 0.031f32;

    while sigma < 2000.0 {
      let axis = Axis::new(sigma);
      let everywhere = Span(i64::MIN / 4, i64::MAX / 4);
      let layout = Layout::new(&axis, axis.halvings, Span(0, 1), everywhere);

      assert!((-layout.up[0].0) as f32 <= reach(sigma), "sigma {sigma}");
      sigma *= 1.01;
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
  fn a_large_sigma_on_one_axis_still_blurs_the_other() {
    let (width, height) = (32, 32);
    let mut pixels = vec![[255u8; 4]; width * height];

    blur_rgba_axes(&mut pixels, width, height, Point { x: 400.0, y: 1.0 });

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

    for sigma in [2.0, 200.0] {
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

  /// The pixels of `region` that blurring the whole `image` with `mask` at `mask_at` leaves.
  fn whole_image_region(
    mask: &[u8],
    mask_at: Placement,
    image: Size<u32>,
    region: Placement,
    sigma: f32,
  ) -> Vec<u8> {
    let mut whole = vec![0u8; (image.width * image.height) as usize];

    for (row, source) in mask.chunks_exact(mask_at.width as usize).enumerate() {
      let top = mask_at.top + row as i32;

      if !(0..image.height as i32).contains(&top) {
        continue;
      }
      for (column, value) in source.iter().enumerate() {
        let left = mask_at.left + column as i32;

        if (0..image.width as i32).contains(&left) {
          whole[top as usize * image.width as usize + left as usize] = *value;
        }
      }
    }
    blur_alpha(
      &mut whole,
      image.width as usize,
      image.height as usize,
      sigma,
    );

    (region.top..region.bottom())
      .flat_map(|row| {
        let start = row as usize * image.width as usize + region.left as usize;

        whole[start..start + region.width as usize].iter().copied()
      })
      .collect()
  }

  /// A box's mask blurs to the pixels the whole image blurs to, whichever part the region reaches.
  #[test]
  fn a_box_region_blurs_as_the_whole_image_does() {
    for (width, height, radius) in [
      (300, 220, 0.0),
      (300, 220, 24.0),
      (411, 97, 40.0),
      (64, 400, 9.0),
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
        let regions = [
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

          assert_eq!(
            blur_alpha_region(&mask, mask_at, image, region, sigma),
            whole_image_region(&mask, mask_at, image, region, sigma),
            "box {width}x{height} radius {radius} sigma {sigma} region {region:?}"
          );
        }
      }
    }
  }

  /// A region blurs to the pixels the whole image blurs to, for any sigma and a mask the image
  /// clips.
  #[test]
  fn a_region_blurs_as_the_whole_image_does() {
    let image = Size {
      width: 360,
      height: 280,
    };
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

    for mask_at in [
      Placement {
        left: 90,
        top: 70,
        width: 113,
        height: 61,
      },
      Placement {
        left: -40,
        top: 250,
        width: 113,
        height: 61,
      },
    ] {
      let mask: Vec<u8> = (0..mask_at.width * mask_at.height)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();

      for sigma in [
        -1e30, -50.0, 0.0, 0.02, 0.5, 1.7, 3.0, 11.0, 40.0, 134.0, 190.0, 400.0,
      ] {
        for region in regions {
          assert_eq!(
            blur_alpha_region(&mask, mask_at, image, region, sigma),
            whole_image_region(&mask, mask_at, image, region, sigma),
            "mask {mask_at:?} sigma {sigma} region {region:?}"
          );
        }
      }
    }
  }
}

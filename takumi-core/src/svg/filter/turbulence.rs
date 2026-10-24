// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use super::Turbulence;
use crate::geometry::Rect;
use crate::svg::approx_eq::ApproxEq;

const RAND_M: i32 = 2147483647; // 2**31 - 1
const RAND_A: i32 = 16807; // 7**5; primitive root of m
const RAND_Q: i32 = 127773; // m / a
const RAND_R: i32 = 2836; // m % a
const B_SIZE: usize = 0x100;
const B_SIZE_32: i32 = 0x100;
const B_LEN: usize = B_SIZE + B_SIZE + 2;
const BM: i32 = 0xff;
const PERLIN_N: i32 = 0x1000;

#[derive(Clone, Copy)]
struct StitchInfo {
  width: i32, // How much to subtract to wrap for stitching.
  height: i32,
  wrap_x: i32, // Minimum value to wrap.
  wrap_y: i32,
}

/// Each channel's gradients, indexed by lattice point.
type Gradients = [[[f64; 2]; B_LEN]; 4];

/// Where the pixels of the image sit in the space the noise is sampled in.
pub(super) struct Canvas {
  /// The offset of the filter region.
  pub(super) offset: (f64, f64),
  /// The scale of the canvas.
  pub(super) scale: (f64, f64),
}

/// The lattice the noise reads: the selector of each lattice point and its gradients.
struct Lattice {
  selector: [usize; B_LEN],
  gradient: Box<Gradients>,
}

/// A tile that stitched noise repeats at the edges of.
#[derive(Clone, Copy)]
struct Tile {
  x: f64,
  y: f64,
  width: f64,
  height: f64,
}

impl Turbulence {
  /// Blink's `FETurbulence` in `fe_turbulence.cc`, which Skia runs as `SkShaders::MakeTurbulence`
  /// and `MakeFractalNoise`. Approximate: the noise follows the reference code of the specification.
  ///
  /// `dest` is `width` pixels wide and gets straight alpha within `window`.
  pub(super) fn apply(
    &self,
    canvas: &Canvas,
    dest: &mut [[u8; 4]],
    width: usize,
    window: Rect<usize>,
  ) {
    let lattice = Lattice::new(self.seed);
    let height = dest.len() / width;
    let fill_row = |(y, row): (usize, &mut [[u8; 4]])| {
      for (x, pixel) in row
        .iter_mut()
        .enumerate()
        .take(window.right)
        .skip(window.left)
      {
        let noise = turbulence(
          (
            (x as f64 + canvas.offset.0) / canvas.scale.0,
            (y as f64 + canvas.offset.1) / canvas.scale.1,
          ),
          Tile {
            x: x as f64,
            y: y as f64,
            width: width as f64,
            height: height as f64,
          },
          self,
          &lattice,
        );
        *pixel = noise.map(|n| {
          let n = if self.fractal_noise {
            (n * 255.0 + 255.0) / 2.0
          } else {
            n * 255.0
          };

          ((n as f32).clamp(0.0, 255.0) + 0.5) as u8
        });
      }
    };

    let rows = &mut dest[window.top * width..window.bottom * width];

    #[cfg(feature = "rayon")]
    rows
      .par_chunks_mut(width)
      .enumerate()
      .for_each(|(y, row)| fill_row((y + window.top, row)));
    #[cfg(not(feature = "rayon"))]
    rows
      .chunks_mut(width)
      .enumerate()
      .for_each(|(y, row)| fill_row((y + window.top, row)));
  }
}

impl Lattice {
  fn new(mut seed: i32) -> Self {
    let mut selector = [0; B_LEN];
    let mut gradient = Box::new([[[0.0; 2]; B_LEN]; 4]);

    if seed <= 0 {
      seed = -(seed % (RAND_M - 1)) + 1;
    }

    if seed > RAND_M - 1 {
      seed = RAND_M - 1;
    }

    for (index, entry) in selector.iter_mut().take(B_SIZE).enumerate() {
      *entry = index;
    }

    for channel in gradient.iter_mut() {
      for point in channel.iter_mut().take(B_SIZE) {
        for component in point.iter_mut() {
          seed = random(seed);
          *component = ((seed % (B_SIZE_32 + B_SIZE_32)) - B_SIZE_32) as f64 / B_SIZE_32 as f64;
        }

        let length = (point[0] * point[0] + point[1] * point[1]).sqrt();

        point[0] /= length;
        point[1] /= length;
      }
    }

    for i in (1..B_SIZE).rev() {
      seed = random(seed);
      selector.swap(i, (seed % B_SIZE_32) as usize);
    }

    selector.copy_within(0..B_SIZE + 2, B_SIZE);

    for channel in gradient.iter_mut() {
      channel.copy_within(0..B_SIZE + 2, B_SIZE);
    }

    Self { selector, gradient }
  }
}

/// The turbulence of all four channels at a point, which share every lattice lookup.
fn turbulence(point: (f64, f64), tile: Tile, fe: &Turbulence, lattice: &Lattice) -> [f64; 4] {
  let (mut x, mut y) = point;
  let (mut base_freq_x, mut base_freq_y) = (
    f64::from(fe.base_frequency_x),
    f64::from(fe.base_frequency_y),
  );
  let (tile_width, tile_height) = (tile.width, tile.height);
  let (tile_x, tile_y) = (tile.x, tile.y);

  // Adjust the base frequencies if necessary for stitching.
  let mut stitch = if fe.stitch_tiles {
    // When stitching tiled turbulence, the frequencies must be adjusted
    // so that the tile borders will be continuous.
    if !base_freq_x.approx_eq(0.0) {
      let lo_freq = (tile_width * base_freq_x).floor() / tile_width;
      let hi_freq = (tile_width * base_freq_x).ceil() / tile_width;
      if base_freq_x / lo_freq < hi_freq / base_freq_x {
        base_freq_x = lo_freq;
      } else {
        base_freq_x = hi_freq;
      }
    }

    if !base_freq_y.approx_eq(0.0) {
      let lo_freq = (tile_height * base_freq_y).floor() / tile_height;
      let hi_freq = (tile_height * base_freq_y).ceil() / tile_height;
      if base_freq_y / lo_freq < hi_freq / base_freq_y {
        base_freq_y = lo_freq;
      } else {
        base_freq_y = hi_freq;
      }
    }

    // Set up initial stitch values.
    let width = (tile_width * base_freq_x + 0.5) as i32;
    let height = (tile_height * base_freq_y + 0.5) as i32;
    let wrap_x = (tile_x * base_freq_x + PERLIN_N as f64 + width as f64) as i32;
    let wrap_y = (tile_y * base_freq_y + PERLIN_N as f64 + height as f64) as i32;
    Some(StitchInfo {
      width,
      height,
      wrap_x,
      wrap_y,
    })
  } else {
    None
  };

  let mut sum = [0.0; 4];
  x *= base_freq_x;
  y *= base_freq_y;
  let mut ratio = 1.0;
  for _ in 0..fe.num_octaves {
    let noise = noise2(x, y, lattice, stitch);

    for (sum, noise) in sum.iter_mut().zip(noise) {
      if fe.fractal_noise {
        *sum += noise / ratio;
      } else {
        *sum += noise.abs() / ratio;
      }
    }
    x *= 2.0;
    y *= 2.0;
    ratio *= 2.0;

    if let Some(ref mut stitch) = stitch {
      // Update stitch values. Subtracting PerlinN before the multiplication and
      // adding it afterward simplifies to subtracting it once.
      stitch.width *= 2;
      stitch.wrap_x = 2 * stitch.wrap_x - PERLIN_N;
      stitch.height *= 2;
      stitch.wrap_y = 2 * stitch.wrap_y - PERLIN_N;
    }
  }

  sum
}

/// Perlin noise of all four channels at a point, finding its lattice cell once.
fn noise2(x: f64, y: f64, lattice: &Lattice, stitch_info: Option<StitchInfo>) -> [f64; 4] {
  let lattice_selector = &lattice.selector;

  let t = x + PERLIN_N as f64;
  let mut bx0 = t as i32;
  let mut bx1 = bx0 + 1;
  let rx0 = t - t as i64 as f64;
  let rx1 = rx0 - 1.0;
  let t = y + PERLIN_N as f64;
  let mut by0 = t as i32;
  let mut by1 = by0 + 1;
  let ry0 = t - t as i64 as f64;
  let ry1 = ry0 - 1.0;

  // If stitching, adjust lattice points accordingly.
  if let Some(info) = stitch_info {
    if bx0 >= info.wrap_x {
      bx0 -= info.width;
    }

    if bx1 >= info.wrap_x {
      bx1 -= info.width;
    }

    if by0 >= info.wrap_y {
      by0 -= info.height;
    }

    if by1 >= info.wrap_y {
      by1 -= info.height;
    }
  }

  bx0 &= BM;
  bx1 &= BM;
  by0 &= BM;
  by1 &= BM;
  let i = lattice_selector[bx0 as usize];
  let j = lattice_selector[bx1 as usize];
  let b00 = lattice_selector[i + by0 as usize];
  let b10 = lattice_selector[j + by0 as usize];
  let b01 = lattice_selector[i + by1 as usize];
  let b11 = lattice_selector[j + by1 as usize];
  let sx = s_curve(rx0);
  let sy = s_curve(ry0);

  lattice.gradient.each_ref().map(|channel| {
    let q = &channel[b00];
    let u = rx0 * q[0] + ry0 * q[1];
    let q = &channel[b10];
    let v = rx1 * q[0] + ry0 * q[1];
    let a = lerp(sx, u, v);
    let q = &channel[b01];
    let u = rx0 * q[0] + ry1 * q[1];
    let q = &channel[b11];
    let v = rx1 * q[0] + ry1 * q[1];
    let b = lerp(sx, u, v);
    lerp(sy, a, b)
  })
}

fn random(seed: i32) -> i32 {
  let mut result = RAND_A * (seed % RAND_Q) - RAND_R * (seed / RAND_Q);
  if result <= 0 {
    result += RAND_M;
  }

  result
}

#[inline]
fn s_curve(t: f64) -> f64 {
  t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
  a + t * (b - a)
}

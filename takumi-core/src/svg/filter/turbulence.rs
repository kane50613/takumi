#[cfg(feature = "rayon")]
use rayon::prelude::*;

use super::Turbulence;
use crate::geometry::Rect;

/// Skia's `SkPerlinNoiseShader::kBlockSize`, the lattice points per axis before it repeats.
const BLOCK: usize = 256;

/// `2^31 - 1`, the modulus of the specification's random number generator.
const RAND_MAXIMUM: i32 = i32::MAX;

/// Where the pixels of the layer sit in the user space the noise is sampled in.
pub(super) struct Canvas {
  /// Where the user space origin sits, in the layer's pixels.
  pub(super) offset: (f32, f32),
  /// The layer's pixels per unit of user space.
  pub(super) scale: (f32, f32),
}

/// What Skia's `SkPerlinNoiseShader::PaintingData` derives from the seed, base frequency, and tile.
struct Noise {
  lattice: [u8; BLOCK],
  /// The unit gradients at each lattice point, permuted by `lattice`: every channel's `x`, then
  /// every channel's `y`, decoded from the 16 bits Skia stores them in.
  gradients: [[[f32; 4]; 2]; BLOCK],
  frequency: (f32, f32),
  /// The noise coordinates a stitched tile wraps at, or none without stitching.
  stitch: Option<(f32, f32)>,
}

impl Turbulence {
  /// Blink's `FETurbulence` as Skia's `perlin_noise` raster stage runs it, over the `window` of
  /// `dest`, `width` pixels wide, as premultiplied pixels. `tile` is the primitive subregion in
  /// user space, which stitching repeats at.
  pub(super) fn apply(
    &self,
    canvas: &Canvas,
    tile: (f32, f32),
    dest: &mut [[u8; 4]],
    width: usize,
    window: Rect<usize>,
  ) {
    let noise = Noise::new(self, tile);
    let rows = &mut dest[window.top * width..window.bottom * width];
    let fill_row = |(row, line): (usize, &mut [[u8; 4]])| {
      // The shader reads each pixel at its center, in user space.
      let y = ((window.top + row) as f32 + 0.5 - canvas.offset.1) / canvas.scale.1;

      for (x, pixel) in line[window.left..window.right]
        .iter_mut()
        .enumerate()
        .map(|(column, pixel)| (window.left + column, pixel))
      {
        let x = (x as f32 + 0.5 - canvas.offset.0) / canvas.scale.0;

        *pixel = noise.sample(self, x, y);
      }
    };

    #[cfg(feature = "rayon")]
    rows
      .par_chunks_mut(width)
      .with_min_len(16)
      .enumerate()
      .for_each(fill_row);
    #[cfg(not(feature = "rayon"))]
    rows.chunks_mut(width).enumerate().for_each(fill_row);
  }
}

impl Noise {
  /// `PaintingData::init` and `stitch` in `SkPerlinNoiseShaderImpl.h`.
  fn new(turbulence: &Turbulence, tile: (f32, f32)) -> Self {
    let mut seed = turbulence.seed;

    if seed <= 0 {
      seed = -(seed % (RAND_MAXIMUM - 1)) + 1;
    }
    seed = seed.min(RAND_MAXIMUM - 1);

    // https://www.w3.org/TR/SVG11/filters.html#feTurbulenceElement
    let mut random = || {
      let result = 16807 * (seed % 127_773) - 2836 * (seed / 127_773);

      seed = if result <= 0 {
        result + RAND_MAXIMUM
      } else {
        result
      };
      seed
    };
    let mut lattice: [u8; BLOCK] = std::array::from_fn(|index| index as u8);
    let mut drawn = [[[0_u16; 2]; BLOCK]; 4];

    for channel in &mut drawn {
      for point in channel.iter_mut() {
        point[0] = (random() % (2 * BLOCK as i32)) as u16;
        point[1] = (random() % (2 * BLOCK as i32)) as u16;
      }
    }

    for index in (1..BLOCK).rev() {
      let other = (random() % BLOCK as i32) as usize;

      lattice.swap(index, other);
    }

    let gradients = std::array::from_fn(|index| {
      let channels = drawn.map(|channel| {
        let [x, y] = channel[usize::from(lattice[index])];
        let scale = 1.0 / BLOCK as f32;
        let (x, y) = unit(
          (f32::from(x) - BLOCK as f32) * scale,
          (f32::from(y) - BLOCK as f32) * scale,
        );
        // `compute_perlin_vector` reads back the 16 bits `init` stores.
        let round_trip = |component: f32| {
          let stored = ((component + 1.0) * 32767.5 + 0.5).floor() as u16;

          f32::from(stored) * (2.0 / 65535.0) - 1.0
        };

        (round_trip(x), round_trip(y))
      });

      [channels.map(|(x, _)| x), channels.map(|(_, y)| y)]
    });
    let mut frequency = (turbulence.base_frequency_x, turbulence.base_frequency_y);
    let stitch = turbulence.stitch_tiles.then(|| {
      // Blink truncates the subregion's size to whole units.
      let (tile_width, tile_height) = (tile.0 as i32 as f32, tile.1 as i32 as f32);

      frequency = (
        stitched_frequency(frequency.0, tile_width),
        stitched_frequency(frequency.1, tile_height),
      );
      (
        (tile_width * frequency.0).round(),
        (tile_height * frequency.1).round(),
      )
    });

    Self {
      lattice,
      gradients,
      frequency,
      stitch,
    }
  }

  /// The `perlin_noise` stage at user space point `x`, `y`, as premultiplied bytes.
  fn sample(&self, turbulence: &Turbulence, x: f32, y: f32) -> [u8; 4] {
    let mut vector = ((x + 0.5) * self.frequency.0, (y + 0.5) * self.frequency.1);
    let mut stitch = self.stitch;
    let mut ratio = 1.0;
    let mut sum = [0.0_f32; 4];

    for _ in 0..turbulence.num_octaves {
      let (floor_x, floor_y) = (vector.0.floor(), vector.1.floor());
      let (fract_x, fract_y) = (vector.0 - floor_x, vector.1 - floor_y);
      let (mut floor_x, mut floor_y) = (floor_x, floor_y);
      let (mut ceil_x, mut ceil_y) = (floor_x + 1.0, floor_y + 1.0);

      if let Some((width, height)) = stitch {
        let wrap = |value: f32, at: f32| if value >= at { value - at } else { value };

        floor_x = wrap(floor_x, width);
        floor_y = wrap(floor_y, height);
        ceil_x = wrap(ceil_x, width);
        ceil_y = wrap(ceil_y, height);
      }

      let byte = |value: f32| (value as i32 as u32 & 0xFF) as usize;
      let lattice_x = f32::from(self.lattice[byte(floor_x)]);
      let lattice_y = f32::from(self.lattice[byte(ceil_x)]);
      let corners = [
        byte(lattice_x + floor_y),
        byte(lattice_y + floor_y),
        byte(lattice_x + ceil_y),
        byte(lattice_y + ceil_y),
      ];
      let smooth_x = fract_x * fract_x * (3.0 - 2.0 * fract_x);
      let smooth_y = fract_y * fract_y * (3.0 - 2.0 * fract_y);

      let dot = |corner: usize, x: f32, y: f32| -> [f32; 4] {
        let [gradient_x, gradient_y] = &self.gradients[corners[corner]];

        std::array::from_fn(|channel| gradient_x[channel] * x + gradient_y[channel] * y)
      };
      let lerp = |from: [f32; 4], to: [f32; 4], t: f32| -> [f32; 4] {
        std::array::from_fn(|channel| (to[channel] - from[channel]) * t + from[channel])
      };
      let top = lerp(
        dot(0, fract_x, fract_y),
        dot(1, fract_x - 1.0, fract_y),
        smooth_x,
      );
      let bottom = lerp(
        dot(2, fract_x, fract_y - 1.0),
        dot(3, fract_x - 1.0, fract_y - 1.0),
        smooth_x,
      );
      let value = lerp(top, bottom, smooth_y);

      for (sum, value) in sum.iter_mut().zip(value) {
        let value = if turbulence.fractal_noise {
          value
        } else {
          value.abs()
        };

        *sum += value * ratio;
      }

      vector = (vector.0 * 2.0, vector.1 * 2.0);
      stitch = stitch.map(|(width, height)| (width * 2.0, height * 2.0));
      ratio *= 0.5;
    }

    if turbulence.fractal_noise {
      sum = sum.map(|channel| channel * 0.5 + 0.5);
    }

    let alpha = sum[3].clamp(0.0, 1.0);

    [
      sum[0].clamp(0.0, 1.0) * alpha,
      sum[1].clamp(0.0, 1.0) * alpha,
      sum[2].clamp(0.0, 1.0) * alpha,
      alpha,
    ]
    .map(|channel| (channel * 255.0 + 0.5) as u8)
  }
}

/// The frequency nearest `frequency` that repeats a whole number of times across `tile`, as
/// `PaintingData::stitch` picks it.
fn stitched_frequency(frequency: f32, tile: f32) -> f32 {
  if frequency == 0.0 {
    return frequency;
  }

  let low = (tile * frequency).floor() / tile;
  let high = (tile * frequency).ceil() / tile;

  if frequency / low < high / frequency {
    low
  } else {
    high
  }
}

/// `SkPoint::normalize`, which scales in double precision and leaves a zero vector at zero.
fn unit(x: f32, y: f32) -> (f32, f32) {
  let (x64, y64) = (f64::from(x), f64::from(y));
  let scale = 1.0 / (x64 * x64 + y64 * y64).sqrt();
  let (x, y) = ((f64::from(x) * scale) as f32, (f64::from(y) * scale) as f32);

  if x.is_finite() && y.is_finite() && (x, y) != (0.0, 0.0) {
    (x, y)
  } else {
    (0.0, 0.0)
  }
}

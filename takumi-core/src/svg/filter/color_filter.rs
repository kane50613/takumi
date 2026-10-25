//! The pointwise primitives of a filter and its color space conversions as Skia's color filters run
//! them: each in floats on unpremultiplied colors, composed so a run of them stores bytes once.

use std::sync::OnceLock;

use super::{BYTE_TO_FLOAT, ColorSpace};
use crate::{
  filter::ColorMatrixChain,
  style::{Color, TransferTable},
};

/// One color filter.
#[derive(Clone, Debug)]
pub(super) enum ColorFilter {
  /// `SkColorFilters::SRGBToLinearGamma` or `LinearToSRGBGamma`, into this space.
  Space(ColorSpace),
  /// `SkColorFilters::Matrix`.
  Matrix(ColorMatrixChain),
  /// `SkColorFilters::TableARGB`, with each channel through its table, if any.
  Table(Box<[Option<TransferTable>; 4]>),
}

/// Color filters a result holds until a primitive reads its pixels, applied in order.
#[derive(Clone, Debug, Default)]
pub(super) struct ColorFilters(Vec<ColorFilter>);

impl ColorFilters {
  pub(super) fn push(&mut self, filter: ColorFilter) {
    self.0.push(filter);
  }

  pub(super) fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Whether the chain is as long as a result may defer, which bounds the filters every result
  /// copies when a document chains many pointwise primitives.
  pub(super) fn is_full(&self) -> bool {
    self.0.len() >= MAX_DEFERRED
  }

  /// Runs every filter over premultiplied pixels, storing each pixel once.
  pub(super) fn apply(&self, pixels: &mut [[u8; 4]]) {
    if let [ColorFilter::Space(space)] = self.0[..] {
      let table = space_table(space);

      for pixel in pixels {
        let row = &table[usize::from(pixel[3])];

        *pixel = [
          row[usize::from(pixel[0])],
          row[usize::from(pixel[1])],
          row[usize::from(pixel[2])],
          pixel[3],
        ];
      }

      return;
    }

    // Transparent pixels all map alike, and a layer is mostly transparent.
    let transparent = self.run([0; 4]);
    let mut lanes = Lanes::default();

    for chunk in pixels.chunks_mut(CHUNK) {
      if chunk.iter().all(|pixel| *pixel == [0; 4]) {
        chunk.fill(transparent);
        continue;
      }

      lanes.load(chunk);

      for filter in &self.0 {
        filter.apply(&mut lanes);
      }

      lanes.store(chunk);
    }
  }

  /// The filters over one premultiplied pixel.
  fn run(&self, pixel: [u8; 4]) -> [u8; 4] {
    let mut lanes = Lanes::default();
    let mut pixels = [pixel];

    lanes.load(&pixels);

    for filter in &self.0 {
      filter.apply(&mut lanes);
    }

    lanes.store(&mut pixels);
    pixels[0]
  }
}

/// Most color filters a result defers before it runs them.
const MAX_DEFERRED: usize = 8;

/// Pixels a chunk of `ColorFilters::apply` holds as floats.
const CHUNK: usize = 64;

/// A chunk of premultiplied pixels as floats, one array per channel, so each step of a filter
/// runs down an array and vectorizes.
struct Lanes {
  channels: [[f32; CHUNK]; 4],
  len: usize,
}

impl Default for Lanes {
  fn default() -> Self {
    Self {
      channels: [[0.0; CHUNK]; 4],
      len: 0,
    }
  }
}

impl Lanes {
  fn load(&mut self, pixels: &[[u8; 4]]) {
    self.len = pixels.len();

    for (index, pixel) in pixels.iter().enumerate() {
      for (lane, channel) in self.channels.iter_mut().zip(pixel) {
        lane[index] = f32::from(*channel) * BYTE_TO_FLOAT;
      }
    }
  }

  fn store(&self, pixels: &mut [[u8; 4]]) {
    for (index, pixel) in pixels.iter_mut().enumerate() {
      *pixel = std::array::from_fn(|channel| to_byte(self.channels[channel][index]));
    }
  }
}

/// Each color byte by alpha, converted into `space` as `ColorFilter::Space` converts it, so a lone
/// conversion looks up what it would compute.
fn space_table(space: ColorSpace) -> &'static [[u8; 256]; 256] {
  static TO_LINEAR: OnceLock<Box<[[u8; 256]; 256]>> = OnceLock::new();
  static TO_SRGB: OnceLock<Box<[[u8; 256]; 256]>> = OnceLock::new();

  let cell = match space {
    ColorSpace::LinearRgb => &TO_LINEAR,
    ColorSpace::Srgb => &TO_SRGB,
  };

  cell.get_or_init(|| {
    let filters = ColorFilters(vec![ColorFilter::Space(space)]);
    let mut table = Box::new([[0; 256]; 256]);

    for (alpha, row) in table.iter_mut().enumerate() {
      let mut pixels: [[u8; 4]; 256] = std::array::from_fn(|byte| [byte as u8, 0, 0, alpha as u8]);

      for chunk in pixels.chunks_mut(CHUNK) {
        let mut lanes = Lanes::default();

        lanes.load(chunk);
        filters.0[0].apply(&mut lanes);
        lanes.store(chunk);
      }

      *row = pixels.map(|pixel| pixel[0]);
    }

    table
  })
}

impl ColorFilter {
  /// The filter over a chunk of premultiplied colors, unpremultiplying them first and
  /// premultiplying after as Skia's stages around a color filter do.
  fn apply(&self, lanes: &mut Lanes) {
    let len = lanes.len;
    let [red, green, blue, alpha] = &mut lanes.channels;
    let (red, green, blue, alpha) = (
      &mut red[..len],
      &mut green[..len],
      &mut blue[..len],
      &mut alpha[..len],
    );

    for (((red, green), blue), alpha) in red
      .iter_mut()
      .zip(green.iter_mut())
      .zip(blue.iter_mut())
      .zip(alpha.iter())
    {
      let scale = if *alpha > 0.0 { 1.0 / *alpha } else { 0.0 };

      *red *= scale;
      *green *= scale;
      *blue *= scale;
    }

    match self {
      Self::Space(space) => {
        let curve = match space {
          ColorSpace::LinearRgb => &SRGB_TO_LINEAR,
          ColorSpace::Srgb => &LINEAR_TO_SRGB,
        };

        for channel in [&mut *red, &mut *green, &mut *blue] {
          for value in channel.iter_mut() {
            *value = curve.evaluate(*value);
          }
        }
      }
      Self::Matrix(chain) => {
        for index in 0..len {
          [red[index], green[index], blue[index], alpha[index]] =
            chain.apply([red[index], green[index], blue[index], alpha[index]]);
        }
      }
      Self::Table(tables) => {
        for (channel, table) in [&mut *red, &mut *green, &mut *blue, &mut *alpha]
          .into_iter()
          .zip(tables.iter())
        {
          if let Some(table) = table {
            for value in channel.iter_mut() {
              *value = f32::from(table[usize::from(to_byte(*value))]) * BYTE_TO_FLOAT;
            }
          }
        }
      }
    }

    for (((red, green), blue), alpha) in red
      .iter_mut()
      .zip(green.iter_mut())
      .zip(blue.iter_mut())
      .zip(alpha.iter())
    {
      *red *= *alpha;
      *green *= *alpha;
      *blue *= *alpha;
    }
  }
}

impl ColorSpace {
  /// The sRGB `color` converted into this space, as Blink's `AdaptColorToOperatingInterpolationSpace`
  /// runs the conversion filter on it.
  pub(super) fn convert_color(self, Color([red, green, blue, alpha]): Color) -> Color {
    match self {
      Self::Srgb => Color([red, green, blue, alpha]),
      Self::LinearRgb => {
        let [red, green, blue, _] = SRGB_TO_LINEAR
          .on_colors([red, green, blue, alpha].map(|channel| f32::from(channel) * BYTE_TO_FLOAT));

        Color([to_byte(red), to_byte(green), to_byte(blue), alpha])
      }
    }
  }
}

/// An `skcms_TransferFunction`, as Skia's `parametric` raster stage evaluates it.
struct TransferFunction {
  g: f32,
  a: f32,
  b: f32,
  c: f32,
  d: f32,
  e: f32,
  f: f32,
}

/// The sRGB curve in `skcms_sRGB_profile`.
const SRGB_TO_LINEAR: TransferFunction = TransferFunction {
  g: 2.4,
  a: (1.0 / 1.055_f64) as f32,
  b: (0.055 / 1.055_f64) as f32,
  c: (1.0 / 12.92_f64) as f32,
  d: 0.04045,
  e: 0.0,
  f: 0.0,
};

/// `skcms_sRGB_Inverse_TransferFunction`.
const LINEAR_TO_SRGB: TransferFunction = TransferFunction {
  g: 0.416_666_66,
  a: 1.137_283_3,
  b: -0.0,
  c: 12.92,
  d: 0.003_130_805,
  e: -0.054_969_788,
  f: -0.0,
};

impl TransferFunction {
  /// The curve on the color channels of a straight color, leaving alpha.
  fn on_colors(&self, [red, green, blue, alpha]: [f32; 4]) -> [f32; 4] {
    [
      self.evaluate(red),
      self.evaluate(green),
      self.evaluate(blue),
      alpha,
    ]
  }

  fn evaluate(&self, value: f32) -> f32 {
    let magnitude = value.abs();
    let result = if magnitude <= self.d {
      self.c * magnitude + self.f
    } else {
      approx_powf(self.a * magnitude + self.b, self.g) + self.e
    };

    result.copysign(value)
  }
}

/// Skia's `approx_powf`, which Chrome's color conversions run on.
fn approx_powf(x: f32, y: f32) -> f32 {
  if x == 0.0 || x == 1.0 {
    x
  } else {
    approx_pow2(approx_log2(x) * y)
  }
}

/// Skia's `approx_log2`.
fn approx_log2(x: f32) -> f32 {
  let bits = x.to_bits();
  let exponent = bits as f32 * (1.0 / (1 << 23) as f32);
  let mantissa = f32::from_bits((bits & 0x007f_ffff) | 0x3f00_0000);

  (exponent - 124.225_52 - mantissa * 1.498_030_3) - 1.725_88 / (0.352_088_72 + mantissa)
}

/// Skia's `approx_pow2`.
fn approx_pow2(x: f32) -> f32 {
  let fraction = x - x.floor();
  let mut approx = x + 121.274_055 - fraction * 1.490_129_1;

  approx += 27.728_024 / (4.842_525_7 - fraction);
  approx *= (1 << 23) as f32;
  approx = approx.max(0.0).min(0x7f80_0000_u32 as f32);

  f32::from_bits(approx.round_ties_even() as u32)
}

/// A channel as `store_8888` writes it: clamped to `0..=1` and rounded.
fn to_byte(channel: f32) -> u8 {
  (channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn approx_powf_stays_near_powf() {
    for x in (1..=100).map(|step| step as f32 / 100.0) {
      for y in [0.416_666_66, 2.4] {
        let error = (approx_powf(x, y) - x.powf(y)).abs();

        assert!(error < 1.0 / 1024.0, "{x}^{y} is off by {error}");
      }
    }
  }

  #[test]
  fn a_round_trip_through_linear_keeps_every_byte() {
    let filters = ColorFilters(vec![
      ColorFilter::Space(ColorSpace::LinearRgb),
      ColorFilter::Space(ColorSpace::Srgb),
    ]);

    for byte in 0..=u8::MAX {
      let mut pixels = [[byte, byte, byte, 255]];

      filters.apply(&mut pixels);

      assert!(
        pixels[0][0].abs_diff(byte) <= 1,
        "{byte} became {}",
        pixels[0][0]
      );
    }
  }
}

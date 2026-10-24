//! A CSS `filter` function as a colour matrix.
//!
//! `grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness` and
//! `contrast` are linear transforms of the source colour, and `opacity` scales
//! alpha. Filter Effects defines each as an `feColorMatrix`, so one matrix per
//! function serves a rasterizer transforming pixels and a vector backend
//! transforming the colours it writes alike. The primitives that need a
//! convolution (`blur`, `drop-shadow`) are not here.

use smallvec::SmallVec;

use crate::style::{Angle, Filter, LUMA_WEIGHTS, PercentageNumber, SEPIA_WEIGHTS, TransferTable};

/// Four rows of `[r, g, b, a, offset]`, one per output channel, as `feColorMatrix` takes them.
/// Colors are transformed in the 0..1 range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix {
  rows: [[f32; 5]; 4],
}

/// Colour-transforming filters applied in order, each clamped as Filter Effects asks, with the
/// channels quantized to 8 bits once at the end.
#[derive(Clone, Debug, Default)]
pub struct ColorMatrixChain(pub SmallVec<[ColorMatrix; 2]>);

impl ColorMatrixChain {
  /// Appends `matrix` to the chain, folding it into the last matrix when that one can never leave
  /// 0..1, so the clamp between them would do nothing.
  pub fn push(&mut self, matrix: ColorMatrix) {
    match self.0.last_mut() {
      Some(last) if last.stays_in_range() => *last = matrix.after(last),
      _ => self.0.push(matrix),
    }
  }

  /// The chain's coefficients as bits, in order, so equal chains compare equal as a cache key.
  pub fn coefficient_bits(&self) -> impl Iterator<Item = u32> + '_ {
    self.0.iter().flat_map(|matrix| {
      matrix
        .rows
        .iter()
        .flatten()
        .map(|coefficient| coefficient.to_bits())
    })
  }

  /// Runs every matrix over an 8-bit straight colour.
  #[inline]
  pub fn apply_rgba8(&self, rgba: [u8; 4]) -> [u8; 4] {
    to_rgba8(
      self
        .0
        .iter()
        .fold(from_rgba8(rgba), |color, matrix| matrix.apply(color)),
    )
  }
}

#[inline]
fn from_rgba8(rgba: [u8; 4]) -> [f32; 4] {
  rgba.map(|value| f32::from(value) * (1.0 / 255.0))
}

/// Rounds channels clamped to 0..1: adding a half and truncating avoids the slower `f32::round`.
#[inline]
fn to_rgba8(color: [f32; 4]) -> [u8; 4] {
  color.map(|value| (value * 255.0 + 0.5) as u8)
}

impl From<ColorMatrix> for ColorMatrixChain {
  fn from(matrix: ColorMatrix) -> Self {
    Self(SmallVec::from_buf([matrix]).into_iter().collect())
  }
}

impl Default for ColorMatrix {
  fn default() -> Self {
    IDENTITY
  }
}

const IDENTITY: ColorMatrix = ColorMatrix {
  rows: [
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 1.0, 0.0],
  ],
};

impl ColorMatrix {
  /// The matrix of `feColorMatrix` `values`: four rows of five, the alpha row last.
  #[cfg(feature = "svg-raster")]
  pub(crate) fn from_values(values: [f32; 20]) -> Self {
    let (rows, _) = values.as_chunks::<5>();

    Self {
      rows: [rows[0], rows[1], rows[2], rows[3]],
    }
  }

  /// The matrix as `feColorMatrix` `values`.
  pub fn fe_color_matrix_values(self) -> [f32; 20] {
    let mut values = [0.0; 20];

    values.copy_from_slice(self.rows.as_flattened());
    values
  }

  /// `feColorMatrix type="saturate"`, which Filter Effects defines for `saturate()` as well.
  pub(crate) fn saturate(amount: f32) -> Self {
    let mut matrix = IDENTITY;
    let rows = [
      [
        0.213 + 0.787 * amount,
        0.715 - 0.715 * amount,
        0.072 - 0.072 * amount,
      ],
      [
        0.213 - 0.213 * amount,
        0.715 + 0.285 * amount,
        0.072 - 0.072 * amount,
      ],
      [
        0.213 - 0.213 * amount,
        0.715 - 0.715 * amount,
        0.072 + 0.928 * amount,
      ],
    ];

    for (row, weights) in matrix.rows.iter_mut().zip(rows) {
      row[..3].copy_from_slice(&weights);
    }
    matrix
  }

  /// `feColorMatrix type="luminanceToAlpha"`.
  #[cfg(feature = "svg-raster")]
  pub(crate) fn luminance_to_alpha() -> Self {
    let mut matrix = IDENTITY;

    matrix.rows[..3].fill([0.0; 5]);
    matrix.rows[3] = [0.2125, 0.7154, 0.0721, 0.0, 0.0];
    matrix
  }

  /// Applies the matrix to a straight (non-premultiplied) colour in 0..1,
  /// clamping as Filter Effects clamps each primitive's output.
  #[inline]
  pub fn apply(&self, color: [f32; 4]) -> [f32; 4] {
    self.rows.map(|[r, g, b, a, offset]| {
      (r * color[0] + g * color[1] + b * color[2] + a * color[3] + offset).clamp(0.0, 1.0)
    })
  }

  /// Whether every colour in 0..1 maps to another in 0..1: no negative weight or offset, and no
  /// row that can sum past 1.
  fn stays_in_range(&self) -> bool {
    self
      .rows
      .iter()
      .all(|row| row.iter().all(|weight| *weight >= 0.0) && row.iter().sum::<f32>() <= 1.0)
  }

  /// The single matrix that applies `first` and then this one.
  fn after(&self, first: &Self) -> Self {
    let mut rows = [[0.0; 5]; 4];

    for (index, row) in rows.iter_mut().enumerate() {
      for (column, cell) in row.iter_mut().enumerate() {
        let carried = (0..4)
          .map(|inner| self.rows[index][inner] * first.rows[inner][column])
          .sum::<f32>();

        *cell = carried
          + if column == 4 {
            self.rows[index][4]
          } else {
            0.0
          };
      }
    }

    Self { rows }
  }

  /// The matrix a colour-transforming `filter` applies, or `None` for one
  /// that needs a convolution.
  pub fn from_filter(filter: &Filter) -> Option<Self> {
    match *filter {
      Filter::Brightness(PercentageNumber(value)) => Some(Self::scale(value, 0.0)),
      Filter::Contrast(PercentageNumber(value)) => Some(Self::scale(value, 0.5 - 0.5 * value)),
      Filter::Invert(PercentageNumber(value)) => {
        let value = value.clamp(0.0, 1.0);

        Some(Self::scale(1.0 - 2.0 * value, value))
      }
      Filter::Opacity(PercentageNumber(value)) => {
        let mut matrix = IDENTITY;

        matrix.rows[3][3] = value.clamp(0.0, 1.0);
        Some(matrix)
      }
      Filter::Grayscale(PercentageNumber(value)) => Some(Self::toward_luma(value.clamp(0.0, 1.0))),
      Filter::Sepia(PercentageNumber(value)) => Some(Self::sepia(value.clamp(0.0, 1.0))),
      Filter::Saturate(PercentageNumber(value)) => Some(Self::saturate(value)),
      Filter::HueRotate(angle) => Some(Self::hue_rotate(angle)),
      _ => None,
    }
  }

  /// A per-channel `value * scale + offset`.
  fn scale(scale: f32, offset: f32) -> Self {
    let mut filter = IDENTITY;

    for (index, row) in filter.rows.iter_mut().take(3).enumerate() {
      row[index] = scale;
      row[4] = offset;
    }
    filter
  }

  /// Mixes each channel toward the luma of the color. `amount` of 1 is fully gray.
  fn toward_luma(amount: f32) -> Self {
    Self::toward([LUMA_WEIGHTS; 3], amount)
  }

  fn sepia(amount: f32) -> Self {
    Self::toward(SEPIA_WEIGHTS, amount)
  }

  /// Mixes each channel toward the row of `weights` that produces it, by `amount`.
  fn toward(weights: [[f32; 3]; 3], amount: f32) -> Self {
    let mut filter = IDENTITY;

    for (index, row) in filter.rows.iter_mut().take(3).enumerate() {
      for (column, weight) in weights[index].iter().enumerate() {
        row[column] = amount * weight + if index == column { 1.0 - amount } else { 0.0 };
      }
    }
    filter
  }

  /// `feColorMatrix type="hueRotate"`, which Filter Effects defines for `hue-rotate()` as well: a
  /// luma column plus cosine and sine terms. The coefficients are the spec's own, not derivable
  /// from the luma weights.
  pub(crate) fn hue_rotate(angle: Angle) -> Self {
    const BASE: [[f32; 3]; 3] = [
      [0.213, 0.715, 0.072],
      [0.213, 0.715, 0.072],
      [0.213, 0.715, 0.072],
    ];
    const COSINE: [[f32; 3]; 3] = [
      [0.787, -0.715, -0.072],
      [-0.213, 0.285, -0.072],
      [-0.213, -0.715, 0.928],
    ];
    const SINE: [[f32; 3]; 3] = [
      [-0.213, -0.715, 0.928],
      [0.143, 0.140, -0.283],
      [-0.787, 0.715, 0.072],
    ];
    // `Angle` derefs to its degree value, so the conversion runs on the f32.
    let (sin, cos) = f32::to_radians(*angle).sin_cos();
    let mut matrix = IDENTITY;

    for (index, row) in matrix.rows.iter_mut().take(3).enumerate() {
      for (column, cell) in row.iter_mut().take(3).enumerate() {
        *cell = BASE[index][column] + cos * COSINE[index][column] + sin * SINE[index][column];
      }
    }
    matrix
  }
}

/// The function of a channel of `feComponentTransfer`, which Blink builds the brightness,
/// contrast, invert, and opacity filters through as well.
/// <https://www.w3.org/TR/SVG11/filters.html#transferFuncElements>
#[derive(Debug, Clone, PartialEq)]
pub enum TransferFunction {
  /// The channel as it is.
  Identity,
  /// A linear interpolation between the values.
  Table(Vec<f32>),
  /// A step function over the values.
  Discrete(Vec<f32>),
  /// `slope * value + intercept`.
  Linear {
    /// The multiplier of the value.
    slope: f32,
    /// What is added to the product.
    intercept: f32,
  },
  /// `amplitude * value ^ exponent + offset`.
  Gamma {
    /// The multiplier of the power.
    amplitude: f32,
    /// The power of the value.
    exponent: f32,
    /// What is added to the product.
    offset: f32,
  },
}

impl TransferFunction {
  /// The result for each byte of a channel, none when the function leaves the channel as it is.
  /// Blink's `FEComponentTransfer::GetValues` in `fe_component_transfer.cc`, which truncates where
  /// this rounds, as the compositor does for the CSS functions.
  pub fn table(&self) -> Option<TransferTable> {
    if matches!(self, Self::Identity)
      || matches!(self, Self::Table(values) | Self::Discrete(values) if values.is_empty())
    {
      return None;
    }

    let mut table = [0; 256];

    for (byte, entry) in table.iter_mut().enumerate() {
      *entry = (self.entry(byte) + 0.5).clamp(0.0, 255.0) as u8;
    }
    Some(table)
  }

  /// The result for `byte`, before it is rounded down to one.
  fn entry(&self, byte: usize) -> f64 {
    let channel = byte as f64 / 255.0;

    match self {
      Self::Identity => byte as f64,
      Self::Table(values) => {
        let last = values.len() - 1;
        let position = channel * last as f64;
        let index = position as usize;
        let below = f64::from(values[index]);
        let above = f64::from(values[(index + 1).min(last)]);

        255.0 * (below + (position - index as f64) * (above - below))
      }
      Self::Discrete(values) => {
        let index = ((byte * values.len()) as f64 / 255.0) as usize;

        255.0 * f64::from(values[index.min(values.len() - 1)])
      }
      &Self::Linear { slope, intercept } => {
        f64::from(slope) * byte as f64 + 255.0 * f64::from(intercept)
      }
      &Self::Gamma {
        amplitude,
        exponent,
        offset,
      } => 255.0 * (f64::from(amplitude) * channel.powf(f64::from(exponent)) + f64::from(offset)),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{ColorMatrix, ColorMatrixChain, TransferFunction};
  use crate::style::{Filter, PercentageNumber};

  #[test]
  fn transfer_tables_follow_blink() {
    let table = TransferFunction::Table(vec![1.0, 0.0])
      .table()
      .expect("a table");

    assert_eq!((table[0], table[255]), (255, 0));

    let single = TransferFunction::Table(vec![0.5]).table().expect("a table");

    assert!(single.iter().all(|entry| *entry == 128));

    let discrete = TransferFunction::Discrete(vec![0.0, 1.0])
      .table()
      .expect("a table");

    assert_eq!((discrete[127], discrete[128]), (0, 255));

    let linear = TransferFunction::Linear {
      slope: 2.0,
      intercept: 0.0,
    }
    .table()
    .expect("a table");

    assert_eq!((linear[100], linear[200]), (200, 255));

    let gamma = TransferFunction::Gamma {
      amplitude: 1.0,
      exponent: 2.0,
      offset: 0.0,
    }
    .table()
    .expect("a table");

    assert_eq!(gamma[128], 64);
    assert!(TransferFunction::Identity.table().is_none());
    assert!(TransferFunction::Table(Vec::new()).table().is_none());
  }

  #[cfg(feature = "svg-raster")]
  #[test]
  fn svg_matrices_keep_their_values() {
    let values: [f32; 20] = std::array::from_fn(|index| index as f32 / 20.0);

    assert_eq!(
      ColorMatrix::from_values(values).fe_color_matrix_values(),
      values
    );
    assert_eq!(
      ColorMatrixChain::from(ColorMatrix::luminance_to_alpha()).apply_rgba8([255, 255, 255, 255]),
      [0, 0, 0, 255]
    );

    let unchanged =
      ColorMatrixChain::from(ColorMatrix::saturate(1.0)).apply_rgba8([10, 200, 30, 255]);

    assert!(
      unchanged
        .iter()
        .zip([10u8, 200, 30, 255])
        .all(|(a, b)| a.abs_diff(b) <= 1)
    );
  }

  #[test]
  fn a_folded_chain_matches_each_matrix_in_turn() {
    let matrices = [
      Filter::Grayscale(PercentageNumber(0.5)),
      Filter::Sepia(PercentageNumber(1.0)),
    ]
    .map(|filter| ColorMatrix::from_filter(&filter).expect("a color filter"));
    let mut chain = ColorMatrixChain::default();

    for matrix in matrices {
      chain.push(matrix);
    }

    let color = [0.9, 0.2, 0.4, 1.0];
    let sequential = matrices
      .iter()
      .fold(color, |color, matrix| matrix.apply(color));

    assert_eq!(chain.0.len(), 1);
    for (expected, folded) in sequential.iter().zip(chain.0[0].apply(color)) {
      assert!((expected - folded).abs() < 1e-5);
    }
  }

  #[test]
  fn a_matrix_that_can_leave_the_range_keeps_its_clamp() {
    let mut chain = ColorMatrixChain::default();

    for filter in [
      Filter::Saturate(PercentageNumber(2.0)),
      Filter::Grayscale(PercentageNumber(1.0)),
    ] {
      chain.push(ColorMatrix::from_filter(&filter).expect("a color filter"));
    }

    assert_eq!(chain.0.len(), 2);
  }
}

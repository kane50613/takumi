//! A CSS `filter` function as a colour matrix.
//!
//! `grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness` and
//! `contrast` are linear transforms of the source colour, and `opacity` scales
//! alpha. Filter Effects defines each as an `feColorMatrix`, so one matrix per
//! function serves a rasterizer transforming pixels and a vector backend
//! transforming the colours it writes alike. The primitives that need a
//! convolution (`blur`, `drop-shadow`) are not here.

use smallvec::SmallVec;

use crate::style::{Angle, Filter, LUMA_WEIGHTS, PercentageNumber, SEPIA_WEIGHTS};

/// Rows of `[r, g, b, offset]` for the three color channels, plus an alpha
/// multiplier. Colors are transformed in the 0..1 range.
#[derive(Clone, Copy, Debug)]
pub struct ColorMatrix {
  rows: [[f32; 4]; 3],
  alpha: f32,
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

  /// Runs every matrix over an 8-bit straight colour.
  #[inline]
  pub fn apply_rgba8(&self, rgba: [u8; 4]) -> [u8; 4] {
    let color = rgba.map(|value| f32::from(value) * (1.0 / 255.0));

    // Every channel ends clamped to 0..1, so adding a half and truncating rounds it without the
    // slower `f32::round`.
    self
      .0
      .iter()
      .fold(color, |color, matrix| matrix.apply(color))
      .map(|value| (value * 255.0 + 0.5) as u8)
  }
}

const IDENTITY: ColorMatrix = ColorMatrix {
  rows: [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
  ],
  alpha: 1.0,
};

impl ColorMatrix {
  /// Applies the matrix to a straight (non-premultiplied) colour in 0..1,
  /// clamping as Filter Effects clamps each primitive's output.
  #[inline]
  pub fn apply(&self, color: [f32; 4]) -> [f32; 4] {
    let [r, g, b, a] = color;
    let mixed = self
      .rows
      .map(|row| (row[0] * r + row[1] * g + row[2] * b + row[3]).clamp(0.0, 1.0));

    [
      mixed[0],
      mixed[1],
      mixed[2],
      (a * self.alpha).clamp(0.0, 1.0),
    ]
  }

  /// Whether every colour in 0..1 maps to another in 0..1: no negative weight or offset, and no
  /// row that can sum past 1.
  fn stays_in_range(&self) -> bool {
    (0.0..=1.0).contains(&self.alpha)
      && self
        .rows
        .iter()
        .all(|row| row.iter().all(|weight| *weight >= 0.0) && row.iter().sum::<f32>() <= 1.0)
  }

  /// The single matrix that applies `first` and then this one.
  fn after(&self, first: &Self) -> Self {
    let mut rows = [[0.0; 4]; 3];

    for (index, row) in rows.iter_mut().enumerate() {
      for (column, cell) in row.iter_mut().enumerate() {
        let carried = (0..3)
          .map(|inner| self.rows[index][inner] * first.rows[inner][column])
          .sum::<f32>();

        *cell = carried
          + if column == 3 {
            self.rows[index][3]
          } else {
            0.0
          };
      }
    }

    Self {
      rows,
      alpha: self.alpha * first.alpha,
    }
  }

  /// The matrix as `feColorMatrix` `values`: four rows of five, the alpha row last.
  pub fn fe_color_matrix_values(self) -> [f32; 20] {
    let mut values = [0.0; 20];

    for (index, row) in self.rows.iter().enumerate() {
      values[index * 5..index * 5 + 3].copy_from_slice(&row[..3]);
      values[index * 5 + 4] = row[3];
    }
    values[18] = self.alpha;
    values
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
      Filter::Opacity(PercentageNumber(value)) => Some(Self {
        alpha: value.clamp(0.0, 1.0),
        ..IDENTITY
      }),
      Filter::Grayscale(PercentageNumber(value)) => Some(Self::toward_luma(value.clamp(0.0, 1.0))),
      Filter::Sepia(PercentageNumber(value)) => Some(Self::sepia(value.clamp(0.0, 1.0))),
      Filter::Saturate(PercentageNumber(value)) => Some(Self::toward_luma(1.0 - value)),
      Filter::HueRotate(angle) => Some(Self::hue_rotate(angle)),
      _ => None,
    }
  }

  /// A per-channel `value * scale + offset`.
  fn scale(scale: f32, offset: f32) -> Self {
    let mut filter = IDENTITY;

    for (index, row) in filter.rows.iter_mut().enumerate() {
      row[index] = scale;
      row[3] = offset;
    }
    filter
  }

  /// Mixes each channel toward the luma of the color. `amount` of 1 is fully
  /// gray; negative amounts push past the original, which is how `saturate`
  /// above 1 works.
  fn toward_luma(amount: f32) -> Self {
    Self::toward([LUMA_WEIGHTS; 3], amount)
  }

  fn sepia(amount: f32) -> Self {
    Self::toward(SEPIA_WEIGHTS, amount)
  }

  /// Mixes each channel toward the row of `weights` that produces it, by `amount`.
  fn toward(weights: [[f32; 3]; 3], amount: f32) -> Self {
    let mut filter = IDENTITY;

    for (index, row) in filter.rows.iter_mut().enumerate() {
      for (column, weight) in weights[index].iter().enumerate() {
        row[column] = amount * weight + if index == column { 1.0 - amount } else { 0.0 };
      }
    }
    filter
  }

  /// The `hue-rotate` matrix from Filter Effects: a luma column plus cosine and
  /// sine terms. The coefficients are the spec's own, not derivable from the
  /// luma weights, and match this repo's SVG filter implementation.
  fn hue_rotate(angle: Angle) -> Self {
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
    let mut rows = [[0.0; 4]; 3];

    for (index, row) in rows.iter_mut().enumerate() {
      for (column, cell) in row.iter_mut().take(3).enumerate() {
        *cell = BASE[index][column] + cos * COSINE[index][column] + sin * SINE[index][column];
      }
    }

    Self { rows, alpha: 1.0 }
  }
}

#[cfg(test)]
mod tests {
  use super::{ColorMatrix, ColorMatrixChain};
  use crate::style::{Filter, PercentageNumber};

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

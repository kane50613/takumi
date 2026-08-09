//! A CSS `filter` function as a colour matrix.
//!
//! `grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness` and
//! `contrast` are linear transforms of the source colour, and `opacity` scales
//! alpha. Filter Effects defines each as an `feColorMatrix`, so one matrix per
//! function serves a rasterizer transforming pixels and a vector backend
//! transforming the colours it writes alike. The primitives that need a
//! convolution (`blur`, `drop-shadow`) are not here.

use crate::style::{Angle, Filter, LUMA_WEIGHTS, PercentageNumber, SEPIA_WEIGHTS};

/// Rows of `[r, g, b, offset]` for the three color channels, plus an alpha
/// multiplier. Colors are transformed in the 0..1 range.
#[derive(Clone, Copy, Debug)]
pub struct ColorMatrix {
  rows: [[f32; 4]; 3],
  alpha: f32,
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
  pub fn apply(self, color: [f32; 4]) -> [f32; 4] {
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
    let mut filter = IDENTITY;

    for (index, row) in filter.rows.iter_mut().enumerate() {
      for (column, weight) in LUMA_WEIGHTS.iter().enumerate() {
        row[column] = amount * weight + if index == column { 1.0 - amount } else { 0.0 };
      }
    }
    filter
  }

  fn sepia(amount: f32) -> Self {
    let mut filter = IDENTITY;

    for (index, row) in filter.rows.iter_mut().enumerate() {
      for (column, weight) in SEPIA_WEIGHTS[index].iter().enumerate() {
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

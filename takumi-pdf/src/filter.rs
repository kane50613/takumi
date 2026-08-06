//! The color half of CSS `filter`, as a matrix applied to every color the
//! filtered subtree paints.
//!
//! `grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness` and
//! `contrast` are linear transforms of the source color, so a vector backend
//! can fold them into the colors it writes instead of rasterizing. `opacity`
//! folds into the alpha channel. The primitives that need a convolution
//! (`blur`, `drop-shadow`) and referenced SVG filters are left alone.

use takumi_core::style::{Angle, Color, Filter, LUMA_WEIGHTS, PercentageNumber, SEPIA_WEIGHTS};

/// The filter list as written, applied in order. CSS clamps each function's
/// result before handing it to the next, so the matrices stay separate rather
/// than composing into one.
#[derive(Clone, Debug, Default)]
pub(crate) struct ColorFilter {
  functions: Vec<ColorMatrix>,
}

/// Rows of `[r, g, b, offset]` for the three color channels, plus an alpha
/// multiplier. Colors are transformed in the 0..1 range.
#[derive(Clone, Copy, Debug)]
struct ColorMatrix {
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

impl ColorFilter {
  /// Appends a filter list to whatever the ancestors already apply. Returns
  /// `None` when nothing in the list changes color, so the common case stays
  /// free.
  pub(crate) fn compose(outer: Option<&Self>, filters: &[Filter]) -> Option<Self> {
    let mut functions = outer
      .map(|filter| filter.functions.clone())
      .unwrap_or_default();

    functions.extend(filters.iter().filter_map(ColorMatrix::from_filter));
    (!functions.is_empty()).then_some(Self { functions })
  }

  /// Applies every filter in order, clamping between them like CSS does.
  pub(crate) fn apply(&self, rgba: [u8; 4]) -> [u8; 4] {
    self
      .functions
      .iter()
      .fold(rgba, |color, function| function.apply(color))
  }

  /// Applies the filter to a color value.
  pub(crate) fn apply_color(&self, color: Color) -> Color {
    Color(self.apply(color.0))
  }
}

impl ColorMatrix {
  /// Applies the matrix to a straight (non-premultiplied) RGBA color.
  fn apply(self, rgba: [u8; 4]) -> [u8; 4] {
    let channel = |value: u8| value as f32 / 255.0;
    let (r, g, b) = (channel(rgba[0]), channel(rgba[1]), channel(rgba[2]));
    let mixed = self
      .rows
      .map(|row| (row[0] * r + row[1] * g + row[2] * b + row[3]).clamp(0.0, 1.0));

    [
      (mixed[0] * 255.0).round() as u8,
      (mixed[1] * 255.0).round() as u8,
      (mixed[2] * 255.0).round() as u8,
      (channel(rgba[3]) * self.alpha * 255.0).round() as u8,
    ]
  }

  fn from_filter(filter: &Filter) -> Option<Self> {
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

  /// The CSS hue-rotate matrix, which rotates around the luma axis.
  fn hue_rotate(angle: Angle) -> Self {
    let radians = angle.to_degrees().to_radians();
    let (sin, cos) = radians.sin_cos();
    let [lr, lg, lb] = LUMA_WEIGHTS;
    let row = |base: [f32; 3], cosine: [f32; 3], sine: [f32; 3]| {
      [
        base[0] + cos * cosine[0] + sin * sine[0],
        base[1] + cos * cosine[1] + sin * sine[1],
        base[2] + cos * cosine[2] + sin * sine[2],
        0.0,
      ]
    };

    Self {
      rows: [
        row([lr, lg, lb], [1.0 - lr, -lg, -lb], [-lr, -lg, 1.0 - lb]),
        row([lr, lg, lb], [-lr, 1.0 - lg, -lb], [lr, lg - 1.0, lb]),
        row([lr, lg, lb], [-lr, -lg, 1.0 - lb], [lr - 1.0, lg, lb]),
      ],
      alpha: 1.0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::ColorFilter;
  use takumi_core::style::{Filter, PercentageNumber};

  #[test]
  fn each_function_clamps_before_the_next_one_runs() {
    // brightness(200%) drives #cc3333 past white on the red channel; CSS clamps
    // it to #ff6666 before grayscale sees it, which lands on #878787.
    let filters = [
      Filter::Brightness(PercentageNumber(2.0)),
      Filter::Grayscale(PercentageNumber(1.0)),
    ];
    let filter = ColorFilter::compose(None, &filters).expect("a filter");
    let [red, green, blue, _] = filter.apply([0xcc, 0x33, 0x33, 255]);

    assert_eq!([red, green, blue], [0x87, 0x87, 0x87]);
  }

  #[test]
  fn invert_clamps_its_amount() {
    let filter =
      ColorFilter::compose(None, &[Filter::Invert(PercentageNumber(2.0))]).expect("a filter");

    assert_eq!(
      filter.apply([0x40, 0x40, 0x40, 255]),
      [0xbf, 0xbf, 0xbf, 255]
    );
  }
}

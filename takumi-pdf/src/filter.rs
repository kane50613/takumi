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
  /// Prepends a filter list to whatever the ancestors already apply: CSS
  /// filters the element first and the ancestor's group afterwards, and the
  /// matrices do not commute. Returns `None` when nothing changes color.
  pub(crate) fn compose(outer: Option<&Self>, filters: &[Filter]) -> Option<Self> {
    let mut functions: Vec<ColorMatrix> = filters
      .iter()
      .filter_map(ColorMatrix::from_filter)
      .collect();

    if let Some(outer) = outer {
      functions.extend_from_slice(&outer.functions);
    }
    (!functions.is_empty()).then_some(Self { functions })
  }

  /// Applies every filter in order. Each result is clamped before the next one
  /// runs, as CSS requires, but the pipeline stays in floats so the channels
  /// are quantized once at the end rather than between functions.
  pub(crate) fn apply(&self, rgba: [u8; 4]) -> [u8; 4] {
    let channel = |value: u8| f32::from(value) / 255.0;
    let color = [
      channel(rgba[0]),
      channel(rgba[1]),
      channel(rgba[2]),
      channel(rgba[3]),
    ];
    let mixed = self
      .functions
      .iter()
      .fold(color, |color, function| function.apply(color));

    mixed.map(|value| (value * 255.0).round() as u8)
  }

  /// Applies the filter to a color value.
  pub(crate) fn apply_color(&self, color: Color) -> Color {
    Color(self.apply(color.0))
  }
}

impl ColorMatrix {
  /// Applies the matrix to a straight (non-premultiplied) color in 0..1.
  fn apply(self, color: [f32; 4]) -> [f32; 4] {
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
  use super::ColorFilter;
  use takumi_core::style::{Angle, Filter, PercentageNumber};

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
  fn an_ancestor_filter_runs_after_the_element_own() {
    // CSS renders the child, filters it, then filters the parent's group.
    let own = ColorFilter::compose(None, &[Filter::Contrast(PercentageNumber(2.0))]);
    let composed = ColorFilter::compose(
      ColorFilter::compose(None, &[Filter::Brightness(PercentageNumber(0.5))]).as_ref(),
      &[Filter::Contrast(PercentageNumber(2.0))],
    )
    .expect("a filter");

    assert!(own.is_some());
    assert_eq!(
      composed.apply([0x80, 0x80, 0x80, 255]),
      [0x40, 0x40, 0x40, 255]
    );
  }

  #[test]
  fn hue_rotate_reads_its_angle_in_degrees() {
    let filter =
      ColorFilter::compose(None, &[Filter::HueRotate(Angle::new(180.0))]).expect("filter");

    assert_eq!(filter.apply([225, 29, 72, 255]), [0, 119, 76, 255]);
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

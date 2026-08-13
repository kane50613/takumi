//! The color half of CSS `filter`, as a matrix applied to every color the
//! filtered subtree paints.
//!
//! `grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness` and
//! `contrast` are linear transforms of the source color, so a vector backend
//! can fold them into the colors it writes instead of rasterizing. `opacity`
//! folds into the alpha channel. The primitives that need a convolution
//! (`blur`, `drop-shadow`) and referenced SVG filters are left alone.

use takumi_core::{
  filter::ColorMatrix,
  style::{Color, Filter, ToCss},
};

/// The filter list as written, applied in order. CSS clamps each function's
/// result before handing it to the next, so the matrices stay separate rather
/// than composing into one.
#[derive(Clone, Debug, Default)]
pub(crate) struct ColorFilter {
  functions: Vec<ColorMatrix>,
}

/// The first filter function that does not fold into a color matrix, written
/// back as CSS. Those are the ones a vector backend has to rasterize, which is
/// what this backend does not do.
pub(crate) fn unsupported_filter(filters: &[Filter]) -> Option<String> {
  filters
    .iter()
    .find(|filter| ColorMatrix::from_filter(filter).is_none())
    .map(ToCss::to_css_string)
}

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

#[cfg(test)]
mod tests {
  use takumi_core::style::{Angle, Filter, PercentageNumber};

  use super::ColorFilter;

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

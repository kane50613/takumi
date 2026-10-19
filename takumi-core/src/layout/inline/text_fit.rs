//! Blink's `TextFitScale` as paint applies it: a line's text scaled about its origin.

use crate::{geometry::Point, style::Affine};

/// A run's text-fit scale, applied about `layout_origin`.
#[derive(Clone, Copy)]
pub(crate) struct LineScaleState {
  /// Text-fit scale factor for the run.
  pub(crate) scale: f32,
  /// The origin the scale is applied about: the run's start on its baseline, in the border box.
  pub(crate) layout_origin: Point<f32>,
}

impl LineScaleState {
  /// `base` composed with the scale about the origin.
  pub(crate) fn transform(self, base: Affine) -> Affine {
    base
      * Affine::translation(self.layout_origin.x, self.layout_origin.y)
      * Affine::scale(self.scale, self.scale)
      * Affine::translation(-self.layout_origin.x, -self.layout_origin.y)
  }
}

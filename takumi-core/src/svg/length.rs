//! Lengths against the nearest SVG viewport, after Blink's `SVGLengthContext`.

use crate::{
  geometry::Rect,
  style::{Length, SizingContext},
};

/// Which dimension of the viewport a percentage is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LengthAxis {
  /// The width, for `x`, `cx`, `rx`, and `width`.
  X,
  /// The height, for `y`, `cy`, `ry`, and `height`.
  Y,
  /// The diagonal over the square root of two, for every other length.
  Other,
}

/// Resolves lengths in user units against one viewport.
#[derive(Debug, Clone, Copy)]
pub(super) struct SvgLengthContext {
  width: f32,
  height: f32,
}

impl SvgLengthContext {
  pub(super) fn new(view_box: Rect<f32>) -> Self {
    Self {
      width: view_box.width(),
      height: view_box.height(),
    }
  }

  /// `length` in user units, with `em` and `rem` read from `sizing` at a device pixel ratio of one.
  pub(super) fn resolve(self, length: Length, axis: LengthAxis, sizing: &SizingContext) -> f32 {
    let base = match axis {
      LengthAxis::X => self.width,
      LengthAxis::Y => self.height,
      LengthAxis::Other => ((self.width.powi(2) + self.height.powi(2)) / 2.0).sqrt(),
    };

    match length {
      Length::Percentage(percent) => base * percent / 100.0,
      length => length.to_px(sizing, base),
    }
  }

  /// `length` in the fractions of a bounding box, where a percentage is a hundredth.
  pub(super) fn resolve_in_bounding_box(length: Length, sizing: &SizingContext) -> f32 {
    match length {
      Length::Percentage(percent) => percent / 100.0,
      length => length.to_px(sizing, 0.0),
    }
  }
}

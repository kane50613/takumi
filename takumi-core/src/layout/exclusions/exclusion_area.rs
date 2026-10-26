//! Blink's [`ExclusionArea`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/exclusions/exclusion_area.h).

use crate::layout::{bfc_rect::BfcRect, floats_utils::Float};

/// Blink's `ExclusionArea`: a float's margin box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ExclusionArea {
  pub(crate) rect: BfcRect,
  /// `Left` or `Right`, resolved against the containing block's direction.
  pub(crate) float_type: Float,
  /// Whether it starts at or below every exclusion before it.
  pub(crate) is_past_other_exclusions: bool,
}

impl ExclusionArea {
  pub(crate) fn new(rect: BfcRect, float_type: Float) -> Self {
    Self {
      rect,
      float_type,
      is_past_other_exclusions: false,
    }
  }
}

//! Blink's [`LeadingFloats`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/leading_floats.h),
//! without block fragmentation.

use crate::layout::floats_utils::PositionedFloat;

/// Blink's `LeadingFloats`: the floats ahead of a line's content, placed before its line breaks.
#[derive(Debug, Clone, Default)]
pub(crate) struct LeadingFloats {
  pub(crate) floats: Vec<PositionedFloat>,
  /// The index of the first item after them.
  pub(crate) handled_index: usize,
}

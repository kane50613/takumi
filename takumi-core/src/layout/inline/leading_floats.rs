//! Blink's [`LeadingFloats` and `LeadingFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/leading_floats.h).

#[cfg(feature = "fragmentation")]
use super::inline_break_token::InlineBreakToken;
use crate::layout::floats_utils::PositionedFloat;

/// Blink's `LeadingFloat`: a float ahead of a line's content, and where it resumes on a later page.
#[derive(Debug, Clone)]
pub(crate) struct LeadingFloat {
  pub(crate) positioned_float: PositionedFloat,
  /// Blink's `parallel_flow_break_token`: where the float the page broke resumes.
  #[cfg(feature = "fragmentation")]
  pub(crate) parallel_flow_break_token: Option<InlineBreakToken>,
}

/// Blink's `LeadingFloats`: the floats ahead of a line's content, placed before its line breaks.
#[derive(Debug, Clone, Default)]
pub(crate) struct LeadingFloats {
  pub(crate) floats: Vec<LeadingFloat>,
  /// The index of the first item after them.
  pub(crate) handled_index: usize,
}

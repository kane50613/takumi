//! Blink's [`FragmentationLayoutInput`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/fragmentation_layout_input.h),
//! empty without the `fragmentation` feature.

#[cfg(feature = "fragmentation")]
use std::rc::Rc;

#[cfg(feature = "fragmentation")]
use crate::layout::{block_break_token::BlockBreakToken, early_break::EarlyBreak};

/// Where a layout resumes the box, and where it has to break.
#[derive(Debug, Clone, Default)]
pub(crate) struct FragmentationLayoutInput {
  #[cfg(feature = "fragmentation")]
  pub(crate) break_token: Option<Rc<BlockBreakToken>>,
  #[cfg(feature = "fragmentation")]
  pub(crate) early_break: Option<Rc<EarlyBreak>>,
}

impl FragmentationLayoutInput {
  /// Whether the layout starts the box afresh with no breakpoint to honor, which a cached layout
  /// stands in for.
  pub(crate) fn is_fresh(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    return self.break_token.is_none() && self.early_break.is_none();
    #[cfg(not(feature = "fragmentation"))]
    return true;
  }
}

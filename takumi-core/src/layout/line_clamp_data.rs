//! Blink's [`LineClampData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/line_clamp_data.h),
//! clamping by lines alone.

/// Blink's `LineClampFloatState`: what a line-clamp container does with a float inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineClampFloatState {
  Show,
  Hide,
  /// Clipped to the container's block-end content edge.
  Clip,
}

/// Blink's `LineClampData`: the lines until the clamp, none outside a line-clamp container.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LineClampData {
  /// Blink's `lines_until_clamp`: 1 on the line the clamp ellipsizes, and may go negative.
  pub(crate) lines_until_clamp: Option<i32>,
}

impl LineClampData {
  /// Blink's `IsAtClampPoint`.
  pub(crate) fn is_at_clamp_point(self) -> bool {
    self.lines_until_clamp == Some(1)
  }

  /// Blink's `IsPastClampPoint`.
  pub(crate) fn is_past_clamp_point(self) -> bool {
    self.lines_until_clamp.is_some_and(|lines| lines <= 0)
  }

  /// Blink's `FloatState`.
  pub(crate) fn float_state(self) -> LineClampFloatState {
    if self.is_past_clamp_point() {
      LineClampFloatState::Hide
    } else if self.lines_until_clamp.is_some() {
      LineClampFloatState::Clip
    } else {
      LineClampFloatState::Show
    }
  }
}

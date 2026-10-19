//! Blink's [`InlineChildLayoutContext`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_child_layout_context.cc),
//! without fragment items or parallel flows.

use super::{
  inline_box_state::InlineLayoutStateStack, score_line_breaker::ScoreLineBreakContext,
  text_fit_utils::ParagraphScale,
};
use crate::layout_unit::LayoutUnit;

/// Blink's `InlineChildLayoutContext`: what one line's layout leaves for the next.
#[derive(Clone, Default)]
pub(crate) struct InlineChildLayoutContext<'a, 'c> {
  box_states: Option<InlineLayoutStateStack<'a, 'c>>,
  /// The item the box states end at.
  item_index: usize,
  /// The width `text-wrap: balance` breaks the lines at.
  pub(crate) balanced_available_width: Option<LayoutUnit>,
  /// For `text-wrap: pretty` and `balance`, unless the content disables scoring.
  pub(crate) score_line_break_context: Option<ScoreLineBreakContext>,
  /// Blink's `IsMeasuringScale`: whether the lines lay out to measure a `text-fit` scale.
  pub(crate) is_measuring_scale: bool,
  /// Blink's `MeasuredScale`: the scale every line of a consistent `text-fit` takes.
  pub(crate) measured_scale: ParagraphScale,
}

impl<'a, 'c> InlineChildLayoutContext<'a, 'c> {
  /// Blink's `EnableMeasuringModeIfNecessary`: measures without a paragraph scale, and applies
  /// one when given.
  pub(crate) fn enable_measuring_mode_if_necessary(
    &mut self,
    paragraph_scale: Option<ParagraphScale>,
  ) {
    match paragraph_scale {
      Some(paragraph_scale) => {
        debug_assert!(!self.is_measuring_scale);
        debug_assert!(paragraph_scale.scale > 0.0);

        self.measured_scale = paragraph_scale;
      }
      None => self.is_measuring_scale = true,
    }
  }

  /// Blink's `ResetBoxStates`.
  pub(crate) fn reset_box_states(&mut self) -> &mut InlineLayoutStateStack<'a, 'c> {
    self.box_states.insert(InlineLayoutStateStack::default())
  }

  /// The box states the last line left, or new ones.
  pub(crate) fn box_states(&mut self) -> &mut InlineLayoutStateStack<'a, 'c> {
    self.box_states.get_or_insert_default()
  }

  /// Blink's `BoxStatesIfValidForItemIndex`: whether the box states are the ones a line ending at
  /// `item_index` left.
  pub(crate) fn has_box_states_for_item_index(&self, item_index: usize) -> bool {
    self.box_states.is_some() && self.item_index == item_index
  }

  /// Blink's `SetItemIndex`.
  pub(crate) fn set_item_index(&mut self, item_index: usize) {
    self.item_index = item_index;
  }
}

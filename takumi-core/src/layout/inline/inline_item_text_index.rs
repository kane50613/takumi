//! Blink's [`InlineItemTextIndex`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_item_text_index.h).

/// A position in an inline formatting context: an item and an offset in the text content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InlineItemTextIndex {
  pub(crate) item_index: usize,
  pub(crate) text_offset: usize,
}

impl InlineItemTextIndex {
  pub(crate) fn new(item_index: usize, text_offset: usize) -> Self {
    Self {
      item_index,
      text_offset,
    }
  }

  pub(crate) fn is_zero(self) -> bool {
    self.text_offset == 0 && self.item_index == 0
  }

  /// Blink's `operator<`, true when either part is less.
  pub(crate) fn is_less_than(self, other: Self) -> bool {
    self.text_offset < other.text_offset || self.item_index < other.item_index
  }

  /// Blink's `operator>=`.
  pub(crate) fn is_at_least(self, other: Self) -> bool {
    !self.is_less_than(other)
  }
}

//! Blink's [`BfcOffset`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/bfc_offset.h)
//! and [`BfcRect`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/bfc_rect.h):
//! positions in a block formatting context's line-relative coordinates.

use std::ops::{Add, Neg};

use crate::layout_unit::LayoutUnit;

/// Blink's `BfcOffset`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BfcOffset {
  pub(crate) line_offset: LayoutUnit,
  pub(crate) block_offset: LayoutUnit,
}

impl BfcOffset {
  pub(crate) fn new(line_offset: LayoutUnit, block_offset: LayoutUnit) -> Self {
    Self {
      line_offset,
      block_offset,
    }
  }
}

impl Add for BfcOffset {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self::new(
      self.line_offset + other.line_offset,
      self.block_offset + other.block_offset,
    )
  }
}

impl Neg for BfcOffset {
  type Output = Self;

  fn neg(self) -> Self {
    Self::new(-self.line_offset, -self.block_offset)
  }
}

/// Blink's `BfcRect`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BfcRect {
  pub(crate) start_offset: BfcOffset,
  pub(crate) end_offset: BfcOffset,
}

impl BfcRect {
  pub(crate) fn new(start_offset: BfcOffset, end_offset: BfcOffset) -> Self {
    debug_assert!(end_offset.line_offset >= start_offset.line_offset);
    debug_assert!(end_offset.block_offset >= start_offset.block_offset);

    Self {
      start_offset,
      end_offset,
    }
  }

  pub(crate) fn line_start_offset(&self) -> LayoutUnit {
    self.start_offset.line_offset
  }

  pub(crate) fn line_end_offset(&self) -> LayoutUnit {
    self.end_offset.line_offset
  }

  pub(crate) fn block_start_offset(&self) -> LayoutUnit {
    self.start_offset.block_offset
  }

  pub(crate) fn block_end_offset(&self) -> LayoutUnit {
    self.end_offset.block_offset
  }

  pub(crate) fn block_size(&self) -> LayoutUnit {
    if self.end_offset.block_offset == LayoutUnit::MAX {
      return LayoutUnit::MAX;
    }

    self.end_offset.block_offset - self.start_offset.block_offset
  }

  pub(crate) fn inline_size(&self) -> LayoutUnit {
    if self.end_offset.line_offset == LayoutUnit::MAX {
      return if self.start_offset.line_offset == LayoutUnit::MAX {
        LayoutUnit::ZERO
      } else {
        LayoutUnit::MAX
      };
    }

    self.end_offset.line_offset - self.start_offset.line_offset
  }
}

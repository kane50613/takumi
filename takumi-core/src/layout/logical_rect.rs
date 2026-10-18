//! Blink's [`LogicalOffset`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/logical_offset.h),
//! [`LogicalSize`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/logical_size.h)
//! and [`LogicalRect`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/logical_rect.h).

use std::ops::{Add, AddAssign, Sub};

use crate::layout_unit::LayoutUnit;

/// Blink's `LogicalOffset`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LogicalOffset {
  pub(crate) inline_offset: LayoutUnit,
  pub(crate) block_offset: LayoutUnit,
}

impl LogicalOffset {
  pub(crate) fn new(inline_offset: LayoutUnit, block_offset: LayoutUnit) -> Self {
    Self {
      inline_offset,
      block_offset,
    }
  }
}

impl Add for LogicalOffset {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self::new(
      self.inline_offset + other.inline_offset,
      self.block_offset + other.block_offset,
    )
  }
}

impl AddAssign for LogicalOffset {
  fn add_assign(&mut self, other: Self) {
    *self = *self + other;
  }
}

impl Sub for LogicalOffset {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self::new(
      self.inline_offset - other.inline_offset,
      self.block_offset - other.block_offset,
    )
  }
}

/// Blink's `LogicalSize`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LogicalSize {
  pub(crate) inline_size: LayoutUnit,
  pub(crate) block_size: LayoutUnit,
}

impl LogicalSize {
  pub(crate) fn new(inline_size: LayoutUnit, block_size: LayoutUnit) -> Self {
    Self {
      inline_size,
      block_size,
    }
  }
}

/// Blink's `LogicalRect`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LogicalRect {
  pub(crate) offset: LogicalOffset,
  pub(crate) size: LogicalSize,
}

impl LogicalRect {
  pub(crate) fn new(
    inline_offset: LayoutUnit,
    block_offset: LayoutUnit,
    inline_size: LayoutUnit,
    block_size: LayoutUnit,
  ) -> Self {
    Self {
      offset: LogicalOffset::new(inline_offset, block_offset),
      size: LogicalSize::new(inline_size, block_size),
    }
  }
}

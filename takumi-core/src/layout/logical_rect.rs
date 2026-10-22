//! Blink's [`LogicalOffset`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/logical_offset.h),
//! [`LogicalSize`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/logical_size.h)
//! [`LogicalRect`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/logical_rect.h)
//! and [`BoxStrut`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/box_strut.h),
//! in a horizontal writing mode.

use std::ops::{Add, AddAssign, Sub};

use crate::{geometry::Rect, layout_unit::LayoutUnit, style::Direction};

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

/// Blink's logical `BoxStrut`: a length on each side of a box, its inline sides in a direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LogicalBoxStrut {
  pub(crate) inline_start: LayoutUnit,
  pub(crate) inline_end: LayoutUnit,
  pub(crate) block_start: LayoutUnit,
  pub(crate) block_end: LayoutUnit,
}

impl LogicalBoxStrut {
  /// The physical `edges` with their inline sides in `direction`.
  pub(crate) fn from_physical(edges: Rect<LayoutUnit>, direction: Direction) -> Self {
    let (inline_start, inline_end) = direction.inline_sides(edges.left, edges.right);

    Self {
      inline_start,
      inline_end,
      block_start: edges.top,
      block_end: edges.bottom,
    }
  }

  pub(crate) fn inline_sum(self) -> LayoutUnit {
    self.inline_start + self.inline_end
  }

  pub(crate) fn block_sum(self) -> LayoutUnit {
    self.block_start + self.block_end
  }

  /// Blink's `LineLeft`: the side the line starts from in `direction`.
  pub(crate) fn line_left(self, direction: Direction) -> LayoutUnit {
    match direction {
      Direction::Ltr => self.inline_start,
      Direction::Rtl => self.inline_end,
    }
  }

  /// Blink's `LineRight`.
  pub(crate) fn line_right(self, direction: Direction) -> LayoutUnit {
    match direction {
      Direction::Ltr => self.inline_end,
      Direction::Rtl => self.inline_start,
    }
  }
}

impl Add for LogicalBoxStrut {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self {
      inline_start: self.inline_start + other.inline_start,
      inline_end: self.inline_end + other.inline_end,
      block_start: self.block_start + other.block_start,
      block_end: self.block_end + other.block_end,
    }
  }
}

/// Blink's `LogicalSize` where a size may be indefinite, as its `kIndefiniteSize` marks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AvailableLogicalSize {
  pub(crate) inline_size: Option<LayoutUnit>,
  pub(crate) block_size: Option<LayoutUnit>,
}

impl AvailableLogicalSize {
  /// Blink's `ShrinkLogicalSize`: the size less `insets`, an indefinite size left indefinite.
  pub(crate) fn shrink(self, insets: LogicalBoxStrut) -> Self {
    Self {
      inline_size: self
        .inline_size
        .map(|inline_size| (inline_size - insets.inline_sum()).clamp_negative_to_zero()),
      block_size: self
        .block_size
        .map(|block_size| (block_size - insets.block_sum()).clamp_negative_to_zero()),
    }
  }
}

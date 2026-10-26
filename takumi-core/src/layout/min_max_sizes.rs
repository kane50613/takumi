//! Blink's [`MinMaxSizes`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/min_max_sizes.h).

use std::ops::{Add, AddAssign, SubAssign};

use crate::layout_unit::LayoutUnit;

/// Blink's `MinMaxSizes`: the min-content and max-content inline sizes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MinMaxSizes {
  pub(crate) min_size: LayoutUnit,
  pub(crate) max_size: LayoutUnit,
}

impl MinMaxSizes {
  /// A min size of zero and no max size, which clamps nothing.
  pub(crate) const UNCONSTRAINED: Self = Self {
    min_size: LayoutUnit::ZERO,
    max_size: LayoutUnit::MAX,
  };

  /// Both sizes `size`.
  pub(crate) const fn both(size: LayoutUnit) -> Self {
    Self {
      min_size: size,
      max_size: size,
    }
  }

  /// Blink's `ShrinkToFit`: `available_size` clamped between the two.
  pub(crate) fn shrink_to_fit(self, available_size: LayoutUnit) -> LayoutUnit {
    self.max_size.min(available_size).max(self.min_size)
  }

  /// Blink's `ClampSizeToMinAndMax`: `size` clamped by the max size, then the min size.
  pub(crate) fn clamp_size_to_min_and_max(self, size: LayoutUnit) -> LayoutUnit {
    size.min(self.max_size).max(self.min_size)
  }

  /// Blink's `Encompass`: both sizes at least `value`.
  pub(crate) fn encompass(&mut self, value: LayoutUnit) {
    self.min_size = self.min_size.max(value);
    self.max_size = self.max_size.max(value);
  }

  /// Blink's `Constrain`: both sizes at most `value`.
  pub(crate) fn constrain(&mut self, value: LayoutUnit) {
    self.min_size = self.min_size.min(value);
    self.max_size = self.max_size.min(value);
  }
}

impl Add<LayoutUnit> for MinMaxSizes {
  type Output = Self;

  fn add(self, value: LayoutUnit) -> Self {
    Self {
      min_size: self.min_size + value,
      max_size: self.max_size + value,
    }
  }
}

impl AddAssign<LayoutUnit> for MinMaxSizes {
  fn add_assign(&mut self, value: LayoutUnit) {
    *self = *self + value;
  }
}

impl SubAssign<LayoutUnit> for MinMaxSizes {
  fn sub_assign(&mut self, value: LayoutUnit) {
    self.min_size -= value;
    self.max_size -= value;
  }
}

/// Blink's `MinMaxSizesType`: whether a min-content size takes a box's own min-intrinsic size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SizeType {
  Content,
  Intrinsic,
}

/// Blink's `MinMaxSizesResult`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MinMaxSizesResult {
  pub(crate) sizes: MinMaxSizes,
  /// Whether the sizes change with the percentage block size they resolve against.
  pub(crate) depends_on_block_constraints: bool,
  /// Whether an aspect-ratio gave the sizes.
  pub(crate) applied_aspect_ratio: bool,
}

impl MinMaxSizesResult {
  pub(crate) fn new(sizes: MinMaxSizes, depends_on_block_constraints: bool) -> Self {
    Self {
      sizes,
      depends_on_block_constraints,
      applied_aspect_ratio: false,
    }
  }
}

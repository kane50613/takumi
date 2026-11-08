//! Blink's [`LineClampData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/line_clamp_data.h).

use std::rc::Rc;

use crate::{
  geometry::NodeId,
  layout::{margin_strut::MarginStrut, min_max_sizes::MinMaxSizes},
  layout_unit::LayoutUnit,
};

/// Blink's `LineClampFloatState`: what a line-clamp container does with a float inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineClampFloatState {
  Show,
  Hide,
  /// Clipped to the container's block-end content edge.
  Clip,
}

/// Blink's `LineClampData::State`, without `kClampByLinesWithBfcOffset`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LineClampState {
  #[default]
  Disabled,
  /// Clamps after a number of lines.
  ClampByLines,
  /// Clamps just after the end of a box.
  ClampAfterLayoutObject,
  /// Counts the lines up to the clamp block offset, where the container lays out again to clamp.
  MeasureLinesUntilBfcOffset,
  /// Counts the lines for an ancestor measuring up to its clamp block offset.
  CountLines,
}

/// Blink's `LineClampData`: where a line-clamp container clamps, none outside one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LineClampData {
  pub(crate) state: LineClampState,
  /// Blink's `lines_until_clamp`: when clamping by lines, 1 on the line the clamp ellipsizes, and
  /// may go negative. When counting lines, the lines so far.
  pub(crate) lines_until_clamp: i32,
  /// The block offset in the formatting context the container clamps at, when measuring.
  pub(crate) clamp_bfc_offset: LayoutUnit,
  /// The lineless box the container clamps right after.
  pub(crate) clamp_after_layout_object: Option<NodeId>,
}

impl LineClampData {
  /// Blink's `IsLineClampContext`.
  pub(crate) fn is_line_clamp_context(self) -> bool {
    self.state != LineClampState::Disabled
  }

  /// Blink's `IsClampByLines`.
  pub(crate) fn is_clamp_by_lines(self) -> bool {
    self.state == LineClampState::ClampByLines
  }

  /// Blink's `IsMeasureUntilBfcOffset`.
  pub(crate) fn is_measure_until_bfc_offset(self) -> bool {
    self.state == LineClampState::MeasureLinesUntilBfcOffset
  }

  /// Blink's `IsCountLines`.
  pub(crate) fn is_count_lines(self) -> bool {
    matches!(
      self.state,
      LineClampState::MeasureLinesUntilBfcOffset | LineClampState::CountLines
    )
  }

  /// Blink's `LinesUntilClamp`, with the lines counted so far too if `show_measured_lines`.
  pub(crate) fn lines_until_clamp(self, show_measured_lines: bool) -> Option<i32> {
    (self.is_clamp_by_lines() || (show_measured_lines && self.is_count_lines()))
      .then_some(self.lines_until_clamp)
  }

  /// Blink's `IsAtClampPoint`.
  pub(crate) fn is_at_clamp_point(self) -> bool {
    self.is_clamp_by_lines() && self.lines_until_clamp == 1
  }

  /// Blink's `IsPastClampPoint`.
  pub(crate) fn is_past_clamp_point(self) -> bool {
    self.is_clamp_by_lines() && self.lines_until_clamp <= 0
  }

  /// Blink's `FloatState`.
  pub(crate) fn float_state(self) -> LineClampFloatState {
    if self.is_past_clamp_point() {
      LineClampFloatState::Hide
    } else if self.is_line_clamp_context() {
      LineClampFloatState::Clip
    } else {
      LineClampFloatState::Show
    }
  }
}

/// Blink's `LineClampAncestorChain`: the boxes from a clamp point up to its line-clamp container,
/// to size the container as if it clamped there.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LineClampAncestorChain {
  bfc_offset: LayoutUnit,
  end_border_padding: LayoutUnit,
  end_margin: LayoutUnit,
  block_min_max_sizes: MinMaxSizes,
  parent: Option<Rc<LineClampAncestorChain>>,
}

impl LineClampAncestorChain {
  /// The chain of the line-clamp container itself.
  pub(crate) fn root(end_border_padding: LayoutUnit) -> Self {
    Self {
      bfc_offset: LayoutUnit::ZERO,
      end_border_padding,
      end_margin: LayoutUnit::ZERO,
      block_min_max_sizes: MinMaxSizes::UNCONSTRAINED,
      parent: None,
    }
  }

  /// The chain of a block inside `parent`'s box.
  pub(crate) fn child(
    bfc_offset: LayoutUnit,
    end_border_padding: LayoutUnit,
    end_margin: LayoutUnit,
    block_min_max_sizes: MinMaxSizes,
    parent: Rc<Self>,
  ) -> Self {
    Self {
      bfc_offset,
      end_border_padding,
      end_margin,
      block_min_max_sizes,
      parent: Some(parent),
    }
  }

  /// Blink's `FinalLineClampBlockSize`: the line-clamp container's block size for a clamp point in
  /// this box at `inflow_block_offset` with the margins `margin_strut` after it.
  pub(crate) fn final_line_clamp_block_size(
    &self,
    inflow_block_offset: LayoutUnit,
    mut margin_strut: MarginStrut,
  ) -> LayoutUnit {
    let mut block_size = inflow_block_offset;

    if self.end_border_padding != LayoutUnit::ZERO || self.parent.is_none() {
      block_size += margin_strut.sum() + self.end_border_padding;
      margin_strut = MarginStrut::default();
    }

    let Some(parent) = &self.parent else {
      return block_size;
    };

    let clamped_size = self
      .block_min_max_sizes
      .clamp_size_to_min_and_max(block_size);

    if clamped_size != block_size || self.block_min_max_sizes.max_size == block_size {
      margin_strut = MarginStrut::default();
    }
    margin_strut.append(self.end_margin);

    parent.final_line_clamp_block_size(
      self.bfc_offset + clamped_size - parent.bfc_offset,
      margin_strut,
    )
  }
}

//! Blink's [`StyleContentAlignmentData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/style_content_alignment_data.h):
//! the alignment `justify-content` and `align-content` ask of a container's content, and the one
//! [`block_layout_algorithm_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_layout_algorithm_utils.cc)
//! gives a block container or table cell.

use crate::{
  layout_unit::LayoutUnit,
  style::{ComputedStyle, JustifyContent, VerticalAlign, VerticalAlignKeyword},
};

/// Blink's `ContentPosition`, after `GetUsedPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentPosition {
  Normal,
  Start,
  End,
  FlowStart,
  FlowEnd,
  Center,
}

/// Blink's `ContentDistributionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentDistribution {
  Default,
  SpaceBetween,
  SpaceAround,
  SpaceEvenly,
  Stretch,
}

/// Blink's `OverflowAlignment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverflowAlignment {
  Default,
  Safe,
  Unsafe,
}

/// Blink's `StyleContentAlignmentData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContentAlignment {
  pub(crate) position: ContentPosition,
  pub(crate) distribution: ContentDistribution,
  pub(crate) overflow: OverflowAlignment,
}

impl ContentAlignment {
  pub(crate) fn of(value: JustifyContent) -> Self {
    let (position, overflow) = match value {
      JustifyContent::Start => (ContentPosition::Start, OverflowAlignment::Default),
      JustifyContent::SafeStart => (ContentPosition::Start, OverflowAlignment::Safe),
      JustifyContent::UnsafeStart => (ContentPosition::Start, OverflowAlignment::Unsafe),
      JustifyContent::End => (ContentPosition::End, OverflowAlignment::Default),
      JustifyContent::SafeEnd => (ContentPosition::End, OverflowAlignment::Safe),
      JustifyContent::UnsafeEnd => (ContentPosition::End, OverflowAlignment::Unsafe),
      JustifyContent::FlexStart | JustifyContent::FlowStart => {
        (ContentPosition::FlowStart, OverflowAlignment::Default)
      }
      JustifyContent::SafeFlexStart | JustifyContent::SafeFlowStart => {
        (ContentPosition::FlowStart, OverflowAlignment::Safe)
      }
      JustifyContent::UnsafeFlexStart | JustifyContent::UnsafeFlowStart => {
        (ContentPosition::FlowStart, OverflowAlignment::Unsafe)
      }
      JustifyContent::FlexEnd | JustifyContent::FlowEnd => {
        (ContentPosition::FlowEnd, OverflowAlignment::Default)
      }
      JustifyContent::SafeFlexEnd | JustifyContent::SafeFlowEnd => {
        (ContentPosition::FlowEnd, OverflowAlignment::Safe)
      }
      JustifyContent::UnsafeFlexEnd | JustifyContent::UnsafeFlowEnd => {
        (ContentPosition::FlowEnd, OverflowAlignment::Unsafe)
      }
      JustifyContent::Center => (ContentPosition::Center, OverflowAlignment::Default),
      JustifyContent::SafeCenter => (ContentPosition::Center, OverflowAlignment::Safe),
      JustifyContent::UnsafeCenter => (ContentPosition::Center, OverflowAlignment::Unsafe),
      JustifyContent::Normal
      | JustifyContent::Stretch
      | JustifyContent::SpaceBetween
      | JustifyContent::SpaceAround
      | JustifyContent::SpaceEvenly => (ContentPosition::Normal, OverflowAlignment::Default),
    };
    let distribution = match value {
      JustifyContent::Stretch => ContentDistribution::Stretch,
      JustifyContent::SpaceBetween => ContentDistribution::SpaceBetween,
      JustifyContent::SpaceAround => ContentDistribution::SpaceAround,
      JustifyContent::SpaceEvenly => ContentDistribution::SpaceEvenly,
      _ => ContentDistribution::Default,
    };

    Self {
      position,
      distribution,
      overflow,
    }
  }

  /// Whether an overflowing content stays at the start edge, as only an explicit `safe` keeps it
  /// in a flex or grid container.
  pub(crate) fn is_safe(self) -> bool {
    self.overflow == OverflowAlignment::Safe
  }
}

/// Blink's `BlockContentAlignment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockContentAlignment {
  Start,
  Baseline,
  SafeCenter,
  UnsafeCenter,
  SafeEnd,
  UnsafeEnd,
}

impl BlockContentAlignment {
  /// Blink's `ComputeContentAlignmentForBlock`.
  pub(crate) fn for_block(align_content: JustifyContent) -> Self {
    Self::compute(align_content, None)
  }

  /// Blink's `ComputeContentAlignmentForTableCell`.
  pub(crate) fn for_table_cell(style: &ComputedStyle) -> Self {
    Self::compute(
      style.misc_data.align_content,
      Some(&style.base_data.vertical_align),
    )
  }

  /// Blink's `ComputeContentAlignment`, given a table cell's `vertical-align`: a distribution
  /// takes its fallback, and a block container aligns safely unless `unsafe` says otherwise.
  fn compute(
    align_content: JustifyContent,
    table_cell_vertical_align: Option<&VerticalAlign>,
  ) -> Self {
    let alignment = ContentAlignment::of(align_content);
    let mut position = alignment.position;
    let mut overflow = alignment.overflow;

    match alignment.distribution {
      ContentDistribution::Default => {}
      ContentDistribution::SpaceBetween | ContentDistribution::Stretch => {
        position = ContentPosition::FlowStart;
      }
      ContentDistribution::SpaceAround | ContentDistribution::SpaceEvenly => {
        overflow = OverflowAlignment::Safe;
        position = ContentPosition::Center;
      }
    }

    let is_safe = overflow != OverflowAlignment::Unsafe;

    match position {
      ContentPosition::Center if is_safe => Self::SafeCenter,
      ContentPosition::Center => Self::UnsafeCenter,
      ContentPosition::End | ContentPosition::FlowEnd if is_safe => Self::SafeEnd,
      ContentPosition::End | ContentPosition::FlowEnd => Self::UnsafeEnd,
      ContentPosition::Normal => match table_cell_vertical_align {
        None | Some(VerticalAlign::Keyword(VerticalAlignKeyword::Top)) => Self::Start,
        Some(VerticalAlign::Keyword(VerticalAlignKeyword::Middle)) => Self::SafeCenter,
        Some(VerticalAlign::Keyword(VerticalAlignKeyword::Bottom)) => Self::SafeEnd,
        Some(_) => Self::Baseline,
      },
      ContentPosition::Start | ContentPosition::FlowStart => Self::Start,
    }
  }

  /// How far the alignment moves content down in `free_space`, which a safe alignment keeps from
  /// going below zero. A baseline alignment leaves it to its caller.
  pub(crate) fn offset(self, free_space: LayoutUnit) -> LayoutUnit {
    match self {
      Self::Start | Self::Baseline => LayoutUnit::ZERO,
      Self::SafeCenter => free_space.clamp_negative_to_zero() / 2,
      Self::UnsafeCenter => free_space / 2,
      Self::SafeEnd => free_space.clamp_negative_to_zero(),
      Self::UnsafeEnd => free_space,
    }
  }
}

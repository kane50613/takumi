//! Blink's [`FlexItem`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_item.h)
//! and [`FlexLine`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_line.h),
//! without fragmentation.

use crate::{
  geometry::NodeId,
  layout::{
    flex::flex_layout_algorithm::{BaselineGroup, ItemPosition},
    min_max_sizes::MinMaxSizes,
  },
  layout_unit::{BoxStrut, LayoutUnit},
};

/// Blink's `FlexerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlexerState {
  None,
  MinViolation,
  MaxViolation,
  Frozen,
}

/// Blink's `FlexItem`: an item's sizes as flexing reads and writes them.
#[derive(Debug, Clone)]
pub(crate) struct FlexItem {
  pub(crate) node: NodeId,
  pub(crate) flex_grow: f32,
  pub(crate) flex_shrink: f32,
  pub(crate) base_content_size: LayoutUnit,
  pub(crate) hypothetical_content_size: LayoutUnit,
  pub(crate) main_axis_min_max_sizes: MinMaxSizes,
  pub(crate) main_axis_border_padding: LayoutUnit,
  pub(crate) max_content_contribution: Option<LayoutUnit>,
  pub(crate) initial_margins: BoxStrut,
  pub(crate) main_axis_auto_margin_count: u8,
  pub(crate) alignment: ItemPosition,
  pub(crate) baseline_group: BaselineGroup,
  pub(crate) is_initial_block_size_indefinite: bool,
  pub(crate) is_used_flex_basis_indefinite: bool,
  pub(crate) is_horizontal_flow: bool,
  pub(crate) free_space_fraction: f64,
  pub(crate) state: FlexerState,
  pub(crate) flexed_content_size: LayoutUnit,
}

impl FlexItem {
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn new(
    node: NodeId,
    flex_grow: f32,
    flex_shrink: f32,
    base_content_size: LayoutUnit,
    main_axis_min_max_sizes: MinMaxSizes,
    main_axis_border_padding: LayoutUnit,
    max_content_contribution: Option<LayoutUnit>,
    initial_margins: BoxStrut,
    main_axis_auto_margin_count: u8,
    alignment: ItemPosition,
    baseline_group: BaselineGroup,
    is_initial_block_size_indefinite: bool,
    is_used_flex_basis_indefinite: bool,
    is_horizontal_flow: bool,
  ) -> Self {
    let hypothetical_content_size =
      main_axis_min_max_sizes.clamp_size_to_min_and_max(base_content_size);

    Self {
      node,
      flex_grow,
      flex_shrink,
      base_content_size,
      hypothetical_content_size,
      main_axis_min_max_sizes,
      main_axis_border_padding,
      max_content_contribution,
      initial_margins,
      main_axis_auto_margin_count,
      alignment,
      baseline_group,
      is_initial_block_size_indefinite,
      is_used_flex_basis_indefinite,
      is_horizontal_flow,
      free_space_fraction: 0.0,
      state: FlexerState::None,
      flexed_content_size: hypothetical_content_size,
    }
  }

  pub(crate) fn hypothetical_main_axis_margin_box_size(&self) -> LayoutUnit {
    self.hypothetical_content_size + self.main_axis_border_padding + self.main_axis_margin_extent()
  }

  pub(crate) fn flex_base_margin_box_size(&self) -> LayoutUnit {
    self.base_content_size + self.main_axis_border_padding + self.main_axis_margin_extent()
  }

  pub(crate) fn flexed_border_box_size(&self) -> LayoutUnit {
    self.flexed_content_size + self.main_axis_border_padding
  }

  pub(crate) fn flexed_margin_box_size(&self) -> LayoutUnit {
    self.flexed_content_size + self.main_axis_border_padding + self.main_axis_margin_extent()
  }

  pub(crate) fn main_axis_margin_extent(&self) -> LayoutUnit {
    if self.is_horizontal_flow {
      self.initial_margins.horizontal_sum()
    } else {
      self.initial_margins.vertical_sum()
    }
  }

  pub(crate) fn cross_axis_margin_extent(&self) -> LayoutUnit {
    if self.is_horizontal_flow {
      self.initial_margins.vertical_sum()
    } else {
      self.initial_margins.horizontal_sum()
    }
  }
}

/// Blink's `FlexLine`.
#[derive(Debug, Clone, Default)]
pub(crate) struct FlexLine {
  pub(crate) item_indices: Vec<usize>,
  pub(crate) main_axis_free_space: LayoutUnit,
  pub(crate) line_cross_size: LayoutUnit,
  pub(crate) major_baseline: LayoutUnit,
  pub(crate) minor_baseline: LayoutUnit,
  pub(crate) main_axis_auto_margin_count: u32,
  pub(crate) cross_axis_offset: LayoutUnit,
}

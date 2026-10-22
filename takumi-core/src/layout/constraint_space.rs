//! Blink's [`ConstraintSpace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/constraint_space.h):
//! the constraints a box lays out under, without writing modes, columns, or line clamp relayout.

use std::rc::Rc;

use bitflags::bitflags;

#[cfg(feature = "fragmentation")]
use crate::layout::break_appeal::BreakAppeal;
use crate::{
  layout::{
    bfc_rect::BfcOffset, exclusions::exclusion_space::ExclusionSpace,
    line_clamp_data::LineClampData, logical_rect::AvailableLogicalSize, margin_strut::MarginStrut,
    table::TablePartSpace,
  },
  layout_unit::LayoutUnit,
  style::Direction,
};

/// Blink's `AutoSizeBehavior`: what an `auto` size resolves to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum AutoSizeBehavior {
  /// Shrink to fit the content.
  #[default]
  FitContent,
  /// Stretch to the available size, unless an aspect-ratio applies.
  StretchImplicit,
  /// Stretch to the available size.
  StretchExplicit,
}

/// Blink's `LayoutResultCacheSlot`: a layout that places the boxes it lays out, or one that only
/// measures. A measure lays out its children to measure too, so it never moves them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CacheSlot {
  #[default]
  Layout,
  Measure,
}

/// Blink's `BaselineAlgorithmType`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BaselineAlgorithmType {
  #[default]
  Default,
  /// An inline-block's baselines: the last line's, or the block-end margin edge.
  InlineBlock,
}

bitflags! {
  /// Blink's `AdjoiningObjectTypes`: what precedes a box in its block formatting context while
  /// its block offset is unknown.
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
  pub(crate) struct AdjoiningObjectTypes: u8 {
    const FLOAT_LEFT = 0b001;
    const FLOAT_RIGHT = 0b010;
    const FLOAT_BOTH = 0b011;
    const INLINE_OUT_OF_FLOW = 0b100;
  }
}

/// Blink's `ConstraintSpace`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConstraintSpace {
  /// Blink's `AvailableSize().inline_size`, none while computing intrinsic sizes.
  pub(crate) available_inline_size: Option<LayoutUnit>,
  /// Blink's `AvailableSize().block_size`, none where indefinite.
  pub(crate) available_block_size: Option<LayoutUnit>,
  /// Blink's `GetBfcOffset`: the border box's offset in its block formatting context.
  pub(crate) bfc_offset: BfcOffset,
  /// Blink's `Direction`: the box's own, which its content lays out in.
  pub(crate) direction: Direction,
  /// Blink's `PercentageResolutionSize`, none where indefinite.
  pub(crate) percentage_resolution_inline_size: Option<LayoutUnit>,
  pub(crate) percentage_resolution_block_size: Option<LayoutUnit>,
  /// Blink's `GetLineClampData`.
  pub(crate) line_clamp_data: LineClampData,
  /// Blink's `IsLineClampClippedFloat`: a float a line-clamp container clips.
  pub(crate) is_line_clamp_clipped_float: bool,
  pub(crate) is_fixed_inline_size: bool,
  pub(crate) is_fixed_block_size: bool,
  pub(crate) inline_auto_behavior: AutoSizeBehavior,
  pub(crate) block_auto_behavior: AutoSizeBehavior,
  pub(crate) is_initial_block_size_indefinite: bool,
  pub(crate) is_new_formatting_context: bool,
  /// Blink's `IsTableCell`.
  pub(crate) is_table_cell: bool,
  /// The floats placed so far in the block formatting context.
  pub(crate) exclusion_space: Rc<ExclusionSpace>,
  /// The margins adjoining the box's block start.
  pub(crate) margin_strut: MarginStrut,
  /// Blink's `ClearanceOffset`, `LayoutUnit::MIN` for none.
  pub(crate) clearance_offset: LayoutUnit,
  pub(crate) forced_bfc_block_offset: Option<LayoutUnit>,
  pub(crate) optimistic_bfc_block_offset: Option<LayoutUnit>,
  pub(crate) adjoining_object_types: AdjoiningObjectTypes,
  pub(crate) ancestor_has_clearance_past_adjoining_floats: bool,
  pub(crate) is_pushed_by_floats: bool,
  pub(crate) baseline_algorithm: BaselineAlgorithmType,
  pub(crate) cache_slot: CacheSlot,
  /// Blink's `IgnoreMarginsForStretch` on the block start and end.
  pub(crate) ignore_block_margins_for_stretch: (bool, bool),
  /// The table section or row the space lays out, with the grid its table computed.
  pub(crate) table_part: Option<TablePartSpace>,
  /// Blink's block fragmentation fields, none outside a fragmentation context.
  #[cfg(feature = "fragmentation")]
  pub(crate) block_fragmentation: Option<BlockFragmentation>,
  /// Blink's `ShouldRepeat`: a repeated table header or footer repeats again on the next page.
  #[cfg(feature = "fragmentation")]
  pub(crate) should_repeat: bool,
}

/// The fields Blink's `ConstraintSpace` keeps for block fragmentation, for pages alone: a space
/// that has them is Blink's `kFragmentPage`.
#[cfg(feature = "fragmentation")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BlockFragmentation {
  /// Blink's `FragmentainerBlockSize`.
  pub(crate) fragmentainer_block_size: LayoutUnit,
  /// Blink's `FragmentainerOffset`: the border box's block offset from the page's block start.
  pub(crate) fragmentainer_offset: LayoutUnit,
  /// Blink's `RequiresContentBeforeBreaking`.
  pub(crate) requires_content_before_breaking: bool,
  /// Blink's `ShouldPropagateChildBreakValues`.
  pub(crate) should_propagate_child_break_values: bool,
  /// Blink's `MinBreakAppeal`.
  pub(crate) min_break_appeal: BreakAppeal,
  /// Blink's `MinBlockSizeShouldEncompassIntrinsicSize`: a fixed block size grows to the content
  /// a break pushed down.
  pub(crate) min_block_size_should_encompass_intrinsic_size: bool,
}

impl ConstraintSpace {
  /// A space for a container of `direction`, its percentages against an indefinite block size
  /// until the caller knows one.
  pub(crate) fn new(available_inline_size: Option<LayoutUnit>, direction: Direction) -> Self {
    Self {
      available_inline_size,
      available_block_size: None,
      bfc_offset: BfcOffset::default(),
      direction,
      percentage_resolution_inline_size: available_inline_size,
      percentage_resolution_block_size: None,
      line_clamp_data: LineClampData::default(),
      is_line_clamp_clipped_float: false,
      is_fixed_inline_size: false,
      is_fixed_block_size: false,
      inline_auto_behavior: AutoSizeBehavior::FitContent,
      block_auto_behavior: AutoSizeBehavior::FitContent,
      is_initial_block_size_indefinite: false,
      is_new_formatting_context: false,
      is_table_cell: false,
      exclusion_space: Rc::default(),
      margin_strut: MarginStrut::default(),
      clearance_offset: LayoutUnit::MIN,
      forced_bfc_block_offset: None,
      optimistic_bfc_block_offset: None,
      adjoining_object_types: AdjoiningObjectTypes::empty(),
      ancestor_has_clearance_past_adjoining_floats: false,
      is_pushed_by_floats: false,
      baseline_algorithm: BaselineAlgorithmType::Default,
      cache_slot: CacheSlot::Layout,
      ignore_block_margins_for_stretch: (false, false),
      table_part: None,
      #[cfg(feature = "fragmentation")]
      block_fragmentation: None,
      #[cfg(feature = "fragmentation")]
      should_repeat: false,
    }
  }

  /// Blink's `CloneWithoutFragmentation`: the space with no pages to break across.
  pub(crate) fn without_fragmentation(&self) -> Self {
    Self {
      #[cfg(feature = "fragmentation")]
      block_fragmentation: None,
      ..self.clone()
    }
  }

  /// Blink's `HasBlockFragmentation`, which for pages is `HasKnownFragmentainerBlockSize` too.
  pub(crate) fn has_block_fragmentation(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    {
      self.block_fragmentation.is_some()
    }
    #[cfg(not(feature = "fragmentation"))]
    {
      false
    }
  }

  /// Blink's `MinBlockSizeShouldEncompassIntrinsicSize`.
  pub(crate) fn min_block_size_should_encompass_intrinsic_size(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    {
      self
        .block_fragmentation
        .is_some_and(|fragmentation| fragmentation.min_block_size_should_encompass_intrinsic_size)
    }
    #[cfg(not(feature = "fragmentation"))]
    {
      false
    }
  }

  /// Whether a box laid out in `self` lays out as in `other`, which may only measure it.
  pub(crate) fn lays_out_like(&self, other: &Self) -> bool {
    *self
      == Self {
        cache_slot: self.cache_slot,
        ..other.clone()
      }
  }

  /// Blink's `AvailableSize` and `PercentageResolutionSize`.
  pub(crate) fn available_and_percentage_sizes(
    &self,
  ) -> (AvailableLogicalSize, AvailableLogicalSize) {
    (
      AvailableLogicalSize {
        inline_size: self.available_inline_size,
        block_size: self.available_block_size,
      },
      AvailableLogicalSize {
        inline_size: self.percentage_resolution_inline_size,
        block_size: self.percentage_resolution_block_size,
      },
    )
  }

  /// The available inline size, zero while computing intrinsic sizes.
  pub(crate) fn inline_size(&self) -> LayoutUnit {
    self.available_inline_size.unwrap_or(LayoutUnit::ZERO)
  }

  /// Blink's `HasClearanceOffset`.
  pub(crate) fn has_clearance_offset(&self) -> bool {
    self.clearance_offset != LayoutUnit::MIN
  }

  /// Blink's `ExpectedBfcBlockOffset`: where the box expects its block offset to resolve.
  pub(crate) fn expected_bfc_block_offset(&self) -> LayoutUnit {
    self
      .forced_bfc_block_offset
      .or(self.optimistic_bfc_block_offset)
      .unwrap_or(self.bfc_offset.block_offset)
  }

  pub(crate) fn is_inline_auto_behavior_stretch(&self) -> bool {
    self.inline_auto_behavior != AutoSizeBehavior::FitContent
  }

  pub(crate) fn is_block_auto_behavior_stretch(&self) -> bool {
    self.block_auto_behavior != AutoSizeBehavior::FitContent
  }
}

#[cfg(feature = "fragmentation")]
impl ConstraintSpace {
  /// Blink's `RequiresContentBeforeBreaking`.
  pub(crate) fn requires_content_before_breaking(&self) -> bool {
    self
      .block_fragmentation
      .is_some_and(|fragmentation| fragmentation.requires_content_before_breaking)
  }

  /// Blink's `MinBreakAppeal`.
  pub(crate) fn min_break_appeal(&self) -> BreakAppeal {
    self
      .block_fragmentation
      .map_or(BreakAppeal::LastResort, |fragmentation| {
        fragmentation.min_break_appeal
      })
  }

  /// Blink's `ShouldPropagateChildBreakValues`.
  pub(crate) fn should_propagate_child_break_values(&self) -> bool {
    self
      .block_fragmentation
      .is_some_and(|fragmentation| fragmentation.should_propagate_child_break_values)
  }
}

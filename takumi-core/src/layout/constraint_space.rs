//! Blink's [`ConstraintSpace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/constraint_space.h):
//! the constraints a box lays out under, without writing modes, fragmentation, or line clamp
//! relayout.

use std::rc::Rc;

use bitflags::bitflags;

use crate::{
  layout::{
    bfc_rect::BfcOffset, exclusions::exclusion_space::ExclusionSpace, margin_strut::MarginStrut,
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
  /// Blink's `GetLineClampData().LinesUntilClamp()`, for a box clamped by lines.
  pub(crate) lines_until_clamp: Option<i32>,
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
  /// taffy's `SizingMode::ContentSize`: the box sized by its content, its own sizes left out, for
  /// taffy's flexbox and grid algorithms.
  pub(crate) sizes_by_content: bool,
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
      lines_until_clamp: None,
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
      sizes_by_content: false,
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

  /// Blink's `LineClampData::IsAtClampPoint`: the line the clamp ellipsizes.
  pub(crate) fn is_at_clamp_point(&self) -> bool {
    self.lines_until_clamp == Some(1)
  }

  /// Blink's `IsPastClampPoint`.
  pub(crate) fn is_past_clamp_point(&self) -> bool {
    self.lines_until_clamp.is_some_and(|lines| lines <= 0)
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

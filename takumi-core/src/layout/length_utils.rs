//! Blink's [`length_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/length_utils.cc),
//! in a horizontal writing mode, without `calc-size()`, quirks mode, scrollbars, or block
//! fragmentation. An aspect ratio transfers sizes in floating point where Blink multiplies and
//! divides layout units, which can differ by a layout unit.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::cell::Cell;

use crate::{
  layout::{
    constraint_space::{AutoSizeBehavior, ConstraintSpace},
    layout_style::{LayoutLength, LayoutStyle},
    logical_rect::{AvailableLogicalSize, LogicalBoxStrut, LogicalSize},
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
  },
  layout_unit::LayoutUnit,
  style::{BoxSizing, Direction, Overflow},
};

/// Blink's `LengthTypeInternal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LengthType {
  Min,
  Main,
  Max,
}

/// Blink's `FitContentMode`: what `fit-content` resolves to in an indefinite available size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FitContentMode {
  /// min-content for a min size, max-content for a max size.
  #[default]
  Normal,
  MinContribution,
  MaxContribution,
}

/// A box's min-content and max-content sizes, computed on demand.
pub(crate) type MinMaxSizesFn<'f> = dyn FnMut(SizeType) -> MinMaxSizesResult + 'f;

/// A box's intrinsic block size, computed on demand, `None` where indefinite.
pub(crate) type BlockSizeFn<'f> = dyn FnMut(SizeType) -> Option<LayoutUnit> + 'f;

/// The replaced content a box lays out, for Blink's `ComputeReplacedSize`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NaturalSize {
  /// The natural content-box width, if it has one.
  pub(crate) inline_size: Option<LayoutUnit>,
  /// The natural content-box height, if it has one.
  pub(crate) block_size: Option<LayoutUnit>,
  /// The aspect ratio as width over height, `aspect-ratio` or the natural one.
  pub(crate) aspect_ratio: Option<f32>,
}

/// The default object size of replaced content with no natural size and no aspect ratio.
/// <https://www.w3.org/TR/CSS22/visudet.html#inline-replaced-width>
const DEFAULT_NATURAL_SIZE: LogicalSize = LogicalSize {
  inline_size: LayoutUnit::from_int(300),
  block_size: LayoutUnit::from_int(150),
};

/// Blink's `InlineSizeFromAspectRatio`.
pub(crate) fn inline_size_from_aspect_ratio(
  border_padding: LogicalBoxStrut,
  aspect_ratio: f32,
  box_sizing: BoxSizing,
  block_size: LayoutUnit,
) -> LayoutUnit {
  if box_sizing == BoxSizing::BorderBox {
    return border_padding
      .inline_sum()
      .max(LayoutUnit::from_float(block_size.to_float() * aspect_ratio));
  }

  let content_block_size = block_size - border_padding.block_sum();

  LayoutUnit::from_float(content_block_size.to_float() * aspect_ratio) + border_padding.inline_sum()
}

/// Blink's `BlockSizeFromAspectRatio`.
pub(crate) fn block_size_from_aspect_ratio(
  border_padding: LogicalBoxStrut,
  aspect_ratio: f32,
  box_sizing: BoxSizing,
  inline_size: LayoutUnit,
) -> LayoutUnit {
  if box_sizing == BoxSizing::BorderBox {
    return border_padding.block_sum().max(LayoutUnit::from_float(
      inline_size.to_float() / aspect_ratio,
    ));
  }

  let content_inline_size = inline_size - border_padding.inline_sum();

  LayoutUnit::from_float(content_inline_size.to_float() / aspect_ratio) + border_padding.block_sum()
}

/// Blink's `ComputeTransferredMinMaxInlineSizes`.
pub(crate) fn transferred_min_max_inline_sizes(
  aspect_ratio: f32,
  block_min_max: MinMaxSizes,
  border_padding: LogicalBoxStrut,
  box_sizing: BoxSizing,
) -> MinMaxSizes {
  let mut transferred = MinMaxSizes::UNCONSTRAINED;

  if block_min_max.min_size > LayoutUnit::ZERO {
    transferred.min_size = inline_size_from_aspect_ratio(
      border_padding,
      aspect_ratio,
      box_sizing,
      block_min_max.min_size,
    );
  }
  if block_min_max.max_size != LayoutUnit::MAX {
    transferred.max_size = inline_size_from_aspect_ratio(
      border_padding,
      aspect_ratio,
      box_sizing,
      block_min_max.max_size,
    );
  }
  // Minimum size wins over maximum size.
  transferred.max_size = transferred.max_size.max(transferred.min_size);
  transferred
}

/// Blink's `ResolveInlineAutoMargins`: `margins` with its auto inline sides taking the space a box
/// `inline_size` wide leaves.
pub(crate) fn resolve_inline_auto_margins(
  style: &LayoutStyle,
  container_direction: Direction,
  available_inline_size: LayoutUnit,
  inline_size: LayoutUnit,
  margins: &mut LogicalBoxStrut,
) {
  let used_space = inline_size + margins.inline_sum();
  let available_space = available_inline_size - used_space;
  let (start, end) = container_direction.inline_sides(style.margin.left, style.margin.right);

  match (start.is_auto(), end.is_auto()) {
    (true, true) => {
      margins.inline_start = (available_space / 2).clamp_negative_to_zero();
      margins.inline_end = available_inline_size - inline_size - margins.inline_start;
    }
    (true, false) => margins.inline_start = available_space.clamp_negative_to_zero(),
    (false, true) => {
      margins.inline_end = available_inline_size - inline_size - margins.inline_start
    }
    (false, false) => {}
  }
}

/// Blink's `ComputePhysicalMargins` converted to `direction`: `style`'s margins, percentages against
/// `percentage_inline_size`, `auto` as zero.
pub(crate) fn margins_for(
  style: &LayoutStyle,
  percentage_inline_size: Option<LayoutUnit>,
  direction: Direction,
) -> LogicalBoxStrut {
  LogicalBoxStrut::from_physical(
    style
      .margin
      .map(|margin| margin.resolve_or_zero(percentage_inline_size)),
    direction,
  )
}

/// Blink's `ComputeBorders`: the used border widths in the box's own direction.
pub(crate) fn borders(style: &LayoutStyle) -> LogicalBoxStrut {
  LogicalBoxStrut::from_physical(style.border.map(LayoutUnit::from_float), style.direction)
}

/// Blink's `ComputePadding`: percentages against the space's inline percentage size, zero where
/// indefinite.
pub(crate) fn padding(space: &ConstraintSpace, style: &LayoutStyle) -> LogicalBoxStrut {
  LogicalBoxStrut::from_physical(
    style
      .padding
      .map(|padding| padding.resolve_or_zero(space.percentage_resolution_inline_size)),
    style.direction,
  )
}

/// A box's style and border box resolved under a constraint space, which resolve its sizes: the
/// arguments every Blink `length_utils` function shares.
pub(crate) struct LengthResolver<'a> {
  pub(crate) space: &'a ConstraintSpace,
  pub(crate) style: &'a LayoutStyle,
  pub(crate) border_padding: LogicalBoxStrut,
}

impl<'a> LengthResolver<'a> {
  /// The resolver for `style` under `space`, its border and padding computed.
  pub(crate) fn new(space: &'a ConstraintSpace, style: &'a LayoutStyle) -> Self {
    Self {
      space,
      style,
      border_padding: borders(style) + padding(space, style),
    }
  }

  /// Blink's `ComputeMarginsForSelf`.
  pub(crate) fn margins_for_self(&self) -> LogicalBoxStrut {
    margins_for(
      self.style,
      self.space.percentage_resolution_inline_size,
      self.style.direction,
    )
  }

  /// Blink's `ResolveInlineLengthInternal`.
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn resolve_inline_length(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
    original_length: LayoutLength,
    auto_length: Option<LayoutLength>,
    length_type: LengthType,
    fit_content_mode: FitContentMode,
    override_available_size: Option<LayoutUnit>,
  ) -> Option<LayoutUnit> {
    let length = match (original_length, auto_length) {
      (LayoutLength::Auto, Some(auto_length)) => auto_length,
      (length, _) => length,
    };
    let border_padding = self.border_padding.inline_sum();
    let available_size = override_available_size.or(self.space.available_inline_size);

    match length {
      LayoutLength::Stretch => {
        let available_size = available_size?;
        let margins = self.margins_for_self();

        Some(border_padding.max(available_size - margins.inline_sum()))
      }
      LayoutLength::Fixed(_) | LayoutLength::Percent(_) | LayoutLength::Calc { .. } => {
        let mut percentage_resolution_size = self.space.percentage_resolution_inline_size;

        if length.has_percent() && percentage_resolution_size.is_none() {
          if length_type != LengthType::Min {
            return None;
          }
          percentage_resolution_size = Some(LayoutUnit::ZERO);
        }

        let value = length.resolve(percentage_resolution_size)?;

        Some(match self.style.box_sizing {
          BoxSizing::BorderBox => border_padding.max(value),
          BoxSizing::ContentBox => value + border_padding,
        })
      }
      LayoutLength::MaxContent => Some(min_max_sizes(SizeType::Content).sizes.max_size),
      LayoutLength::MinContent => Some(min_max_sizes(SizeType::Content).sizes.min_size),
      LayoutLength::MinIntrinsic => Some(min_max_sizes(SizeType::Intrinsic).sizes.min_size),
      LayoutLength::FitContent => {
        let Some(available_size) = available_size else {
          return match (fit_content_mode, length_type) {
            (FitContentMode::Normal, LengthType::Min) | (FitContentMode::MinContribution, _) => {
              Some(min_max_sizes(SizeType::Content).sizes.min_size)
            }
            (FitContentMode::Normal, LengthType::Main) => None,
            (FitContentMode::Normal, LengthType::Max) | (FitContentMode::MaxContribution, _) => {
              Some(min_max_sizes(SizeType::Content).sizes.max_size)
            }
          };
        };
        let margins = self.margins_for_self();

        Some(
          min_max_sizes(SizeType::Content)
            .sizes
            .shrink_to_fit((available_size - margins.inline_sum()).clamp_negative_to_zero()),
        )
      }
      LayoutLength::Auto if length_type == LengthType::Min => Some(border_padding),
      LayoutLength::Auto => None,
    }
  }

  /// Blink's `ResolveMinInlineLength`.
  pub(crate) fn resolve_min_inline_length(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
    length: LayoutLength,
    auto_length: Option<LayoutLength>,
    override_available_size: Option<LayoutUnit>,
    fit_content_mode: FitContentMode,
  ) -> LayoutUnit {
    self
      .resolve_inline_length(
        min_max_sizes,
        length,
        auto_length,
        LengthType::Min,
        fit_content_mode,
        override_available_size,
      )
      .unwrap_or(self.border_padding.inline_sum())
  }

  /// Blink's `ResolveMaxInlineLength`.
  pub(crate) fn resolve_max_inline_length(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
    length: LayoutLength,
    override_available_size: Option<LayoutUnit>,
    fit_content_mode: FitContentMode,
  ) -> LayoutUnit {
    self
      .resolve_inline_length(
        min_max_sizes,
        length,
        None,
        LengthType::Max,
        fit_content_mode,
        override_available_size,
      )
      .unwrap_or(LayoutUnit::MAX)
  }

  /// Blink's `ResolveMainInlineLength`.
  pub(crate) fn resolve_main_inline_length(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
    length: LayoutLength,
    auto_length: Option<LayoutLength>,
    override_available_size: Option<LayoutUnit>,
  ) -> Option<LayoutUnit> {
    self.resolve_inline_length(
      min_max_sizes,
      length,
      auto_length,
      LengthType::Main,
      FitContentMode::Normal,
      override_available_size,
    )
  }

  /// Blink's `ResolveBlockLengthInternal`.
  pub(crate) fn resolve_block_length(
    &self,
    original_length: LayoutLength,
    auto_length: Option<LayoutLength>,
    length_type: LengthType,
    override_available_size: Option<LayoutUnit>,
    override_percentage_resolution_size: Option<Option<LayoutUnit>>,
    block_size: &mut BlockSizeFn<'_>,
  ) -> Option<LayoutUnit> {
    let length = match (original_length, auto_length) {
      (LayoutLength::Auto, Some(auto_length)) => auto_length,
      (length, _) => length,
    };
    let border_padding = self.border_padding.block_sum();

    match length {
      LayoutLength::Stretch => {
        let Some(available_size) = override_available_size.or(self.space.available_block_size)
        else {
          return match length_type {
            LengthType::Main => block_size(SizeType::Content),
            LengthType::Min | LengthType::Max => None,
          };
        };
        let margins = self.margins_for_self();
        let (ignore_start, ignore_end) = self.space.ignore_block_margins_for_stretch;
        let start = if ignore_start {
          LayoutUnit::ZERO
        } else {
          margins.block_start
        };
        let end = if ignore_end {
          LayoutUnit::ZERO
        } else {
          margins.block_end
        };

        Some(border_padding.max(available_size - start - end))
      }
      LayoutLength::Fixed(_) | LayoutLength::Percent(_) | LayoutLength::Calc { .. } => {
        let mut percentage_resolution_size = override_percentage_resolution_size
          .unwrap_or(self.space.percentage_resolution_block_size);

        if length.has_percent() && percentage_resolution_size.is_none() {
          match length_type {
            LengthType::Min => percentage_resolution_size = Some(LayoutUnit::ZERO),
            LengthType::Main => return block_size(SizeType::Content),
            LengthType::Max => return None,
          }
        }

        let value = length.resolve(percentage_resolution_size)?;

        Some(match self.style.box_sizing {
          BoxSizing::BorderBox => border_padding.max(value),
          BoxSizing::ContentBox => value + border_padding,
        })
      }
      LayoutLength::MinContent | LayoutLength::MaxContent | LayoutLength::FitContent => {
        block_size(SizeType::Content)
      }
      LayoutLength::MinIntrinsic => block_size(SizeType::Intrinsic),
      LayoutLength::Auto if length_type == LengthType::Min => Some(border_padding),
      LayoutLength::Auto => None,
    }
  }

  /// Blink's `ResolveInitialMinBlockLength`.
  pub(crate) fn resolve_initial_min_block_length(
    &self,
    length: LayoutLength,
    override_available_size: Option<LayoutUnit>,
  ) -> LayoutUnit {
    self
      .resolve_block_length(
        length,
        Some(LayoutLength::Auto),
        LengthType::Min,
        override_available_size,
        None,
        &mut |_| None,
      )
      .unwrap_or(self.border_padding.block_sum())
  }

  /// Blink's `ResolveMinBlockLength`.
  pub(crate) fn resolve_min_block_length(
    &self,
    block_size: &mut BlockSizeFn<'_>,
    length: LayoutLength,
    auto_length: Option<LayoutLength>,
    override_available_size: Option<LayoutUnit>,
    override_percentage_resolution_size: Option<Option<LayoutUnit>>,
  ) -> LayoutUnit {
    self
      .resolve_block_length(
        length,
        auto_length,
        LengthType::Min,
        override_available_size,
        override_percentage_resolution_size,
        block_size,
      )
      .unwrap_or(self.border_padding.block_sum())
  }

  /// Blink's `ResolveInitialMaxBlockLength`.
  pub(crate) fn resolve_initial_max_block_length(
    &self,
    length: LayoutLength,
    override_available_size: Option<LayoutUnit>,
  ) -> LayoutUnit {
    self
      .resolve_block_length(
        length,
        Some(LayoutLength::Auto),
        LengthType::Max,
        override_available_size,
        None,
        &mut |_| None,
      )
      .unwrap_or(LayoutUnit::MAX)
  }

  /// Blink's `ResolveMaxBlockLength`.
  pub(crate) fn resolve_max_block_length(
    &self,
    length: LayoutLength,
    block_size: &mut BlockSizeFn<'_>,
    override_available_size: Option<LayoutUnit>,
    override_percentage_resolution_size: Option<Option<LayoutUnit>>,
  ) -> LayoutUnit {
    self
      .resolve_block_length(
        length,
        Some(LayoutLength::Auto),
        LengthType::Max,
        override_available_size,
        override_percentage_resolution_size,
        block_size,
      )
      .unwrap_or(LayoutUnit::MAX)
  }

  /// Blink's `ResolveMainBlockLength`.
  pub(crate) fn resolve_main_block_length(
    &self,
    length: LayoutLength,
    auto_length: Option<LayoutLength>,
    block_size: &mut BlockSizeFn<'_>,
    override_available_size: Option<LayoutUnit>,
  ) -> Option<LayoutUnit> {
    self.resolve_block_length(
      length,
      auto_length,
      LengthType::Main,
      override_available_size,
      None,
      block_size,
    )
  }

  /// Blink's `ComputeInitialMinMaxBlockSizes`.
  pub(crate) fn initial_min_max_block_sizes(
    &self,
    override_available_size: Option<LayoutUnit>,
  ) -> MinMaxSizes {
    let mut sizes = MinMaxSizes {
      min_size: self
        .resolve_initial_min_block_length(self.style.min_size.height, override_available_size),
      max_size: self
        .resolve_initial_max_block_length(self.style.max_size.height, override_available_size),
    };

    sizes.max_size = sizes.max_size.max(sizes.min_size);
    sizes
  }

  /// Blink's `ComputeMinMaxBlockSizes`.
  pub(crate) fn min_max_block_sizes(
    &self,
    auto_min_length: Option<LayoutLength>,
    block_size: &mut BlockSizeFn<'_>,
    override_available_size: Option<LayoutUnit>,
  ) -> MinMaxSizes {
    let mut sizes = MinMaxSizes {
      min_size: self.resolve_min_block_length(
        block_size,
        self.style.min_size.height,
        auto_min_length,
        override_available_size,
        None,
      ),
      max_size: self.resolve_max_block_length(
        self.style.max_size.height,
        block_size,
        override_available_size,
        None,
      ),
    };

    // Clamp the auto min-size by the max-size.
    if auto_min_length.is_some() && self.style.min_size.height.is_auto() {
      sizes.min_size = sizes.min_size.min(sizes.max_size);
    }

    sizes.max_size = sizes.max_size.max(sizes.min_size);
    sizes
  }

  /// Blink's `ComputeMinMaxInlineSizesFromAspectRatio`.
  pub(crate) fn min_max_inline_sizes_from_aspect_ratio(&self, aspect_ratio: f32) -> MinMaxSizes {
    transferred_min_max_inline_sizes(
      aspect_ratio,
      self.initial_min_max_block_sizes(None),
      self.border_padding,
      self.style.box_sizing_for_aspect_ratio(),
    )
  }

  /// Blink's `ComputeMinMaxInlineSizes`.
  pub(crate) fn min_max_inline_sizes(
    &self,
    auto_min_length: Option<LayoutLength>,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
    transfers_sizes: bool,
    fit_content_mode: FitContentMode,
    override_available_size: Option<LayoutUnit>,
  ) -> MinMaxSizes {
    let style = self.style;
    let mut sizes = MinMaxSizes {
      min_size: self.resolve_min_inline_length(
        min_max_sizes,
        style.min_size.width,
        auto_min_length,
        override_available_size,
        fit_content_mode,
      ),
      max_size: self.resolve_max_inline_length(
        min_max_sizes,
        style.max_size.width,
        override_available_size,
        fit_content_mode,
      ),
    };

    // Clamp the auto min-size by the max-size.
    if auto_min_length.is_some() && style.min_size.width.is_auto() {
      sizes.min_size = sizes.min_size.min(sizes.max_size);
    }

    // https://drafts.csswg.org/css-sizing-4/#aspect-ratio-size-transfers
    if transfers_sizes
      && let Some(aspect_ratio) = style.aspect_ratio
      && style.size.width.is_auto()
      && self.space.inline_auto_behavior != AutoSizeBehavior::StretchExplicit
    {
      let transferred = self.min_max_inline_sizes_from_aspect_ratio(aspect_ratio);

      sizes.min_size = sizes.min_size.max(transferred.min_size.min(sizes.max_size));
      sizes.max_size = sizes.max_size.min(transferred.max_size);
    }

    // Tables can't shrink below their min-intrinsic size.
    if style.display.is_table() {
      sizes.encompass(min_max_sizes(SizeType::Intrinsic).sizes.min_size);
    }

    sizes.max_size = sizes.max_size.max(sizes.min_size);
    sizes
  }

  /// Whether the box resolves its block size with no content, for its aspect ratio.
  fn resolves_block_size_without_content(&self) -> bool {
    self.block_size_for_fragment(None, None, None).is_some()
  }

  /// Blink's `ComputeInlineSizeForFragmentInternal`.
  pub(crate) fn inline_size_for_fragment_ignoring_fixed(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
  ) -> LayoutUnit {
    let style = self.style;
    let width = style.size.width;
    let may_apply_aspect_ratio = style.aspect_ratio.is_some()
      && !(style.size.height.is_auto()
        && self.space.block_auto_behavior != AutoSizeBehavior::StretchExplicit)
      && self.resolves_block_size_without_content();
    let auto_length = if self.space.available_inline_size.is_none() {
      LayoutLength::MinContent
    } else if self.space.inline_auto_behavior == AutoSizeBehavior::StretchExplicit {
      LayoutLength::Stretch
    } else if may_apply_aspect_ratio {
      LayoutLength::FitContent
    } else if self.space.inline_auto_behavior == AutoSizeBehavior::StretchImplicit {
      LayoutLength::Stretch
    } else {
      LayoutLength::FitContent
    };
    // https://drafts.csswg.org/css-sizing-4/#aspect-ratio-minimum
    let applies_automatic_min_size = style.overflow.x == Overflow::Visible
      && may_apply_aspect_ratio
      && (width.is_content_or_intrinsic()
        || (width.is_auto() && auto_length.is_content_or_intrinsic()));
    let extent = self.resolve_main_inline_length(min_max_sizes, width, Some(auto_length), None);
    let min_max = self.min_max_inline_sizes(
      applies_automatic_min_size.then_some(LayoutLength::MinIntrinsic),
      min_max_sizes,
      true,
      FitContentMode::Normal,
      None,
    );

    min_max.clamp_size_to_min_and_max(extent.unwrap_or(LayoutUnit::ZERO))
  }

  /// Blink's `ComputeInlineSizeForFragment`, for a box that is not a table.
  pub(crate) fn inline_size_for_fragment(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
  ) -> LayoutUnit {
    if self.space.is_fixed_inline_size {
      return self.space.inline_size();
    }

    self.inline_size_for_fragment_ignoring_fixed(min_max_sizes)
  }

  /// Blink's `ComputeBlockSizeForFragmentInternal`.
  fn block_size_for_fragment_ignoring_fixed(
    &self,
    intrinsic_size: Option<LayoutUnit>,
    inline_size: Option<LayoutUnit>,
    override_available_size: Option<LayoutUnit>,
  ) -> Option<LayoutUnit> {
    let style = self.style;
    let height = style.size.height;
    let may_apply_aspect_ratio = style.aspect_ratio.is_some() && inline_size.is_some();
    let auto_length = if self.space.available_block_size.is_none() {
      LayoutLength::FitContent
    } else if self.space.block_auto_behavior == AutoSizeBehavior::StretchExplicit {
      LayoutLength::Stretch
    } else if may_apply_aspect_ratio {
      LayoutLength::FitContent
    } else if self.space.block_auto_behavior == AutoSizeBehavior::StretchImplicit {
      LayoutLength::Stretch
    } else {
      LayoutLength::FitContent
    };
    // https://drafts.csswg.org/css-sizing-4/#aspect-ratio-minimum
    let applies_automatic_min_size = intrinsic_size.is_some()
      && style.overflow.y == Overflow::Visible
      && may_apply_aspect_ratio
      && (height.is_content_or_intrinsic()
        || (height.is_auto() && auto_length.is_content_or_intrinsic()));
    let mut block_size = |size_type: SizeType| match (size_type, style.aspect_ratio, inline_size) {
      (SizeType::Content, Some(aspect_ratio), Some(inline_size)) => {
        Some(block_size_from_aspect_ratio(
          self.border_padding,
          aspect_ratio,
          style.box_sizing_for_aspect_ratio(),
          inline_size,
        ))
      }
      _ => intrinsic_size,
    };
    let extent = self.resolve_main_block_length(
      height,
      Some(auto_length),
      &mut block_size,
      override_available_size,
    )?;
    let min_max = self.min_max_block_sizes(
      applies_automatic_min_size.then_some(LayoutLength::MinIntrinsic),
      &mut block_size,
      override_available_size,
    );

    Some(min_max.clamp_size_to_min_and_max(extent))
  }

  /// Blink's `ComputeBlockSizeForFragment`: `None` where it depends on an indefinite
  /// `intrinsic_size`.
  pub(crate) fn block_size_for_fragment(
    &self,
    intrinsic_size: Option<LayoutUnit>,
    inline_size: Option<LayoutUnit>,
    override_available_size: Option<LayoutUnit>,
  ) -> Option<LayoutUnit> {
    if self.space.is_fixed_block_size {
      return override_available_size.or(self.space.available_block_size);
    }
    if self.space.is_table_cell && intrinsic_size.is_some() {
      return intrinsic_size;
    }

    self.block_size_for_fragment_ignoring_fixed(
      intrinsic_size,
      inline_size,
      override_available_size,
    )
  }

  /// Blink's `ComputeInitialBlockSizeForFragment`.
  pub(crate) fn initial_block_size_for_fragment(
    &self,
    intrinsic_size: Option<LayoutUnit>,
    inline_size: Option<LayoutUnit>,
  ) -> Option<LayoutUnit> {
    if self.space.is_initial_block_size_indefinite {
      return intrinsic_size;
    }

    self.block_size_for_fragment(intrinsic_size, inline_size, None)
  }

  /// Blink's `ComputeNormalizedNaturalSize`: the border-box natural size of replaced content.
  fn normalized_natural_size(&self, natural: NaturalSize) -> Option<LogicalSize> {
    let border_padding = self.border_padding;
    let box_sizing = self.style.box_sizing_for_aspect_ratio();
    let mut inline_size = natural
      .inline_size
      .map(|size| size + border_padding.inline_sum())
      .or_else(|| {
        natural
          .aspect_ratio
          .is_none()
          .then(|| DEFAULT_NATURAL_SIZE.inline_size + border_padding.inline_sum())
      });
    let mut block_size = natural
      .block_size
      .map(|size| size + border_padding.block_sum())
      .or_else(|| {
        natural
          .aspect_ratio
          .is_none()
          .then(|| DEFAULT_NATURAL_SIZE.block_size + border_padding.block_sum())
      });

    // If we have one natural size reflect via. the aspect-ratio.
    if let (None, Some(block), Some(aspect_ratio)) = (inline_size, block_size, natural.aspect_ratio)
    {
      inline_size = Some(inline_size_from_aspect_ratio(
        border_padding,
        aspect_ratio,
        box_sizing,
        block,
      ));
    }
    // The natural block size always follows the aspect ratio when there is one.
    if let (Some(inline), Some(aspect_ratio)) = (inline_size, natural.aspect_ratio) {
      block_size = Some(block_size_from_aspect_ratio(
        border_padding,
        aspect_ratio,
        box_sizing,
        inline,
      ));
    }

    Some(LogicalSize::new(inline_size?, block_size?))
  }

  /// Blink's `ComputeReplacedSize`: the border box of replaced content with natural size
  /// `natural`, ignoring the inline or block lengths as `mode` says.
  pub(crate) fn replaced_size(&self, natural: NaturalSize, mode: ReplacedSizeMode) -> LogicalSize {
    let style = self.style;
    let border_padding = self.border_padding;
    let box_sizing = style.box_sizing_for_aspect_ratio();
    let aspect_ratio = natural.aspect_ratio;
    let natural_size = self.normalized_natural_size(natural);
    let mut block_size_fn = |_: SizeType| -> Option<LayoutUnit> {
      let Some(_) = aspect_ratio else {
        return natural_size.map(|size| size.block_size);
      };

      if mode == ReplacedSizeMode::Normal {
        return Some(
          self
            .replaced_size(natural, ReplacedSizeMode::IgnoreBlockLengths)
            .block_size,
        );
      }

      natural_size.map(|size| size.block_size)
    };

    let mut replaced_block = None;
    let block_min_max_sizes = if mode == ReplacedSizeMode::IgnoreBlockLengths {
      MinMaxSizes::UNCONSTRAINED
    } else {
      let mut sizes = MinMaxSizes {
        min_size: self.resolve_min_block_length(
          &mut block_size_fn,
          style.min_size.height,
          None,
          None,
          None,
        ),
        max_size: self.resolve_max_block_length(
          style.max_size.height,
          &mut block_size_fn,
          None,
          None,
        ),
      };

      sizes.max_size = sizes.min_size.max(sizes.max_size);

      if self.space.is_fixed_block_size {
        replaced_block = self.space.available_block_size;
      } else {
        let auto_block_length = if self.space.is_block_auto_behavior_stretch() {
          LayoutLength::Stretch
        } else {
          LayoutLength::FitContent
        };

        replaced_block = self
          .resolve_main_block_length(
            style.size.height,
            Some(auto_block_length),
            &mut block_size_fn,
            None,
          )
          .map(|block_size| sizes.clamp_size_to_min_and_max(block_size));
      }

      sizes
    };

    let transferred_min_max_sizes = match aspect_ratio {
      None => MinMaxSizes::UNCONSTRAINED,
      Some(aspect_ratio) => transferred_min_max_inline_sizes(
        aspect_ratio,
        block_min_max_sizes,
        border_padding,
        box_sizing,
      ),
    };

    let mut min_max_sizes_fn = |_: SizeType| -> MinMaxSizesResult {
      let size = match (aspect_ratio, replaced_block, natural_size) {
        (None, _, natural_size) => natural_size.map(|size| size.inline_size),
        (Some(aspect_ratio), Some(block), _) => Some(inline_size_from_aspect_ratio(
          border_padding,
          aspect_ratio,
          box_sizing,
          block,
        )),
        (Some(_), None, Some(natural_size)) => Some(if mode == ReplacedSizeMode::Normal {
          self
            .replaced_size(natural, ReplacedSizeMode::IgnoreInlineLengths)
            .inline_size
        } else {
          natural_size.inline_size
        }),
        (Some(_), None, None) => None,
      };

      // An unknown natural size resolves as indefinite, which Blink spells `kIndefiniteSize`.
      MinMaxSizesResult::new(
        MinMaxSizes::both(size.unwrap_or(LayoutUnit::from_int(-1))),
        false,
      )
    };

    let mut replaced_inline = None;
    let inline_min_max_sizes = if mode == ReplacedSizeMode::IgnoreInlineLengths {
      transferred_min_max_sizes
    } else {
      let mut sizes = MinMaxSizes {
        min_size: self.resolve_min_inline_length(
          &mut min_max_sizes_fn,
          style.min_size.width,
          None,
          None,
          FitContentMode::Normal,
        ),
        max_size: self.resolve_max_inline_length(
          &mut min_max_sizes_fn,
          style.max_size.width,
          None,
          FitContentMode::Normal,
        ),
      };

      sizes.max_size = sizes.min_size.max(sizes.max_size);

      if self.space.is_fixed_inline_size {
        replaced_inline = self.space.available_inline_size;
      } else {
        let auto_length = if self.space.is_inline_auto_behavior_stretch() {
          LayoutLength::Stretch
        } else {
          LayoutLength::FitContent
        };

        replaced_inline = self
          .resolve_main_inline_length(
            &mut min_max_sizes_fn,
            style.size.width,
            Some(auto_length),
            None,
          )
          .filter(|size| *size >= LayoutUnit::ZERO)
          .map(|inline_size| sizes.clamp_size_to_min_and_max(inline_size));
      }

      // https://drafts.csswg.org/css-sizing-4/#aspect-ratio-size-transfers
      if replaced_inline.is_none() {
        sizes.min_size = sizes
          .min_size
          .max(transferred_min_max_sizes.min_size.min(sizes.max_size));
        sizes.max_size = sizes.max_size.min(transferred_min_max_sizes.max_size);
      }

      sizes
    };

    if let (Some(inline), Some(block)) = (replaced_inline, replaced_block) {
      return LogicalSize::new(inline, block);
    }

    let stretch_fit = || -> LayoutUnit {
      if let Some(available) = self.space.available_inline_size {
        return border_padding
          .inline_sum()
          .max(available - self.margins_for_self().inline_sum());
      }
      // All browsers now use the default natural-size for a percentage.
      if style.size.width.has_percent() {
        return DEFAULT_NATURAL_SIZE.inline_size + border_padding.inline_sum();
      }

      border_padding.inline_sum()
    };

    // An aspect-ratio alone, with no sizes natural or otherwise, stretches.
    if natural_size.is_none() && replaced_inline.is_none() && replaced_block.is_none() {
      replaced_inline = Some(inline_min_max_sizes.clamp_size_to_min_and_max(stretch_fit()));
    }

    if let Some(inline) = replaced_inline {
      let block = match (aspect_ratio, natural_size) {
        (Some(aspect_ratio), _) => {
          block_size_from_aspect_ratio(border_padding, aspect_ratio, box_sizing, inline)
        }
        (None, Some(natural_size)) => natural_size.block_size,
        (None, None) => LayoutUnit::ZERO,
      };

      return LogicalSize::new(inline, block_min_max_sizes.clamp_size_to_min_and_max(block));
    }

    if let Some(block) = replaced_block {
      let inline = match (aspect_ratio, natural_size) {
        (Some(aspect_ratio), _) => {
          inline_size_from_aspect_ratio(border_padding, aspect_ratio, box_sizing, block)
        }
        (None, Some(natural_size)) => natural_size.inline_size,
        (None, None) => LayoutUnit::ZERO,
      };

      return LogicalSize::new(
        inline_min_max_sizes.clamp_size_to_min_and_max(inline),
        block,
      );
    }

    let natural_size = natural_size.unwrap_or(DEFAULT_NATURAL_SIZE);

    LogicalSize::new(
      inline_min_max_sizes.clamp_size_to_min_and_max(natural_size.inline_size),
      block_min_max_sizes.clamp_size_to_min_and_max(natural_size.block_size),
    )
  }

  /// Blink's `ComputeMinAndMaxContentContributionInternal`: the box's min-content and max-content
  /// contributions from its content sizes `min_max_sizes`, in an indefinite available inline size.
  pub(crate) fn min_and_max_content_contribution(
    &self,
    min_max_sizes: &mut MinMaxSizesFn<'_>,
  ) -> MinMaxSizesResult {
    let style = self.style;
    let depends_on_block_constraints = Cell::new(false);
    let applied_aspect_ratio = Cell::new(false);
    let mut sizes_fn = |size_type: SizeType| {
      let result = min_max_sizes(size_type);

      depends_on_block_constraints
        .set(depends_on_block_constraints.get() | result.depends_on_block_constraints);
      applied_aspect_ratio.set(applied_aspect_ratio.get() | result.applied_aspect_ratio);
      result
    };
    let extent = self.resolve_main_inline_length(
      &mut sizes_fn,
      style.size.width,
      Some(LayoutLength::FitContent),
      None,
    );
    let mut sizes = match extent {
      Some(extent) => MinMaxSizes::both(extent),
      None => sizes_fn(SizeType::Content).sizes,
    };
    // https://drafts.csswg.org/css-sizing-4/#aspect-ratio-minimum
    let auto_min_length = (style.overflow.x == Overflow::Visible && applied_aspect_ratio.get())
      .then_some(LayoutLength::MinIntrinsic);

    // A `fit-content` min or max size resolves once as min-content and once as max-content.
    // https://github.com/w3c/csswg-drafts/issues/10721
    if style.min_size.width == LayoutLength::FitContent
      || style.max_size.width == LayoutLength::FitContent
    {
      let mut clamp =
        |mode| self.min_max_inline_sizes(auto_min_length, &mut sizes_fn, true, mode, None);
      let min_sizes = clamp(FitContentMode::MinContribution);
      let max_sizes = clamp(FitContentMode::MaxContribution);

      sizes.min_size = min_sizes.clamp_size_to_min_and_max(sizes.min_size);
      sizes.max_size = max_sizes.clamp_size_to_min_and_max(sizes.max_size);
    } else {
      let min_max_sizes = self.min_max_inline_sizes(
        auto_min_length,
        &mut sizes_fn,
        true,
        FitContentMode::Normal,
        None,
      );

      sizes.constrain(min_max_sizes.max_size);
      sizes.encompass(min_max_sizes.min_size);
    }

    MinMaxSizesResult::new(sizes, depends_on_block_constraints.get())
  }

  /// Blink's `ComputeMinAndMaxContentContributionForReplaced`.
  pub(crate) fn min_and_max_content_contribution_for_replaced(
    &self,
    natural: NaturalSize,
  ) -> MinMaxSizesResult {
    let style = self.style;
    let mut sizes = MinMaxSizes::both(
      self
        .replaced_size(natural, ReplacedSizeMode::Normal)
        .inline_size,
    );

    if style.size.width.has_percent() || style.max_size.width.has_percent() {
      let border_padding = self.border_padding.inline_sum();

      sizes.min_size = self.resolve_min_inline_length(
        &mut |_| MinMaxSizesResult::new(MinMaxSizes::both(border_padding), false),
        style.min_size.width,
        None,
        None,
        FitContentMode::Normal,
      );
    }

    // Replaced elements which have a percentage block-size always depend on their block
    // constraints, as their aspect ratio changes their min and max content sizes.
    let depends_on_block_constraints = style.size.height.has_percent()
      || style.min_size.height.has_percent()
      || style.max_size.height.has_percent()
      || (style.size.height.is_auto() && self.space.is_block_auto_behavior_stretch());

    MinMaxSizesResult::new(sizes, depends_on_block_constraints)
  }
}

/// Blink's `ReplacedSizeMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplacedSizeMode {
  Normal,
  IgnoreInlineLengths,
  IgnoreBlockLengths,
}

/// Blink's `CalculateChildAvailableSize`.
pub(crate) fn child_available_size(
  border_box_size: AvailableLogicalSize,
  border_padding: LogicalBoxStrut,
) -> AvailableLogicalSize {
  border_box_size.shrink(border_padding)
}

/// Blink's `CalculateChildPercentageSize`: an anonymous block passes its parent's percentage block
/// size through.
pub(crate) fn child_percentage_size(
  space: &ConstraintSpace,
  is_anonymous: bool,
  child_available_size: AvailableLogicalSize,
) -> AvailableLogicalSize {
  if is_anonymous {
    return AvailableLogicalSize {
      inline_size: child_available_size.inline_size,
      block_size: space.percentage_resolution_block_size,
    };
  }

  child_available_size
}

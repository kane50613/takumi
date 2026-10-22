//! Blink's [`OutOfFlowLayoutPart`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/out_of_flow_layout_part.cc)
//! and [`absolute_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/absolute_utils.cc):
//! absolutely and fixed positioned boxes laid out in their containing block. Without anchor
//! positioning, `position-try`, fragmentation, or scroll containers, and `justify-self: auto` takes
//! no `justify-items`.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::take, rc::Rc};

use crate::{
  geometry::{NodeId, Point, Rect, Size},
  layout::{
    constraint_space::{AutoSizeBehavior, CacheSlot, ConstraintSpace},
    layout_result::{
      BlockEdge, InlineEdge, LayoutResult, LogicalAlignmentDirection, OutOfFlowCandidate,
    },
    layout_style::{LayoutLength, LayoutStyle},
    length_utils::{FitContentMode, LengthResolver, ReplacedSizeMode},
    logical_rect::LogicalSize,
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
  style::{AlignItems, Direction, Overflow, Position},
};

/// Which out-of-flow boxes a box is the containing block for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ContainingBlockClaims {
  pub(crate) absolute: bool,
  pub(crate) fixed: bool,
}

impl ContainingBlockClaims {
  /// The initial containing block's: every out-of-flow box.
  pub(crate) const ALL: Self = Self {
    absolute: true,
    fixed: true,
  };

  /// Whether a box positioned `position` takes this box as its containing block.
  pub(crate) fn claims(self, position: Position) -> bool {
    match position {
      Position::Absolute => self.absolute,
      Position::Fixed => self.fixed,
      Position::Static | Position::Relative => false,
    }
  }
}

/// Blink's `ContainingBlockInfo`: the rectangle out-of-flow boxes resolve against, in the border box
/// of the box that lays them out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContainingBlock {
  pub(crate) origin: Point<LayoutUnit>,
  pub(crate) size: Size<LayoutUnit>,
}

impl ContainingBlock {
  /// The padding box of a box whose border box is `size` with `border`.
  pub(crate) fn padding_box(size: Size<LayoutUnit>, border: Rect<f32>) -> Self {
    let border = border.map(LayoutUnit::from_float);

    Self {
      origin: Point::new(border.left, border.top),
      size: Size {
        width: (size.width - border.left - border.right).clamp_negative_to_zero(),
        height: (size.height - border.top - border.bottom).clamp_negative_to_zero(),
      },
    }
  }
}

/// Blink's `InsetModifiedContainingBlock::InsetBias`: which inset edge stays put.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsetBias {
  Start,
  End,
  Equal,
}

/// Blink's `InsetModifiedContainingBlock` in one axis.
#[derive(Debug, Clone, Copy)]
struct ImcbAxis {
  start: LayoutUnit,
  end: LayoutUnit,
  has_auto_inset: bool,
  inset_bias: InsetBias,
  safe_inset_bias: Option<InsetBias>,
  default_inset_bias: Option<InsetBias>,
  has_default_alignment_overflow: bool,
}

impl ImcbAxis {
  fn size(&self, available_size: LayoutUnit) -> LayoutUnit {
    available_size - self.start - self.end
  }
}

/// Blink's `GetAlignmentInsetBias`, its overflow behavior beside it.
struct Alignment {
  bias: InsetBias,
  is_safe: bool,
  is_normal: bool,
  is_stretch: bool,
}

impl Alignment {
  /// `justify-self` or `align-self`, its `auto` as `normal`.
  fn of(alignment: AlignItems) -> Self {
    let (bias, is_safe) = match alignment {
      AlignItems::Normal
      | AlignItems::Start
      | AlignItems::FlexStart
      | AlignItems::SelfStart
      | AlignItems::Baseline
      | AlignItems::Stretch => (InsetBias::Start, false),
      AlignItems::Center => (InsetBias::Equal, false),
      AlignItems::End | AlignItems::FlexEnd | AlignItems::SelfEnd => (InsetBias::End, false),
      AlignItems::SafeStart | AlignItems::SafeFlexStart | AlignItems::SafeSelfStart => {
        (InsetBias::Start, true)
      }
      AlignItems::SafeCenter => (InsetBias::Equal, true),
      AlignItems::SafeEnd | AlignItems::SafeFlexEnd | AlignItems::SafeSelfEnd => {
        (InsetBias::End, true)
      }
    };

    Self {
      bias,
      is_safe,
      is_normal: alignment == AlignItems::Normal,
      is_stretch: alignment == AlignItems::Stretch,
    }
  }
}

/// Blink's `ResizeIMCBInOneAxis`.
fn resize_imcb(bias: InsetBias, amount: LayoutUnit, start: &mut LayoutUnit, end: &mut LayoutUnit) {
  match bias {
    InsetBias::Start => *end += amount,
    InsetBias::End => *start += amount,
    InsetBias::Equal => {
      *start += amount / 2;
      *end += amount / 2;
    }
  }
}

/// Blink's `ComputeUnclampedIMCBInOneAxis`, clamped to a non-negative size as
/// `ComputeInsetModifiedContainingBlock` does.
#[allow(clippy::too_many_arguments)]
fn imcb_axis(
  available_size: LayoutUnit,
  inset_start: Option<LayoutUnit>,
  inset_end: Option<LayoutUnit>,
  is_static_alignment_parallel: bool,
  static_position_offset: LayoutUnit,
  static_position_bias: InsetBias,
  alignment: &Alignment,
  alternate_alignment: &Alignment,
  is_table: bool,
) -> ImcbAxis {
  let mut axis = ImcbAxis {
    start: LayoutUnit::ZERO,
    end: LayoutUnit::ZERO,
    has_auto_inset: inset_start.is_none() || inset_end.is_none(),
    inset_bias: InsetBias::Start,
    safe_inset_bias: None,
    default_inset_bias: None,
    has_default_alignment_overflow: !alignment.is_safe && !alignment.is_normal,
  };
  let safe_bias = alignment.is_safe.then_some(InsetBias::Start);
  let alternate_safe_bias = alternate_alignment.is_safe.then_some(InsetBias::Start);

  match (inset_start, inset_end) {
    (None, None) => {
      match static_position_bias {
        InsetBias::Start => axis.start = static_position_offset,
        InsetBias::Equal => {
          let half_size = static_position_offset.min(available_size - static_position_offset);

          axis.start = static_position_offset - half_size;
          axis.end = available_size - static_position_offset - half_size;
        }
        InsetBias::End => axis.end = available_size - static_position_offset,
      }
      axis.inset_bias = static_position_bias;
      axis.safe_inset_bias = if is_static_alignment_parallel {
        safe_bias
      } else {
        alternate_safe_bias
      };
    }
    (start, end) => {
      axis.start = start.unwrap_or(LayoutUnit::ZERO);
      axis.end = end.unwrap_or(LayoutUnit::ZERO);

      if start.is_none() || end.is_none() {
        axis.inset_bias = if start.is_some() {
          InsetBias::Start
        } else {
          InsetBias::End
        };
      } else {
        axis.inset_bias = alignment.bias;
        axis.safe_inset_bias = safe_bias;
        axis.default_inset_bias = Some(InsetBias::Start);
      }
    }
  }

  let bias = axis.default_inset_bias.unwrap_or(axis.inset_bias);
  let size = axis.size(available_size);

  if size < LayoutUnit::ZERO {
    resize_imcb(bias, size, &mut axis.start, &mut axis.end);
  }
  // Tables should not be larger than the container.
  if is_table && axis.size(available_size) > available_size {
    resize_imcb(
      bias,
      axis.size(available_size) - available_size,
      &mut axis.start,
      &mut axis.end,
    );
  }

  axis
}

/// Blink's `ComputeMargins`: the margins in one axis, auto ones taking the free space when both
/// insets are set. Returns whether auto margins applied.
fn margins_axis(
  start_length: LayoutLength,
  end_length: LayoutLength,
  percentage_inline_size: Option<LayoutUnit>,
  imcb_size: LayoutUnit,
  size: LayoutUnit,
  has_auto_inset: bool,
  is_block_direction: bool,
) -> (LayoutUnit, LayoutUnit, bool) {
  let resolve = |length: LayoutLength| {
    (!length.is_auto()).then(|| length.resolve_or_zero(percentage_inline_size))
  };
  let (start, end) = (resolve(start_length), resolve(end_length));

  if has_auto_inset || (start.is_some() && end.is_some()) {
    return (
      start.unwrap_or(LayoutUnit::ZERO),
      end.unwrap_or(LayoutUnit::ZERO),
      false,
    );
  }

  let free_space =
    imcb_size - size - start.unwrap_or(LayoutUnit::ZERO) - end.unwrap_or(LayoutUnit::ZERO);
  let (start, end) = match (start, end) {
    (None, None) if free_space > LayoutUnit::ZERO || is_block_direction => {
      let start = free_space / 2;

      (start, free_space - start)
    }
    // Negative free space goes to the end margin, as the start side dominates.
    (None, None) => (LayoutUnit::ZERO, free_space),
    (None, Some(end)) => (free_space, end),
    (Some(start), None) => (start, free_space),
    (Some(start), Some(end)) => (start, end),
  };

  (start, end, true)
}

/// Blink's `ComputeInsets`: the inset of each margin edge after aligning the margin box in the
/// inset-modified containing block.
fn insets_axis(
  available_size: LayoutUnit,
  imcb: &ImcbAxis,
  margin_start: LayoutUnit,
  margin_end: LayoutUnit,
  size: LayoutUnit,
) -> (LayoutUnit, LayoutUnit) {
  let margin_box_size = margin_start + size + margin_end;
  let (mut imcb_start, mut imcb_end) = (imcb.start, imcb.end);
  let mut free_space = available_size - imcb_start - imcb_end - margin_box_size;
  let mut bias = imcb.inset_bias;
  let apply_safe_bias = imcb.safe_inset_bias.is_some() && free_space < LayoutUnit::ZERO;

  if let (true, Some(safe_bias)) = (apply_safe_bias, imcb.safe_inset_bias) {
    free_space = LayoutUnit::ZERO;
    bias = safe_bias;
  }

  resize_imcb(bias, free_space, &mut imcb_start, &mut imcb_end);

  // The default overflow alignment shifts the box back inside the containing block.
  if imcb.has_default_alignment_overflow
    && let Some(default_bias) = imcb.default_inset_bias
    && !apply_safe_bias
  {
    let use_imcb = margin_box_size <= available_size - imcb.start - imcb.end;
    let safe_start = if use_imcb {
      imcb.start
    } else {
      imcb.start.min(LayoutUnit::ZERO)
    };
    let safe_end = if use_imcb {
      imcb.end
    } else {
      imcb.end.min(LayoutUnit::ZERO)
    };
    let adjust_start = |imcb_start: &mut LayoutUnit, imcb_end: &mut LayoutUnit| {
      if *imcb_start < safe_start {
        *imcb_end += *imcb_start - safe_start;
        *imcb_start = safe_start;
      }
    };
    let adjust_end = |imcb_start: &mut LayoutUnit, imcb_end: &mut LayoutUnit| {
      if *imcb_end < safe_end {
        *imcb_start += *imcb_end - safe_end;
        *imcb_end = safe_end;
      }
    };

    if default_bias == InsetBias::Start {
      adjust_end(&mut imcb_start, &mut imcb_end);
      adjust_start(&mut imcb_start, &mut imcb_end);
    } else {
      adjust_start(&mut imcb_start, &mut imcb_end);
      adjust_end(&mut imcb_start, &mut imcb_end);
    }
  }

  (imcb_start + margin_start, imcb_end + margin_end)
}

/// Blink's `CanComputeBlockSizeWithoutLayout`.
fn can_compute_block_size_without_layout(
  style: &LayoutStyle,
  is_table: bool,
  block_auto_behavior: AutoSizeBehavior,
) -> bool {
  if is_table
    || style.size.height.is_content_or_intrinsic()
    || style.min_size.height.is_content_or_intrinsic()
    || style.max_size.height.is_content_or_intrinsic()
  {
    return false;
  }
  if !style.size.height.is_auto() {
    return true;
  }

  match block_auto_behavior {
    AutoSizeBehavior::FitContent => false,
    AutoSizeBehavior::StretchExplicit => true,
    // An aspect-ratio will trigger fit-content.
    AutoSizeBehavior::StretchImplicit => style.aspect_ratio.is_none(),
  }
}

impl<'r> LayoutTree<'r> {
  /// Blink's `OutOfFlowLayoutPart::Run` for the box `container`: lays out the `candidates` it
  /// claims in `containing_block`, places them, and returns the rest for an ancestor.
  pub(crate) fn lay_out_out_of_flow(
    &mut self,
    container: NodeId,
    claims: ContainingBlockClaims,
    mut candidates: Vec<OutOfFlowCandidate>,
    containing_block: ContainingBlock,
    space: &ConstraintSpace,
  ) -> Vec<OutOfFlowCandidate> {
    let mut unclaimed = Vec::new();

    while !candidates.is_empty() {
      for candidate in take(&mut candidates) {
        if !claims.claims(self.layout_style(candidate.node).position) {
          unclaimed.push(candidate);
          continue;
        }

        let containing_block = self.grid_area(container, candidate.node, containing_block);
        let (offset, result) = self.lay_out_out_of_flow_box(candidate, containing_block, space);

        if space.cache_slot == CacheSlot::Layout {
          self.place_child(candidate.node, offset, &result);
          self.add_hoisted_child(container, candidate.node);
        }

        // The box's own out-of-flow descendants it does not contain come back to this one.
        candidates.extend(
          result
            .out_of_flow
            .iter()
            .map(|nested| nested.moved_by(offset)),
        );
      }
    }

    unclaimed
  }

  /// Blink's `CalculateOffset` and `GenerateFragment` for one box: its layout and the offset of
  /// its border box in the border box of the box laying it out.
  pub(crate) fn lay_out_out_of_flow_box(
    &mut self,
    candidate: OutOfFlowCandidate,
    containing_block: ContainingBlock,
    container_space: &ConstraintSpace,
  ) -> (Point<LayoutUnit>, Rc<LayoutResult>) {
    let node = candidate.node;
    let style = Rc::clone(self.layout_style_rc(node));
    let direction = style.direction;
    let available_size =
      LogicalSize::new(containing_block.size.width, containing_block.size.height);
    let is_table = self.is_table(node);
    let is_replaced = style.is_replaced;
    let mut space = ConstraintSpace::new(Some(available_size.inline_size), direction);

    space.available_block_size = Some(available_size.block_size);
    space.percentage_resolution_block_size = Some(available_size.block_size);
    space.is_new_formatting_context = true;
    space.cache_slot = container_space.cache_slot;

    // The static position, relative to the containing block in the box's own direction.
    let mut static_position = candidate.static_position;

    static_position.offset = static_position.offset - containing_block.origin;
    let static_position = static_position.to_logical(direction, available_size.inline_size);

    let inset = |length: LayoutLength, size: LayoutUnit| {
      (!length.is_auto()).then(|| length.resolve_or_zero(Some(size)))
    };
    let (inline_start_inset, inline_end_inset) =
      direction.inline_sides(style.inset.left, style.inset.right);
    let justify = Alignment::of(style.justify_self);
    let align = Alignment::of(style.align_self);
    let is_static_alignment_parallel =
      static_position.align_self_direction == LogicalAlignmentDirection::Block;
    let inline_imcb = imcb_axis(
      available_size.inline_size,
      inset(inline_start_inset, available_size.inline_size),
      inset(inline_end_inset, available_size.inline_size),
      is_static_alignment_parallel,
      static_position.offset.inline_offset,
      match static_position.inline_edge {
        InlineEdge::Start => InsetBias::Start,
        InlineEdge::Center => InsetBias::Equal,
        InlineEdge::End => InsetBias::End,
      },
      &justify,
      &align,
      is_table,
    );
    let block_imcb = imcb_axis(
      available_size.block_size,
      inset(style.inset.top, available_size.block_size),
      inset(style.inset.bottom, available_size.block_size),
      is_static_alignment_parallel,
      static_position.offset.block_offset,
      match static_position.block_edge {
        BlockEdge::Start => InsetBias::Start,
        BlockEdge::Center => InsetBias::Equal,
        BlockEdge::End => InsetBias::End,
      },
      &align,
      &justify,
      is_table,
    );
    let auto_size_behavior = |alignment: &Alignment, has_auto_inset: bool| {
      if has_auto_inset {
        AutoSizeBehavior::FitContent
      } else if alignment.is_stretch {
        AutoSizeBehavior::StretchExplicit
      } else if is_table || is_replaced || !alignment.is_normal {
        AutoSizeBehavior::FitContent
      } else {
        AutoSizeBehavior::StretchImplicit
      }
    };
    let inline_auto_behavior = auto_size_behavior(&justify, inline_imcb.has_auto_inset);
    let block_auto_behavior = auto_size_behavior(&align, block_imcb.has_auto_inset);
    let imcb_size = LogicalSize::new(
      inline_imcb.size(available_size.inline_size),
      block_imcb.size(available_size.block_size),
    );

    let resolver = LengthResolver::new(&space, &style);
    let border_padding = resolver.border_padding;
    let replaced_size = is_replaced.then(|| {
      let mut replaced_space = ConstraintSpace::new(Some(imcb_size.inline_size), direction);

      replaced_space.available_block_size = Some(imcb_size.block_size);
      replaced_space.percentage_resolution_inline_size = space.percentage_resolution_inline_size;
      replaced_space.percentage_resolution_block_size = space.percentage_resolution_block_size;
      replaced_space.inline_auto_behavior = inline_auto_behavior;
      replaced_space.block_auto_behavior = block_auto_behavior;

      LengthResolver::new(&replaced_space, &style)
        .replaced_size(self.natural_size(node), ReplacedSizeMode::Normal)
    });

    // Blink's `ComputeOofInlineDimensions`.
    let inline_size = match replaced_size {
      Some(size) => size.inline_size,
      None => {
        let mut min_max_space = ConstraintSpace::new(Some(imcb_size.inline_size), direction);

        min_max_space.available_block_size = Some(imcb_size.block_size);
        min_max_space.percentage_resolution_inline_size = space.percentage_resolution_inline_size;
        min_max_space.percentage_resolution_block_size = space.percentage_resolution_block_size;
        min_max_space.block_auto_behavior = block_auto_behavior;
        min_max_space.is_new_formatting_context = true;
        min_max_space.cache_slot = CacheSlot::Measure;

        let mut min_max_sizes = |size_type| self.min_max_sizes(node, &min_max_space, size_type);
        let resolver = LengthResolver::new(&space, &style);
        let may_apply_aspect_ratio = || {
          style.aspect_ratio.is_some()
            && resolver
              .resolve_main_block_length(
                style.size.height,
                Some(if block_auto_behavior == AutoSizeBehavior::FitContent {
                  LayoutLength::FitContent
                } else {
                  LayoutLength::Stretch
                }),
                &mut |_| None,
                Some(imcb_size.block_size),
              )
              .is_some()
        };
        let mut apply_automatic_min_size = false;
        let auto_length = match inline_auto_behavior {
          AutoSizeBehavior::FitContent => {
            apply_automatic_min_size =
              may_apply_aspect_ratio() && style.overflow.x == Overflow::Visible;
            LayoutLength::FitContent
          }
          AutoSizeBehavior::StretchExplicit => LayoutLength::Stretch,
          AutoSizeBehavior::StretchImplicit => {
            let is_block_explicit = !style.size.height.is_auto()
              || block_auto_behavior == AutoSizeBehavior::StretchExplicit;

            if is_block_explicit && may_apply_aspect_ratio() {
              apply_automatic_min_size = style.overflow.x == Overflow::Visible;
              LayoutLength::FitContent
            } else {
              LayoutLength::Stretch
            }
          }
        };
        let main = resolver.resolve_main_inline_length(
          &mut min_max_sizes,
          style.size.width,
          Some(auto_length),
          Some(imcb_size.inline_size),
        );
        let min_max = resolver.min_max_inline_sizes(
          apply_automatic_min_size.then_some(LayoutLength::MinIntrinsic),
          &mut min_max_sizes,
          true,
          FitContentMode::Normal,
          Some(imcb_size.inline_size),
        );

        min_max.clamp_size_to_min_and_max(main.unwrap_or(border_padding.inline_sum()))
      }
    };

    let (margin_inline_start_length, margin_inline_end_length) =
      direction.inline_sides(style.margin.left, style.margin.right);
    let (margin_inline_start, margin_inline_end, inline_auto_margins) = margins_axis(
      margin_inline_start_length,
      margin_inline_end_length,
      space.percentage_resolution_inline_size,
      imcb_size.inline_size,
      inline_size,
      inline_imcb.has_auto_inset,
      false,
    );
    let (inset_inline_start, _) = if inline_auto_margins {
      (
        inline_imcb.start + margin_inline_start,
        inline_imcb.end + margin_inline_end,
      )
    } else {
      insets_axis(
        available_size.inline_size,
        &inline_imcb,
        margin_inline_start,
        margin_inline_end,
        inline_size,
      )
    };

    // Blink's `ComputeOofBlockDimensions`.
    let mut layout_result = None;
    let block_size = if let Some(size) = replaced_size {
      size.block_size
    } else if can_compute_block_size_without_layout(&style, is_table, block_auto_behavior) {
      let resolver = LengthResolver::new(&space, &style);
      let main = resolver.resolve_main_block_length(
        style.size.height,
        Some(LayoutLength::Stretch),
        &mut |_| None,
        Some(imcb_size.block_size),
      );
      let min_max = resolver.initial_min_max_block_sizes(Some(imcb_size.block_size));

      min_max.clamp_size_to_min_and_max(main.unwrap_or(border_padding.block_sum()))
    } else {
      let result = self.layout(
        node,
        &self.out_of_flow_child_space(
          &space,
          inline_size,
          imcb_size.block_size,
          block_auto_behavior,
          false,
        ),
      );
      let block_size = result.size.block_size;

      layout_result = Some(result);
      block_size
    };

    let (margin_block_start, margin_block_end, block_auto_margins) = margins_axis(
      style.margin.top,
      style.margin.bottom,
      space.percentage_resolution_inline_size,
      imcb_size.block_size,
      block_size,
      block_imcb.has_auto_inset,
      true,
    );
    let (inset_block_start, _) = if block_auto_margins {
      (
        block_imcb.start + margin_block_start,
        block_imcb.end + margin_block_end,
      )
    } else {
      insets_axis(
        available_size.block_size,
        &block_imcb,
        margin_block_start,
        margin_block_end,
        block_size,
      )
    };

    // Blink's `GenerateFragment`, unless measuring the block size laid the box out already.
    let result = match layout_result {
      Some(result) => result,
      None if is_replaced => self.layout(
        node,
        &self.out_of_flow_child_space(&space, inline_size, block_size, block_auto_behavior, true),
      ),
      None => self.layout(
        node,
        &self.out_of_flow_child_space(
          &space,
          inline_size,
          imcb_size.block_size,
          block_auto_behavior,
          false,
        ),
      ),
    };

    // The inset is from the containing block's edge on the box's inline-start side.
    let x = match direction {
      Direction::Ltr => inset_inline_start,
      Direction::Rtl => available_size.inline_size - inset_inline_start - result.size.inline_size,
    };

    (
      containing_block.origin + Point::new(x, inset_block_start),
      result,
    )
  }

  /// The space an out-of-flow box lays out in once its inline size is known: its block size
  /// `block_size` when `is_fixed_block_size`, else the inset-modified containing block's.
  fn out_of_flow_child_space(
    &self,
    space: &ConstraintSpace,
    inline_size: LayoutUnit,
    block_size: LayoutUnit,
    block_auto_behavior: AutoSizeBehavior,
    is_fixed_block_size: bool,
  ) -> ConstraintSpace {
    ConstraintSpace {
      available_inline_size: Some(inline_size),
      available_block_size: Some(block_size),
      is_fixed_inline_size: true,
      is_fixed_block_size,
      block_auto_behavior,
      ..space.clone()
    }
  }
}

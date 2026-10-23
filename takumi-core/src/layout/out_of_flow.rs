//! Blink's [`OutOfFlowLayoutPart`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/out_of_flow_layout_part.cc)
//! and [`absolute_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/absolute_utils.cc):
//! absolutely and fixed positioned boxes laid out in their containing block, in its fragments as
//! `FragmentedOofInCb` does. A block resolves a box against the fragments so far stitched together,
//! where Blink uses the current fragment alone. Flex, grid and table containers go beyond Blink,
//! whose iterators cannot resume an out-of-flow break token, and lay their boxes out like a block.
//! Without anchor positioning, `position-try`, or scroll containers, and `justify-self: auto`
//! takes no `justify-items`.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::take, rc::Rc};

#[cfg(feature = "fragmentation")]
use crate::layout::{
  block_break_token::BlockBreakToken,
  box_fragment_builder::BoxFragmentBuilder,
  break_token::BreakToken,
  fragmentation_utils::{is_break_inside, setup_space_for_fragmentation},
};
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
  style::{AlignItems, ComputedStyle, Direction, Overflow, Position},
};

/// Which out-of-flow boxes a box is the containing block for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ContainingBlockClaims {
  pub(crate) absolute: bool,
  pub(crate) fixed: bool,
}

impl ContainingBlockClaims {
  /// The boxes a box styled `style` is the containing block for.
  pub(crate) fn of(style: &ComputedStyle) -> Self {
    let fixed = style.contains_fixed_descendants();

    Self {
      absolute: style.base_data.position.is_positioned() || fixed,
      fixed,
    }
  }

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

/// How a containing block's block size across the container's fragments settles.
#[cfg(feature = "fragmentation")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum StitchedSize {
  /// The fragments so far stitched together, which a later fragment adds to.
  Growing,
  /// A block size no later fragment changes, as the first page's area has for the initial
  /// containing block.
  Final,
  /// This fragment alone, as an inline box's is.
  ConfinedToFragment,
}

/// Blink's `OutOfFlowLayoutPart`: the fragment `fragment_index` of the box `container`, laid out
/// in `space`, laying out the out-of-flow boxes it `claims` in `containing_block`.
pub(crate) struct OutOfFlowLayoutPart<'b> {
  container: NodeId,
  fragment_index: usize,
  claims: ContainingBlockClaims,
  containing_block: ContainingBlock,
  space: &'b ConstraintSpace,
  /// The block size of the container's earlier fragments, which a stitched containing block starts
  /// above this fragment.
  #[cfg(feature = "fragmentation")]
  previously_consumed_block_size: LayoutUnit,
  /// The container's fragment builder when it lays out in fragments, which the boxes break across
  /// as Blink's `FragmentedOofInCb` lays them out.
  #[cfg(feature = "fragmentation")]
  builder: Option<&'b mut BoxFragmentBuilder>,
  #[cfg(feature = "fragmentation")]
  stitched_size: StitchedSize,
}

impl<'b> OutOfFlowLayoutPart<'b> {
  pub(crate) fn new(
    (container, fragment_index): (NodeId, usize),
    claims: ContainingBlockClaims,
    containing_block: ContainingBlock,
    space: &'b ConstraintSpace,
  ) -> Self {
    Self {
      container,
      fragment_index,
      claims,
      containing_block,
      space,
      #[cfg(feature = "fragmentation")]
      previously_consumed_block_size: LayoutUnit::ZERO,
      #[cfg(feature = "fragmentation")]
      builder: None,
      #[cfg(feature = "fragmentation")]
      stitched_size: StitchedSize::Growing,
    }
  }

  /// The part of a container that lays out in fragments with `builder`.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn in_fragments(self, builder: &'b mut BoxFragmentBuilder) -> Self {
    Self {
      builder: Some(builder),
      ..self
    }
  }

  /// Moves the containing block, given in the fragments stitched together, up past the
  /// `previously_consumed_block_size` of the container's earlier fragments to this fragment.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn stitched(mut self, previously_consumed_block_size: LayoutUnit) -> Self {
    self.containing_block.origin.y -= previously_consumed_block_size;
    self.previously_consumed_block_size = previously_consumed_block_size;
    self
  }

  /// Confines the containing block to this fragment, so no box is held for the last fragment.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn confined_to_fragment(self) -> Self {
    Self {
      stitched_size: StitchedSize::ConfinedToFragment,
      ..self
    }
  }

  /// Gives the containing block a block size no later fragment changes.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn with_final_block_size(self) -> Self {
    Self {
      stitched_size: StitchedSize::Final,
      ..self
    }
  }

  /// Blink's `PreviousBreakToken` of the container.
  #[cfg(feature = "fragmentation")]
  fn container_break_token(&self) -> Option<&BlockBreakToken> {
    self.builder.as_deref()?.previous_break_token.as_deref()
  }

  #[cfg(feature = "fragmentation")]
  fn previously_consumed_block_size(&self) -> LayoutUnit {
    self.previously_consumed_block_size
  }

  /// Where a resumed `candidate` starts in the container: it keeps its inline offset, and starts
  /// its fragment past the monolithic overflow an earlier page pushed into this one, where the
  /// token counts from the top of the page.
  #[cfg(feature = "fragmentation")]
  fn resumed_offset(&self, candidate: &OutOfFlowCandidate) -> Option<Point<LayoutUnit>> {
    let token = candidate.break_token.as_ref()?;
    let monolithic_overflow = self
      .container_break_token()
      .map_or(LayoutUnit::ZERO, |container| container.monolithic_overflow);
    let fragmentainer_offset = self
      .space
      .block_fragmentation
      .map_or(LayoutUnit::ZERO, |fragmentation| {
        fragmentation.fragmentainer_offset
      });

    Some(Point::new(
      token.oof_start_offset.x,
      monolithic_overflow.max(token.oof_start_offset.y) - fragmentainer_offset,
    ))
  }

  /// Whether `candidate` lays out again after measuring its block size: it resumes, or breaks
  /// across the container's fragments.
  fn lays_out_again(&self, candidate: &OutOfFlowCandidate) -> bool {
    #[cfg(feature = "fragmentation")]
    if self.builder.is_some() && self.space.has_block_fragmentation() {
      return true;
    }
    candidate.is_resumed()
  }
}

#[cfg(not(feature = "fragmentation"))]
impl OutOfFlowLayoutPart<'_> {
  fn previously_consumed_block_size(&self) -> LayoutUnit {
    LayoutUnit::ZERO
  }

  fn resumed_offset(&self, _candidate: &OutOfFlowCandidate) -> Option<Point<LayoutUnit>> {
    None
  }
}

/// Blink's `OffsetInfo`: where an out-of-flow box goes in the border box of the box laying it
/// out, and the space it lays out in there.
struct OutOfFlowOffset {
  offset: Point<LayoutUnit>,
  space: ConstraintSpace,
  /// The layout that measured the box's block size, which a box that lays out once keeps.
  initial_layout_result: Option<Rc<LayoutResult>>,
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

/// Whether an out-of-flow box's block position or size depends on the block size of its
/// containing block: it sets `bottom`, or a percentage or `stretch` inset, size, or limit.
#[cfg(feature = "fragmentation")]
pub(crate) fn depends_on_containing_block_block_size(style: &LayoutStyle) -> bool {
  !style.inset.bottom.is_auto()
    || style.inset.top.has_percent()
    || [
      style.size.height,
      style.min_size.height,
      style.max_size.height,
    ]
    .into_iter()
    .any(LayoutLength::has_percent_or_stretch)
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
  /// Blink's `OutOfFlowLayoutPart::Run`: lays out the `candidates` the `part`'s container claims,
  /// places them in its fragment, and returns the rest for an ancestor.
  #[cfg_attr(not(feature = "fragmentation"), allow(unused_mut))]
  pub(crate) fn lay_out_out_of_flow(
    &mut self,
    mut part: OutOfFlowLayoutPart<'_>,
    mut candidates: Vec<OutOfFlowCandidate>,
  ) -> Vec<OutOfFlowCandidate> {
    let mut unclaimed = Vec::new();
    // Whether the container has a fragment after this one, as far as its content so far says.
    #[cfg(feature = "fragmentation")]
    let breaks_after = part
      .builder
      .as_deref()
      .is_some_and(BoxFragmentBuilder::should_break);

    while !candidates.is_empty() {
      for candidate in take(&mut candidates) {
        if !candidate.is_resumed()
          && !part
            .claims
            .claims(self.layout_style(candidate.node).position)
        {
          unclaimed.push(candidate);
          continue;
        }

        #[cfg(feature = "fragmentation")]
        if breaks_after
          && part.stitched_size == StitchedSize::Growing
          && !candidate.is_resumed()
          && depends_on_containing_block_block_size(self.layout_style(candidate.node))
          && let Some(builder) = part.builder.as_deref_mut()
        {
          builder.holds_out_of_flow(candidate.node, candidate.static_position);
          continue;
        }

        let containing_block = self.grid_area(
          part.container,
          candidate.node,
          part.containing_block,
          part.previously_consumed_block_size(),
        );
        let offset_info = self.calculate_out_of_flow_offset(&candidate, containing_block, &part);
        let offset = offset_info.offset;

        #[cfg(feature = "fragmentation")]
        if let Some(builder) = part.builder.as_deref_mut()
          && !is_break_inside(candidate.break_token.as_deref())
          && builder.breaks_before_out_of_flow(candidate.node, offset)
        {
          continue;
        }

        let result = self.generate_out_of_flow_fragment(&candidate, offset_info, &part);

        #[cfg(feature = "fragmentation")]
        if let Some(builder) = part.builder.as_deref_mut() {
          builder.add_out_of_flow_result(&result, offset);
        }
        if part.space.cache_slot == CacheSlot::Layout {
          self.place_child(
            candidate.node,
            candidate.fragment_index(),
            part.fragment_index,
            offset,
            &result,
          );
          self.add_hoisted_child(part.container, candidate.node);
        }

        // The box's own out-of-flow descendants it does not contain come back to this one.
        candidates.extend(result.out_of_flow_moved_by(offset));
      }
    }

    unclaimed
  }

  /// Blink's `HandleOofsAndSpecialDescendants`: lays out the `candidates` the box `node` claims,
  /// its fragment `fragment_index` of `size` styled `style` and laid out in `space`, in the
  /// fragments `builder` makes when its out-of-flow boxes break across them. Returns the rest for
  /// an ancestor.
  pub(crate) fn lay_out_container_out_of_flow(
    &mut self,
    (node, fragment_index): (NodeId, usize),
    style: &LayoutStyle,
    size: LogicalSize,
    space: &ConstraintSpace,
    candidates: Vec<OutOfFlowCandidate>,
    #[cfg(feature = "fragmentation")] builder: &mut BoxFragmentBuilder,
  ) -> Vec<OutOfFlowCandidate> {
    if candidates.is_empty() {
      return candidates;
    }

    // The boxes resolve against this fragment and those before it stitched together.
    #[cfg(feature = "fragmentation")]
    let block_size = builder.stitched_block_size(size.block_size);
    #[cfg(not(feature = "fragmentation"))]
    let block_size = size.block_size;
    let part = OutOfFlowLayoutPart::new(
      (node, fragment_index),
      self.claims(node),
      ContainingBlock::padding_box(
        Size {
          width: size.inline_size,
          height: block_size,
        },
        style.border,
      ),
      space,
    );

    #[cfg(feature = "fragmentation")]
    let part = {
      let part = part.stitched(builder.previously_consumed_block_size());

      if builder.breaks_out_of_flow_children(style) {
        part.in_fragments(builder)
      } else {
        part
      }
    };
    self.lay_out_out_of_flow(part, candidates)
  }

  /// Blink's `HandleOutOfFlowPositioned` for the out-of-flow children of the box `builder` builds
  /// that an earlier fragment broke inside or pushed here: a candidate for each, resuming at its
  /// break token. Flex, grid and table iterators cannot consume these tokens, which Blink's do
  /// not, so they skip them through [`Self::without_out_of_flow_tokens`].
  #[cfg(feature = "fragmentation")]
  pub(crate) fn resumed_out_of_flow(
    &self,
    builder: &BoxFragmentBuilder,
  ) -> Vec<OutOfFlowCandidate> {
    builder
      .previous_break_token
      .iter()
      .flat_map(|token| &token.child_break_tokens)
      .filter_map(|token| match token {
        BreakToken::Block(token) if self.layout_style(token.node).is_out_of_flow() => {
          OutOfFlowCandidate::resuming(
            token.node,
            token,
            builder.previously_consumed_block_size(),
            builder.requires_content_before_breaking,
          )
        }
        _ => None,
      })
      .collect()
  }

  /// `token` without the break tokens of its out-of-flow children, which the item, row and section
  /// iterators of a flex, grid or table container would take for those of an in-flow child.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn without_out_of_flow_tokens(
    &self,
    token: Option<&Rc<BlockBreakToken>>,
  ) -> Option<Rc<BlockBreakToken>> {
    let token = token?;
    let is_out_of_flow = |child: &BreakToken| matches!(child, BreakToken::Block(child) if self.layout_style(child.node).is_out_of_flow());

    if !token.child_break_tokens.iter().any(is_out_of_flow) {
      return Some(Rc::clone(token));
    }

    Some(Rc::new(BlockBreakToken {
      child_break_tokens: token
        .child_break_tokens
        .iter()
        .filter(|child| !is_out_of_flow(child))
        .cloned()
        .collect(),
      ..(**token).clone()
    }))
  }

  /// Blink's `CalculateOffset` and `GenerateFragment` for a box in `containing_block` of a box laid
  /// out in `space` without fragments: the offset of its border box in the border box of that box,
  /// and its layout.
  pub(crate) fn lay_out_out_of_flow_box(
    &mut self,
    candidate: &OutOfFlowCandidate,
    containing_block: ContainingBlock,
    space: &ConstraintSpace,
  ) -> (Point<LayoutUnit>, Rc<LayoutResult>) {
    let part = OutOfFlowLayoutPart::new(
      (NodeId::ROOT, 0),
      ContainingBlockClaims::ALL,
      containing_block,
      space,
    );
    let offset_info = self.calculate_out_of_flow_offset(candidate, containing_block, &part);
    let offset = offset_info.offset;

    (
      offset,
      self.generate_out_of_flow_fragment(candidate, offset_info, &part),
    )
  }

  /// Blink's `CalculateOffset` for `candidate`, which `part` lays out in `containing_block`.
  fn calculate_out_of_flow_offset(
    &mut self,
    candidate: &OutOfFlowCandidate,
    containing_block: ContainingBlock,
    part: &OutOfFlowLayoutPart<'_>,
  ) -> OutOfFlowOffset {
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
    space.cache_slot = part.space.cache_slot;

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
      let lays_out_again = part.lays_out_again(candidate);
      let mut measure_space = self.out_of_flow_child_space(
        &space,
        inline_size,
        imcb_size.block_size,
        block_auto_behavior,
        false,
      );

      // Blink's `DisableLayoutSideEffectsScope`: the layout to come writes the box's fragment.
      if lays_out_again {
        measure_space.cache_slot = CacheSlot::Measure;
      }

      let result = self.layout(node, &measure_space);
      let block_size = result.size.block_size;

      layout_result = (!lays_out_again).then_some(result);
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

    let space = if is_replaced {
      self.out_of_flow_child_space(&space, inline_size, block_size, block_auto_behavior, true)
    } else {
      self.out_of_flow_child_space(
        &space,
        inline_size,
        imcb_size.block_size,
        block_auto_behavior,
        false,
      )
    };
    // The inset is from the containing block's edge on the box's inline-start side.
    let x = match direction {
      Direction::Ltr => inset_inline_start,
      Direction::Rtl => available_size.inline_size - inset_inline_start - inline_size,
    };
    let offset = part
      .resumed_offset(candidate)
      .unwrap_or(containing_block.origin + Point::new(x, inset_block_start));

    OutOfFlowOffset {
      offset,
      space,
      initial_layout_result: layout_result,
    }
  }

  /// Blink's `GenerateFragment` for `candidate` at `offset_info`, unless measuring its block size
  /// laid it out already: in a container that lays out in fragments, it breaks across them.
  fn generate_out_of_flow_fragment(
    &mut self,
    candidate: &OutOfFlowCandidate,
    offset_info: OutOfFlowOffset,
    part: &OutOfFlowLayoutPart<'_>,
  ) -> Rc<LayoutResult> {
    if let Some(result) = offset_info.initial_layout_result {
      return result;
    }

    let space = self.fragmented_space(candidate, offset_info.space, offset_info.offset, part);

    self.layout_fragment(candidate.node, &space, &candidate.layout_input())
  }

  /// `space` for `candidate` at `offset`, set up to break in the container's fragments. A box that
  /// starts in an earlier fragment of a stitched containing block lays out whole, for the pages to
  /// paint it across.
  // Blink passes the page's own size, which leaves cloned borders of the containing blocks out.
  // https://github.com/w3c/csswg-drafts/issues/10553
  #[cfg(feature = "fragmentation")]
  fn fragmented_space(
    &self,
    candidate: &OutOfFlowCandidate,
    mut space: ConstraintSpace,
    offset: Point<LayoutUnit>,
    part: &OutOfFlowLayoutPart<'_>,
  ) -> ConstraintSpace {
    if part.builder.is_some()
      && let Some(fragmentation) = part.space.block_fragmentation
      && (fragmentation.fragmentainer_offset + offset.y >= LayoutUnit::ZERO
        || part.stitched_size == StitchedSize::ConfinedToFragment)
    {
      setup_space_for_fragmentation(
        part.space,
        self.is_monolithic(candidate.node),
        fragmentation.fragmentainer_offset + offset.y,
        fragmentation.fragmentainer_block_size,
        candidate.requires_content_before_breaking,
        &mut space,
      );
    }
    space
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

#[cfg(not(feature = "fragmentation"))]
impl LayoutTree<'_> {
  fn fragmented_space(
    &self,
    _candidate: &OutOfFlowCandidate,
    space: ConstraintSpace,
    _offset: Point<LayoutUnit>,
    _part: &OutOfFlowLayoutPart<'_>,
  ) -> ConstraintSpace {
    space
  }
}

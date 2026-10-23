//! Blink's [`FlexLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_layout_algorithm.cc),
//! in a horizontal writing mode, without fragmentation, gap decorations, `reading-flow`, scrollbars,
//! `calc-size()`, or `box-flex` and `box-ordinal-group` for a `-webkit-box`. Every item's
//! baseline is alphabetic, and a column container synthesizes its items' baselines.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{cell::Cell, mem::take, rc::Rc};

use crate::{
  geometry::{NodeId, Point, Size},
  layout::{
    constraint_space::{AutoSizeBehavior, CacheSlot, ConstraintSpace},
    content_alignment::{ContentAlignment, ContentDistribution, ContentPosition},
    flex::{
      flex_item::{FlexItem, FlexLine},
      flex_line_breaker::break_flex_items_into_lines,
      line_flexer::LineFlexer,
    },
    layout_result::{
      BlockEdge, InlineEdge, LayoutResult, LogicalAlignmentDirection, LogicalStaticPosition,
      OutOfFlowCandidate,
    },
    layout_style::{LayoutLength, LayoutStyle},
    length_utils::{
      FitContentMode, LengthResolver, ReplacedSizeMode, block_size_from_aspect_ratio,
      child_available_size, child_percentage_size, margins_for, transferred_min_max_block_sizes,
    },
    logical_rect::{AvailableLogicalSize, LogicalBoxStrut, LogicalOffset, LogicalSize},
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
    out_of_flow::ContainingBlock,
    relative_utils::compute_relative_offset,
    tree::LayoutTree,
  },
  layout_unit::{BoxStrut, LayoutUnit},
  style::{AlignItems, BoxSizing, Direction, Display, FlexDirection, FlexWrap},
};

/// Blink's `ItemPosition`, resolved for a flex item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemPosition {
  FlowStart,
  FlowEnd,
  Center,
  Stretch,
  Baseline,
}

/// Blink's `BaselineGroup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineGroup {
  Major,
  Minor,
}

impl ContentAlignment {
  /// Blink's `InitialContentPositionOffset`.
  fn initial_offset(
    self,
    free_space: LayoutUnit,
    number_of_items: usize,
    is_reverse: bool,
  ) -> LayoutUnit {
    let flow_start = if is_reverse {
      free_space
    } else {
      LayoutUnit::ZERO
    };
    let count = number_of_items as i32;

    match self.distribution {
      ContentDistribution::Default => {}
      ContentDistribution::SpaceBetween => {
        if free_space > LayoutUnit::ZERO && number_of_items > 1 {
          return LayoutUnit::ZERO;
        }
        return flow_start;
      }
      ContentDistribution::SpaceAround => {
        if free_space > LayoutUnit::ZERO && number_of_items > 0 {
          return free_space / (2 * count);
        }
        return (free_space / 2).clamp_negative_to_zero();
      }
      ContentDistribution::SpaceEvenly => {
        if free_space > LayoutUnit::ZERO && number_of_items > 0 {
          return free_space / (count + 1);
        }
        return (free_space / 2).clamp_negative_to_zero();
      }
      ContentDistribution::Stretch => return flow_start,
    }

    if free_space <= LayoutUnit::ZERO && self.is_safe() {
      return LayoutUnit::ZERO;
    }

    match self.position {
      ContentPosition::Center => free_space / 2,
      ContentPosition::Start => LayoutUnit::ZERO,
      ContentPosition::End => free_space,
      ContentPosition::FlowEnd => {
        if is_reverse {
          LayoutUnit::ZERO
        } else {
          free_space
        }
      }
      ContentPosition::FlowStart | ContentPosition::Normal => flow_start,
    }
  }

  /// Blink's `ContentDistributionSpace`.
  fn distribution_space(
    self,
    free_space: LayoutUnit,
    number_of_items: usize,
  ) -> LayoutUnitDiffuser {
    if free_space <= LayoutUnit::ZERO || number_of_items <= 1 {
      return LayoutUnitDiffuser::default();
    }

    match self.distribution {
      ContentDistribution::Default | ContentDistribution::Stretch => LayoutUnitDiffuser::default(),
      ContentDistribution::SpaceBetween => LayoutUnitDiffuser::new(free_space, number_of_items - 1),
      ContentDistribution::SpaceEvenly => LayoutUnitDiffuser::new(free_space, number_of_items + 1),
      ContentDistribution::SpaceAround => LayoutUnitDiffuser::new(free_space, number_of_items),
    }
  }
}

/// Blink's `LayoutUnitDiffuser`: `size` split into `buckets` that differ by at most a layout unit.
#[derive(Debug, Default)]
struct LayoutUnitDiffuser {
  base: LayoutUnit,
  dx: u64,
  dy: u64,
  x: u64,
  y: u64,
  count: usize,
}

impl LayoutUnitDiffuser {
  fn new(size: LayoutUnit, buckets: usize) -> Self {
    let raw = size.raw_value().max(0) as u64;
    let buckets_raw = buckets as u64;

    Self {
      base: LayoutUnit::from_raw_value((raw / buckets_raw) as i32),
      dx: (raw % buckets_raw) * 2,
      dy: buckets_raw * 2,
      x: 0,
      y: buckets_raw,
      count: buckets,
    }
  }

  fn next(&mut self) -> LayoutUnit {
    if self.count == 0 {
      return LayoutUnit::ZERO;
    }
    self.count -= 1;
    self.x += self.dx;

    if self.x >= self.y {
      self.y += self.dy;
      return self.base + LayoutUnit::from_raw_value(1);
    }

    self.base
  }
}

/// Blink's `PhysicalToFlex`: the four sides of a physical strut in flex-relative terms.
#[derive(Debug, Clone, Copy)]
struct FlexSides<T> {
  main_start: T,
  main_end: T,
  cross_start: T,
  cross_end: T,
}

impl<T: Copy> FlexSides<T> {
  fn of(top: T, right: T, bottom: T, left: T, direction: Direction, is_column: bool) -> Self {
    let (inline_start, inline_end) = match direction {
      Direction::Ltr => (left, right),
      Direction::Rtl => (right, left),
    };

    if is_column {
      Self {
        main_start: top,
        main_end: bottom,
        cross_start: inline_start,
        cross_end: inline_end,
      }
    } else {
      Self {
        main_start: inline_start,
        main_end: inline_end,
        cross_start: top,
        cross_end: bottom,
      }
    }
  }
}

/// Blink's `FlexLayoutAlgorithm::Phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
  Layout,
  RowIntrinsicSize,
  ColumnWrapIntrinsicSize,
}

/// Blink's `BaselineAccumulator`.
#[derive(Default)]
struct BaselineAccumulator {
  first_major_baseline: Option<LayoutUnit>,
  first_minor_baseline: Option<LayoutUnit>,
  first_fallback_baseline: Option<LayoutUnit>,
  last_major_baseline: Option<LayoutUnit>,
  last_minor_baseline: Option<LayoutUnit>,
  last_fallback_baseline: Option<LayoutUnit>,
}

impl BaselineAccumulator {
  fn accumulate_item(
    &mut self,
    result: &LayoutResult,
    block_offset: LayoutUnit,
    is_first_line: bool,
    is_last_line: bool,
  ) {
    let block_size = result.size.block_size;

    if is_first_line && self.first_fallback_baseline.is_none() {
      self.first_fallback_baseline =
        Some(block_offset + result.first_baseline.unwrap_or(block_size));
    }
    if is_last_line {
      self.last_fallback_baseline = Some(block_offset + result.last_baseline.unwrap_or(block_size));
    }
  }

  fn accumulate_line(&mut self, line: &FlexLine, is_first_line: bool, is_last_line: bool) {
    let major = (line.major_baseline != LayoutUnit::MIN)
      .then(|| line.cross_axis_offset + line.major_baseline);
    let minor = (line.minor_baseline != LayoutUnit::MIN)
      .then(|| line.cross_axis_offset + line.line_cross_size - line.minor_baseline);

    if is_first_line {
      self.first_major_baseline = self.first_major_baseline.or(major);
      self.first_minor_baseline = self.first_minor_baseline.or(minor);
    }
    if is_last_line {
      self.last_major_baseline = major.or(self.last_major_baseline);
      self.last_minor_baseline = minor.or(self.last_minor_baseline);
    }
  }

  fn first_baseline(&self) -> Option<LayoutUnit> {
    self
      .first_major_baseline
      .or(self.first_minor_baseline)
      .or(self.first_fallback_baseline)
  }

  fn last_baseline(&self) -> Option<LayoutUnit> {
    self
      .last_minor_baseline
      .or(self.last_major_baseline)
      .or(self.last_fallback_baseline)
  }
}

/// A child placed in the fragment: its border box's location and its layout.
struct PlacedChild {
  node: NodeId,
  location: Point<LayoutUnit>,
  result: Rc<LayoutResult>,
}

/// Blink's `FlexLayoutAlgorithm` with its `BoxFragmentBuilder`.
pub(crate) struct FlexLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  node: NodeId,
  space: &'t ConstraintSpace,
  style: Rc<LayoutStyle>,
  border_padding: LogicalBoxStrut,
  /// The border-box inline size, none while sizing intrinsically.
  inline_size: Option<LayoutUnit>,
  child_available_size: AvailableLogicalSize,
  child_percentage_size: AvailableLogicalSize,
  flex_items: Vec<FlexItem>,
  /// The widest min-content contribution of an item, for a column-wrap container's max-content
  /// width.
  largest_min_content_contribution: LayoutUnit,
  is_webkit_box: bool,
  is_column: bool,
  is_wrap_reverse: bool,
  is_reverse_direction: bool,
  is_multi_line: bool,
  is_horizontal_flow: bool,
  is_cross_size_definite: bool,
  balance_min_line_count: Option<usize>,
  gap_between_items: LayoutUnit,
  gap_between_lines: LayoutUnit,
  total_block_size: LayoutUnit,
  children: Vec<PlacedChild>,
  out_of_flow: Vec<OutOfFlowCandidate>,
  first_baseline: Option<LayoutUnit>,
  last_baseline: Option<LayoutUnit>,
}

impl<'t, 'r> FlexLayoutAlgorithm<'t, 'r> {
  /// The algorithm for `node` in `space`, with its intrinsic fragment geometry, an indefinite
  /// inline size, when `is_intrinsic`.
  pub(crate) fn new(
    tree: &'t mut LayoutTree<'r>,
    node: NodeId,
    space: &'t ConstraintSpace,
    is_intrinsic: bool,
  ) -> Self {
    let style = Rc::clone(tree.layout_style_rc(node));
    let resolver = LengthResolver::new(space, &style);
    let border_padding = resolver.border_padding;
    let inline_size = (!is_intrinsic).then(|| {
      resolver.inline_size_for_fragment(&mut |size_type| tree.min_max_sizes(node, space, size_type))
    });
    let initial_block_size = resolver.initial_block_size_for_fragment(None, inline_size);
    let child_available_size = child_available_size(
      AvailableLogicalSize {
        inline_size,
        block_size: initial_block_size,
      },
      border_padding,
    );
    let child_percentage_size =
      child_percentage_size(space, tree.is_anonymous(node), child_available_size);
    let is_column = matches!(
      style.flex_direction,
      FlexDirection::Column | FlexDirection::ColumnReverse
    );
    let is_webkit_box = style.display.is_deprecated_webkit_box();
    let is_multi_line = style.flex_wrap != FlexWrap::NoWrap;
    let is_balance = matches!(
      style.flex_wrap,
      FlexWrap::Balance | FlexWrap::BalanceReverse
    );
    let row_gap = style
      .row_gap
      .resolve(child_percentage_size.block_size)
      .unwrap_or(LayoutUnit::ZERO);
    let column_gap = style
      .column_gap
      .resolve(child_percentage_size.inline_size)
      .unwrap_or(LayoutUnit::ZERO);

    Self {
      tree,
      node,
      space,
      border_padding,
      inline_size,
      child_available_size,
      child_percentage_size,
      flex_items: Vec::new(),
      largest_min_content_contribution: LayoutUnit::ZERO,
      is_webkit_box,
      is_column,
      is_wrap_reverse: matches!(
        style.flex_wrap,
        FlexWrap::WrapReverse | FlexWrap::BalanceReverse
      ),
      is_reverse_direction: matches!(
        style.flex_direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
      ),
      is_multi_line,
      is_horizontal_flow: !is_column,
      // A column container's cross axis is its inline axis, which is definite.
      is_cross_size_definite: is_column || child_available_size.block_size.is_some(),
      balance_min_line_count: is_balance.then_some(style.flex_line_count.max(1)),
      gap_between_items: if is_column { row_gap } else { column_gap },
      gap_between_lines: if is_column { column_gap } else { row_gap },
      total_block_size: LayoutUnit::ZERO,
      children: Vec::new(),
      out_of_flow: Vec::new(),
      first_baseline: None,
      last_baseline: None,
      style,
    }
  }

  fn direction(&self) -> Direction {
    self.space.direction
  }

  /// Blink's `ResolvedJustifyContent`, without `left` and `right`, which takumi does not parse.
  fn resolved_justify_content(&self) -> ContentAlignment {
    ContentAlignment::of(self.style.justify_content)
  }

  /// Blink's `ResolvedAlignSelf`.
  fn resolved_align_self(&self, child_style: &LayoutStyle, is_out_of_flow: bool) -> ItemPosition {
    // Auto margins in the cross axis align to the start.
    if !is_out_of_flow {
      let (start, end) = if self.is_horizontal_flow {
        (child_style.margin.top, child_style.margin.bottom)
      } else {
        (child_style.margin.left, child_style.margin.right)
      };

      if start.is_auto() || end.is_auto() {
        return ItemPosition::FlowStart;
      }
    }

    // `-box-align` reaches takumi as the container's `align-items`.
    if self.is_webkit_box {
      return match self.style.align_items {
        AlignItems::Baseline => ItemPosition::Baseline,
        AlignItems::Center | AlignItems::SafeCenter => ItemPosition::Center,
        AlignItems::Start
        | AlignItems::FlexStart
        | AlignItems::SafeStart
        | AlignItems::SafeFlexStart => ItemPosition::FlowStart,
        AlignItems::End | AlignItems::FlexEnd | AlignItems::SafeEnd | AlignItems::SafeFlexEnd => {
          ItemPosition::FlowEnd
        }
        _ => ItemPosition::Stretch,
      };
    }

    let align = match (child_style.align_self, self.style.align_items) {
      (AlignItems::Normal, AlignItems::Normal) => AlignItems::Stretch,
      (AlignItems::Normal, align_items) => align_items,
      (align_self, _) => align_self,
    };

    let self_start = |is_start: bool| {
      // The cross axis is the block axis of a row, and the inline axis of a column.
      let flips = self.is_column && child_style.direction != self.direction();

      if is_start != flips {
        ItemPosition::FlowStart
      } else {
        ItemPosition::FlowEnd
      }
    };
    let position = match align {
      AlignItems::Start | AlignItems::SafeStart => return ItemPosition::FlowStart,
      AlignItems::End | AlignItems::SafeEnd => return ItemPosition::FlowEnd,
      AlignItems::SelfStart | AlignItems::SafeSelfStart => return self_start(true),
      AlignItems::SelfEnd | AlignItems::SafeSelfEnd => return self_start(false),
      AlignItems::FlexStart | AlignItems::SafeFlexStart => ItemPosition::FlowStart,
      AlignItems::FlexEnd | AlignItems::SafeFlexEnd => ItemPosition::FlowEnd,
      AlignItems::Center | AlignItems::SafeCenter => ItemPosition::Center,
      AlignItems::Baseline => ItemPosition::Baseline,
      AlignItems::Stretch | AlignItems::Normal => ItemPosition::Stretch,
    };

    match (self.is_wrap_reverse, position) {
      (true, ItemPosition::FlowStart) => ItemPosition::FlowEnd,
      (true, ItemPosition::FlowEnd) => ItemPosition::FlowStart,
      _ => position,
    }
  }

  /// Whether the item's own `align-self` is `safe`, as Blink reads its `Overflow`.
  fn is_align_self_safe(&self, child_style: &LayoutStyle) -> bool {
    let align = match child_style.align_self {
      AlignItems::Normal => self.style.align_items,
      align_self => align_self,
    };

    !self.is_webkit_box
      && matches!(
        align,
        AlignItems::SafeStart
          | AlignItems::SafeEnd
          | AlignItems::SafeFlexStart
          | AlignItems::SafeFlexEnd
          | AlignItems::SafeSelfStart
          | AlignItems::SafeSelfEnd
          | AlignItems::SafeCenter
      )
  }

  /// Blink's `MainAxisContentExtent`.
  fn main_axis_content_extent(&self, sum_hypothetical_main_size: Option<LayoutUnit>) -> LayoutUnit {
    if !self.is_column {
      return self
        .child_available_size
        .inline_size
        .unwrap_or(LayoutUnit::ZERO);
    }

    let border_padding = self.border_padding.block_sum();
    let resolver = LengthResolver::new(self.space, &self.style);
    let intrinsic_size = sum_hypothetical_main_size.map(|sum| sum + border_padding);

    if let Some(block_size) =
      resolver.block_size_for_fragment(intrinsic_size, self.inline_size, None)
    {
      return (block_size - border_padding).clamp_negative_to_zero();
    }

    // An indefinite block size takes the max block size.
    let max_block_size = resolver.initial_min_max_block_sizes(None).max_size;

    if max_block_size != LayoutUnit::MAX {
      return (max_block_size - border_padding).clamp_negative_to_zero();
    }

    LayoutUnit::MAX
  }

  /// The ascent of `baseline` in an item `block_size` tall, from its cross-start margin edge.
  fn ascent(&self, item: &FlexItem, baseline: LayoutUnit, block_size: LayoutUnit) -> LayoutUnit {
    let baseline = if self.is_wrap_reverse {
      block_size - baseline
    } else {
      baseline
    };
    let margins = self.flex_margins(item.initial_margins);

    match item.baseline_group {
      BaselineGroup::Major => margins.cross_start + baseline,
      BaselineGroup::Minor => margins.cross_end + baseline,
    }
  }

  /// Blink's `BaselineAscent`. A column's items share no writing mode with its cross axis, so
  /// their baselines are synthesized.
  fn baseline_ascent(&self, item: &FlexItem, result: &LayoutResult) -> LayoutUnit {
    if self.is_column {
      return self.synthesized_baseline_ascent(item, result.size.inline_size);
    }

    let block_size = result.size.block_size;

    self.ascent(
      item,
      result.first_baseline.unwrap_or(block_size),
      block_size,
    )
  }

  /// Blink's `SynthesizedBaselineAscent`: the alphabetic baseline at the line-under edge, which in a
  /// column's `vertical-lr` or `vertical-rl` baseline writing mode is the left edge.
  fn synthesized_baseline_ascent(&self, item: &FlexItem, block_size: LayoutUnit) -> LayoutUnit {
    let baseline = if !self.is_column || self.direction() == Direction::Rtl {
      block_size
    } else {
      LayoutUnit::ZERO
    };

    self.ascent(item, baseline, block_size)
  }

  fn flex_margins(&self, margins: BoxStrut) -> FlexSides<LayoutUnit> {
    FlexSides::of(
      margins.top,
      margins.right,
      margins.bottom,
      margins.left,
      self.direction(),
      self.is_column,
    )
  }

  /// Blink's `ShouldApplyAutoMinSize`.
  fn should_apply_auto_min_size(&self, child_style: &LayoutStyle) -> bool {
    if self.is_webkit_box {
      return false;
    }

    let (overflow, min) = if self.is_horizontal_flow {
      (child_style.overflow.x, child_style.min_size.width)
    } else {
      (child_style.overflow.y, child_style.min_size.height)
    };

    overflow.is_clip_or_visible() && min.is_auto()
  }

  /// Blink's `BuildSpaceForIntrinsicInlineSize`.
  fn space_for_intrinsic_inline_size(
    &self,
    child_style: &LayoutStyle,
    alignment: ItemPosition,
  ) -> ConstraintSpace {
    let mut space = ConstraintSpace::new(None, child_style.direction);

    space.available_block_size = self.child_available_size.block_size;
    space.percentage_resolution_block_size = self.child_percentage_size.block_size;
    space.is_new_formatting_context = true;
    space.cache_slot = CacheSlot::Measure;
    if !self.is_column && !self.is_multi_line && alignment == ItemPosition::Stretch {
      space.block_auto_behavior = AutoSizeBehavior::StretchExplicit;
    }
    space
  }

  /// Blink's `BuildSpaceForFlexBasis`: a space only for resolving lengths.
  fn space_for_flex_basis(&self, child_style: &LayoutStyle) -> ConstraintSpace {
    let mut space =
      ConstraintSpace::new(self.child_available_size.inline_size, child_style.direction);

    space.available_block_size = self.child_available_size.block_size;
    space.percentage_resolution_inline_size = self.child_percentage_size.inline_size;
    space.percentage_resolution_block_size = self.child_percentage_size.block_size;
    space.is_new_formatting_context = true;
    space.cache_slot = CacheSlot::Measure;
    space
  }

  /// Blink's `BuildSpaceForLayout`.
  fn space_for_layout(
    &self,
    child_style: &LayoutStyle,
    alignment: ItemPosition,
    is_initial_block_size_indefinite: bool,
    override_inline_size: Option<LayoutUnit>,
    main_axis_final_size: Option<LayoutUnit>,
    line_cross_size: Option<LayoutUnit>,
  ) -> ConstraintSpace {
    let mut available_size = self.child_available_size;
    let mut percentage_size = self.child_percentage_size;
    let mut space = ConstraintSpace::new(None, child_style.direction);

    space.is_new_formatting_context = true;
    // Until the line has a cross size every layout is a measure.
    space.cache_slot = if line_cross_size.is_some() {
      self.space.cache_slot
    } else {
      CacheSlot::Measure
    };

    // Balancing into a minimum line count divides a definite cross size among the lines.
    if let Some(line_count) = self.balance_min_line_count {
      let gap_size = self.gap_between_lines * (line_count as i32 - 1);
      let divide = |size: LayoutUnit| (size - gap_size) / line_count as i32;

      if self.is_column {
        available_size.inline_size = available_size.inline_size.map(divide);
      } else {
        available_size.block_size = available_size.block_size.map(divide);
      }
    }

    if self.is_column {
      if let Some(inline_size) = override_inline_size {
        available_size.inline_size = Some(inline_size);
        space.is_fixed_inline_size = true;
      } else if let Some(line_cross_size) = line_cross_size {
        available_size.inline_size = Some(line_cross_size);
      }
      if let Some(block_size) = main_axis_final_size {
        available_size.block_size = Some(block_size);
        space.is_fixed_block_size = true;
      }
    } else {
      if let Some(line_cross_size) = line_cross_size {
        available_size.block_size = Some(line_cross_size);
      }
      if let Some(inline_size) = main_axis_final_size {
        available_size.inline_size = Some(inline_size);
        space.is_fixed_inline_size = true;
      }
    }

    let is_cross_size_definite =
      (!self.is_multi_line && self.is_cross_size_definite) || line_cross_size.is_some();

    if is_cross_size_definite && alignment == ItemPosition::Stretch {
      if self.is_column {
        space.inline_auto_behavior = AutoSizeBehavior::StretchExplicit;
      } else {
        space.block_auto_behavior = AutoSizeBehavior::StretchExplicit;
      }
    }

    if is_initial_block_size_indefinite {
      space.is_initial_block_size_indefinite = true;

      // A measure for a column takes no block constraints, which keeps the measure cache small.
      if main_axis_final_size.is_none() {
        available_size.block_size = None;
        percentage_size.block_size = None;
      }
    }

    space.available_inline_size = available_size.inline_size;
    space.available_block_size = available_size.block_size;
    space.percentage_resolution_inline_size = percentage_size.inline_size;
    space.percentage_resolution_block_size = percentage_size.block_size;
    space
  }

  /// The in-flow and out-of-flow children, in `order`, laying out empty the ones `display: none`
  /// hides.
  fn flex_children(&mut self, phase: Phase) -> Vec<NodeId> {
    let children = self.tree.child_nodes(self.node);
    let mut shown = Vec::with_capacity(children.len());

    for child in children {
      if self.tree.layout_style(child).display == Display::None {
        if phase == Phase::Layout && self.space.cache_slot == CacheSlot::Layout {
          self.tree.hide(child);
        }
        continue;
      }
      shown.push(child);
    }

    shown
  }

  /// The block size of `child` laid out in `child_space`, Blink's `BlockSizeFunc` in
  /// `ConstructAndAppendFlexItems`, keeping the layout it takes in `layout_result`.
  #[allow(clippy::too_many_arguments)]
  fn child_block_size(
    &mut self,
    child: NodeId,
    child_style: &LayoutStyle,
    child_space: &ConstraintSpace,
    flex_basis_space: &ConstraintSpace,
    border_padding: LogicalBoxStrut,
    size_type: SizeType,
    layout_result: &mut Option<Rc<LayoutResult>>,
  ) -> Option<LayoutUnit> {
    // A replaced item takes no aspect-ratio treatment of its own.
    if self.tree.is_replaced(child) {
      let natural = self.tree.natural_size(child);

      return Some(
        LengthResolver::new(child_space, child_style)
          .replaced_size(natural, ReplacedSizeMode::IgnoreBlockLengths)
          .block_size,
      );
    }

    if let (Some(aspect_ratio), SizeType::Content) = (child_style.aspect_ratio, size_type) {
      let inline_size = self.child_inline_size(child, child_style, child_space);

      return Some(block_size_from_aspect_ratio(
        border_padding,
        aspect_ratio,
        child_style.box_sizing_for_aspect_ratio(),
        inline_size,
      ));
    }

    let result = match layout_result {
      Some(result) => Rc::clone(result),
      None => {
        let result = self.tree.layout(child, child_space);

        *layout_result = Some(Rc::clone(&result));
        result
      }
    };
    let intrinsic_size = result.intrinsic_block_size;

    // The transferred min and max sizes hold the intrinsic size.
    if let Some(aspect_ratio) = child_style.aspect_ratio {
      let tree = &mut *self.tree;
      let inline_min_max = LengthResolver::new(flex_basis_space, child_style).min_max_inline_sizes(
        None,
        &mut |size_type| tree.min_max_sizes(child, child_space, size_type),
        false,
        FitContentMode::Normal,
        None,
      );
      let min_max = transferred_min_max_block_sizes(
        aspect_ratio,
        inline_min_max,
        border_padding,
        child_style.box_sizing_for_aspect_ratio(),
      );

      return Some(min_max.clamp_size_to_min_and_max(intrinsic_size));
    }

    Some(intrinsic_size)
  }

  /// Blink's `InlineSizeFunc`: the border-box inline size `child` takes in `child_space`.
  fn child_inline_size(
    &mut self,
    child: NodeId,
    child_style: &LayoutStyle,
    child_space: &ConstraintSpace,
  ) -> LayoutUnit {
    if self.tree.is_replaced(child) {
      let natural = self.tree.natural_size(child);

      return LengthResolver::new(child_space, child_style)
        .replaced_size(natural, ReplacedSizeMode::Normal)
        .inline_size;
    }

    let tree = &mut *self.tree;

    LengthResolver::new(child_space, child_style)
      .inline_size_for_fragment(&mut |size_type| tree.min_max_sizes(child, child_space, size_type))
  }

  /// Blink's `ConstructAndAppendFlexItems`: the items with their flex base and hypothetical main
  /// sizes, and the out-of-flow children.
  fn construct_and_append_flex_items(&mut self, phase: Phase) -> Vec<NodeId> {
    let children = self.flex_children(phase);
    let child_count = children.len();
    let mut out_of_flow = Vec::new();

    for child in children {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));

      if child_style.is_out_of_flow() {
        if phase == Phase::Layout {
          out_of_flow.push(child);
        }
        continue;
      }

      let flex_grow = child_style.flex_grow;
      let flex_shrink = child_style.flex_shrink;
      let alignment = self.resolved_align_self(&child_style, false);
      let mut max_content_contribution = None;

      if phase == Phase::ColumnWrapIntrinsicSize {
        let space = self.space_for_intrinsic_inline_size(&child_style, alignment);
        let mut contributions = self
          .tree
          .min_and_max_content_contribution(child, &space)
          .sizes;
        let margins = margins_for(
          &child_style,
          space.percentage_resolution_inline_size,
          self.direction(),
        );

        max_content_contribution = Some(contributions.max_size);
        contributions += margins.inline_sum();
        self.largest_min_content_contribution = self
          .largest_min_content_contribution
          .max(contributions.min_size);
      }

      // A horizontal writing mode keeps a row's main axis inline and a column's block.
      let is_main_axis_inline_axis = self.is_horizontal_flow;
      let flex_basis_space = self.space_for_flex_basis(&child_style);
      let physical_child_margins = {
        let margins = child_style
          .margin
          .map(|margin| margin.resolve_or_zero(flex_basis_space.percentage_resolution_inline_size));

        BoxStrut {
          top: margins.top,
          right: margins.right,
          bottom: margins.bottom,
          left: margins.left,
        }
      };
      let resolver = LengthResolver::new(&flex_basis_space, &child_style);
      let border_padding = resolver.border_padding;
      let main_axis_auto_margin_count = if self.is_horizontal_flow {
        u8::from(child_style.margin.left.is_auto()) + u8::from(child_style.margin.right.is_auto())
      } else {
        u8::from(child_style.margin.top.is_auto()) + u8::from(child_style.margin.bottom.is_auto())
      };
      let main_axis_border_padding = if self.is_horizontal_flow {
        border_padding.inline_sum()
      } else {
        border_padding.block_sum()
      };
      let child_space = self.space_for_layout(
        &child_style,
        alignment,
        self.is_column && !is_main_axis_inline_axis,
        max_content_contribution,
        None,
        None,
      );
      let is_used_flex_basis_indefinite = Cell::new(false);
      let mut layout_result = None;

      // Blink's `resolve_main_length`: a main-axis length, a content-based one marking the flex
      // basis indefinite.
      let mut resolve_main_length =
        |algorithm: &mut Self, length: LayoutLength, auto_length: Option<LayoutLength>| {
          if is_main_axis_inline_axis {
            let tree = &mut *algorithm.tree;
            let mut min_max = |size_type| {
              is_used_flex_basis_indefinite.set(true);
              tree.min_max_sizes(child, &child_space, size_type)
            };

            if let Some(inline_size) =
              resolver.resolve_main_inline_length(&mut min_max, length, auto_length, None)
            {
              return inline_size;
            }

            // An unresolvable length takes the max-content size.
            is_used_flex_basis_indefinite.set(true);
            return tree
              .min_max_sizes(child, &child_space, SizeType::Content)
              .sizes
              .max_size;
          }

          resolver
            .resolve_main_block_length(
              length,
              auto_length,
              &mut |size_type| {
                is_used_flex_basis_indefinite.set(true);
                algorithm.child_block_size(
                  child,
                  &child_style,
                  &child_space,
                  &flex_basis_space,
                  border_padding,
                  size_type,
                  &mut layout_result,
                )
              },
              None,
            )
            .unwrap_or(border_padding.block_sum())
        };

      let specified_length_in_main_axis = if self.is_horizontal_flow {
        child_style.size.width
      } else {
        child_style.size.height
      };

      // An `auto` flex basis takes the main size, an `auto` one of which is its max-content size,
      // or fit-content in a `-webkit-box` that is horizontal or does not stretch.
      let auto_flex_basis_length = if child_style.flex_basis.is_auto() {
        let fits_content = self.is_webkit_box
          && (!self.is_column
            || !matches!(
              self.style.align_items,
              AlignItems::Stretch | AlignItems::Normal
            ));
        let auto_size_length = if fits_content {
          LayoutLength::FitContent
        } else {
          LayoutLength::MaxContent
        };
        let mut size =
          resolve_main_length(self, specified_length_in_main_axis, Some(auto_size_length));

        if child_style.box_sizing == BoxSizing::ContentBox {
          size -= main_axis_border_padding;
        }
        Some(LayoutLength::Fixed(size.to_float()))
      } else {
        None
      };
      let base_border_size =
        resolve_main_length(self, child_style.flex_basis, auto_flex_basis_length);
      let base_content_size = base_border_size - main_axis_border_padding;

      let mut auto_min_length = None;

      if self.should_apply_auto_min_size(&child_style) {
        let specified_size_suggestion = if specified_length_in_main_axis.is_auto() {
          LayoutUnit::MAX
        } else if is_main_axis_inline_axis {
          let tree = &mut *self.tree;

          resolver
            .resolve_main_inline_length(
              &mut |size_type| tree.min_max_sizes(child, &child_space, size_type),
              specified_length_in_main_axis,
              None,
              None,
            )
            .unwrap_or(LayoutUnit::MAX)
        } else {
          resolver
            .resolve_main_block_length(
              specified_length_in_main_axis,
              None,
              &mut |size_type| {
                self.child_block_size(
                  child,
                  &child_style,
                  &child_space,
                  &flex_basis_space,
                  border_padding,
                  size_type,
                  &mut layout_result,
                )
              },
              None,
            )
            .unwrap_or(LayoutUnit::MAX)
        };

        // The automatic minimum only matters when the item shrinks, which it never does in some
        // cases this skips.
        let skips_content_size = specified_size_suggestion <= base_border_size
          && (flex_shrink == 0.0 || {
            let main_axis_content_size = self.main_axis_content_extent(None);
            let main_axis_margins = if self.is_horizontal_flow {
              physical_child_margins.horizontal_sum()
            } else {
              physical_child_margins.vertical_sum()
            };

            main_axis_content_size != LayoutUnit::MAX
              && specified_size_suggestion + main_axis_margins <= main_axis_content_size
              && (self.is_multi_line || child_count == 1)
          });
        let content_size_suggestion = if skips_content_size {
          LayoutUnit::MAX
        } else {
          let mut content_size = |algorithm: &mut Self, size_type: SizeType| {
            if is_main_axis_inline_axis {
              algorithm
                .tree
                .min_max_sizes(child, &child_space, size_type)
                .sizes
                .min_size
            } else {
              algorithm
                .child_block_size(
                  child,
                  &child_style,
                  &child_space,
                  &flex_basis_space,
                  border_padding,
                  size_type,
                  &mut layout_result,
                )
                .unwrap_or(LayoutUnit::ZERO)
            }
          };
          let size = content_size(self, SizeType::Content);

          // An aspect ratio's size encompasses the min-intrinsic size.
          if !self.tree.is_replaced(child) && child_style.aspect_ratio.is_some() {
            size.max(content_size(self, SizeType::Intrinsic))
          } else {
            size
          }
        };
        let mut auto_min_size = specified_size_suggestion.min(content_size_suggestion);

        if child_style.box_sizing == BoxSizing::ContentBox {
          auto_min_size -= main_axis_border_padding;
        }
        auto_min_length = Some(LayoutLength::Fixed(auto_min_size.to_float()));
      }

      let mut min_max_sizes_in_main_axis_direction = if is_main_axis_inline_axis {
        let tree = &mut *self.tree;

        resolver.min_max_inline_sizes(
          auto_min_length,
          &mut |size_type| tree.min_max_sizes(child, &child_space, size_type),
          false,
          FitContentMode::Normal,
          None,
        )
      } else {
        resolver.min_max_block_sizes(
          auto_min_length,
          &mut |size_type| {
            self.child_block_size(
              child,
              &child_style,
              &child_space,
              &flex_basis_space,
              border_padding,
              size_type,
              &mut layout_result,
            )
          },
          None,
        )
      };

      min_max_sizes_in_main_axis_direction -= main_axis_border_padding;

      // A column item's initial block size is indefinite when its container's main size and its
      // flex basis are, and no aspect ratio gives it.
      // https://drafts.csswg.org/css-flexbox/#definite-sizes
      let aspect_ratio_provides_block_main_size = !is_main_axis_inline_axis
        && !self.tree.is_replaced(child)
        && child_style.aspect_ratio.is_some();
      let is_initial_block_size_indefinite = self.is_column
        && !is_main_axis_inline_axis
        && self.child_available_size.block_size.is_none()
        && is_used_flex_basis_indefinite.get()
        && !aspect_ratio_provides_block_main_size;
      let baseline_group = if self.is_wrap_reverse {
        BaselineGroup::Minor
      } else {
        BaselineGroup::Major
      };
      self.flex_items.push(FlexItem::new(
        child,
        flex_grow,
        flex_shrink,
        base_content_size,
        min_max_sizes_in_main_axis_direction,
        main_axis_border_padding,
        max_content_contribution,
        physical_child_margins,
        main_axis_auto_margin_count,
        alignment,
        baseline_group,
        is_initial_block_size_indefinite,
        is_used_flex_basis_indefinite.get(),
        self.is_horizontal_flow,
      ));
    }

    out_of_flow
  }

  /// Blink's `PlaceFlexItems`: the items in lines, flexed, with each line's cross size, and the
  /// intrinsic block size.
  fn place_flex_items(&mut self, phase: Phase) -> (Vec<FlexLine>, Vec<NodeId>, LayoutUnit) {
    let out_of_flow = self.construct_and_append_flex_items(phase);
    let line_break_size = self.main_axis_content_extent(None);
    let result = break_flex_items_into_lines(
      &self.flex_items,
      line_break_size,
      self.gap_between_items,
      self.is_multi_line,
      self.balance_min_line_count,
    );
    // A column flexes all its lines to its intrinsic block size.
    let main_axis_inner_size =
      self.main_axis_content_extent(Some(result.max_sum_hypothetical_main_size));
    let definite_line_cross_size = self.definite_line_cross_size();
    let mut flex_lines = Vec::with_capacity(result.flex_lines.len());
    let mut sum_line_cross_size = LayoutUnit::ZERO;
    let mut start = 0;

    for line in &result.flex_lines {
      let range = start..start + line.count;

      start += line.count;
      LineFlexer::new(
        &mut self.flex_items[range.clone()],
        main_axis_inner_size,
        line.sum_hypothetical_main_size,
        self.gap_between_items,
      )
      .run();

      let mut main_axis_free_space =
        main_axis_inner_size - self.gap_between_items * (line.count as i32 - 1);
      let mut line_cross_size = LayoutUnit::ZERO;
      let mut max_major_ascent = LayoutUnit::MIN;
      let mut max_minor_ascent = LayoutUnit::MIN;
      let mut max_major_descent = LayoutUnit::MIN;
      let mut max_minor_descent = LayoutUnit::MIN;
      let mut main_axis_auto_margin_count = 0;

      for index in range.clone() {
        let item = self.flex_items[index].clone();

        main_axis_free_space -= item.flexed_margin_box_size();
        main_axis_auto_margin_count += u32::from(item.main_axis_auto_margin_count);

        let has_baseline_alignment = item.alignment == ItemPosition::Baseline;

        // A line with a definite cross size and no baselines needs no more.
        if !has_baseline_alignment && definite_line_cross_size.is_some() {
          continue;
        }

        let item_style = Rc::clone(self.tree.layout_style_rc(item.node));
        let space = self.space_for_layout(
          &item_style,
          item.alignment,
          item.is_initial_block_size_indefinite,
          item.max_content_contribution,
          Some(item.flexed_border_box_size()),
          None,
        );
        let mut layout_result = None;
        let cross_axis_size = if self.tree.is_replaced(item.node) {
          let natural = self.tree.natural_size(item.node);
          let size = LengthResolver::new(&space, &item_style)
            .replaced_size(natural, ReplacedSizeMode::Normal);

          if self.is_horizontal_flow {
            size.block_size
          } else {
            size.inline_size
          }
        } else if !self.is_horizontal_flow {
          self.child_inline_size(item.node, &item_style, &space)
        } else if phase == Phase::ColumnWrapIntrinsicSize {
          item.max_content_contribution.unwrap_or_default()
        } else {
          let result = self.tree.layout(item.node, &space);
          let size = result.size.block_size;

          layout_result = Some(result);
          size
        };
        let mut cross_axis_margin_size = cross_axis_size + item.cross_axis_margin_extent();

        if has_baseline_alignment {
          // Only a laid-out item has a baseline of its own; the rest synthesize one.
          let ascent = match &layout_result {
            Some(result) => self.baseline_ascent(&item, result),
            None => self.synthesized_baseline_ascent(&item, cross_axis_size),
          };
          let descent = cross_axis_margin_size - ascent;

          match item.baseline_group {
            BaselineGroup::Major => {
              max_major_ascent = max_major_ascent.max(ascent);
              max_major_descent = max_major_descent.max(descent);
              cross_axis_margin_size = max_major_ascent + max_major_descent;
            }
            BaselineGroup::Minor => {
              max_minor_ascent = max_minor_ascent.max(ascent);
              max_minor_descent = max_minor_descent.max(descent);
              cross_axis_margin_size = max_minor_ascent + max_minor_descent;
            }
          }
        }
        line_cross_size = line_cross_size.max(cross_axis_margin_size);
      }

      let line_cross_size = definite_line_cross_size.unwrap_or(line_cross_size);

      flex_lines.push(FlexLine {
        item_indices: range.collect(),
        main_axis_free_space,
        line_cross_size,
        major_baseline: max_major_ascent,
        minor_baseline: max_minor_ascent,
        main_axis_auto_margin_count,
        ..FlexLine::default()
      });
      sum_line_cross_size += line_cross_size;
    }

    let mut total_intrinsic_block_size = self.border_padding.block_sum();

    if !flex_lines.is_empty() {
      if self.is_column {
        total_intrinsic_block_size += result.max_sum_hypothetical_main_size;
      } else {
        total_intrinsic_block_size +=
          sum_line_cross_size + self.gap_between_lines * (flex_lines.len() as i32 - 1);
      }
    }

    (flex_lines, out_of_flow, total_intrinsic_block_size)
  }

  /// The cross size a single line takes when its container's is definite, from `PlaceFlexItems`.
  fn definite_line_cross_size(&self) -> Option<LayoutUnit> {
    if self.is_multi_line {
      return None;
    }

    let cross_available_size = if self.is_column {
      self.child_available_size.inline_size
    } else {
      self.child_available_size.block_size
    }?;

    if !self.is_column {
      let style = &self.style;

      // The automatic minimum of an aspect ratio, or a content-based min or max block size,
      // leaves the block size indefinite. https://github.com/w3c/csswg-drafts/issues/12123
      if style.aspect_ratio.is_some()
        && style.overflow.y.is_clip_or_visible()
        && style.min_size.height.is_auto()
      {
        return None;
      }
      if style.min_size.height.is_content_or_intrinsic()
        || style.max_size.height.is_content_or_intrinsic()
      {
        return None;
      }
    }

    Some(cross_available_size)
  }

  /// Blink's `ApplyReversals`.
  fn apply_reversals(&self, flex_lines: &mut [FlexLine]) {
    if self.is_wrap_reverse {
      flex_lines.reverse();
    }

    if self.is_reverse_direction {
      for line in flex_lines {
        line.item_indices.reverse();
      }
    }
  }

  /// Blink's `GiveItemsFinalPositionAndSize`.
  fn give_items_final_position_and_size(&mut self, flex_lines: &mut [FlexLine]) {
    if flex_lines.is_empty() {
      return;
    }

    let inline_size = self.inline_size.unwrap_or_default();
    let justify_content = self.resolved_justify_content();
    let align_content = ContentAlignment::of(self.style.align_content);
    let num_lines = flex_lines.len();
    let cross_axis_content_size = if self.is_column {
      inline_size - self.border_padding.inline_sum()
    } else {
      self.total_block_size - self.border_padding.block_sum()
    }
    .clamp_negative_to_zero();
    let mut cross_axis_free_space = cross_axis_content_size
      - flex_lines
        .iter()
        .fold(LayoutUnit::ZERO, |sum, line| sum + line.line_cross_size)
      - self.gap_between_lines * (num_lines as i32 - 1);
    let is_align_content_stretch = align_content.distribution == ContentDistribution::Stretch
      || (align_content.position == ContentPosition::Normal
        && align_content.distribution == ContentDistribution::Default);

    if !self.is_multi_line {
      // A single line takes the cross-axis content size.
      if let Some(line) = flex_lines.last_mut() {
        line.line_cross_size = cross_axis_content_size;
      }
      cross_axis_free_space = LayoutUnit::ZERO;
    } else if cross_axis_free_space >= LayoutUnit::ZERO && is_align_content_stretch {
      // Multiple lines stretch into the free space.
      let mut extra = LayoutUnitDiffuser::new(cross_axis_free_space, num_lines);

      for line in flex_lines.iter_mut() {
        line.line_cross_size += extra.next();
      }
      cross_axis_free_space = LayoutUnit::ZERO;
    }

    let mut space_between_lines =
      align_content.distribution_space(cross_axis_free_space, num_lines);
    let mut line_cross_axis_offset =
      if self.is_column {
        self.border_padding.inline_start
      } else {
        self.border_padding.block_start
      } + align_content.initial_offset(cross_axis_free_space, num_lines, self.is_wrap_reverse);
    let mut baselines = BaselineAccumulator::default();
    let direction = self.direction();

    for (line_index, flex_line) in flex_lines.iter_mut().enumerate() {
      let is_first_line = line_index == 0;
      let is_last_line = line_index == num_lines - 1;

      flex_line.cross_axis_offset = line_cross_axis_offset;

      if !self.is_column {
        baselines.accumulate_line(flex_line, is_first_line, is_last_line);
      }

      let line = flex_line.clone();
      let applies_main_axis_auto_margin =
        line.main_axis_auto_margin_count > 0 && line.main_axis_free_space > LayoutUnit::ZERO;
      let main_axis_free_space = if applies_main_axis_auto_margin {
        LayoutUnit::ZERO
      } else {
        line.main_axis_free_space
      };
      let mut main_axis_auto_margin = if applies_main_axis_auto_margin {
        LayoutUnitDiffuser::new(
          line.main_axis_free_space,
          line.main_axis_auto_margin_count as usize,
        )
      } else {
        LayoutUnitDiffuser::default()
      };
      let line_items_size = line.item_indices.len();
      let mut space_between_items =
        justify_content.distribution_space(main_axis_free_space, line_items_size);
      let mut main_axis_offset = if self.is_column {
        self.border_padding.block_start
      } else {
        self.border_padding.inline_start
      } + justify_content.initial_offset(
        main_axis_free_space,
        line_items_size,
        self.is_reverse_direction,
      );

      for &item_index in &line.item_indices {
        let item = self.flex_items[item_index].clone();
        let item_style = Rc::clone(self.tree.layout_style_rc(item.node));
        let child_space = self.space_for_layout(
          &item_style,
          item.alignment,
          item.is_initial_block_size_indefinite,
          None,
          Some(item.flexed_border_box_size()),
          Some(line.line_cross_size),
        );
        let result = self.tree.layout(item.node, &child_space);
        let cross_axis_size = if self.is_column {
          result.size.inline_size
        } else {
          result.size.block_size
        };
        let mut margin = self.flex_margins(item.initial_margins);
        let cross_axis_space =
          line.line_cross_size - margin.cross_start - cross_axis_size - margin.cross_end;
        let is_margin_auto = FlexSides::of(
          item_style.margin.top.is_auto(),
          item_style.margin.right.is_auto(),
          item_style.margin.bottom.is_auto(),
          item_style.margin.left.is_auto(),
          direction,
          self.is_column,
        );

        // Cross-axis auto margins take the space as usual.
        let margin_space = cross_axis_space.clamp_negative_to_zero();

        match (is_margin_auto.cross_start, is_margin_auto.cross_end) {
          (true, true) => {
            margin.cross_start = margin_space / 2;
            margin.cross_end = margin_space / 2;
          }
          (true, false) => margin.cross_start = margin_space,
          (false, true) => margin.cross_end = margin_space,
          (false, false) => {}
        }

        // Main-axis auto margins share the line's free space.
        if is_margin_auto.main_start {
          margin.main_start = main_axis_auto_margin.next();
        }
        if is_margin_auto.main_end {
          margin.main_end = main_axis_auto_margin.next();
        }

        let space = if self.is_align_self_safe(&item_style) {
          cross_axis_space.clamp_negative_to_zero()
        } else {
          cross_axis_space
        };
        let offset_in_line = match item.alignment {
          ItemPosition::Center => space / 2,
          ItemPosition::FlowStart => LayoutUnit::ZERO,
          ItemPosition::FlowEnd => space,
          ItemPosition::Stretch => {
            if self.is_wrap_reverse {
              space
            } else {
              LayoutUnit::ZERO
            }
          }
          ItemPosition::Baseline => {
            let is_major = item.baseline_group == BaselineGroup::Major;
            let ascent = self.baseline_ascent(&item, &result);
            let max_ascent = if is_major {
              line.major_baseline
            } else {
              line.minor_baseline
            };
            let baseline_delta = max_ascent - ascent;

            if is_major {
              baseline_delta
            } else {
              space - baseline_delta
            }
          }
        };
        let cross_axis_offset = line_cross_axis_offset + offset_in_line + margin.cross_start;

        main_axis_offset += margin.main_start;

        let offset = if self.is_column {
          LogicalOffset::new(cross_axis_offset, main_axis_offset)
        } else {
          LogicalOffset::new(main_axis_offset, cross_axis_offset)
        };

        main_axis_offset += item.flexed_border_box_size()
          + margin.main_end
          + space_between_items.next()
          + self.gap_between_items;

        baselines.accumulate_item(&result, offset.block_offset, is_first_line, is_last_line);
        self.add_result(item.node, result, offset);
      }

      line_cross_axis_offset +=
        line.line_cross_size + space_between_lines.next() + self.gap_between_lines;
    }

    self.first_baseline = baselines.first_baseline();
    self.last_baseline = baselines.last_baseline();
  }

  /// The border-box location of a child `child_inline_size` wide at the logical `offset`.
  fn physical_offset(
    &self,
    offset: LogicalOffset,
    child_inline_size: LayoutUnit,
  ) -> Point<LayoutUnit> {
    let x = match self.direction() {
      Direction::Ltr => offset.inline_offset,
      Direction::Rtl => {
        self.inline_size.unwrap_or_default() - offset.inline_offset - child_inline_size
      }
    };

    Point::new(x, offset.block_offset)
  }

  /// Blink's `BoxFragmentBuilder::AddResult`: the item moved by `position: relative`, its
  /// out-of-flow boxes along with it.
  fn add_result(&mut self, node: NodeId, result: Rc<LayoutResult>, offset: LogicalOffset) {
    let relative_offset = compute_relative_offset(
      self.tree.render_node(node),
      self.direction(),
      self
        .child_available_size
        .inline_size
        .map(LayoutUnit::to_float),
      self
        .child_available_size
        .block_size
        .map(LayoutUnit::to_float),
    );
    let location = self.physical_offset(offset, result.size.inline_size)
      + relative_offset.map(LayoutUnit::from_float);

    self.out_of_flow.extend(
      result
        .out_of_flow
        .iter()
        .map(|candidate| candidate.moved_by(location)),
    );
    self.children.push(PlacedChild {
      node,
      location,
      result,
    });
  }

  /// Blink's `HandleOutOfFlowPositionedItems`: each out-of-flow child's static position in the
  /// content box, aligned by `justify-content` and its own `align-self`.
  fn handle_out_of_flow_positioned_items(&mut self, children: Vec<NodeId>) {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum AxisEdge {
      Start,
      Center,
      End,
    }

    let inline_size = self.inline_size.unwrap_or_default();
    let content_size = LogicalSize::new(
      (inline_size - self.border_padding.inline_sum()).clamp_negative_to_zero(),
      (self.total_block_size - self.border_padding.block_sum()).clamp_negative_to_zero(),
    );
    let justify_content = self.resolved_justify_content();
    // Blink's `MainAxisStaticPositionEdge`.
    let main_axis_edge = if justify_content.position == ContentPosition::FlowEnd {
      if self.is_reverse_direction {
        AxisEdge::Start
      } else {
        AxisEdge::End
      }
    } else if justify_content.position == ContentPosition::Center
      || matches!(
        justify_content.distribution,
        ContentDistribution::SpaceAround | ContentDistribution::SpaceEvenly
      )
    {
      AxisEdge::Center
    } else if justify_content.position == ContentPosition::Start {
      AxisEdge::Start
    } else if justify_content.position == ContentPosition::End || self.is_reverse_direction {
      AxisEdge::End
    } else {
      AxisEdge::Start
    };

    for child in children {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));
      let position = self.resolved_align_self(&child_style, true);
      // Blink's `CrossAxisStaticPositionEdge`: `stretch` stays at the start, which `wrap-reverse`
      // moves to the end.
      let cross_axis_edge = match position {
        ItemPosition::Stretch if self.is_wrap_reverse => AxisEdge::End,
        ItemPosition::FlowEnd => AxisEdge::End,
        ItemPosition::Center => AxisEdge::Center,
        _ => AxisEdge::Start,
      };
      let (inline_axis_edge, block_axis_edge) = if self.is_column {
        (cross_axis_edge, main_axis_edge)
      } else {
        (main_axis_edge, cross_axis_edge)
      };
      let mut static_position = LogicalStaticPosition {
        offset: LogicalOffset::new(
          self.border_padding.inline_start,
          self.border_padding.block_start,
        ),
        align_self_direction: if self.is_column {
          LogicalAlignmentDirection::Inline
        } else {
          LogicalAlignmentDirection::Block
        },
        ..LogicalStaticPosition::default()
      };

      match block_axis_edge {
        AxisEdge::Start => static_position.block_edge = BlockEdge::Start,
        AxisEdge::Center => {
          static_position.block_edge = BlockEdge::Center;
          static_position.offset.block_offset += content_size.block_size / 2;
        }
        AxisEdge::End => {
          static_position.block_edge = BlockEdge::End;
          static_position.offset.block_offset += content_size.block_size;
        }
      }

      match inline_axis_edge {
        AxisEdge::Start => static_position.inline_edge = InlineEdge::Start,
        AxisEdge::Center => {
          static_position.inline_edge = InlineEdge::Center;
          static_position.offset.inline_offset += content_size.inline_size / 2;
        }
        AxisEdge::End => {
          static_position.inline_edge = InlineEdge::End;
          static_position.offset.inline_offset += content_size.inline_size;
        }
      }

      self.out_of_flow.push(OutOfFlowCandidate {
        node: child,
        static_position: static_position.to_physical(self.direction(), inline_size),
      });
    }
  }

  /// Blink's `Layout`.
  pub(crate) fn layout(mut self) -> LayoutResult {
    let (mut flex_lines, out_of_flow, total_intrinsic_block_size) =
      self.place_flex_items(Phase::Layout);
    let inline_size = self.inline_size.unwrap_or_default();
    let style = Rc::clone(&self.style);
    let resolver = LengthResolver::new(self.space, &style);

    self.total_block_size = resolver
      .block_size_for_fragment(Some(total_intrinsic_block_size), Some(inline_size), None)
      .unwrap_or(total_intrinsic_block_size);
    self.apply_reversals(&mut flex_lines);
    self.give_items_final_position_and_size(&mut flex_lines);
    self.handle_out_of_flow_positioned_items(out_of_flow);

    let size = LogicalSize::new(inline_size, self.total_block_size);
    let mut result = LayoutResult::of_box(size, &style, self.space);

    if !self.out_of_flow.is_empty() {
      let claims = self.tree.claims(self.node);
      let candidates = take(&mut self.out_of_flow);
      let containing_block = ContainingBlock::padding_box(
        Size {
          width: size.inline_size,
          height: size.block_size,
        },
        style.border,
      );

      self.out_of_flow =
        self
          .tree
          .lay_out_out_of_flow(self.node, claims, candidates, containing_block, self.space);
    }

    if self.space.cache_slot == CacheSlot::Layout {
      for child in take(&mut self.children) {
        self
          .tree
          .place_child(child.node, 0, 0, child.location, &child.result);
      }
    }

    result.intrinsic_block_size = total_intrinsic_block_size;
    result.bfc_line_offset = self.space.bfc_offset.line_offset;
    result.bfc_block_offset = Some(self.space.bfc_offset.block_offset);
    result.first_baseline = self.first_baseline;
    result.last_baseline = self.last_baseline;
    result.out_of_flow = self.out_of_flow;
    result
  }

  /// Blink's `ComputeMinMaxSizes`.
  pub(crate) fn compute_min_max_sizes(mut self) -> MinMaxSizesResult {
    if !self.is_column {
      return self.compute_min_max_size_of_row_container();
    }

    if self.is_multi_line {
      return self.compute_min_max_size_of_multiline_column_container();
    }

    // A single column takes its widest item.
    let mut sizes = MinMaxSizes::default();
    let mut depends_on_block_constraints = false;

    for child in self.flex_children(Phase::RowIntrinsicSize) {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));

      if child_style.is_out_of_flow() {
        continue;
      }

      let alignment = self.resolved_align_self(&child_style, false);
      let space = self.space_for_intrinsic_inline_size(&child_style, alignment);
      let mut child_result = self.tree.min_and_max_content_contribution(child, &space);
      let margins = margins_for(
        &child_style,
        space.percentage_resolution_inline_size,
        self.direction(),
      );

      child_result.sizes += margins.inline_sum();
      depends_on_block_constraints |= child_result.depends_on_block_constraints;
      sizes.min_size = sizes.min_size.max(child_result.sizes.min_size);
      sizes.max_size = sizes.max_size.max(child_result.sizes.max_size);
    }

    sizes.max_size = sizes.max_size.max(sizes.min_size);
    sizes.encompass(LayoutUnit::ZERO);
    sizes += self.border_padding.inline_sum();
    MinMaxSizesResult::new(sizes, depends_on_block_constraints)
  }

  /// Blink's `ComputeMinMaxSizeOfMultilineColumnContainer`: the items laid out at the widest
  /// max-content width of any of them, as wide as their lines reach.
  fn compute_min_max_size_of_multiline_column_container(mut self) -> MinMaxSizesResult {
    let (flex_lines, _, _) = self.place_flex_items(Phase::ColumnWrapIntrinsicSize);
    let mut sizes = MinMaxSizes {
      min_size: self.largest_min_content_contribution,
      max_size: LayoutUnit::ZERO,
    };

    if !flex_lines.is_empty() {
      for line in &flex_lines {
        sizes.max_size += line.line_cross_size;
      }
      sizes.max_size += self.gap_between_lines * (flex_lines.len() as i32 - 1);
    }

    sizes.max_size = sizes.max_size.max(sizes.min_size);
    sizes += self.border_padding.inline_sum();

    // Block constraints may change how many columns it has.
    MinMaxSizesResult::new(sizes, true)
  }

  /// Blink's `ComputeMinMaxSizeOfRowContainer`.
  fn compute_min_max_size_of_row_container(mut self) -> MinMaxSizesResult {
    let mut container_sizes = MinMaxSizes::default();
    let mut depends_on_block_constraints = false;

    self.construct_and_append_flex_items(Phase::RowIntrinsicSize);

    // Balancing breaks lines to find the sizes; otherwise every item shares one line.
    let flex_lines = if self.balance_min_line_count.is_some() {
      break_flex_items_into_lines(
        &self.flex_items,
        LayoutUnit::MAX,
        self.gap_between_items,
        self.is_multi_line,
        self.balance_min_line_count,
      )
      .flex_lines
      .into_iter()
      .map(|line| line.count)
      .collect()
    } else {
      vec![self.flex_items.len()]
    };
    let mut largest_outer_min_content_contribution = LayoutUnit::ZERO;
    let mut start = 0;

    for count in flex_lines {
      let mut line_sizes = MinMaxSizes::default();

      for index in start..start + count {
        let item = self.flex_items[index].clone();
        let child_style = Rc::clone(self.tree.layout_style_rc(item.node));
        let space = self.space_for_intrinsic_inline_size(&child_style, item.alignment);
        let contributions = self
          .tree
          .min_and_max_content_contribution(item.node, &space);

        depends_on_block_constraints |= contributions.depends_on_block_constraints;

        let flex_base_size_border_box = item.base_content_size + item.main_axis_border_padding;
        let hypothetical_main_size_border_box =
          item.hypothetical_content_size + item.main_axis_border_padding;
        let main_axis_margins = item.initial_margins.horizontal_sum();
        // An item that cannot grow or shrink toward its contribution keeps its hypothetical size.
        let cant_move = |contribution: LayoutUnit| {
          (contribution > flex_base_size_border_box && item.flex_grow == 0.0)
            || (contribution < flex_base_size_border_box && item.flex_shrink == 0.0)
        };
        let final_contribution = |contribution: LayoutUnit| {
          if cant_move(contribution) && !item.is_used_flex_basis_indefinite {
            hypothetical_main_size_border_box
          } else {
            contribution
          }
        };
        let mut item_final_contribution = MinMaxSizes::default();

        if self.is_multi_line {
          largest_outer_min_content_contribution = largest_outer_min_content_contribution
            .max(contributions.sizes.min_size + main_axis_margins);
        } else {
          item_final_contribution.min_size = final_contribution(contributions.sizes.min_size);
        }
        item_final_contribution.max_size = final_contribution(contributions.sizes.max_size);

        line_sizes.min_size += item_final_contribution.min_size + main_axis_margins;
        line_sizes.max_size += item_final_contribution.max_size + main_axis_margins;
      }

      if count > 0 {
        line_sizes += self.gap_between_items * (count as i32 - 1);
      }
      container_sizes.min_size = container_sizes.min_size.max(line_sizes.min_size);
      container_sizes.max_size = container_sizes.max_size.max(line_sizes.max_size);
      start += count;
    }

    // A wrapping container puts each item on a line of its own.
    if self.is_multi_line {
      container_sizes.min_size = largest_outer_min_content_contribution;
    }

    container_sizes.max_size = container_sizes.max_size.max(container_sizes.min_size);
    container_sizes.encompass(LayoutUnit::ZERO);
    container_sizes += self.border_padding.inline_sum();
    MinMaxSizesResult::new(container_sizes, depends_on_block_constraints)
  }
}

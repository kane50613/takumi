//! Blink's [`LineBreaker`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_breaker.cc),
//! without ruby, SVG text, block-in-inline, initial letters, `text-combine-upright`, hyphenation
//! dictionaries, `word-break: auto-phrase`, block fragmentation, or `::first-line`.

use std::rc::Rc;

use icu_properties::{CodePointMapData, props::BidiClass};

use super::{
  inline_break_token::InlineBreakToken,
  inline_item::{CollapseType, InlineItem, InlineItemType, TextItemType},
  inline_item_result::{
    HyphenResult, InlineItemResult, InlineItemResults, LayoutResult, LineBoxStrut,
  },
  inline_item_text_index::InlineItemTextIndex,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  leading_floats::LeadingFloats,
  line_break_candidate::{CandidateState, LineBreakCandidateContext},
  line_break_point::LineBreakPoint,
  line_info::LineInfo,
};
use crate::{
  context::RenderContext,
  fonts::shaping::{
    harfbuzz_shaper::HarfBuzzShaper,
    shape_result::ShapeResult,
    shape_result_spacing::ShapeResultSpacing,
    shape_result_view::{Segment, SegmentSource, ShapeResultView},
    shaping_line_breaker::{ShapingLineBreaker, ShapingLineBreakerResult},
  },
  layout::{
    bfc_rect::BfcOffset,
    constraint_space::ConstraintSpace,
    exclusions::{exclusion_space::ExclusionSpace, layout_opportunity::LineLayoutOpportunity},
    floats_utils::{
      UnpositionedFloat, compute_margin_box_inline_size_for_unpositioned_float, position_float,
    },
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::{
    BoxDecorationBreak, ComputedStyle, Direction as TextDirection, Hyphens, LineBreak, UnicodeBidi,
    WordBreak,
  },
  text::{
    character::{
      CARRIAGE_RETURN, FORM_FEED, LINE_FEED, OBJECT_REPLACEMENT_CHARACTER,
      POP_DIRECTIONAL_FORMATTING, POP_DIRECTIONAL_ISOLATE, SOFT_HYPHEN, TAB, ZERO_WIDTH_SPACE,
      char_at, char_before, is_other_space_separator,
    },
    line_break_iterator::{
      BreakOptions, BreakSpaceType, LineBreakIterator, LineBreakStrictness, LineBreakType,
    },
  },
};

/// Blink's `ShapeResultWrapper` in `AppendCandidates`: positions in the candidates' coordinates.
struct ShapeResultPositions<'s> {
  shape_result: &'s ShapeResult,
  start_index: u32,
  base_position: f32,
  is_ltr: bool,
}

impl<'s> ShapeResultPositions<'s> {
  fn new(shape_result: &'s ShapeResult) -> Self {
    shape_result.ensure_position_data();

    Self {
      shape_result,
      start_index: shape_result.start_index,
      base_position: 0.0,
      is_ltr: !shape_result.is_rtl(),
    }
  }

  fn position_for_offset(&self, offset: usize) -> f32 {
    let position = self
      .shape_result
      .cached_position_for_offset(offset as u32 - self.start_index)
      .to_float();

    if self.is_ltr {
      self.base_position + position
    } else {
      self.base_position - position
    }
  }

  /// Moves the shape result's coordinates so `offset` sits at `adjusted`.
  fn set_base_position(&mut self, offset: usize, adjusted: f32) {
    let position = self
      .shape_result
      .cached_position_for_offset(offset as u32 - self.start_index)
      .to_float();

    self.base_position = if self.is_ltr {
      adjusted - position
    } else {
      adjusted + position
    };
  }
}

/// Blink's `LineBreakerMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineBreakerMode {
  Content,
  MinContent,
  MaxContent,
}

/// Lays out an atomic inline or a block-in-inline for a line: Blink's `LayoutAtomicInline`,
/// `BlockNode::Layout`, and `ComputeMinAndMaxContentContribution`.
pub(crate) trait AtomicInlineLayout<'c> {
  /// The box laid out in the line's available space.
  fn layout(&mut self, node: &'c RenderNode) -> LayoutResult;

  /// The block-in-inline laid out in the containing block's available space.
  fn layout_block_in_inline(&mut self, node: &'c RenderNode) -> LayoutResult;

  /// The box's min-content and max-content contributions, without margins.
  fn min_max_sizes(&mut self, node: &'c RenderNode) -> (LayoutUnit, LayoutUnit);
}

/// Blink's `WhitespaceState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhitespaceState {
  Leading,
  None,
  Unknown,
  Collapsible,
  Collapsed,
  Preserved,
}

/// Blink's `LineBreakState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineBreakState {
  /// The line breaking is complete.
  Done,
  /// Overflow without an earlier break opportunity: break at the earliest one.
  Overflow,
  /// Only trailing spaces, `<br>`, or close tags may join the line.
  Trailing,
  /// Looking for more items to fit.
  Continue,
}

/// Blink's `BreakResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakResult {
  Success,
  Overflow,
  BreakAt,
}

/// Blink's `TrailingCollapsibleSpace`.
#[derive(Debug, Clone)]
struct TrailingCollapsibleSpace {
  item_result_index: usize,
  collapsed_shape_result: Option<Rc<ShapeResultView>>,
}

/// Blink's `ShouldApplyTextIndent`.
fn should_apply_text_indent(
  context: &RenderContext,
  is_first_formatted_line: bool,
  is_after_forced_break: bool,
) -> bool {
  let text_indent = &context.style.misc_inherited1_data.text_indent;

  if text_indent.amount.to_px(&context.sizing, 1.0) == 0.0 {
    return false;
  }

  let is_first_line = is_first_formatted_line || (is_after_forced_break && text_indent.each_line);

  if text_indent.hanging {
    !is_first_line
  } else {
    is_first_line
  }
}

/// Blink's `IsBreakableSpace`: CSS white space, without the newline a line never breaks before.
fn is_breakable_space(c: char) -> bool {
  c == ' ' || c == TAB
}

/// Blink's `IsBreakableSpaceOrOtherSeparator`.
fn is_breakable_space_or_other_separator(c: char) -> bool {
  is_breakable_space(c) || is_other_space_separator(c)
}

/// Blink's `IsAllBreakableSpaces`.
fn is_all_breakable_spaces(text: &str, start: usize, end: usize) -> bool {
  text[start..end].chars().all(is_breakable_space)
}

/// Blink's `IsBidiTrailingSpace`: a white space neutral.
fn is_bidi_trailing_space(c: char) -> bool {
  CodePointMapData::<BidiClass>::new().get(c) == BidiClass::WhiteSpace
}

/// Blink's `IsTrailableItemType`: items that may follow the soft wrap point on its line.
fn is_trailable_item_type(item_type: InlineItemType) -> bool {
  !matches!(
    item_type,
    InlineItemType::AtomicInline | InlineItemType::OutOfFlowPositioned | InlineItemType::ListMarker
  )
}

fn can_break_after_last(item_results: &InlineItemResults<'_, '_>) -> bool {
  item_results
    .last()
    .is_some_and(|result| result.can_break_after)
}

fn should_create_line_box(item_results: &InlineItemResults<'_, '_>) -> bool {
  item_results
    .last()
    .is_some_and(|result| result.should_create_line_box)
}

fn has_unpositioned_floats(item_results: &InlineItemResults<'_, '_>) -> bool {
  item_results
    .last()
    .is_some_and(|result| result.has_unpositioned_floats)
}

/// Blink's `ComputeLineMarginsForSelf`.
fn line_margins(node: &RenderNode) -> LineBoxStrut {
  LineBoxStrut::from_physical(
    node.margin_px(),
    node.context.style.inherited_data.direction,
  )
}

/// Blink's `ComputeLineBorders`.
fn line_borders(node: &RenderNode) -> LineBoxStrut {
  LineBoxStrut::from_physical(
    node.border_px(),
    node.context.style.inherited_data.direction,
  )
}

/// Blink's `ComputeLinePadding`.
fn line_padding(node: &RenderNode) -> LineBoxStrut {
  LineBoxStrut::from_physical(
    node.padding_px(),
    node.context.style.inherited_data.direction,
  )
}

/// Blink's `ComputeInlineEndSize`.
fn compute_inline_end_size(node: &RenderNode) -> LayoutUnit {
  line_margins(node).inline_end + line_borders(node).inline_end + line_padding(node).inline_end
}

/// Blink's `NeedsAccurateEndPosition`: the line's end paints, so it must not be approximate.
fn item_needs_accurate_end_position(item: &InlineItem<'_>) -> bool {
  debug_assert!(matches!(
    item.item_type,
    InlineItemType::Text | InlineItemType::Control
  ));

  item.layout_object.is_some_and(|object| {
    let node = object.node();

    !node.context.text_decorations.as_slice().is_empty() || node.has_box_decoration_background()
  })
}

/// Blink's `ComputedStyle::ShouldBreakOnlyAfterWhiteSpace`, with `line-break` always `auto`.
fn should_break_only_after_white_space(style: &ComputedStyle) -> bool {
  style
    .inherited_data
    .white_space_collapse
    .should_preserve_white_spaces()
    && style.inherited_data.text_wrap_mode.should_wrap_line()
}

/// Blink's `FastMinTextContext`: the widest word of a text item.
#[derive(Default)]
struct FastMinTextContext {
  min_inline_size: LayoutUnit,
  hyphen_inline_size: Option<LayoutUnit>,
}

impl FastMinTextContext {
  fn hyphen_inline_size(
    &mut self,
    item_result: &mut InlineItemResult<'_, '_>,
    items_data: &InlineNodeData<'_>,
  ) -> LayoutUnit {
    *self.hyphen_inline_size.get_or_insert_with(|| {
      if item_result.hyphen.is_none() {
        item_result.shape_hyphen(&items_data.fonts);
      }

      item_result
        .hyphen
        .as_ref()
        .map_or(LayoutUnit::ZERO, HyphenResult::inline_size)
    })
  }

  /// Blink's `Add` of the width between `start_offset` and `end_offset`.
  fn add(
    &mut self,
    shape_result: &ShapeResult,
    start_offset: usize,
    end_offset: usize,
    has_hyphen: bool,
    item_result: &mut InlineItemResult<'_, '_>,
    items_data: &InlineNodeData<'_>,
  ) {
    let mut width = shape_result.cached_width(start_offset as u32, end_offset as u32);

    if has_hyphen {
      width += self.hyphen_inline_size(item_result, items_data);
    }

    self.min_inline_size = self.min_inline_size.max(width);
  }
}

/// Blink's `LineBreaker`: measures each item and decides which form a line.
pub(crate) struct LineBreaker<'a, 'b, 'c> {
  node: InlineNode<'c>,
  items_data: &'a InlineNodeData<'c>,
  mode: LineBreakerMode,
  constraint_space: ConstraintSpace,
  atomic_inline: &'b mut dyn AtomicInlineLayout<'c>,
  exclusion_space: &'b mut ExclusionSpace,
  leading_floats: &'b LeadingFloats,
  leading_floats_index: usize,
  state: LineBreakState,
  current: InlineItemTextIndex,
  break_at: LineBreakPoint,
  pub(crate) trailing_whitespace: WhitespaceState,
  initial_whitespace: WhitespaceState,
  /// The position from the inline start, in logical order.
  position: LayoutUnit,
  applied_text_indent: LayoutUnit,
  available_width: LayoutUnit,
  base_available_width: LayoutUnit,
  line_opportunity: LineLayoutOpportunity,
  is_first_formatted_line: bool,
  auto_wrap: bool,
  break_anywhere_if_overflow: bool,
  override_break_anywhere: bool,
  pub(crate) disable_score_line_break: bool,
  pub(crate) disable_bisect_line_break: bool,
  disable_trailing_whitespace_collapsing: bool,
  force_non_empty_if_last_line: bool,
  is_forced_break: bool,
  previous_line_had_forced_break: bool,
  end_item_index: usize,
  break_token: Option<InlineBreakToken<'c>>,
  current_style: Option<&'c RenderContext>,
  break_iterator: Rc<LineBreakIterator<'a>>,
  shaper: Rc<HarfBuzzShaper>,
  hyphen_index: Option<usize>,
  has_any_hyphens: bool,
  trailing_collapsible_space: Option<TrailingCollapsibleSpace>,
  override_available_width: LayoutUnit,
  line_clamp_ellipsis_width: LayoutUnit,
  cloned_box_decorations_count: usize,
  cloned_box_decorations_initial_size: LayoutUnit,
  cloned_box_decorations_end_size: LayoutUnit,
  pub(crate) has_cloned_box_decorations: bool,
  base_direction: TextDirection,
  last_rewind: Option<(usize, usize)>,
}

impl<'a, 'b, 'c> LineBreaker<'a, 'b, 'c> {
  #[expect(clippy::too_many_arguments, reason = "Blink's constructor takes these")]
  pub(crate) fn new(
    node: InlineNode<'c>,
    items_data: &'a InlineNodeData<'c>,
    mode: LineBreakerMode,
    constraint_space: ConstraintSpace,
    line_opportunity: LineLayoutOpportunity,
    leading_floats: &'b LeadingFloats,
    break_token: Option<InlineBreakToken<'c>>,
    atomic_inline: &'b mut dyn AtomicInlineLayout<'c>,
    exclusion_space: &'b mut ExclusionSpace,
  ) -> Self {
    let break_iterator = Rc::new(items_data.break_iterator(BreakOptions::default()));
    let mut line_breaker = Self {
      node,
      items_data,
      mode,
      constraint_space,
      atomic_inline,
      exclusion_space,
      leading_floats,
      leading_floats_index: 0,
      state: LineBreakState::Continue,
      current: InlineItemTextIndex::default(),
      break_at: LineBreakPoint::default(),
      trailing_whitespace: WhitespaceState::Unknown,
      initial_whitespace: WhitespaceState::Leading,
      position: LayoutUnit::ZERO,
      applied_text_indent: LayoutUnit::ZERO,
      available_width: LayoutUnit::ZERO,
      base_available_width: LayoutUnit::ZERO,
      line_opportunity,
      is_first_formatted_line: break_token
        .as_ref()
        .is_none_or(|break_token| !break_token.is_past_first_formatted_line),
      auto_wrap: false,
      break_anywhere_if_overflow: false,
      override_break_anywhere: false,
      disable_score_line_break: false,
      disable_bisect_line_break: false,
      disable_trailing_whitespace_collapsing: false,
      force_non_empty_if_last_line: false,
      is_forced_break: false,
      previous_line_had_forced_break: false,
      end_item_index: items_data.items.len(),
      break_token,
      current_style: None,
      break_iterator,
      shaper: Rc::new(HarfBuzzShaper::new(items_data.text_content.clone())),
      hyphen_index: None,
      has_any_hyphens: false,
      trailing_collapsible_space: None,
      override_available_width: LayoutUnit::ZERO,
      line_clamp_ellipsis_width: LayoutUnit::ZERO,
      cloned_box_decorations_count: 0,
      cloned_box_decorations_initial_size: LayoutUnit::ZERO,
      cloned_box_decorations_end_size: LayoutUnit::ZERO,
      has_cloned_box_decorations: false,
      base_direction: items_data.base_direction,
      last_rewind: None,
    };

    line_breaker.update_available_width();

    if let Some(break_token) = break_token
      && let Some(style) = break_token.style
    {
      line_breaker.current = break_token.start;
      line_breaker.is_forced_break = break_token.is_forced_break;
      line_breaker.set_current_style(style);
    }

    line_breaker
  }

  fn text(&self) -> &'a str {
    &self.items_data.text_content
  }

  fn items(&self) -> &'a [InlineItem<'c>] {
    &self.items_data.items
  }

  fn block_context(&self) -> &'c RenderContext {
    &self.node.block.context
  }

  /// Blink's `IsFinished`.
  pub(crate) fn is_finished(&self) -> bool {
    self.current.item_index >= self.items().len()
  }

  /// Blink's `IsComputingContentSize`.
  fn is_computing_content_size(&self) -> bool {
    self.constraint_space.available_inline_size.is_none()
  }

  /// Blink's `ShouldAutoWrap`.
  fn should_auto_wrap(style: &ComputedStyle) -> bool {
    style.inherited_data.text_wrap_mode.should_wrap_line()
  }

  /// Blink's `UpdateAvailableWidth`.
  fn update_available_width(&mut self) {
    let available_inline_size = self.line_opportunity.available_inline_size();
    let mut available_width = if self.override_available_width != LayoutUnit::ZERO {
      if self.line_clamp_ellipsis_width != LayoutUnit::ZERO {
        (available_inline_size - self.line_clamp_ellipsis_width).min(self.override_available_width)
      } else {
        self.override_available_width
      }
    } else {
      available_inline_size - self.line_clamp_ellipsis_width
    };

    // At least the initial size, which `box-decoration-break: clone` may make non-zero.
    available_width = available_width.max(self.cloned_box_decorations_initial_size);
    // Smaller than the maximum, so the position can be larger.
    available_width = available_width.min(LayoutUnit::NEARLY_MAX);
    self.base_available_width = available_width;
    self.update_available_width_from_base_available_width();
  }

  /// Blink's `UpdateAvailableWidthFromBaseAvailableWidth`.
  fn update_available_width_from_base_available_width(&mut self) {
    self.available_width = if self.cloned_box_decorations_end_size != LayoutUnit::ZERO {
      (self.base_available_width - self.cloned_box_decorations_end_size).max(LayoutUnit::ZERO)
    } else {
      self.base_available_width
    };
  }

  /// Blink's `SetLineOpportunity`.
  pub(crate) fn set_line_opportunity(&mut self, line_opportunity: LineLayoutOpportunity) {
    self.line_opportunity = line_opportunity;
    self.update_available_width();
  }

  /// Blink's `OverrideAvailableWidth`: breaks against `available_width` until the next line.
  pub(crate) fn override_available_width(&mut self, available_width: LayoutUnit) {
    debug_assert!(available_width >= LayoutUnit::ZERO);

    self.override_available_width = available_width;
    self.update_available_width();
  }

  /// Blink's `SetBreakAt`: breaks at `offset` rather than at the available width.
  pub(crate) fn set_break_at(&mut self, offset: LineBreakPoint) {
    self.break_at = offset;
    self.override_available_width(LayoutUnit::NEARLY_MAX);
  }

  fn available_width_to_fit(&self) -> LayoutUnit {
    self.available_width.add_epsilon()
  }

  fn remaining_available_width(&self) -> LayoutUnit {
    self.available_width_to_fit() - self.position
  }

  fn can_fit_on_line(&self) -> bool {
    self.position <= self.available_width_to_fit()
  }

  fn is_at_end(&self) -> bool {
    self.current.item_index >= self.end_item_index
  }

  /// Blink's `AddItem` from the current offset to `end_offset`.
  fn add_item_to(
    &self,
    item_index: usize,
    end_offset: usize,
    line_info: &mut LineInfo<'a, 'c>,
  ) -> usize {
    let item = &self.items()[item_index];

    debug_assert_eq!(item_index, self.current.item_index);
    debug_assert!(self.current.text_offset >= item.start_offset());
    debug_assert!(end_offset >= self.current.text_offset);
    debug_assert!(end_offset <= item.end_offset());

    let results = &mut line_info.results;
    let result = InlineItemResult::new(
      item,
      self.current.item_index,
      self.current.text_offset..end_offset,
      self.break_anywhere_if_overflow,
      should_create_line_box(results),
      has_unpositioned_floats(results),
    );

    results.push(result);
    results.len() - 1
  }

  /// Blink's `AddItem` to the end of the item.
  fn add_item(&self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) -> usize {
    self.add_item_to(item_index, self.items()[item_index].end_offset(), line_info)
  }

  /// Blink's `AddEmptyItem`: no break before an empty item, but one after it when the item before
  /// allowed one.
  fn add_empty_item(&self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) -> usize {
    let index = self.add_item_to(item_index, self.current.text_offset, line_info);
    let results = &mut line_info.results;

    debug_assert!(!results[index].can_break_after);

    if index >= 1 && results[index - 1].can_break_after {
      results[index - 1].can_break_after = false;
      results[index].can_break_after = true;
    }

    index
  }

  /// Blink's `HandleOverflowIfNeeded`.
  fn handle_overflow_if_needed(&mut self, line_info: &mut LineInfo<'a, 'c>) -> bool {
    if self.state == LineBreakState::Continue && !self.can_fit_on_line() {
      self.handle_overflow(line_info);

      return true;
    }

    false
  }

  /// Blink's `RecalcClonedBoxDecorations`.
  fn recalc_cloned_box_decorations(&mut self) {
    self.cloned_box_decorations_count = 0;
    self.cloned_box_decorations_initial_size = LayoutUnit::ZERO;
    self.cloned_box_decorations_end_size = LayoutUnit::ZERO;
    self.has_cloned_box_decorations = false;

    // The tags open at the current item.
    let mut open_items: Vec<&InlineItem<'c>> = Vec::new();

    for item in &self.items()[..self.current.item_index] {
      match item.item_type {
        InlineItemType::OpenTag => open_items.push(item),
        InlineItemType::CloseTag => {
          open_items.pop();
        }
        _ => {}
      }
    }

    for item in open_items {
      let Some(object) = item.layout_object else {
        continue;
      };

      if object.style().box_data.box_decoration_break == BoxDecorationBreak::Clone {
        self.has_cloned_box_decorations = true;
        self.disable_score_line_break = true;
        self.cloned_box_decorations_count += 1;

        let mut item_result = InlineItemResult::new(item, 0, 0..0, false, false, false);

        Self::compute_open_tag_result(item, &mut item_result);
        self.cloned_box_decorations_initial_size += item_result.inline_size;
        self.cloned_box_decorations_end_size += item_result.margins.inline_end
          + item_result.borders.inline_end
          + item_result.padding.inline_end;
      }
    }

    // Tab stops start after the cloned decorations.
    self.position += self.cloned_box_decorations_initial_size;
    self.update_available_width();
  }

  /// Blink's `AddHyphen`: adds the hyphen to the item result at `index`, without moving the
  /// position.
  fn add_hyphen(
    &mut self,
    item_results: &mut InlineItemResults<'a, 'c>,
    index: usize,
  ) -> LayoutUnit {
    debug_assert!(self.hyphen_index.is_none());

    self.hyphen_index = Some(index);

    let item_result = &mut item_results[index];

    if item_result.hyphen.is_none() {
      item_result.shape_hyphen(&self.items_data.fonts);
      self.has_any_hyphens = true;
    }

    let hyphen_inline_size = item_result
      .hyphen
      .as_ref()
      .map_or(LayoutUnit::ZERO, HyphenResult::inline_size);

    item_result.inline_size += hyphen_inline_size;
    hyphen_inline_size
  }

  /// Blink's `RemoveHyphen`: removes the hyphen, without moving the position.
  fn remove_hyphen(&mut self, item_results: &mut InlineItemResults<'a, 'c>) -> LayoutUnit {
    let Some(index) = self.hyphen_index.take() else {
      return LayoutUnit::ZERO;
    };
    let item_result = &mut item_results[index];
    let hyphen_inline_size = item_result
      .hyphen
      .as_ref()
      .map_or(LayoutUnit::ZERO, HyphenResult::inline_size);

    item_result.inline_size -= hyphen_inline_size;
    hyphen_inline_size
  }

  /// Blink's `RestoreLastHyphen`: hyphenates the last item again after a rewind.
  fn restore_last_hyphen(&mut self, item_results: &mut InlineItemResults<'a, 'c>) {
    debug_assert!(self.hyphen_index.is_none());
    debug_assert!(self.has_any_hyphens);

    for index in (0..item_results.len()).rev() {
      if item_results[index].hyphen.is_some() {
        self.add_hyphen(item_results, index);

        return;
      }

      if matches!(
        item_results[index].item.item_type,
        InlineItemType::Text | InlineItemType::AtomicInline
      ) {
        return;
      }
    }
  }

  /// Blink's `FinalizeHyphen`.
  fn finalize_hyphen(&mut self, item_results: &mut InlineItemResults<'a, 'c>) {
    if let Some(index) = self.hyphen_index {
      item_results[index].is_hyphenated = true;
    }
  }

  /// Blink's `PrepareNextLine`.
  /// Blink's `ComputeBaseDirection`: a `unicode-bidi: plaintext` block takes each paragraph's
  /// direction from its first strong character. A forced break ends a paragraph with a line feed,
  /// so the paragraph a line is in is the one after the last line feed before it.
  fn compute_base_direction(&mut self) {
    if self.node.block.context.style.misc_data.unicode_bidi != UnicodeBidi::Plaintext {
      return;
    }

    if let Some(direction) = self
      .items_data
      .plaintext_paragraph_direction(self.current.text_offset)
    {
      self.base_direction = direction;
    }
  }

  fn prepare_next_line(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    line_info.reset();

    if !self.current.is_zero() {
      self.previous_line_had_forced_break = self.is_forced_break;
      self.is_forced_break = false;

      // Past the line the break token resumed at, this is past the first formatted line.
      if self
        .break_token
        .is_none_or(|break_token| self.current != break_token.start)
      {
        self.is_first_formatted_line = false;
      }
    }

    line_info.start = self.current;
    line_info.is_first_formatted_line = self.is_first_formatted_line;
    line_info.is_start_of_paragraph = self.current.is_zero() || self.previous_line_had_forced_break;
    self.compute_base_direction();
    line_info.base_direction = self.base_direction;
    line_info.set_line_style(self.block_context());

    let line_style = line_info.line_style;

    if should_apply_text_indent(
      line_style,
      self.is_first_formatted_line,
      self.previous_line_had_forced_break,
    ) {
      let length = line_style.style.misc_inherited1_data.text_indent.amount;
      // Percentages resolve against nothing while computing intrinsic sizes.
      let maximum_value = match (self.mode, self.constraint_space.available_inline_size) {
        (LineBreakerMode::Content, Some(available_inline_size)) if length.has_percentage() => {
          available_inline_size
        }
        _ => LayoutUnit::ZERO,
      };

      line_info.text_indent = length.minimum_value_for(&line_style.sizing, maximum_value);
    }

    // The line starts in the style the last one ended with, or the block's.
    self.override_break_anywhere = false;
    self.disable_score_line_break = self.auto_wrap
      && self.current_style.is_some_and(|context| {
        context
          .style
          .inherited_data
          .white_space_collapse
          .should_break_spaces()
      });
    self.disable_bisect_line_break = false;

    if self.current_style.is_none() {
      self.set_current_style(line_style);
    }

    self.hyphen_index = None;
    self.has_any_hyphens = false;

    // Tab stops align regardless of `text-indent`.
    self.applied_text_indent = line_info.text_indent;
    self.position = self.applied_text_indent;
    self.has_cloned_box_decorations = false;

    if self
      .break_token
      .is_some_and(|break_token| break_token.has_cloned_box_decorations)
      || self.cloned_box_decorations_count != 0
    {
      self.recalc_cloned_box_decorations();
    }

    self.last_rewind = None;
  }

  /// Blink's `NextLine`: the next line's break point and item results.
  pub(crate) fn next_line(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    self.prepare_next_line(line_info);
    self.break_line(line_info);

    if self.hyphen_index.is_some() {
      self.finalize_hyphen(&mut line_info.results);
    }

    if !self.disable_trailing_whitespace_collapsing {
      self.remove_trailing_collapsible_space(line_info);
      self.split_trailing_bidi_preserved_space(line_info);
    }

    // A line box holds text, a list marker on the last line, or anything while measuring.
    let should_create_line_box = should_create_line_box(&line_info.results)
      || (self.force_non_empty_if_last_line && line_info.is_last_line)
      || self.mode != LineBreakerMode::Content;

    if self.line_clamp_ellipsis_width != LayoutUnit::ZERO && !should_create_line_box {
      self.line_clamp_ellipsis_width = LayoutUnit::ZERO;
    }

    // An ellipsis that overflows replaces the whole line.
    if self.line_clamp_ellipsis_width != LayoutUnit::ZERO && !self.can_fit_on_line() {
      self.rewind(0, line_info);
      line_info.is_last_line = false;

      // Overflowing only when the ellipsis itself does not fit.
      let has_overflow = self.available_width == LayoutUnit::ZERO
        && self.line_opportunity.available_inline_size().add_epsilon()
          < self.line_clamp_ellipsis_width;

      line_info.has_overflow = has_overflow;
      self.disable_bisect_line_break = has_overflow;
    }

    if !should_create_line_box {
      line_info.is_empty_line = true;
    }

    line_info.end_item_index = self.current.item_index;

    if !self.disable_trailing_whitespace_collapsing
      && self.trailing_whitespace == WhitespaceState::Preserved
    {
      line_info.has_trailing_spaces = true;
    }

    // The original available width aligns the line.
    if self.override_available_width != LayoutUnit::ZERO {
      self.override_available_width = LayoutUnit::ZERO;
      self.update_available_width();
    }

    self.compute_line_location(line_info);

    if self.mode == LineBreakerMode::Content {
      line_info.break_token = self.create_break_token(line_info);
    }
  }

  /// Blink's `BreakLine`.
  fn break_line(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    debug_assert!(!line_info.is_last_line);

    // A min-content line overflows anyway, so it skips `handle_overflow`.
    self.state = if self.mode == LineBreakerMode::MinContent {
      LineBreakState::Overflow
    } else {
      LineBreakState::Continue
    };
    self.trailing_whitespace = self.initial_whitespace;

    while self.state != LineBreakState::Done {
      if self.is_at_end() {
        // The last item may have overflowed.
        if self.handle_overflow_if_needed(line_info) && !self.is_at_end() {
          continue;
        }

        if self.hyphen_index.is_some() {
          let hyphen_inline_size = self.remove_hyphen(&mut line_info.results);

          self.position -= hyphen_inline_size;
        }

        line_info.is_last_line = true;

        return;
      }

      if self.break_at.is_set() && self.current.is_at_least(self.break_at.offset) {
        return;
      }

      // Overflowing, break at the earliest opportunity.
      if self.state == LineBreakState::Overflow && can_break_after_last(&line_info.results) {
        self.state = LineBreakState::Trailing;
      }

      // Trailable items first: they may not break before, and may overhang.
      let item_index = self.current.item_index;
      let item = &self.items()[item_index];

      match item.item_type {
        InlineItemType::Text => {
          if item.length() > 0 {
            if let Some(shape_result) = item.shape_result.clone() {
              self.handle_text(item_index, &shape_result, line_info);
            } else {
              self.handle_empty_text(item_index, line_info);
            }
          } else {
            self.handle_empty_text(item_index, line_info);
          }

          continue;
        }
        InlineItemType::OpenTag => {
          self.handle_open_tag(item_index, line_info);

          continue;
        }
        InlineItemType::CloseTag => {
          self.handle_close_tag(item_index, line_info);

          continue;
        }
        InlineItemType::Control => {
          self.handle_control_item(item_index, line_info);

          continue;
        }
        InlineItemType::Floating => {
          self.handle_float(item_index, line_info);

          continue;
        }
        InlineItemType::BidiControl => {
          self.handle_bidi_control_item(item_index, line_info);

          continue;
        }
        InlineItemType::BlockInInline => {
          self.handle_block_in_inline(item_index, line_info);

          continue;
        }
        _ => {}
      }

      // Items from here on are not trailable: a trailing line breaks before them.
      debug_assert!(!is_trailable_item_type(item.item_type));

      if self.state == LineBreakState::Trailing {
        debug_assert!(!line_info.is_last_line);

        return;
      }

      match item.item_type {
        InlineItemType::AtomicInline => self.handle_atomic_inline(item_index, line_info),
        InlineItemType::OutOfFlowPositioned => {
          self.handle_out_of_flow_positioned(item_index, line_info)
        }
        InlineItemType::ListMarker => {
          let index = self.add_item(item_index, line_info);

          self.force_non_empty_if_last_line = true;
          debug_assert!(!line_info.results[index].can_break_after);
          self.move_to_next_of_item(item_index);
        }
        _ => unreachable!("an item with characters the line breaker does not handle"),
      }
    }
  }

  /// Blink's `ComputeLineLocation`.
  fn compute_line_location(&self, line_info: &mut LineInfo<'a, 'c>) {
    // Negative margins can make the position negative; the inline size never is.
    line_info.set_width(
      self.base_available_width + self.line_clamp_ellipsis_width,
      self.position + self.cloned_box_decorations_end_size + self.line_clamp_ellipsis_width,
    );
    line_info.bfc_offset = BfcOffset::new(
      self.line_opportunity.line_left_offset,
      self.line_opportunity.bfc_block_offset,
    );

    if self.mode == LineBreakerMode::Content {
      line_info.update_text_align();
    }
  }

  /// Blink's `CanBreakAfterAtomicInline`: atomic inlines break before and after, even beside
  /// U+00A0.
  fn can_break_after_atomic_inline(&self, item: &InlineItem<'_>) -> bool {
    debug_assert_eq!(item.item_type, InlineItemType::AtomicInline);

    self.auto_wrap
  }

  /// Blink's `CanBreakAfter`.
  fn can_break_after(&self, item: &InlineItem<'_>) -> bool {
    debug_assert!(self.auto_wrap);

    let can_break_after = self.break_iterator.is_breakable(item.end_offset());

    if item.item_type != InlineItemType::Text {
      debug_assert_eq!(item.item_type, InlineItemType::Control);

      return can_break_after;
    }

    if self.try_get_atomic_inline_item_after(item).is_none() {
      return can_break_after;
    }

    true
  }

  /// Blink's `MayBeAtomicInline`.
  fn may_be_atomic_inline(&self, offset: usize) -> bool {
    char_at(self.text(), offset) == OBJECT_REPLACEMENT_CHARACTER
  }

  /// Blink's `TryGetAtomicInlineItemAfter`: the atomic inline the U+FFFC after `item` stands for.
  fn try_get_atomic_inline_item_after(&self, item: &InlineItem<'_>) -> Option<&'a InlineItem<'c>> {
    debug_assert!(self.auto_wrap);

    let text = self.text();

    if item.end_offset() == text.len() || !self.may_be_atomic_inline(item.end_offset()) {
      return None;
    }

    for next_item in &self.items()[item.index + 1..] {
      debug_assert_eq!(next_item.start_offset(), item.end_offset());

      if next_item.item_type == InlineItemType::AtomicInline {
        return Some(next_item);
      }

      if next_item.end_offset() > item.end_offset() {
        return None;
      }
    }

    None
  }

  /// Blink's `HandleText`.
  fn handle_text(
    &mut self,
    item_index: usize,
    shape_result: &Rc<ShapeResult>,
    line_info: &mut LineInfo<'a, 'c>,
  ) {
    let item = &self.items()[item_index];
    let text = self.text();

    debug_assert!(
      item.item_type == InlineItemType::Text
        || (item.item_type == InlineItemType::Control && char_at(text, item.start_offset()) == TAB)
    );

    // Trailing, only trailing spaces may join the line.
    if self.state == LineBreakState::Trailing {
      self.handle_trailing_spaces(item_index, Some(shape_result), line_info);

      return;
    }

    // Leading collapsible spaces go; the builder collapsed them to one.
    if self.trailing_whitespace == WhitespaceState::Leading
      && item.style().is_some_and(|style| {
        style
          .inherited_data
          .white_space_collapse
          .should_collapse_white_spaces()
      })
      && char_at(text, self.current.text_offset) == ' '
    {
      self.current.text_offset += 1;

      if self.current.text_offset == item.end_offset() {
        self.handle_empty_text(item_index, line_info);

        return;
      }
    }

    // The last item overflowed and text follows.
    if self.state == LineBreakState::Continue && !self.can_fit_on_line() {
      // `handle_overflow` expects the trailable spaces this text starts with.
      if self.auto_wrap && is_breakable_space(char_at(text, self.current.text_offset)) {
        self.handle_trailing_spaces(item_index, Some(shape_result), line_info);

        if self.state != LineBreakState::Done {
          self.state = LineBreakState::Continue;

          return;
        }
      }

      self.handle_overflow(line_info);

      return;
    }

    if self.hyphen_index.is_some() {
      let hyphen_inline_size = self.remove_hyphen(&mut line_info.results);

      self.position -= hyphen_inline_size;
    }

    let result_index = self.add_item(item_index, line_info);

    line_info.results[result_index].should_create_line_box = true;

    if self.auto_wrap {
      if self.mode == LineBreakerMode::MinContent
        && self.handle_text_for_fast_min_content(result_index, item_index, shape_result, line_info)
      {
        return;
      }

      // Break inside this text item.
      let available_width = self.remaining_available_width();
      let break_result = self.break_text(
        result_index,
        item_index,
        shape_result,
        available_width,
        available_width,
        line_info,
      );
      let item_result = &line_info.results[result_index];

      self.position += item_result.inline_size;
      self.move_to_next_of_result(result_index, line_info);

      let item_result = &line_info.results[result_index];

      match break_result {
        BreakResult::Success => {
          // A break inside the item leaves only trailable spaces after it.
          if item_result.end_offset() < item.end_offset() {
            self.handle_trailing_spaces(item_index, Some(shape_result), line_info);
          }

          // At its end, a trailable item may follow, or one that prevents breaking before it.
          return;
        }
        BreakResult::BreakAt => {
          if item_result.end_offset() < item.end_offset() {
            self.handle_trailing_spaces(item_index, Some(shape_result), line_info);

            return;
          }

          self.state = LineBreakState::Trailing;

          return;
        }
        BreakResult::Overflow => {}
      }

      // `overflow-wrap` breaks this overflowing text.
      if item_result.shape_result.is_none() {
        debug_assert!(self.break_anywhere_if_overflow && !self.override_break_anywhere);

        self.handle_overflow(line_info);

        return;
      }

      // Hanging trailing spaces may resolve the overflow.
      if item_result.has_only_pre_wrap_trailing_spaces {
        self.state = LineBreakState::Trailing;

        if item_result.item.style().is_some_and(|style| {
          style
            .inherited_data
            .white_space_collapse
            .should_preserve_white_spaces()
        }) && is_breakable_space(char_before(text, item_result.end_offset()))
        {
          self.rewind(result_index, line_info);
        }

        return;
      }

      // Seeking the first break opportunity.
      if self.state == LineBreakState::Overflow {
        if item_result.can_break_after {
          self.state = LineBreakState::Trailing;
        }

        return;
      }

      // All trailable spaces: the next item may be trailable too.
      if is_all_breakable_spaces(text, item_result.start_offset(), item_result.end_offset()) {
        return;
      }

      self.handle_overflow(line_info);

      return;
    }

    // Without wrapping, the item joins the line whole, or from where `<wbr>` broke it.
    let item_result = &mut line_info.results[result_index];

    debug_assert_eq!(item_result.end_offset(), item.end_offset());

    if item_result.start_offset() == item.start_offset() {
      item_result.inline_size = shape_result.snapped_width().clamp_negative_to_zero();
      item_result.shape_result = Some(Rc::new(ShapeResultView::create_from_result(shape_result)));
    } else {
      debug_assert!(
        self.trailing_whitespace == WhitespaceState::Leading
          && item_result.start_offset() >= item.start_offset()
      );

      let view = ShapeResultView::create_from_result_range(
        shape_result,
        item_result.start_offset() as u32,
        item_result.end_offset() as u32,
      );

      item_result.inline_size = view.snapped_width().clamp_negative_to_zero();
      item_result.shape_result = Some(Rc::new(view));
    }

    debug_assert!(!item_result.may_break_inside);
    debug_assert!(!item_result.can_break_after);

    self.trailing_whitespace = WhitespaceState::Unknown;
    self.position += item_result.inline_size;
    self.move_to_next_of_item(item_index);
  }

  /// Blink's `ShapeText`: shapes `start..end` of the item again, with its spacing.
  fn shape_text(
    items_data: &InlineNodeData<'_>,
    shaper: &HarfBuzzShaper,
    text: &str,
    item: &InlineItem<'_>,
    start: usize,
    end: usize,
  ) -> Rc<ShapeResult> {
    let Some(object) = item.layout_object else {
      return Rc::new(ShapeResult::new(
        start as u32,
        (end - start) as u32,
        item.direction(),
      ));
    };
    let font = items_data.fonts.get(object.context());
    let mut shape_result = match &items_data.segments {
      Some(segments) => {
        segments.shape_text(shaper, &font, item.direction(), start, end, item.index)
      }
      None => {
        let mut range = item.create_run_segmenter_range();

        range.start = start;
        range.end = end;
        shaper.shape_pre_segmented(&font, item.direction(), start, end, &range)
      }
    };
    let mut spacing = ShapeResultSpacing::new(
      text,
      object
        .style()
        .inherited_data
        .white_space_collapse
        .should_preserve_white_spaces(),
    );

    if spacing.set_spacing_from(
      font.description.letter_spacing,
      font.description.word_spacing,
    ) {
      shape_result.apply_spacing(&mut spacing, 0);
    }

    Rc::new(shape_result)
  }

  /// Blink's `BreakText`.
  fn break_text(
    &mut self,
    result_index: usize,
    item_index: usize,
    item_shape_result: &Rc<ShapeResult>,
    mut available_width: LayoutUnit,
    available_width_with_hyphens: LayoutUnit,
    line_info: &mut LineInfo<'a, 'c>,
  ) -> BreakResult {
    let item = &self.items()[item_index];

    debug_assert!(self.hyphen_index.is_none());
    debug_assert_eq!(item_shape_result.start_index as usize, item.start_offset());
    debug_assert_eq!(item_shape_result.end_index() as usize, item.end_offset());

    let items_data = self.items_data;
    let text = self.text();
    let break_iterator = Rc::clone(&self.break_iterator);
    let shaper = Rc::clone(&self.shaper);
    let mut breaker = ShapingLineBreaker::new(item_shape_result, &break_iterator, |start, end| {
      Self::shape_text(items_data, &shaper, text, item, start, end)
    });

    breaker.set_line_start(line_info.start_offset());
    breaker.set_is_after_forced_break(self.previous_line_had_forced_break);

    // Kerning between the last character and trailing spaces shows only where the end does.
    if !line_info.needs_accurate_end_position() && !item_needs_accurate_end_position(item) {
      breaker.set_dont_reshape_end_if_at_space();
    }

    if self.break_at.is_set() {
      let should_break = self.break_text_at(result_index, item_index, &mut breaker, line_info);

      return if should_break {
        BreakResult::BreakAt
      } else {
        BreakResult::Success
      };
    }

    // Under `break-word`, an overflowing item rewinds the line, so its result is not needed.
    if self.break_anywhere_if_overflow && !self.override_break_anywhere {
      breaker.set_no_result_if_overflow();
    }

    let mut result = ShapingLineBreakerResult::default();
    let mut inline_size;

    loop {
      let start_offset = line_info.results[result_index].start_offset();
      let Some(shape_result) = breaker.shape_line(
        start_offset,
        available_width.clamp_negative_to_zero(),
        &mut result,
      ) else {
        // Long enough to overflow, as this line rewinds.
        debug_assert!(breaker.no_result_if_overflow());

        let item_result = &mut line_info.results[result_index];

        item_result.inline_size = available_width_with_hyphens + LayoutUnit::from_int(1);
        item_result.text_offset.end = item.end_offset();
        item_result.shape_result = None;

        return BreakResult::Overflow;
      };

      // Moving forward keeps the line breaker from adding results forever.
      assert!(result.break_offset > start_offset);

      inline_size = shape_result.snapped_width().clamp_negative_to_zero();
      line_info.results[result_index].inline_size = inline_size;

      if result.is_hyphenated {
        let hyphen_inline_size = self.add_hyphen(&mut line_info.results, result_index);

        // A hyphen that overflows retries with less width.
        if !result.is_overflow && inline_size <= available_width {
          let space_for_hyphen = available_width_with_hyphens - inline_size;

          if space_for_hyphen >= LayoutUnit::ZERO && hyphen_inline_size > space_for_hyphen {
            available_width -= hyphen_inline_size;
            self.remove_hyphen(&mut line_info.results);

            continue;
          }
        }

        inline_size = line_info.results[result_index].inline_size;
      }

      let item_result = &mut line_info.results[result_index];

      item_result.text_offset.end = result.break_offset;
      item_result.has_only_pre_wrap_trailing_spaces = result.has_trailing_spaces;
      item_result.has_only_bidi_trailing_spaces = result.has_trailing_spaces;
      item_result.shape_result = Some(Rc::new(shape_result));

      break;
    }

    let item_result = &mut line_info.results[result_index];

    // A break inside fits; one at the end may leave room; a wider result does not fit.
    if item_result.end_offset() < item.end_offset() {
      item_result.can_break_after = true;
      self.trailing_whitespace =
        if self.break_iterator.break_type() == LineBreakType::BreakCharacter {
          WhitespaceState::Unknown
        } else {
          WhitespaceState::None
        };
    } else {
      debug_assert_eq!(item_result.end_offset(), item.end_offset());

      item_result.can_break_after = self.can_break_after(item);
      self.trailing_whitespace = WhitespaceState::Unknown;
    }

    // An overflowing result breaks no further, which `handle_overflow` uses.
    item_result.may_break_inside = !result.is_overflow;

    if inline_size <= available_width_with_hyphens {
      BreakResult::Success
    } else {
      BreakResult::Overflow
    }
  }

  /// Blink's `BreakTextAt`.
  fn break_text_at<S: FnMut(usize, usize) -> Rc<ShapeResult>>(
    &mut self,
    result_index: usize,
    item_index: usize,
    breaker: &mut ShapingLineBreaker<'_, S>,
    line_info: &mut LineInfo<'a, 'c>,
  ) -> bool {
    debug_assert!(self.break_at.is_set());
    debug_assert!(self.current.text_offset <= self.break_at.end.text_offset);
    debug_assert!(self.current.item_index <= self.break_at.offset.item_index);

    let should_break = self.current.item_index >= self.break_at.end.item_index;
    let item_result = &mut line_info.results[result_index];

    if should_break {
      debug_assert!(self.break_at.end.text_offset <= item_result.text_offset.end);

      item_result.text_offset.end = self.break_at.end.text_offset;
    }

    let _ = item_index;

    if item_result.length() > 0 {
      let shape_result =
        breaker.shape_line_at(item_result.start_offset(), item_result.end_offset());

      item_result.inline_size = shape_result.snapped_width().clamp_negative_to_zero();
      item_result.shape_result = Some(Rc::new(shape_result));

      if self.break_at.is_hyphenated {
        self.add_hyphen(&mut line_info.results, result_index);
      }
    } else {
      debug_assert_eq!(item_result.inline_size, LayoutUnit::ZERO);
      debug_assert!(!self.break_at.is_hyphenated);
    }

    line_info.results[result_index].can_break_after = true;
    self.trailing_whitespace = WhitespaceState::None;
    should_break
  }

  /// Blink's `BreakTextAtPreviousBreakOpportunity`: false without an earlier opportunity.
  fn break_text_at_previous_break_opportunity(
    &mut self,
    results: &mut InlineItemResults<'a, 'c>,
    result_index: usize,
  ) -> bool {
    let item_result = &mut results[result_index];
    let item = item_result.item;

    debug_assert!(item_result.may_break_inside);
    debug_assert_eq!(item.item_type, InlineItemType::Text);

    let end_offset = item_result.end_offset();
    let previous = end_offset - char_before(self.text(), end_offset).len_utf8();
    let break_opportunity = self
      .break_iterator
      .previous_break_opportunity(previous, item_result.start_offset());

    if break_opportunity <= item_result.start_offset() {
      return false;
    }

    let Some(shape_result) = &item.shape_result else {
      return false;
    };

    item_result.text_offset.end = break_opportunity;

    let view = ShapeResultView::create_from_result_range(
      shape_result,
      item_result.start_offset() as u32,
      item_result.end_offset() as u32,
    );

    item_result.inline_size = view.snapped_width().clamp_negative_to_zero();
    item_result.shape_result = Some(Rc::new(view));
    item_result.can_break_after = true;

    if self
      .trailing_collapsible_space
      .as_ref()
      .is_some_and(|space| space.item_result_index == result_index)
    {
      self.trailing_collapsible_space = None;
    }

    true
  }

  /// Blink's `HandleTextForFastMinContent`: breaks the text at every opportunity and keeps the
  /// widest word, as min-content would otherwise make a line per word. The first and last words
  /// go through `handle_text`, as they may join the items around them.
  fn handle_text_for_fast_min_content(
    &mut self,
    result_index: usize,
    item_index: usize,
    shape_result: &Rc<ShapeResult>,
    line_info: &mut LineInfo<'a, 'c>,
  ) -> bool {
    debug_assert_eq!(self.mode, LineBreakerMode::MinContent);
    debug_assert!(self.auto_wrap);

    let item = &self.items()[item_index];
    let text = self.text();
    let mut start_offset = line_info.results[result_index].start_offset();
    let item_end_offset = item.end_offset();
    let mut end_offset = item_end_offset;

    debug_assert!(start_offset < item_end_offset);

    let indent = line_info.text_indent;

    if indent != LayoutUnit::ZERO {
      // A word inside a negative indent may not wrap at the first opportunity.
      if indent < LayoutUnit::ZERO {
        return false;
      }

      end_offset = start_offset + char_at(text, start_offset).len_utf8();
    } else if self.position < indent {
      // A negative margin may move the position before the line's start.
      return false;
    } else if self.position != indent {
      // Earlier items make this break at the first opportunity.
      end_offset = start_offset + char_at(text, start_offset).len_utf8();
    }

    shape_result.ensure_position_data();

    let mut context = FastMinTextContext::default();
    let item_style = item.style();
    let should_break_spaces = item_style.is_some_and(|style| {
      style
        .inherited_data
        .white_space_collapse
        .should_break_spaces()
    });
    let mut next_break = 0;
    let mut non_hangable_run_end = 0;
    let mut can_break_after = false;
    let limit = (item_end_offset + 1).min(text.len());

    while start_offset < end_offset {
      next_break = self
        .break_iterator
        .next_break_opportunity(start_offset + char_at(text, start_offset).len_utf8(), limit);

      if next_break > item_end_offset {
        // The item's end is mid-word.
        if start_offset == line_info.results[result_index].start_offset() {
          // The line's first word joins the items after it.
          next_break = item_end_offset;
          can_break_after = false;
        } else if char_at(text, item_end_offset) == LINE_FEED {
          // A forced break follows, so the line ends here.
          next_break = item_end_offset;
          can_break_after = false;
        } else {
          // The last word joins the items after it.
          next_break = start_offset;
          debug_assert!(can_break_after);

          break;
        }
      } else {
        can_break_after = true;
      }

      debug_assert!(next_break <= item_end_offset);

      // Without trailing spaces.
      non_hangable_run_end = next_break;

      if !should_break_spaces {
        while non_hangable_run_end > start_offset
          && is_breakable_space(char_before(text, non_hangable_run_end))
        {
          non_hangable_run_end -= 1;
        }
      }

      if non_hangable_run_end > start_offset {
        let has_hyphen = can_break_after && char_before(text, non_hangable_run_end) == SOFT_HYPHEN;

        context.add(
          shape_result,
          start_offset,
          non_hangable_run_end,
          has_hyphen,
          &mut line_info.results[result_index],
          self.items_data,
        );
      }

      debug_assert!(next_break > start_offset);

      start_offset = next_break;
    }

    // One result, as wide as the widest word.
    let collapses = item_style.is_some_and(|style| {
      style
        .inherited_data
        .white_space_collapse
        .should_collapse_white_spaces()
    });
    let item_result = &mut line_info.results[result_index];

    if collapses {
      item_result.text_offset.end = non_hangable_run_end;
      self.trailing_whitespace = if non_hangable_run_end != next_break {
        WhitespaceState::Collapsed
      } else {
        WhitespaceState::None
      };
    } else {
      item_result.text_offset.end = next_break;
      self.trailing_whitespace = if non_hangable_run_end != next_break {
        WhitespaceState::Preserved
      } else {
        WhitespaceState::None
      };
    }

    item_result.inline_size = context.min_inline_size;
    self.position += item_result.inline_size;
    item_result.can_break_after = can_break_after;
    self.state = if can_break_after {
      LineBreakState::Trailing
    } else {
      LineBreakState::Overflow
    };

    if next_break >= item_end_offset {
      self.move_to_next_of_item(item_index);
    } else {
      // Moving forward keeps the loop finite.
      assert!(next_break > self.current.text_offset);

      self.current.text_offset = next_break;
    }

    true
  }

  /// Blink's `HandleEmptyText`: empty and fully collapsed text still gets a result, so every item
  /// shows up in the line.
  fn handle_empty_text(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    self.add_empty_item(item_index, line_info);
    self.move_to_next_of_item(item_index);
  }

  /// Blink's `TruncateLineEndResult`: the result cut at `end_offset`, its end shaped again where
  /// it is not safe to break.
  fn truncate_line_end_result(
    &self,
    line_info: &LineInfo<'a, 'c>,
    result_index: usize,
    source_result: &Rc<ShapeResultView>,
    end_offset: usize,
  ) -> Rc<ShapeResultView> {
    let item_result = &line_info.results[result_index];
    let item = item_result.item;
    let start_offset = item_result.start_offset();

    debug_assert!(start_offset >= source_result.start_index() as usize);
    debug_assert!(end_offset <= source_result.end_index() as usize);

    if !line_info.needs_accurate_end_position() && !item_needs_accurate_end_position(item) {
      return Rc::new(ShapeResultView::create_from_view_range(
        source_result,
        start_offset as u32,
        end_offset as u32,
      ));
    }

    let last_safe = source_result.previous_safe_to_break_offset(end_offset as u32) as usize;

    debug_assert!(last_safe <= end_offset);

    if last_safe == end_offset || last_safe <= start_offset {
      return Rc::new(ShapeResultView::create_from_view_range(
        source_result,
        start_offset as u32,
        end_offset as u32,
      ));
    }

    let end_result = Self::shape_text(
      self.items_data,
      &self.shaper,
      self.text(),
      item,
      last_safe.max(start_offset),
      end_offset,
    );

    Rc::new(ShapeResultView::create(&[
      Segment {
        source: SegmentSource::View(source_result),
        start_index: start_offset as u32,
        end_index: last_safe as u32,
      },
      Segment {
        source: SegmentSource::Result(&end_result),
        start_index: 0,
        end_index: end_offset as u32,
      },
    ]))
  }

  /// Blink's `UpdateShapeResult`: the result matching its offsets, its end reshaped where not
  /// safe to break.
  fn update_shape_result(&self, line_info: &mut LineInfo<'a, 'c>, result_index: usize) {
    let item_result = &line_info.results[result_index];
    let end_offset = item_result.end_offset();
    let Some(source_result) = item_result.shape_result.clone() else {
      return;
    };
    let shape_result =
      self.truncate_line_end_result(line_info, result_index, &source_result, end_offset);
    let item_result = &mut line_info.results[result_index];

    item_result.inline_size = shape_result.snapped_width();
    item_result.shape_result = Some(shape_result);
  }

  /// Blink's `HandleTrailingSpaces`.
  fn handle_trailing_spaces(
    &mut self,
    item_index: usize,
    shape_result: Option<&Rc<ShapeResult>>,
    line_info: &mut LineInfo<'a, 'c>,
  ) {
    let item = &self.items()[item_index];
    let text = self.text();

    debug_assert!(self.current.text_offset >= item.start_offset());
    debug_assert!(self.current.text_offset < item.end_offset());

    if !self.auto_wrap {
      self.state = LineBreakState::Done;

      return;
    }

    let Some(style) = item.style() else {
      self.state = LineBreakState::Done;

      return;
    };
    let style = style.inherited_data.white_space_collapse;
    let current_char = char_at(text, self.current.text_offset);

    if style.should_collapse_white_spaces() && !is_other_space_separator(current_char) {
      if current_char != ' ' {
        if self.current.text_offset > 0
          && is_breakable_space(char_before(text, self.current.text_offset))
        {
          self.trailing_whitespace = WhitespaceState::Collapsible;
        }

        self.state = LineBreakState::Done;

        return;
      }

      // One collapsible space stands for all of them; after preserved spaces, they stay preserved.
      self.current.text_offset += 1;

      if self.trailing_whitespace != WhitespaceState::Preserved {
        self.trailing_whitespace = WhitespaceState::Collapsed;
      }

      // Breakable after the last item, even if it was `nowrap`.
      if let Some(last) = line_info.results.last_mut() {
        last.can_break_after = true;
      }
    } else if !style.should_break_spaces() {
      // The run of spaces in this item, without other white space such as tabs.
      let mut end = self.current.text_offset;

      while end < item.end_offset() && is_breakable_space_or_other_separator(char_at(text, end)) {
        end += char_at(text, end).len_utf8();
      }

      if end == self.current.text_offset {
        if is_breakable_space_or_other_separator(char_before(text, end)) {
          self.trailing_whitespace = WhitespaceState::Preserved;
        }

        self.state = LineBreakState::Done;

        return;
      }

      let result_index = self.add_item_to(item_index, end, line_info);
      let item_result = &mut line_info.results[result_index];

      item_result.should_create_line_box = true;
      item_result.has_only_pre_wrap_trailing_spaces = true;
      item_result.has_only_bidi_trailing_spaces = true;
      item_result.shape_result =
        shape_result.map(|shape_result| Rc::new(ShapeResultView::create_from_result(shape_result)));

      let hangs = self.mode == LineBreakerMode::MinContent
        || self.line_clamp_ellipsis_width != LayoutUnit::ZERO;

      if item_result.start_offset() == item.start_offset()
        && item_result.end_offset() == item.end_offset()
      {
        item_result.inline_size = match &item_result.shape_result {
          Some(shape_result) if !hangs => shape_result.snapped_width(),
          _ => LayoutUnit::ZERO,
        };
      } else {
        self.update_shape_result(line_info, result_index);

        if hangs {
          line_info.results[result_index].inline_size = LayoutUnit::ZERO;
        }
      }

      let item_result = &mut line_info.results[result_index];

      self.position += item_result.inline_size;
      item_result.can_break_after =
        end < text.len() && !is_breakable_space_or_other_separator(char_at(text, end));
      self.current.text_offset = end;
      self.trailing_whitespace = WhitespaceState::Preserved;
    }

    // Characters other than spaces follow: the line is done.
    if self.current.text_offset < item.end_offset() {
      self.state = LineBreakState::Done;

      return;
    }

    // At the end of the item, which needs a result even when empty.
    if line_info
      .results
      .last()
      .is_none_or(|result| !std::ptr::eq(result.item, item))
    {
      self.add_empty_item(item_index, line_info);
    }

    self.current.item_index += 1;
    self.state = LineBreakState::Trailing;
  }

  /// Blink's `RewindTrailingOpenTags`: open tags at the line's end are not trailable after all.
  fn rewind_trailing_open_tags(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    let results = &line_info.results;

    if let Some(index) = results
      .iter()
      .rposition(|result| result.item.item_type != InlineItemType::OpenTag)
    {
      let end_index = index + 1;

      if end_index < results.len() {
        let end = results[end_index].start();

        self.last_rewind = None;
        self.rewind(end_index, line_info);
        self.current = end;
      }
    }
  }

  /// Blink's `RemoveTrailingCollapsibleSpace`.
  /// <https://drafts.csswg.org/css-text-3/#white-space-phase-2>
  fn remove_trailing_collapsible_space(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    // Wrap before trailing open tags, unless the line ends with a forced break.
    if !self.is_forced_break {
      self.rewind_trailing_open_tags(line_info);
    }

    self.compute_trailing_collapsible_space(line_info);

    let Some(space) = self.trailing_collapsible_space.take() else {
      return;
    };
    let position_was_saturated = self.position == LayoutUnit::MAX;
    let item_result = &mut line_info.results[space.item_result_index];

    self.position -= item_result.inline_size;

    if let Some(collapsed_shape_result) = space.collapsed_shape_result {
      item_result.text_offset.end -= 1;
      item_result.inline_size = collapsed_shape_result.snapped_width();
      item_result.shape_result = Some(collapsed_shape_result);
      self.position += item_result.inline_size;
    } else {
      // Empty, but kept; see `handle_empty_text`.
      item_result.text_offset.end = item_result.text_offset.start;
      item_result.shape_result = None;
      item_result.inline_size = LayoutUnit::ZERO;
    }

    if position_was_saturated {
      self.position = line_info.compute_width();
    }

    self.trailing_whitespace = WhitespaceState::Collapsed;
  }

  /// Blink's `TrailingCollapsibleSpaceWidth`: the trailing space's width, without removing it.
  fn trailing_collapsible_space_width(&mut self, line_info: &mut LineInfo<'a, 'c>) -> LayoutUnit {
    self.compute_trailing_collapsible_space(line_info);

    let Some(space) = &self.trailing_collapsible_space else {
      return LayoutUnit::ZERO;
    };
    let item_result = &line_info.results[space.item_result_index];

    // The new result is usually narrower, but a wider one means a negative trailing space.
    item_result.inline_size
      - space
        .collapsed_shape_result
        .as_ref()
        .map_or(LayoutUnit::ZERO, |shape_result| {
          shape_result.snapped_width()
        })
  }

  /// Blink's `ComputeTrailingCollapsibleSpace`: the trailing collapsible space, cached.
  fn compute_trailing_collapsible_space(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    if matches!(
      self.trailing_whitespace,
      WhitespaceState::Leading
        | WhitespaceState::None
        | WhitespaceState::Collapsed
        | WhitespaceState::Preserved
    ) {
      self.trailing_collapsible_space = None;

      return;
    }

    self.trailing_whitespace = WhitespaceState::None;

    if !self.compute_trailing_collapsible_space_helper(line_info) {
      self.trailing_collapsible_space = None;
    }
  }

  /// Blink's `ComputeTrailingCollapsibleSpaceHelper`: true once the trailing white space is known.
  fn compute_trailing_collapsible_space_helper(
    &mut self,
    line_info: &mut LineInfo<'a, 'c>,
  ) -> bool {
    let text = self.text();

    for index in (0..line_info.results.len()).rev() {
      let item_result = &line_info.results[index];
      let item = item_result.item;

      if item.end_collapse_type == CollapseType::OpaqueToCollapsing {
        continue;
      }

      if item.item_type == InlineItemType::Text {
        if item_result.length() == 0 {
          continue;
        }

        let last = char_before(text, item_result.end_offset());

        if is_other_space_separator(last) {
          self.trailing_whitespace = WhitespaceState::Preserved;
          self.trailing_collapsible_space = None;

          return true;
        }

        if !is_breakable_space(last) {
          self.trailing_collapsible_space = None;

          return true;
        }

        if item.style().is_some_and(|style| {
          style
            .inherited_data
            .white_space_collapse
            .should_preserve_white_spaces()
        }) {
          self.trailing_whitespace = WhitespaceState::Preserved;
          self.trailing_collapsible_space = None;

          return true;
        }

        // An overflow under `break-word` has no result.
        if item_result.shape_result.is_none() {
          self.trailing_collapsible_space = None;

          return true;
        }

        if self
          .trailing_collapsible_space
          .as_ref()
          .is_none_or(|space| space.item_result_index != index)
        {
          let collapsed_shape_result = item_result
            .shape_result
            .as_ref()
            .filter(|_| item_result.end_offset() - 1 > item_result.start_offset())
            .map(|source_result| {
              self.truncate_line_end_result(
                line_info,
                index,
                source_result,
                item_result.end_offset() - 1,
              )
            });

          self.trailing_collapsible_space = Some(TrailingCollapsibleSpace {
            item_result_index: index,
            collapsed_shape_result,
          });
        }

        self.trailing_whitespace = WhitespaceState::Collapsible;

        return true;
      }

      if item.item_type == InlineItemType::Control {
        if item.text_type == TextItemType::ForcedLineBreak {
          debug_assert_eq!(char_at(text, item.start_offset()), LINE_FEED);

          continue;
        }

        self.trailing_whitespace = WhitespaceState::Preserved;
        self.trailing_collapsible_space = None;

        return true;
      }

      self.trailing_collapsible_space = None;

      return true;
    }

    false
  }

  /// Blink's `SplitTrailingBidiPreservedSpace`: per UAX #9 rule L1, spaces at a line's end take the
  /// paragraph's level, so they split into results of their own, marked as only trailing spaces.
  fn split_trailing_bidi_preserved_space(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    if matches!(
      self.trailing_whitespace,
      WhitespaceState::Leading | WhitespaceState::None
    ) || !self.items_data.is_bidi_enabled
      || self.mode == LineBreakerMode::MinContent
    {
      return;
    }

    let text = self.text();
    let base_level = self.base_direction.bidi_level();

    for result_index in (0..line_info.results.len()).rev() {
      let item_result = &mut line_info.results[result_index];
      let item = item_result.item;

      if item_result.has_only_bidi_trailing_spaces
        || item.end_collapse_type == CollapseType::OpaqueToCollapsing
        || item.text_type == TextItemType::ForcedLineBreak
      {
        continue;
      }

      if !matches!(
        item.item_type,
        InlineItemType::Text | InlineItemType::Control
      ) {
        return;
      }

      if item_result.length() == 0 {
        item_result.has_only_bidi_trailing_spaces = true;

        continue;
      }

      let mut i = item_result.end_offset();

      while i > item_result.start_offset() {
        let previous = char_before(text, i);

        if !is_breakable_space(previous) && !is_bidi_trailing_space(previous) {
          break;
        }

        i -= previous.len_utf8();
      }

      if i == item_result.start_offset() {
        item_result.has_only_bidi_trailing_spaces = true;

        continue;
      }

      if i == item_result.end_offset() {
        break;
      }

      // Split only when the item's level is not the paragraph's, which a higher level of the
      // same direction also needs.
      if item.bidi_level() != base_level {
        let Some(source_shape_result) = item_result.shape_result.clone() else {
          break;
        };
        let previous_inline_size = item_result.inline_size;
        let start = item_result.start_offset();
        let end = item_result.end_offset();
        let view =
          ShapeResultView::create_from_view_range(&source_shape_result, start as u32, i as u32);

        item_result.text_offset.end = i;
        item_result.inline_size = view.snapped_width();
        item_result.shape_result = Some(Rc::new(view));

        debug_assert!(item_result.inline_size <= previous_inline_size);

        let mut spaces_result = InlineItemResult::new(
          item,
          item_result.item_index,
          i..end,
          item_result.break_anywhere_if_overflow,
          item_result.should_create_line_box,
          item_result.has_unpositioned_floats,
        );

        spaces_result.has_only_bidi_trailing_spaces = true;
        spaces_result.shape_result = Some(Rc::new(ShapeResultView::create_from_view_range(
          &source_shape_result,
          i as u32,
          end as u32,
        )));
        spaces_result.inline_size = previous_inline_size - item_result.inline_size;
        line_info.results.insert(result_index + 1, spaces_result);
      }

      break;
    }
  }

  /// Blink's `HandleForcedLineBreak`, at the forced break item `item_index`, or before a
  /// block-in-inline without one.
  fn handle_forced_line_break(
    &mut self,
    item_index: Option<usize>,
    line_info: &mut LineInfo<'a, 'c>,
  ) {
    // The last item may have overflowed.
    if self.handle_overflow_if_needed(line_info) {
      return;
    }

    if let Some(item_index) = item_index {
      let item = &self.items()[item_index];

      debug_assert_eq!(item.text_type, TextItemType::ForcedLineBreak);
      debug_assert_eq!(char_at(self.text(), item.start_offset()), LINE_FEED);

      let result_index = self.add_item(item_index, line_info);
      let item_result = &mut line_info.results[result_index];

      item_result.should_create_line_box = true;
      item_result.has_only_pre_wrap_trailing_spaces = true;
      item_result.has_only_bidi_trailing_spaces = true;
      item_result.can_break_after = true;
      self.move_to_next_of_item(item_index);

      // Close tags after the break join this line, which their edges show.
      while !self.is_at_end() {
        let next_index = self.current.item_index;
        let next_item = &self.items()[next_index];

        if next_item.item_type == InlineItemType::CloseTag {
          self.handle_close_tag(next_index, line_info);

          continue;
        }

        if next_item.item_type == InlineItemType::Text && next_item.length() == 0 {
          self.handle_empty_text(next_index, line_info);

          continue;
        }

        break;
      }
    }

    if self.hyphen_index.is_some() {
      let hyphen_inline_size = self.remove_hyphen(&mut line_info.results);

      self.position -= hyphen_inline_size;
    }

    self.is_forced_break = true;
    line_info.has_forced_break = true;
    line_info.is_last_line = true;
    self.state = LineBreakState::Done;
  }

  /// Blink's `HandleControlItem`: newlines and tabs affect layout like text, without shaping.
  fn handle_control_item(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let item = &self.items()[item_index];

    debug_assert!(item.length() >= 1);

    if item.text_type == TextItemType::ForcedLineBreak {
      self.handle_forced_line_break(Some(item_index), line_info);

      return;
    }

    debug_assert_eq!(item.text_type, TextItemType::FlowControl);

    match char_at(self.text(), item.start_offset()) {
      TAB => {
        let Some(object) = item.layout_object else {
          self.handle_empty_text(item_index, line_info);

          return;
        };
        // The item's font must have a primary font; tab stops come from the block's, as Blink's
        // `InlineNode::FontForTab`.
        if self
          .items_data
          .fonts
          .get(object.context())
          .primary_font()
          .is_none()
        {
          self.handle_empty_text(item_index, line_info);

          return;
        }

        let font = self.items_data.fonts.get(self.block_context());
        let tab_size = object.style().misc_inherited1_data.tab_size.spaces() as f32;
        // Tab stops count from the container's edge, not from the floats beside the line.
        let shape_result = Rc::new(ShapeResult::create_for_tabulation_characters(
          &font,
          item.direction(),
          tab_size,
          (self.position + self.compute_float_offset()).to_float(),
          item.start_offset() as u32,
          item.length() as u32,
        ));

        self.handle_text(item_index, &shape_result, line_info);

        return;
      }
      ZERO_WIDTH_SPACE => {
        // `<wbr>` breaks whatever the wrapping. A generated opportunity makes no fragment, but
        // rewinding finds it.
        let result_index = self.add_item(item_index, line_info);
        let item_result = &mut line_info.results[result_index];

        if !item.is_generated_for_line_break {
          item_result.should_create_line_box = true;
        }

        item_result.can_break_after = true;
      }
      CARRIAGE_RETURN | FORM_FEED => {
        // <https://drafts.csswg.org/css-text-3/#white-space-processing>
        self.handle_empty_text(item_index, line_info);

        return;
      }
      _ => unreachable!("a control item for another character"),
    }

    self.move_to_next_of_item(item_index);
  }

  /// Blink's `HandleBidiControlItem`: entering characters act as open tags, popping ones as close
  /// tags.
  fn handle_bidi_control_item(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let item = &self.items()[item_index];

    debug_assert_eq!(
      item.length(),
      char_at(self.text(), item.start_offset()).len_utf8()
    );

    let character = char_at(self.text(), item.start_offset());
    let is_pop = character == POP_DIRECTIONAL_ISOLATE || character == POP_DIRECTIONAL_FORMATTING;

    if is_pop {
      if line_info.results.is_empty() {
        self.add_item(item_index, line_info);
      } else {
        let result_index = self.add_item(item_index, line_info);
        let results = &mut line_info.results;

        // Keep a break after the last item, which break-after-space may have set.
        if results[result_index - 1].can_break_after {
          results[result_index].can_break_after = true;
          results[result_index - 1].can_break_after = false;
        } else {
          results[result_index].can_break_after = self.auto_wrap
            && self
              .break_iterator
              .is_breakable(results[result_index].end_offset());
        }
      }
    } else {
      if self.state == LineBreakState::Trailing && can_break_after_last(&line_info.results) {
        debug_assert!(!line_info.is_last_line);

        self.move_to_next_of_item(item_index);
        self.state = LineBreakState::Done;

        return;
      }

      if line_info.results.is_empty() {
        self.add_item(item_index, line_info);
      } else {
        let result_index = self.add_item(item_index, line_info);
        let result = &mut line_info.results[result_index];

        result.can_break_after =
          self.auto_wrap && self.break_iterator.is_breakable(result.end_offset());
      }
    }

    self.move_to_next_of_item(item_index);
  }

  /// Blink's `HandleAtomicInline`.
  fn handle_atomic_inline(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let item = &self.items()[item_index];
    let Some(object) = item.layout_object else {
      return;
    };
    let remaining_width = self.remaining_available_width();
    let mut ignore_overflow_if_negative_margin = false;

    if self.state == LineBreakState::Continue && remaining_width < LayoutUnit::ZERO {
      self.handle_overflow(line_info);

      if !line_info.has_overflow || item_index != self.current.item_index {
        return;
      }

      // Negative margins can put the position back.
      debug_assert_ne!(self.state, LineBreakState::Continue);

      ignore_overflow_if_negative_margin = true;
    }

    // Margins before overflow: negative ones may bring the item back onto the line.
    let result_index = self.add_item(item_index, line_info);
    let margins = line_margins(object.node());
    let inline_margins = margins.inline_sum();

    line_info.results[result_index].margins = margins;

    if ignore_overflow_if_negative_margin {
      debug_assert!(remaining_width < LayoutUnit::ZERO);

      if inline_margins >= remaining_width {
        line_info.results.pop();

        return;
      }

      // The line overflowed, but the negative margin puts the position back.
      self.state = LineBreakState::Continue;
      line_info.has_overflow = false;
    }

    // The last item may have ended with a hyphen, but more items follow.
    if self.hyphen_index.is_some() {
      let hyphen_inline_size = self.remove_hyphen(&mut line_info.results);

      self.position -= hyphen_inline_size;
    }

    let inline_size = if !self.is_computing_content_size() && self.mode == LineBreakerMode::Content
    {
      let layout_result = self.atomic_inline.layout(object.node());

      line_info.results[result_index].layout_result = Some(layout_result);
      layout_result.inline_size
    } else {
      // Blink's `ComputeMinMaxContentSizeForBlockChild`.
      let (min_size, max_size) = self.atomic_inline.min_max_sizes(object.node());

      if self.mode == LineBreakerMode::MinContent {
        min_size
      } else {
        max_size
      }
    };
    let item_result = &mut line_info.results[result_index];

    item_result.inline_size = inline_size + inline_margins;
    item_result.should_create_line_box = true;
    item_result.can_break_after = self.can_break_after_atomic_inline(item);
    self.position += item_result.inline_size;
    self.trailing_whitespace = WhitespaceState::None;
    self.move_to_next_of_item(item_index);
  }

  /// Blink's `HandleBlockInInline`: the block takes a line of its own, a forced break before and
  /// after it.
  fn handle_block_in_inline(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    if !line_info.results.is_empty() {
      self.force_non_empty_if_last_line = false;
      self.handle_forced_line_break(None, line_info);

      return;
    }

    let Some(object) = self.items()[item_index].layout_object else {
      return;
    };
    let result_index = self.add_item(item_index, line_info);

    if self.mode == LineBreakerMode::Content {
      let layout_result = self.atomic_inline.layout_block_in_inline(object.node());
      let item_result = &mut line_info.results[result_index];

      item_result.inline_size = layout_result.inline_size;
      item_result.should_create_line_box = !layout_result
        .collapsed_margins
        .is_some_and(|margins| margins.is_self_collapsing);
      item_result.layout_result = Some(layout_result);
    } else {
      // Blink's `ComputeMinMaxContentSizeForBlockChild`.
      let (min_size, max_size) = self.atomic_inline.min_max_sizes(object.node());

      line_info.results[result_index].inline_size = if self.mode == LineBreakerMode::MinContent {
        min_size
      } else {
        max_size
      };
    }

    self.position += line_info.results[result_index].inline_size;
    line_info.is_block_in_inline = true;
    line_info.has_forced_break = true;
    self.is_forced_break = true;
    self.trailing_whitespace = WhitespaceState::None;
    self.move_to_next_of_item(item_index);
    self.state = LineBreakState::Done;
  }

  /// Blink's `ShouldPushFloatAfterLine`: whether the float goes below this line.
  fn should_push_float_after_line(
    &mut self,
    unpositioned_float: &mut UnpositionedFloat<'c>,
    line_info: &mut LineInfo<'a, 'c>,
  ) -> bool {
    let inline_margin_size = compute_margin_box_inline_size_for_unpositioned_float(
      unpositioned_float,
      &mut *self.atomic_inline,
    );
    let used_size = self.position
      + inline_margin_size
      + self.compute_float_ancestor_inline_end_size(self.current.item_index);
    let available_float_inline_size = self
      .line_opportunity
      .available_float_inline_size()
      .add_epsilon();
    // Trailing spaces collapse across floats when the line breaks here.
    let can_fit_float = used_size <= available_float_inline_size
      || used_size - self.trailing_collapsible_space_width(line_info)
        <= available_float_inline_size;
    let bfc_block_offset = unpositioned_float.origin_bfc_offset.block_offset;

    // Below the line when it does not fit, or the top edge alignment rule or clearance moves it
    // down.
    !can_fit_float
      || self.exclusion_space.last_float_block_start() > bfc_block_offset
      || self
        .exclusion_space
        .clearance_offset(unpositioned_float.clear_type())
        > bfc_block_offset
  }

  /// Blink's `ComputeFloatAncestorInlineEndSize`: the inline end edges of the boxes a float closes
  /// before anything else, which count against its line.
  fn compute_float_ancestor_inline_end_size(&self, item_index: usize) -> LayoutUnit {
    let mut inline_end_size = LayoutUnit::ZERO;

    for item in &self.items()[item_index..] {
      if item.item_type == InlineItemType::CloseTag {
        if let Some(object) = item.layout_object {
          inline_end_size += compute_inline_end_size(object.node());
        }

        continue;
      }

      // An open tag, even an empty one, or any content, lets the float join the line.
      if item.item_type == InlineItemType::OpenTag || !item.is_empty_item {
        break;
      }
    }

    inline_end_size
  }

  /// Blink's `HandleFloat`: places the float on this line when it fits, and narrows the line by
  /// it; otherwise it goes below the line.
  fn handle_float(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let item = &self.items()[item_index];
    let result_index = self.add_item(item_index, line_info);
    let item_result = &mut line_info.results[result_index];

    // Out-of-flow boxes take no part in text processing.
    // <https://drafts.csswg.org/css-text-3/#text-encoding>
    debug_assert!(!item_result.can_break_after);

    // Breaking after leading floats is historical behavior, compatible with WebKit.
    if !item_result.should_create_line_box {
      item_result.can_break_after = self.auto_wrap;
    }

    self.move_to_next_of_item(item_index);

    // Intrinsic sizes only collect the floats.
    if self.is_computing_content_size() || self.mode != LineBreakerMode::Content {
      return;
    }

    // A leading float was placed before the line broke.
    if self.current.item_index <= self.leading_floats.handled_index
      && !self.leading_floats.floats.is_empty()
    {
      let positioned_float = self.leading_floats.floats[self.leading_floats_index];
      let item_result = &mut line_info.results[result_index];

      self.leading_floats_index += 1;
      item_result.positioned_float = Some(positioned_float);
      // Kept even though a leading float leaves the space as it is; see `rewind_floats`.
      item_result.exclusion_space_before_position_float = Some(self.exclusion_space.clone());

      return;
    }

    let Some(object) = item.layout_object else {
      return;
    };
    let mut unpositioned_float = UnpositionedFloat::new(
      object.node(),
      self.constraint_space.inline_size(),
      BfcOffset::new(
        self.constraint_space.bfc_offset.line_offset,
        self.line_opportunity.bfc_block_offset,
      ),
      self.constraint_space.direction,
    );
    let float_after_line = self.should_push_float_after_line(&mut unpositioned_float, line_info);

    // A float sits no higher than one before it.
    if line_info.results[result_index].has_unpositioned_floats || float_after_line {
      line_info.results[result_index].has_unpositioned_floats = true;

      return;
    }

    let exclusion_space_before = self.exclusion_space.clone();
    let positioned_float = position_float(
      &mut unpositioned_float,
      self.exclusion_space,
      &mut *self.atomic_inline,
    );
    let item_result = &mut line_info.results[result_index];

    item_result.exclusion_space_before_position_float = Some(exclusion_space_before);
    item_result.positioned_float = Some(positioned_float);
    self.update_line_opportunity();
  }

  /// Blink's `ComputeFloatOffset`: how far the floats on the line's start side move it from the
  /// container's edge.
  fn compute_float_offset(&self) -> LayoutUnit {
    let Some(available_inline_size) = self.constraint_space.available_inline_size else {
      return LayoutUnit::ZERO;
    };
    let bfc_left = self.constraint_space.bfc_offset.line_offset;
    let bfc_right = bfc_left + available_inline_size;

    match self.base_direction {
      TextDirection::Ltr => {
        (self.line_opportunity.line_left_offset - bfc_left).clamp_negative_to_zero()
      }
      TextDirection::Rtl => {
        (bfc_right - self.line_opportunity.line_right_offset).clamp_negative_to_zero()
      }
    }
  }

  /// Blink's `UpdateLineOpportunity`: the line's extent beside the floats placed so far.
  fn update_line_opportunity(&mut self) {
    let bfc_line_offset = self.constraint_space.bfc_offset.line_offset;
    let available_inline_size = self.constraint_space.inline_size();
    let direction = self.constraint_space.direction;
    let opportunity = self.exclusion_space.find_layout_opportunity(
      BfcOffset::new(bfc_line_offset, self.line_opportunity.bfc_block_offset),
      available_inline_size,
      direction,
      LayoutUnit::ZERO,
    );

    debug_assert_eq!(
      self.line_opportunity.bfc_block_offset,
      opportunity.rect.block_start_offset()
    );

    self.line_opportunity = opportunity.compute_line_layout_opportunity(
      bfc_line_offset,
      available_inline_size,
      direction,
      self.line_opportunity.line_block_size,
      LayoutUnit::ZERO,
    );
    self.update_available_width();
  }

  /// Blink's `RewindFloats`: the exclusion space before the first float in the rewound results.
  fn rewind_floats(&mut self, new_end: usize, line_info: &LineInfo<'a, 'c>) {
    let Some(item_result) = line_info.results[new_end..]
      .iter()
      .find(|result| result.positioned_float.is_some())
    else {
      return;
    };

    // A leading float is handed out again.
    if item_result.item_index < self.leading_floats.handled_index
      && let Some(index) = self
        .leading_floats
        .floats
        .iter()
        .position(|leading_float| Some(*leading_float) == item_result.positioned_float)
    {
      self.leading_floats_index = index;
    }

    if let Some(exclusion_space) = &item_result.exclusion_space_before_position_float {
      *self.exclusion_space = exclusion_space.clone();
      self.update_line_opportunity();
    }
  }

  /// Blink's `HandleOutOfFlowPositioned`.
  fn handle_out_of_flow_positioned(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let result_index = self.add_item(item_index, line_info);
    let results = &mut line_info.results;

    // Out-of-flow boxes take no part in text processing, but which side of a break they take is not
    // defined.
    debug_assert!(!results[result_index].can_break_after);

    if result_index >= 1 {
      let last = &results[result_index - 1];

      results[result_index].can_break_after = if last.is_empty_text() && !last.can_break_after {
        // "Text+SP OP Empty OOF Text" breaks after; "Text+SP OP OOF Text" does not.
        self.auto_wrap
          && self
            .break_iterator
            .is_breakable(results[result_index].end_offset())
      } else {
        last.can_break_after
      };
    }

    self.move_to_next_of_item(item_index);
  }

  /// Blink's `ComputeOpenTagResult`: true when the tag has an edge that may take inline size.
  pub(crate) fn compute_open_tag_result(
    item: &InlineItem<'_>,
    item_result: &mut InlineItemResult<'_, '_>,
  ) -> bool {
    debug_assert_eq!(item.item_type, InlineItemType::OpenTag);

    let Some(object) = item.layout_object else {
      return false;
    };
    let node = object.node();

    if !item.should_create_box_fragment {
      return false;
    }

    let borders = line_borders(node);
    let padding = line_padding(node);
    let margins = line_margins(node);
    let has_edge = |strut: &LineBoxStrut| *strut != LineBoxStrut::default();

    if !has_edge(&borders) && !has_edge(&padding) && !has_edge(&margins) {
      return false;
    }

    item_result.borders = borders;
    item_result.padding = padding;
    item_result.margins = margins;
    item_result.inline_size = margins.inline_start + borders.inline_start + padding.inline_start;

    true
  }

  /// Blink's `HandleOpenTag`.
  fn handle_open_tag(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let item = &self.items()[item_index];
    let result_index = self.add_item(item_index, line_info);
    let Some(object) = item.layout_object else {
      self.move_to_next_of_item(item_index);

      return;
    };

    if Self::compute_open_tag_result(item, &mut line_info.results[result_index]) {
      let inline_size = line_info.results[result_index].inline_size;

      // A negative margin may bring the position back.
      if inline_size < LayoutUnit::ZERO && self.state == LineBreakState::Trailing {
        let available_width = self.available_width_to_fit();

        if self.position > available_width && self.position + inline_size <= available_width {
          self.state = LineBreakState::Continue;
        }
      }

      self.position += inline_size;

      // Inline edges keep a line box from being empty, since they affect line heights.
      let item_result = &mut line_info.results[result_index];

      if !item_result.should_create_line_box && !item.is_empty_item {
        item_result.should_create_line_box = true;
      }
    }

    if object.style().box_data.box_decoration_break == BoxDecorationBreak::Clone {
      let item_result = &line_info.results[result_index];

      // Counted even without edges.
      self.has_cloned_box_decorations = true;
      self.disable_score_line_break = true;
      self.cloned_box_decorations_count += 1;
      self.cloned_box_decorations_end_size += item_result.margins.inline_end
        + item_result.borders.inline_end
        + item_result.padding.inline_end;
      self.update_available_width_from_base_available_width();
    }

    let was_auto_wrap = self.auto_wrap;

    self.set_current_style(object.context());
    self.move_to_next_of_item(item_index);

    debug_assert!(!line_info.results[result_index].can_break_after);

    if !was_auto_wrap
      && self.auto_wrap
      && line_info.results.len() >= 2
      && self.is_previous_item_of_type(InlineItemType::Text)
    {
      let previous = &mut line_info.results[result_index - 1];

      previous.can_break_after =
        self.auto_wrap && self.break_iterator.is_breakable(previous.end_offset());
    }
  }

  /// Blink's `HandleCloseTag`.
  fn handle_close_tag(&mut self, item_index: usize, line_info: &mut LineInfo<'a, 'c>) {
    let item = &self.items()[item_index];
    let result_index = self.add_item(item_index, line_info);

    if let Some(object) = item.layout_object {
      let inline_size = compute_inline_end_size(object.node());
      let item_result = &mut line_info.results[result_index];

      item_result.inline_size = inline_size;
      self.position += inline_size;

      if !item_result.should_create_line_box && !item.is_empty_item {
        item_result.should_create_line_box = true;
      }

      if object.style().box_data.box_decoration_break == BoxDecorationBreak::Clone {
        debug_assert!(self.cloned_box_decorations_count > 0);

        self.cloned_box_decorations_count -= 1;
        self.cloned_box_decorations_end_size -= inline_size;
        self.update_available_width_from_base_available_width();
      }
    }

    let was_auto_wrap = self.auto_wrap;
    let parent = self
      .items_data
      .parent_context(item_index, self.block_context());

    self.set_current_style(parent);
    self.move_to_next_of_item(item_index);

    // A break before a close tag moves after it, even when the tag is `nowrap`.
    if result_index < 1 {
      return;
    }

    let text = self.text();
    let (before, rest) = line_info.results.split_at_mut(result_index);
    let last = &mut before[result_index - 1];
    let item_result = &mut rest[0];

    if last.can_break_after {
      item_result.can_break_after = true;
      last.can_break_after = false;

      return;
    }

    if was_auto_wrap {
      // Breaking before a breakable space when breaking before white space is allowed, or when
      // another breakable space precedes it.
      let end_offset = item_result.end_offset();
      let preceded_by_breakable_space =
        end_offset > 0 && is_breakable_space(char_before(text, end_offset));
      let current_style = self.current_style.map(|context| &context.style);

      item_result.can_break_after = is_breakable_space(char_at(text, end_offset))
        && (current_style.is_none_or(|style| !should_break_only_after_white_space(style))
          || preceded_by_breakable_space)
        && !self.is_next_non_bidi_control_item_open_tag();

      return;
    }

    if self.auto_wrap && !is_breakable_space(char_before(text, item_result.end_offset())) {
      item_result.can_break_after = self.break_iterator.is_breakable(item_result.end_offset());
    }
  }

  /// Blink's `HandleOverflow`: the last item does not fit, and no item breaks before it.
  fn handle_overflow(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    let available_width = self.available_width_to_fit();

    debug_assert!(self.position > available_width);

    // The hyphenation before any change.
    let hyphen_index_before = self.hyphen_index;

    if self.hyphen_index.is_some() {
      let hyphen_inline_size = self.remove_hyphen(&mut line_info.results);

      self.position -= hyphen_inline_size;
    }

    // Items fit once this goes negative.
    let mut width_to_rewind = self.position - available_width;
    // The shortest break opportunity.
    let mut break_before = 0;
    let mut has_break_anywhere_if_overflow = self.break_anywhere_if_overflow;
    let mut i = line_info.results.len();

    // An opportunity that fits.
    while i > 0 {
      i -= 1;

      let item_result = &line_info.results[i];

      has_break_anywhere_if_overflow |= item_result.break_anywhere_if_overflow;

      // Break after this item.
      if i < line_info.results.len() - 1 && item_result.can_break_after {
        if width_to_rewind <= LayoutUnit::ZERO {
          self.position = available_width + width_to_rewind;
          self.rewind_overflow(i + 1, line_info);

          return;
        }

        break_before = i + 1;
      }

      // The position without this item.
      width_to_rewind -= item_result.inline_size;

      if width_to_rewind > LayoutUnit::ZERO {
        continue;
      }

      let item = item_result.item;

      if item.item_type != InlineItemType::Text || item_result.length() == 0 {
        continue;
      }

      // Part of breakable text may fit.
      if width_to_rewind < LayoutUnit::ZERO && item_result.may_break_inside {
        let item_available_width = -width_to_rewind;
        // Narrower than now, so the break is not at the end when the text fits but what follows
        // does not.
        let min_available_width = item_result.inline_size - LayoutUnit::from_int(1);

        // `font-size: 0` cannot narrow: the previous opportunity.
        if min_available_width <= LayoutUnit::ZERO {
          if self.break_text_at_previous_break_opportunity(&mut line_info.results, i) {
            self.rewind_overflow(i + 1, line_info);

            return;
          }

          continue;
        }

        let was_current_style = self.current_style;

        if let Some(object) = item.layout_object {
          self.set_current_style(object.context());
        }

        let item_result_before = line_info.results[i].clone();
        let Some(shape_result) = item.shape_result.clone() else {
          continue;
        };

        self.break_text(
          i,
          item.index,
          &shape_result,
          item_available_width.min(min_available_width),
          item_available_width,
          line_info,
        );

        let item_result = &line_info.results[i];

        // Narrowed to fit: break here.
        if item_result.can_break_after
          && item_result.inline_size <= item_available_width
          && item_result.end_offset() < item_result_before.end_offset()
        {
          let new_end = i + 1;

          // The last item: fit its end.
          if new_end == line_info.results.len() {
            self.position = available_width + width_to_rewind + item_result.inline_size;
            self.current = item_result.end();
            self.handle_trailing_spaces(item.index, Some(&shape_result), line_info);

            return;
          }

          self.state = LineBreakState::Trailing;
          self.rewind(new_end, line_info);

          return;
        }

        // Did not fit: restore. Reshaping or rounding may even have moved the end later.
        if self.hyphen_index.is_some() {
          self.remove_hyphen(&mut line_info.results);
        }

        line_info.results[i] = item_result_before;

        if let Some(style) = was_current_style {
          self.set_current_style(style);
        }
      }
    }

    // With only leading floats, the indent rewinds too, so it alone does not overflow.
    if self.applied_text_indent != LayoutUnit::ZERO
      && width_to_rewind > LayoutUnit::ZERO
      && self.is_first_formatted_line
      && !self.leading_floats.floats.is_empty()
    {
      self.position -= self.applied_text_indent;
      width_to_rewind -= self.applied_text_indent;
      self.applied_text_indent = LayoutUnit::ZERO;

      if width_to_rewind <= LayoutUnit::ZERO {
        self.state = LineBreakState::Done;

        return;
      }
    }

    // No rewind point: `overflow-wrap` retries breaking anywhere.
    if !self.override_break_anywhere && has_break_anywhere_if_overflow {
      self.override_break_anywhere = true;
      self.retry_after_overflow(line_info);

      return;
    }

    // The line overflows.
    line_info.has_overflow = true;
    self.disable_score_line_break = true;
    self.disable_bisect_line_break = true;

    // The hyphenation before the loop, if its item is still there.
    debug_assert!(self.hyphen_index.is_none());

    if let Some(hyphen_index) = hyphen_index_before
      && hyphen_index < line_info.results.len()
    {
      let hyphen_inline_size = self.add_hyphen(&mut line_info.results, hyphen_index);

      self.position += hyphen_inline_size;
    }

    // An earlier opportunity stops the overflow.
    if break_before != 0 {
      self.rewind_overflow(break_before, line_info);

      return;
    }

    if can_break_after_last(&line_info.results) {
      self.state = LineBreakState::Trailing;

      return;
    }

    // No opportunity: break at the earliest one.
    debug_assert!(
      line_info
        .results
        .iter()
        .all(|result| !result.can_break_after)
    );

    self.state = LineBreakState::Overflow;
  }

  /// Blink's `RetryAfterOverflow`: rewinds every item to break the line again.
  fn retry_after_overflow(&mut self, line_info: &mut LineInfo<'a, 'c>) {
    self.disable_score_line_break = true;
    self.disable_bisect_line_break = true;
    self.state = LineBreakState::Continue;

    // The style applies again, as the retry breaks under other conditions.
    if line_info.results.is_empty() {
      if let Some(style) = self.current_style {
        self.set_current_style_force(style);
      }
    } else {
      let style = self.compute_current_style(0, line_info);

      self.set_current_style_force(style);
      self.rewind(0, line_info);
    }

    self.last_rewind = None;
  }

  /// Blink's `RewindOverflow`: rewinds to `new_end`, keeping the trailable items there.
  fn rewind_overflow(&mut self, mut new_end: usize, line_info: &mut LineInfo<'a, 'c>) {
    debug_assert!(new_end < line_info.results.len());

    let text = self.text();
    let mut open_tag_count = 0_usize;
    let mut index = new_end;
    // Blink's `ShouldWrapLine() && !ShouldBreakSpaces()`: white space that trails a line.
    let trails = |item: &InlineItem<'_>| {
      item.style().is_some_and(|style| {
        style.inherited_data.text_wrap_mode.should_wrap_line()
          && !style
            .inherited_data
            .white_space_collapse
            .should_break_spaces()
      })
    };

    while index < line_info.results.len() {
      let item_result = &line_info.results[index];
      let item = item_result.item;

      match item.item_type {
        InlineItemType::Text => {
          // Text is trailable when it starts with trailable spaces.
          if item_result.length() == 0 {
            index += 1;

            continue;
          }

          if (item_result.shape_result.is_some()
            || (self.break_anywhere_if_overflow && !self.override_break_anywhere))
            && trails(item)
            && is_breakable_space(char_at(text, item_result.start_offset()))
          {
            // All trailable spaces: check the next item.
            if item_result.shape_result.is_some()
              && is_all_breakable_spaces(
                text,
                item_result.start_offset() + 1,
                item_result.end_offset(),
              )
            {
              index += 1;

              continue;
            }

            // Spaces then other characters: break after the spaces, which the next `handle_text`
            // handles.
            self.state = LineBreakState::Trailing;
            self.rewind(index, line_info);

            return;
          }
        }
        InlineItemType::Control => {
          // Every control but a newline trails a wrapping line.
          debug_assert_ne!(char_at(text, item_result.start_offset()), LINE_FEED);

          if trails(item) {
            index += 1;

            continue;
          }
        }
        InlineItemType::OpenTag => {
          // Ambiguous: "<span>text" does not trail, "<span> text" does. The nesting level marks
          // where it was zero.
          if open_tag_count == 0 {
            new_end = index;
          }

          open_tag_count += 1;
          index += 1;

          continue;
        }
        InlineItemType::CloseTag => {
          open_tag_count = open_tag_count.saturating_sub(1);
          index += 1;

          continue;
        }
        item_type if is_trailable_item_type(item_type) => {
          index += 1;

          continue;
        }
        _ => {}
      }

      // Not trailable: rewind to before it, or before the open tag at nesting level zero.
      if open_tag_count != 0 {
        index = new_end;
      }

      self.state = LineBreakState::Done;
      debug_assert!(!line_info.is_last_line);
      self.rewind(index, line_info);

      return;
    }

    // An open tag at a non-zero nesting level does not trail.
    if open_tag_count != 0 {
      self.state = LineBreakState::Done;
      debug_assert!(!line_info.is_last_line);
      self.rewind(new_end, line_info);

      return;
    }

    // Everything trails.
    self.trailing_whitespace = WhitespaceState::Unknown;
    self.position = line_info.compute_width();
    self.state = LineBreakState::Done;
    debug_assert!(!line_info.is_last_line);

    if self.is_at_end() {
      line_info.is_last_line = true;
    }
  }

  /// Blink's `Rewind`.
  fn rewind(&mut self, new_end: usize, line_info: &mut LineInfo<'a, 'c>) {
    debug_assert!(new_end < line_info.results.len());

    // Rewinding to the same index twice is a loop.
    if let Some(last_rewind) = self.last_rewind {
      assert!(
        last_rewind != (self.current.item_index, new_end),
        "the line breaker rewinds in a loop"
      );

      self.last_rewind = Some((self.current.item_index, new_end));
    }

    // Floats placed in the rewound results come out of the exclusion space.
    self.rewind_floats(new_end, line_info);

    if new_end != 0 {
      // The previous result's end, which truncation may have moved before the next one's start.
      self.move_to_next_of_result(new_end - 1, line_info);
      self.trailing_whitespace = WhitespaceState::Unknown;

      // Empty text after a space breaks there. <http://crbug.com/1104534>
      while !self.is_at_end() {
        let index = self.current.item_index;
        let item = &self.items()[index];

        if item.item_type != InlineItemType::Text || item.length() != 0 {
          break;
        }

        self.handle_empty_text(index, line_info);
      }
    } else {
      self.current = line_info.start;
      self.trailing_whitespace = WhitespaceState::Leading;
    }

    let style = self.compute_current_style(new_end, line_info);

    self.set_current_style(style);
    line_info.results.truncate(new_end);
    self.trailing_collapsible_space = None;

    if self
      .hyphen_index
      .is_some_and(|hyphen_index| hyphen_index >= new_end)
    {
      self.hyphen_index = None;
    }

    if self.hyphen_index.is_none() && self.has_any_hyphens {
      self.restore_last_hyphen(&mut line_info.results);
    }

    self.position = line_info.compute_width();

    if self.has_cloned_box_decorations {
      self.recalc_cloned_box_decorations();
    }
  }

  /// Blink's `ComputeCurrentStyle`: the style at result `item_result_index`, for a rewind.
  fn compute_current_style(
    &self,
    item_result_index: usize,
    line_info: &LineInfo<'a, 'c>,
  ) -> &'c RenderContext {
    let results = &line_info.results;
    let item = results[item_result_index].item;

    if matches!(
      item.item_type,
      InlineItemType::Text | InlineItemType::CloseTag
    ) && let Some(object) = item.layout_object
    {
      return object.context();
    }

    // An item before it that knows the style.
    for result in results[..item_result_index].iter().rev() {
      let item = result.item;

      match item.item_type {
        InlineItemType::Text | InlineItemType::OpenTag => {
          if let Some(object) = item.layout_object {
            return object.context();
          }
        }
        InlineItemType::CloseTag => {
          return self
            .items_data
            .parent_context(result.item_index, self.block_context());
        }
        _ => {}
      }
    }

    // The style at the line's start.
    self
      .break_token
      .and_then(|break_token| break_token.style)
      .unwrap_or(line_info.line_style)
  }

  /// Blink's `SetCurrentStyle`.
  fn set_current_style(&mut self, style: &'c RenderContext) {
    if self
      .current_style
      .is_some_and(|current_style| std::ptr::eq(current_style, style))
    {
      return;
    }

    self.set_current_style_force(style);
  }

  /// Blink's `SetCurrentStyleForce`: wrapping and breaking rules from `style`.
  fn set_current_style_force(&mut self, context: &'c RenderContext) {
    self.current_style = Some(context);

    let style = &context.style;

    self.auto_wrap = Self::should_auto_wrap(style);

    if !self.auto_wrap {
      return;
    }

    let line_break = style.misc_inherited_data.line_break;
    let disable_soft_hyphen = style.misc_inherited_data.hyphens == Hyphens::None;
    let break_space = if style
      .inherited_data
      .white_space_collapse
      .should_break_spaces()
    {
      self.disable_score_line_break = true;

      BreakSpaceType::AfterEverySpace
    } else {
      BreakSpaceType::AfterSpaceRun
    };

    if line_break == LineBreak::Anywhere {
      self.break_anywhere_if_overflow = false;
      self.set_break_iterator(BreakOptions {
        break_type: LineBreakType::BreakCharacter,
        break_space,
        disable_soft_hyphen,
        ..BreakOptions::default()
      });

      return;
    }

    let mut line_break_type = match style.misc_inherited_data.word_break {
      WordBreak::Normal => {
        self.break_anywhere_if_overflow = false;

        LineBreakType::Normal
      }
      WordBreak::BreakAll => {
        self.break_anywhere_if_overflow = false;

        LineBreakType::BreakAll
      }
      WordBreak::BreakWord => {
        // Not with a line-clamp ellipsis, as `overflow-wrap`.
        self.break_anywhere_if_overflow = self.line_clamp_ellipsis_width == LayoutUnit::ZERO;

        LineBreakType::Normal
      }
      WordBreak::KeepAll => {
        self.break_anywhere_if_overflow = false;

        LineBreakType::KeepAll
      }
    };

    if !self.break_anywhere_if_overflow && self.line_clamp_ellipsis_width == LayoutUnit::ZERO {
      // `anywhere` changes layout and min-content; `break-word` only layout.
      let overflow_wrap = style.misc_inherited_data.overflow_wrap;

      self.break_anywhere_if_overflow = overflow_wrap.is_anywhere()
        || (overflow_wrap.is_break_word() && self.mode == LineBreakerMode::Content);
    }

    if self.break_anywhere_if_overflow {
      if self.override_break_anywhere {
        line_break_type = LineBreakType::BreakCharacter;
      } else if self.mode == LineBreakerMode::MinContent {
        self.override_break_anywhere = true;
        line_break_type = LineBreakType::BreakCharacter;
      }
    }

    self.set_break_iterator(BreakOptions {
      break_type: line_break_type,
      break_space,
      strictness: match line_break {
        LineBreak::Auto | LineBreak::Anywhere => LineBreakStrictness::Default,
        LineBreak::Loose => LineBreakStrictness::Loose,
        LineBreak::Normal => LineBreakStrictness::Normal,
        LineBreak::Strict => LineBreakStrictness::Strict,
      },
      disable_soft_hyphen,
    });
  }

  /// The break iterator of `options`, as Blink's `SetBreakType`, `SetBreakSpace`, and
  /// `SetStrictness` switch one lazily.
  fn set_break_iterator(&mut self, options: BreakOptions) {
    if self.break_iterator.options != options {
      self.break_iterator = Rc::new(self.items_data.break_iterator(options));
    }
  }

  /// Blink's `IsPreviousItemOfType`.
  fn is_previous_item_of_type(&self, item_type: InlineItemType) -> bool {
    self.current.item_index > 0 && self.items()[self.current.item_index - 1].item_type == item_type
  }

  /// Blink's `IsNextNonBidiControlItemOpenTag`.
  fn is_next_non_bidi_control_item_open_tag(&self) -> bool {
    self.items()[self.current.item_index..]
      .iter()
      .map(|item| item.item_type)
      .find(|&item_type| item_type != InlineItemType::BidiControl)
      == Some(InlineItemType::OpenTag)
  }

  /// Blink's `MoveToNextOf` an item.
  fn move_to_next_of_item(&mut self, item_index: usize) {
    self.current.text_offset = self.items()[item_index].end_offset();
    self.current.item_index += 1;
  }

  /// Blink's `MoveToNextOf` a result.
  fn move_to_next_of_result(&mut self, result_index: usize, line_info: &LineInfo<'a, 'c>) {
    let item_result = &line_info.results[result_index];

    self.current = item_result.end();

    if self.current.text_offset == item_result.item.end_offset() {
      self.current.item_index += 1;
    }
  }

  /// Blink's `AppendCandidates`: the break candidates in the text result `result_index` of
  /// `line_info`, without hyphenation dictionaries.
  pub(crate) fn append_candidates(
    &mut self,
    result_index: usize,
    line_info: &LineInfo<'a, 'c>,
    context: &mut LineBreakCandidateContext<'_, 'a, 'c>,
  ) {
    let item_result = &line_info.results[result_index];
    let item = item_result.item;
    let item_index = item_result.item_index;
    let index = |text_offset| InlineItemTextIndex::new(item_index, text_offset);

    debug_assert_eq!(item.item_type, InlineItemType::Text);

    // Fully collapsed spaces have no break opportunities.
    if item.length() == 0 {
      let state = if item_result.can_break_after {
        CandidateState::Break
      } else {
        context.state
      };

      context.append_trailing_spaces(state, index(item.end_offset()), context.position());
      context.set_last(item, item.end_offset());

      return;
    }

    let Some(shape_result) = item.shape_result.as_deref() else {
      return;
    };
    let mut positions = ShapeResultPositions::new(shape_result);
    let text = self.text();
    // Up to the item's end or the line's, whichever is first, for the trailing spaces the line
    // breaker removed.
    let end = item_result
      .end_offset()
      .max(item.end_offset().min(line_info.end_text_offset()));
    let mut start = item_result.start_offset();

    // From where the last item ended, past spaces skipped or already handled.
    if context.last_item.is_some() {
      if context.last_end_offset >= end {
        return;
      }

      start = context.last_end_offset;
      positions.set_base_position(start, context.position());

      if is_breakable_space(char_at(text, start)) {
        while start < end && is_breakable_space(char_at(text, start)) {
          start += char_at(text, start).len_utf8();
        }

        let end_position = positions.position_for_offset(start);

        if start == end {
          let state = if item_result.can_break_after {
            CandidateState::Break
          } else {
            CandidateState::MidWord
          };

          context.append_trailing_spaces(state, index(start), end_position);
          context.set_last(item, end);

          return;
        }

        let state = if self.auto_wrap {
          CandidateState::Break
        } else {
          CandidateState::MidWord
        };

        context.append_trailing_spaces(state, index(start), end_position);
      }
    } else {
      positions.set_base_position(start, context.position());
    }

    context.set_last(item, end);

    if let Some(object) = item.layout_object {
      self.set_current_style(object.context());
    }

    let mut hyphen_advance_cache = None;

    loop {
      let mut next_offset = if self.auto_wrap {
        let len = (end + 1).min(text.len());

        self.break_iterator.next_break_opportunity(start + 1, len)
      } else {
        end + 1
      };

      // A break allowed after the item wins over the next opportunity.
      if next_offset > end && item_result.can_break_after {
        next_offset = end;
      }

      let mut next_state = CandidateState::Break;
      let mut penalty = 0.0;
      let mut is_hyphenated = false;
      let end_offset;
      let next_position;
      let mut end_position;

      if next_offset > end {
        // The next opportunity is past this item: stop mid-word at its end.
        next_offset = end;
        end_offset = end;
        next_position = positions.position_for_offset(end);
        end_position = next_position;
        next_state = CandidateState::MidWord;
      } else {
        // Higher level protocols forbid the break, as in `<span>1 </span>2`.
        if next_offset == end && !item_result.can_break_after {
          next_state = CandidateState::MidWord;
        }

        next_position = positions.position_for_offset(next_offset);

        // The word's end, without its trailing spaces.
        let mut word_end = next_offset;
        let mut last_ch = char_before(text, word_end);

        while is_breakable_space(last_ch) {
          word_end -= last_ch.len_utf8();

          if word_end == start {
            last_ch = '\0';

            break;
          }

          last_ch = char_before(text, word_end);
        }

        end_offset = word_end;

        let end_safe_offset = match next_state {
          CandidateState::Break => {
            (shape_result.cached_previous_safe_to_break_offset(end_offset as u32) as usize)
              .max(start)
          }
          CandidateState::MidWord => end_offset,
        };

        end_position = if end_safe_offset == end_offset {
          if end_offset == next_offset {
            next_position
          } else {
            positions.position_for_offset(end_offset)
          }
        } else {
          positions.position_for_offset(end_safe_offset)
            + Self::shape_text(
              self.items_data,
              &self.shaper,
              text,
              item,
              end_safe_offset,
              end_offset,
            )
            .width()
        };

        is_hyphenated = if end_offset == item_result.end_offset() {
          item_result.is_hyphenated
        } else {
          last_ch == SOFT_HYPHEN && next_state == CandidateState::Break
        };

        if is_hyphenated {
          end_position += self
            .hyphen_advance(
              positions.is_ltr,
              item_result.hyphen.as_ref(),
              &mut hyphen_advance_cache,
            )
            .to_float();
          penalty = context.hyphen_penalty;
        }
      }

      context.append(
        next_state,
        index(next_offset),
        index(end_offset),
        next_position,
        end_position,
        penalty,
        is_hyphenated,
      );

      if next_offset >= end {
        break;
      }

      start = next_offset;
    }
  }

  /// Blink's `HyphenAdvance`: the hyphen's advance, negative in RTL.
  fn hyphen_advance(
    &self,
    is_ltr: bool,
    hyphen: Option<&HyphenResult>,
    cache: &mut Option<LayoutUnit>,
  ) -> LayoutUnit {
    if let Some(advance) = *cache {
      return advance;
    }

    let size = match (hyphen, self.current_style) {
      (Some(hyphen), _) => hyphen.inline_size(),
      (None, Some(style)) => HyphenResult::new(style, &self.items_data.fonts).inline_size(),
      (None, None) => LayoutUnit::ZERO,
    };
    let advance = if is_ltr { size } else { -size };

    *cache = Some(advance);
    advance
  }

  /// Blink's `CanBreakInside`: whether the line has a break opportunity before its end.
  pub(crate) fn can_break_inside(&mut self, line_info: &LineInfo<'a, 'c>) -> bool {
    let item_results = &line_info.results;

    if let Some(break_index) = item_results[..item_results.len().saturating_sub(1)]
      .iter()
      .position(|result| result.can_break_after)
      && item_results[break_index + 1..]
        .iter()
        .any(|result| !result.item.is_floating_or_out_of_flow_positioned())
    {
      return true;
    }

    item_results.iter().any(|item_result| {
      item_result.item.item_type == InlineItemType::Text
        && item_result.may_break_inside
        && self.can_break_inside_result(item_result)
    })
  }

  /// Blink's `CanBreakInside` of a text result.
  fn can_break_inside_result(&mut self, item_result: &InlineItemResult<'a, 'c>) -> bool {
    debug_assert!(item_result.may_break_inside);

    if let Some(object) = item_result.item.layout_object {
      self.set_current_style(object.context());
    }

    if !self.auto_wrap {
      return false;
    }

    let next_offset = self
      .break_iterator
      .next_break_opportunity(item_result.start_offset() + 1, self.text().len());

    next_offset < item_result.end_offset()
  }

  /// Blink's `CreateBreakToken`: where the next line starts, none at the end.
  fn create_break_token(&self, line_info: &LineInfo<'a, 'c>) -> Option<InlineBreakToken<'c>> {
    if self.is_at_end() {
      return None;
    }

    Some(InlineBreakToken {
      style: self.current_style,
      start: self.current,
      is_forced_break: self.is_forced_break,
      has_cloned_box_decorations: self.cloned_box_decorations_count != 0,
      is_past_first_formatted_line: !self.is_first_formatted_line || !line_info.is_empty_line,
      is_line_clamp_displaced_line: self.line_clamp_ellipsis_width != LayoutUnit::ZERO
        && line_info.results.is_empty(),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::{LineBreaker, LineBreakerMode, WhitespaceState};
  use crate::{
    layout::{
      constraint_space::ConstraintSpace,
      exclusions::{exclusion_space::ExclusionSpace, layout_opportunity::LineLayoutOpportunity},
      inline::{
        inline_item::InlineItemType,
        inline_node::InlineNode,
        leading_floats::LeadingFloats,
        line_info::LineInfo,
        testing::{TestAtomicInline, render_tree, span},
      },
      node::Node,
      tree::RenderNode,
    },
    layout_unit::LayoutUnit,
  };

  /// What Blink's `BreakLines` records.
  struct BrokenLines {
    /// Each line's text, and the item index its last result is in.
    lines: Vec<(String, usize)>,
    trailing_whitespaces: Vec<WhitespaceState>,
    first_hang_width: LayoutUnit,
    widths: Vec<LayoutUnit>,
    /// Each line's `is_block_in_inline` and `is_empty_line`.
    flags: Vec<(bool, bool)>,
  }

  /// Blink's `BreakLines`.
  fn break_lines_with_state(block: &RenderNode, available_width: f32) -> BrokenLines {
    let node = InlineNode::new(block);
    let data = node.prepare_layout();
    let available_width = LayoutUnit::from_float(available_width);
    let mut atomic_inline = TestAtomicInline;
    let mut exclusion_space = ExclusionSpace::default();
    let leading_floats = LeadingFloats::default();
    let mut break_token = None;
    let mut broken = BrokenLines {
      lines: Vec::new(),
      trailing_whitespaces: Vec::new(),
      first_hang_width: LayoutUnit::ZERO,
      widths: Vec::new(),
      flags: Vec::new(),
    };

    loop {
      let mut line_info = LineInfo::new(&data, &block.context);
      let mut line_breaker = LineBreaker::new(
        node,
        &data,
        LineBreakerMode::Content,
        ConstraintSpace::new(
          Some(available_width),
          block.context.style.inherited_data.direction,
        ),
        LineLayoutOpportunity::new(available_width),
        &leading_floats,
        break_token,
        &mut atomic_inline,
        &mut exclusion_space,
      );

      line_breaker.next_line(&mut line_info);
      broken
        .trailing_whitespaces
        .push(line_breaker.trailing_whitespace);

      if line_info.results.is_empty() {
        break;
      }

      if broken.lines.is_empty() {
        broken.first_hang_width = line_info.hang_width();
      }

      break_token = line_info.break_token;
      broken.widths.push(line_info.width());
      broken
        .flags
        .push((line_info.is_block_in_inline, line_info.is_empty_line));
      broken.lines.push((
        line_info
          .results
          .iter()
          .map(|result| &data.text_content[result.text_offset.clone()])
          .collect(),
        line_info
          .results
          .last()
          .map_or(0, |result| result.item_index),
      ));

      if break_token.is_none() {
        break;
      }
    }

    broken
  }

  fn break_lines(block: &RenderNode, available_width: f32) -> Vec<(String, usize)> {
    break_lines_with_state(block, available_width).lines
  }

  fn texts(lines: Vec<(String, usize)>) -> Vec<String> {
    lines.into_iter().map(|(text, _)| text).collect()
  }

  /// A span holding `children`.
  fn inline_box(children: impl Into<Vec<Node>>) -> Node {
    Node::container(children).with_class_name("span")
  }

  /// Blink's `InlineLayoutAlgorithmTest.TypesForBlockInInline`: the block takes a line of its own
  /// between the lines before and after it, empty when only tags are on them.
  #[test]
  fn a_block_in_inline_takes_a_line_of_its_own() {
    let tree = render_tree(
      [inline_box([
        Node::container([Node::text("abc")]).with_class_name("block")
      ])],
      ".block { display: block }",
    );
    let broken = break_lines_with_state(&tree, 100.0);

    assert_eq!(texts(broken.lines), ["", "\u{FFFC}", ""]);
    assert_eq!(broken.flags, [(false, true), (true, false), (false, true)]);
  }

  /// Blink's `ScoreLineBreakerTest.BeforeAfter`: the text before and after a block-in-inline breaks
  /// into lines of its own.
  #[test]
  fn text_around_a_block_in_inline_breaks_apart() {
    let tree = render_tree(
      [inline_box([
        Node::text("123 456 "),
        Node::container([]).with_class_name("block"),
        Node::text(" 789"),
      ])],
      ".block { display: block; height: 10px }",
    );
    let broken = break_lines_with_state(&tree, 100.0);

    assert_eq!(texts(broken.lines), ["123 456", "\u{FFFC}", "789"]);
    assert_eq!(
      broken.flags,
      [(false, false), (true, false), (false, false)]
    );
  }

  /// A block-in-inline with no height collapses through, so its line is empty.
  #[test]
  fn an_empty_block_in_inline_leaves_an_empty_line() {
    let tree = render_tree(
      [inline_box([
        Node::text("a"),
        Node::container([]).with_class_name("block"),
        Node::text("b"),
      ])],
      ".block { display: block }",
    );

    assert_eq!(
      break_lines_with_state(&tree, 100.0).flags,
      [(false, false), (true, true), (false, false)]
    );
  }

  #[test]
  fn single_node() {
    let tree = render_tree([Node::text("123 456 789")], "");

    assert_eq!(texts(break_lines(&tree, 48.0)), ["123 456", "789"]);
    assert_eq!(texts(break_lines(&tree, 36.0)), ["123", "456", "789"]);
  }

  #[test]
  fn overflow_word() {
    let tree = render_tree([Node::text("12345 678")], "");

    // The first line overflows, the last does not.
    assert_eq!(texts(break_lines(&tree, 24.0)), ["12345", "678"]);
    // Both overflow.
    assert_eq!(texts(break_lines(&tree, 12.0)), ["12345", "678"]);
  }

  #[test]
  fn overflow_tab() {
    let tree = render_tree(
      [Node::text("12345\t\t678")],
      ".container { tab-size: 8; white-space: pre-wrap }",
    );

    assert_eq!(texts(break_lines(&tree, 60.0)), ["12345\t\t", "678"]);
  }

  #[test]
  fn overflow_tab_break_word() {
    let tree = render_tree(
      [Node::text("12345\t\t678")],
      ".container { tab-size: 8; white-space: pre-wrap; overflow-wrap: break-word }",
    );

    assert_eq!(texts(break_lines(&tree, 60.0)), ["12345\t\t", "678"]);
  }

  #[test]
  fn overflow_atomic_inline() {
    let tree = render_tree(
      [
        Node::text("12345"),
        Node::container([]).with_class_name("atomic"),
        Node::text("678"),
      ],
      "",
    );

    assert_eq!(texts(break_lines(&tree, 48.0)), ["12345\u{FFFC}", "678"]);
    assert_eq!(texts(break_lines(&tree, 42.0)), ["12345", "\u{FFFC}678"]);
    assert_eq!(
      texts(break_lines(&tree, 24.0)),
      ["12345", "\u{FFFC}", "678"]
    );
    assert_eq!(
      texts(break_lines(&tree, 12.0)),
      ["12345", "\u{FFFC}", "678"]
    );
  }

  /// "456" fits, but its span's right margin does not, and nothing breaks between them.
  #[test]
  fn overflow_margin() {
    let tree = render_tree(
      [
        span("123 456").with_class_name("margin"),
        Node::text(" 789"),
      ],
      ".margin { display: inline; margin-right: 24px }",
    );
    let data = InlineNode::new(&tree).prepare_layout();
    let lines = break_lines(&tree, 48.0);

    assert_eq!(texts(lines.clone()), ["123", "456", "789"]);
    assert_eq!(
      data.items[lines[1].1 - 1].item_type,
      InlineItemType::CloseTag
    );

    // "456" with its margin overflows the line.
    let lines = break_lines(&tree, 36.0);

    assert_eq!(texts(lines.clone()), ["123", "456", "789"]);
    assert_eq!(data.items[lines[1].1].item_type, InlineItemType::CloseTag);
  }

  #[test]
  fn overflow_after_spaces_across_elements() {
    let tree = render_tree(
      [span("12345 "), Node::text(" 1234567890123")],
      ".container { white-space: pre-wrap; overflow-wrap: break-word }",
    );

    assert_eq!(
      texts(break_lines(&tree, 60.0)),
      ["12345  ", "1234567890", "123"]
    );
  }

  /// The last word of a node wraps, and another node continues it.
  #[test]
  fn wrap_last_word() {
    let tree = render_tree(
      [Node::text("AAA AAA AAA "), span("BB"), Node::text(" CC")],
      "",
    );

    assert_eq!(texts(break_lines(&tree, 60.0)), ["AAA AAA", "AAA BB CC"]);
  }

  #[test]
  fn wrap_letter_spacing() {
    let tree = render_tree(
      [Node::text("Star Wars")],
      ".container { letter-spacing: 10px }",
    );

    assert_eq!(texts(break_lines(&tree, 0.0)), ["Star", "Wars"]);
  }

  /// An element boundary inside "456789" does not break it.
  #[test]
  fn boundary_in_word() {
    let tree = render_tree([span("123 456"), Node::text("789 abc")], "");

    assert_eq!(texts(break_lines(&tree, 48.0)), ["123", "456789", "abc"]);
    // "456789" overflows.
    assert_eq!(texts(break_lines(&tree, 30.0)), ["123", "456789", "abc"]);
  }

  #[test]
  fn boundary_in_first_word() {
    let tree = render_tree([span("123"), Node::text("456 789")], "");

    assert_eq!(texts(break_lines(&tree, 48.0)), ["123456", "789"]);
    assert_eq!(texts(break_lines(&tree, 30.0)), ["123456", "789"]);
    assert_eq!(texts(break_lines(&tree, 12.0)), ["123456", "789"]);
  }

  fn br() -> Node {
    Node::text("\n").with_tag_name("br")
  }

  #[test]
  fn whitespace_state() {
    use WhitespaceState::*;

    let cases: Vec<(Vec<Node>, &str, WhitespaceState)> = vec![
      // The most common cases.
      (vec![Node::text("12")], "normal", None),
      (vec![Node::text("1234 5678")], "normal", Collapsed),
      // The builder collapses a block's trailing spaces.
      (vec![Node::text("12 ")], "normal", None),
      // `pre` and `pre-wrap` keep trailing spaces.
      (vec![Node::text("1234 5678")], "pre-wrap", Preserved),
      (vec![Node::text("12 ")], "pre", Preserved),
      (vec![Node::text("12 ")], "pre-wrap", Preserved),
      (vec![Node::text("12")], "pre", None),
      (vec![Node::text("12")], "pre-wrap", None),
      // Empty or only spaces.
      (vec![], "normal", Leading),
      (vec![Node::text(" ")], "pre", Preserved),
      (vec![Node::text(" ")], "pre-wrap", Preserved),
      // Rewinding.
      (vec![Node::text("12 34"), span("56")], "normal", Collapsed),
      (vec![Node::text("12 34"), span("56")], "pre-wrap", Preserved),
      // Atomic inlines.
      (
        vec![
          Node::text("12 "),
          Node::container([]).with_class_name("empty-atomic"),
        ],
        "normal",
        None,
      ),
      // fast/text/whitespace/inline-whitespace-wrapping-4.html
      (
        vec![
          Node::text("1234  ").with_class_name("nowrap"),
          Node::text("  5678").with_class_name("normal"),
        ],
        "pre",
        Collapsed,
      ),
    ];

    for (children, white_space, expected) in cases {
      let description = format!("{children:?} with white-space: {white_space}");
      let tree = render_tree(
        children,
        &format!(
          ".container {{ white-space: {white_space} }} \
           .empty-atomic {{ display: inline-block }} \
           .nowrap {{ display: inline; white-space: nowrap }} \
           .normal {{ display: inline; white-space: normal }}"
        ),
      );

      assert_eq!(
        break_lines_with_state(&tree, 30.0).trailing_whitespaces[0],
        expected,
        "{description}"
      );
    }
  }

  #[test]
  fn trailing_space_width() {
    let cases: Vec<(Vec<Node>, &str, i32)> = vec![
      (vec![Node::text(" ")], "pre", 0),
      (vec![Node::text("   ")], "pre", 0),
      (vec![Node::text("1 ")], "pre", 0),
      (vec![Node::text("1  ")], "pre", 0),
      (vec![Node::text("1"), span(" ")], "pre", 0),
      (vec![span("1 "), Node::text(" ")], "pre", 0),
      (vec![Node::text("1"), span(" "), Node::text(" ")], "pre", 0),
      (vec![Node::text("1 "), span(" "), Node::text(" ")], "pre", 0),
      (vec![Node::text("1 \t")], "pre", 0),
      (vec![Node::text("1  \n")], "pre", 0),
      (vec![Node::text("1  "), br()], "pre", 0),
      (vec![Node::text(" ")], "pre-wrap", 0),
      (vec![Node::text("   ")], "pre-wrap", 0),
      (vec![Node::text("1 ")], "pre-wrap", 0),
      (vec![Node::text("1  ")], "pre-wrap", 0),
      (vec![Node::text("1"), span(" ")], "pre-wrap", 0),
      (vec![span("1 "), Node::text(" ")], "pre-wrap", 0),
      (
        vec![Node::text("1"), span(" "), Node::text(" ")],
        "pre-wrap",
        0,
      ),
      (
        vec![Node::text("1 "), span(" "), Node::text(" ")],
        "pre-wrap",
        0,
      ),
      (vec![Node::text("1 \t")], "pre-wrap", 0),
      (vec![Node::text("1  "), br()], "pre-wrap", 0),
      (vec![Node::text("12 1234")], "pre-wrap", 1),
      (vec![Node::text("12  1234")], "pre-wrap", 2),
      (
        vec![Node::text("12  "), br(), Node::text("1234")],
        "pre-wrap",
        0,
      ),
      (vec![Node::text("12    ")], "pre-wrap", 1),
    ];

    for (children, white_space, hanging_spaces) in cases {
      let description = format!("{children:?} with white-space: {white_space}");
      let tree = render_tree(
        children,
        &format!(".container {{ tab-size: 2; white-space: {white_space} }}"),
      );

      assert_eq!(
        break_lines_with_state(&tree, 30.0).first_hang_width,
        LayoutUnit::from_int(6 * hanging_spaces),
        "{description}"
      );
    }
  }

  /// A break before a close tag moves after it, so the tag stays on the line it closes.
  #[test]
  fn break_moves_after_close_tag() {
    let tree = render_tree([span("12 "), Node::text("34")], "");
    let data = InlineNode::new(&tree).prepare_layout();
    let lines = break_lines(&tree, 12.0);

    assert_eq!(texts(lines.clone()), ["12", "34"]);
    assert_eq!(data.items[lines[0].1].item_type, InlineItemType::CloseTag);
  }

  /// A float placed on a line narrows it.
  #[test]
  fn float_narrows_its_line() {
    let tree = render_tree(
      [
        Node::container([]).with_class_name("float"),
        Node::text("123 456 789"),
      ],
      ".float { float: left; width: 24px; height: 10px }",
    );

    assert_eq!(texts(break_lines(&tree, 60.0)), ["123", "456 789"]);
  }

  /// A tab beside a float reaches the container's next tab stop, 48px from its edge, not the
  /// line's.
  #[test]
  fn tab_stops_count_from_the_container_edge() {
    let tree = render_tree(
      [
        Node::container([]).with_class_name("float"),
        span("\tX").with_class_name("span pre"),
      ],
      ".float { float: left; width: 18px; height: 10px } .pre { white-space: pre }",
    );

    assert_eq!(
      break_lines_with_state(&tree, 120.0).widths,
      [LayoutUnit::from_int(30 + 6)]
    );
  }
}

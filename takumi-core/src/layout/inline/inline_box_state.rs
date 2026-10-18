//! Blink's [`InlineBoxState`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_box_state.cc),
//! without ruby, SVG text, block-in-inline, `text-box-trim`, `text-emphasis`, or quirks mode.

use std::{mem, ptr, rc::Rc};

use super::{
  inline_item::{InlineItem, InlineItemType},
  inline_item_result::{InlineItemResult, LineBoxStrut},
  inline_node_data::StyleFonts,
  line_info::LineInfo,
  logical_line_item::{InlineBoxFragment, LineFragment, LogicalLineItem, LogicalLineItems},
  text_fit_scale::TextFitBlockScale,
};
use crate::{
  context::RenderContext,
  font_style::BoxLineHeight,
  fonts::{
    font::Font,
    font_height::{FontHeight, calculate_leading_space},
    shaping::shape_result_view::ShapeResultView,
  },
  layout::{
    constraint_space::ConstraintSpace,
    logical_rect::{LogicalOffset, LogicalRect, LogicalSize},
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  resources::font::PrimaryFontMetrics,
  style::{
    BoxDecorationBreak, Direction as TextDirection, TextFitMode, TextFitTarget, VerticalAlign,
    VerticalAlignKeyword,
  },
};

/// Blink's `ComputedLineHeightAsFixed` of `context` against `font`.
fn computed_line_height_as_fixed(context: &RenderContext, font: &Font) -> LayoutUnit {
  BoxLineHeight::of(&context.style, context).computed_as_fixed(font.primary_font_metrics())
}

/// Blink's `PendingPositions`: fragments whose position waits for an ancestor's size.
#[derive(Debug, Clone, Copy)]
struct PendingPositions {
  fragment_start: usize,
  fragment_end: usize,
  metrics: FontHeight,
  vertical_align: VerticalAlignKeyword,
}

/// Blink's `InlineBoxState`: a box open while a line's items are placed.
#[derive(Clone)]
pub(crate) struct InlineBoxState<'a, 'c> {
  pub(crate) fragment_start: usize,
  pub(crate) item: Option<&'a InlineItem<'c>>,
  pub(crate) style: &'c RenderContext,
  pub(crate) font: Rc<Font>,
  /// Everything in the box, aligned by `vertical-align`.
  pub(crate) metrics: FontHeight,
  /// The box's own font, with the leading `line-height` gives it.
  pub(crate) text_metrics: FontHeight,
  /// The distance from the text top to the baseline, without leading.
  pub(crate) text_top: LayoutUnit,
  pub(crate) text_height: LayoutUnit,
  pub(crate) has_start_edge: bool,
  pub(crate) has_end_edge: bool,
  pub(crate) margins: LineBoxStrut,
  pub(crate) borders: LineBoxStrut,
  pub(crate) padding: LineBoxStrut,
  pending_descendants: Vec<PendingPositions>,
  pub(crate) include_used_fonts: bool,
  pub(crate) needs_box_fragment: bool,
  /// The scale `text-fit` gives the box's text.
  pub(crate) text_fit_scale: f32,
}

impl<'a, 'c> InlineBoxState<'a, 'c> {
  pub(crate) fn new(fragment_start: usize, style: &'c RenderContext, fonts: &StyleFonts) -> Self {
    Self {
      fragment_start,
      item: None,
      style,
      font: fonts.get(style),
      metrics: FontHeight::empty(),
      text_metrics: FontHeight::empty(),
      text_top: LayoutUnit::ZERO,
      text_height: LayoutUnit::ZERO,
      has_start_edge: false,
      has_end_edge: false,
      margins: LineBoxStrut::default(),
      borders: LineBoxStrut::default(),
      padding: LineBoxStrut::default(),
      pending_descendants: Vec::new(),
      include_used_fonts: false,
      needs_box_fragment: false,
      text_fit_scale: 1.0,
    }
  }

  /// Blink's `ResetStyle`.
  fn reset_style(&mut self, style: &'c RenderContext, fonts: &StyleFonts) {
    self.style = style;
    self.font = fonts.get(style);
  }

  /// Blink's `ComputeTextMetrics`: the box's font with its leading, which every text in the box
  /// shares and the line height includes; `text-fit` scales both.
  pub(crate) fn compute_text_metrics(&mut self, scale: Option<&TextFitBlockScale>) {
    let base_font = scale
      .and_then(|scale| scale.scaled_font.as_deref())
      .unwrap_or(&self.font);
    let paint_scale = scale.map_or(1.0, |scale| scale.paint_scale);
    let font_metrics = base_font.primary_font_metrics();

    self.text_metrics = if paint_scale == 1.0 {
      font_metrics.map_or_else(FontHeight::default, PrimaryFontMetrics::font_height)
    } else {
      let mut text_metrics =
        font_metrics.map_or_else(FontHeight::default, PrimaryFontMetrics::float_font_height);

      text_metrics.ascent *= paint_scale;
      text_metrics.descent *= paint_scale;
      text_metrics
    };
    self.text_top = -self.text_metrics.ascent;
    self.text_height = self.text_metrics.line_height();

    let box_line_height = BoxLineHeight::of(&self.style.style, self.style);
    let mut line_height = box_line_height.computed_as_fixed(font_metrics);

    // A `line-height` length, percentages included, does not scale.
    if !matches!(box_line_height, BoxLineHeight::Length(_)) && paint_scale != 1.0 {
      line_height *= paint_scale;
    }
    let leading_space = calculate_leading_space(line_height, &self.text_metrics);

    self.text_metrics.add_leading(leading_space);
    self.metrics.unite(&self.text_metrics);
    self.include_used_fonts = matches!(
      BoxLineHeight::of(&self.style.style, self.style),
      BoxLineHeight::Normal
    );
  }

  /// Blink's `EnsureTextMetrics`.
  pub(crate) fn ensure_text_metrics(&mut self, scale: Option<&TextFitBlockScale>) {
    if self.text_metrics.is_empty() {
      self.compute_text_metrics(scale);
    }
  }

  /// Blink's `ResetTextMetrics`.
  pub(crate) fn reset_text_metrics(&mut self) {
    self.metrics = FontHeight::empty();
    self.text_metrics = FontHeight::empty();
    self.text_top = LayoutUnit::ZERO;
    self.text_height = LayoutUnit::ZERO;
  }

  /// Blink's `AccumulateUsedFonts`: under `line-height: normal`, each font the text used grows the
  /// box to its own leaded height.
  pub(crate) fn accumulate_used_fonts(&mut self, shape_result: &ShapeResultView, scale: f32) {
    for used_font in shape_result.used_fonts() {
      let Some(font_metrics) = used_font.font_metrics() else {
        continue;
      };
      let mut used_metrics = font_metrics.font_height();
      let leading_space = calculate_leading_space(font_metrics.fixed_line_spacing(), &used_metrics);

      used_metrics.add_leading(leading_space);
      used_metrics.ascent *= scale;
      used_metrics.descent *= scale;
      self.metrics.unite(&used_metrics);
    }
  }

  /// Blink's `ComputedFontSizeAsFixed`, times the box's `text-fit` scale.
  fn scaled_font_size_as_fixed(&self) -> LayoutUnit {
    let font_size = LayoutUnit::from_float_round(self.style.sizing.font_size);

    if self.text_fit_scale == 1.0 {
      return font_size;
    }

    LayoutUnit::from_float(font_size.to_float() * self.text_fit_scale)
  }

  /// Blink's `TextTop`: the text top's offset from the baseline, for `vertical-align: text-top`.
  fn text_top(&self) -> LayoutUnit {
    if !self.text_metrics.is_empty() {
      return self.text_top;
    }

    self
      .font
      .primary_font_metrics()
      .map_or(LayoutUnit::ZERO, |metrics| {
        -metrics.float_font_height().ascent
      })
  }
}

/// Blink's `LineHeightMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineHeightMode {
  /// Every inline box has a strut.
  Normal,
  /// Only the root inline box has a strut, for a line the line-clamp ellipsis displaced.
  LineClampDisplacedEllipsis,
}

/// Blink's `PositionPending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionPending {
  NotPending,
  Pending,
}

/// Blink's `BoxData`: an inline box's fragment, built once the line is placed.
#[derive(Clone)]
struct BoxData<'a, 'c> {
  /// The children the box holds.
  fragment_start: usize,
  fragment_end: usize,
  item: &'a InlineItem<'c>,
  node: &'c RenderNode,
  /// The item's font, which its `line-height` resolves against.
  font: Rc<Font>,
  rect: LogicalRect,
  has_line_left_edge: bool,
  has_line_right_edge: bool,
  borders: LineBoxStrut,
  padding: LineBoxStrut,
  margin_line_over: LayoutUnit,
  margin_line_under: LayoutUnit,
  margin_line_left: LayoutUnit,
  margin_line_right: LayoutUnit,
  margin_border_padding_line_left: LayoutUnit,
  margin_border_padding_line_right: LayoutUnit,
  parent_box_data_index: usize,
  fragmented_box_data_index: usize,
}

impl<'a, 'c> BoxData<'a, 'c> {
  fn new(
    fragment_start: usize,
    fragment_end: usize,
    item: &'a InlineItem<'c>,
    node: &'c RenderNode,
    font: Rc<Font>,
    size: LogicalSize,
  ) -> Self {
    Self {
      fragment_start,
      fragment_end,
      item,
      node,
      font,
      rect: LogicalRect {
        offset: LogicalOffset::default(),
        size,
      },
      has_line_left_edge: false,
      has_line_right_edge: false,
      borders: LineBoxStrut::default(),
      padding: LineBoxStrut::default(),
      margin_line_over: LayoutUnit::ZERO,
      margin_line_under: LayoutUnit::ZERO,
      margin_line_left: LayoutUnit::ZERO,
      margin_line_right: LayoutUnit::ZERO,
      margin_border_padding_line_left: LayoutUnit::ZERO,
      margin_border_padding_line_right: LayoutUnit::ZERO,
      parent_box_data_index: 0,
      fragmented_box_data_index: 0,
    }
  }

  /// Blink's copy for another range of a box bidi reordering split: only the item and its rect.
  fn fragmented(other: &Self, start: usize, end: usize) -> Self {
    Self {
      rect: other.rect,
      ..Self::new(
        start,
        end,
        other.item,
        other.node,
        Rc::clone(&other.font),
        other.rect.size,
      )
    }
  }

  /// Blink's `UpdateFragmentEdges`: the line-right edge moves to the last fragment.
  fn update_fragment_edges(list: &mut [Self], index: usize) {
    let last_index = list[index].fragmented_box_data_index;

    debug_assert_ne!(last_index, 0);

    if !list[index].has_line_right_edge {
      return;
    }

    let source = list[index].clone();
    let last = &mut list[last_index];

    last.has_line_right_edge = true;
    last.margin_line_right = source.margin_line_right;
    last.margin_border_padding_line_right = source.margin_border_padding_line_right;
    last.padding.inline_end = source.padding.inline_end;

    let box_data = &mut list[index];

    box_data.has_line_right_edge = false;
    box_data.margin_line_right = LayoutUnit::ZERO;
    box_data.margin_border_padding_line_right = LayoutUnit::ZERO;
    box_data.padding.inline_end = LayoutUnit::ZERO;
  }

  /// Blink's `CreateBoxFragment`: the box's fragment, its children staying in the line's flat
  /// list but for the out-of-flow boxes it takes.
  fn create_box_fragment(
    &self,
    line_box: &mut LogicalLineItems<'a, 'c>,
    line_box_line_height: LayoutUnit,
  ) -> LineFragment<'c> {
    let mut out_of_flow_children = Vec::new();
    let mut i = self.fragment_start;

    while i < self.fragment_end {
      let child = &mut line_box[i];

      // Children already in a box fragment of their own were added with it.
      let skip = child.children_count.saturating_sub(1);

      if let Some(node) = child.out_of_flow_positioned_box.take() {
        // The static position, from this fragment.
        out_of_flow_children.push((
          node,
          child.rect.offset - self.rect.offset,
          line_box_line_height,
        ));
      }

      i += skip + 1;
    }

    LineFragment::InlineBox(InlineBoxFragment {
      node: self.node,
      style_variant: self.item.style_variant,
      size: LogicalSize::new(
        self.rect.size.inline_size.clamp_negative_to_zero(),
        self.rect.size.block_size,
      ),
      borders: self.borders,
      padding: self.padding,
      has_line_left_edge: self.has_line_left_edge,
      has_line_right_edge: self.has_line_right_edge,
      out_of_flow_children,
    })
  }
}

/// Blink's `InlineLayoutStateStack`: the inline boxes open while a line is placed, which align
/// their children once their own sizes are known.
#[derive(Clone, Default)]
pub(crate) struct InlineLayoutStateStack<'a, 'c> {
  stack: Vec<InlineBoxState<'a, 'c>>,
  box_data_list: Vec<BoxData<'a, 'c>>,
  is_empty_line: bool,
}

impl<'a, 'c> InlineLayoutStateStack<'a, 'c> {
  /// Blink's `LineBoxState`.
  pub(crate) fn line_box_state(&mut self) -> &mut InlineBoxState<'a, 'c> {
    &mut self.stack[0]
  }

  pub(crate) fn set_is_empty_line(&mut self, is_empty_line: bool) {
    self.is_empty_line = is_empty_line;
  }

  /// The box open last.
  pub(crate) fn current(&mut self) -> &mut InlineBoxState<'a, 'c> {
    let last = self.stack.len() - 1;

    &mut self.stack[last]
  }

  /// Blink's `HasBoxFragments`.
  pub(crate) fn has_box_fragments(&self) -> bool {
    !self.box_data_list.is_empty()
  }

  /// Blink's `OnBeginPlaceItems`: the stack for a new line, the boxes left open by the last line
  /// carried over.
  pub(crate) fn on_begin_place_items(
    &mut self,
    line_info: &LineInfo<'a, 'c>,
    line_height_mode: LineHeightMode,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    let line_style = line_info.line_style;
    let fonts = &line_info.items_data.fonts;
    let is_first_line = self.stack.is_empty();

    if is_first_line {
      self.stack.push(InlineBoxState::new(0, line_style, fonts));
    } else {
      // The states not shared across lines clear.
      for index in 0..self.stack.len() {
        self.stack[index].fragment_start = line_box.len();

        if self.stack[index].needs_box_fragment {
          debug_assert_ne!(index, 0);

          let text_block_scale = TextFitBlockScale {
            paint_scale: line_info.text_fit_scale,
            scaled_font: None,
          };

          self.add_box_fragment_placeholder(index, &text_block_scale, line_box, false);
        }

        let box_state = &mut self.stack[index];

        if line_height_mode == LineHeightMode::Normal {
          box_state.metrics = box_state.text_metrics;
        } else {
          box_state.reset_text_metrics();
        }

        // Boxes wrapped before closing have no start edge, unless cloned.
        if box_state.has_start_edge {
          box_state.has_start_edge = box_state.needs_box_fragment
            && box_state.style.style.box_data.box_decoration_break == BoxDecorationBreak::Clone;
        }

        debug_assert!(box_state.pending_descendants.is_empty());
      }
    }

    debug_assert!(self.box_data_list.is_empty());

    // The line box's own state starts with a strut, a zero-width inline box with the block's font
    // and line height. <https://drafts.csswg.org/css2/visudet.html#strut>
    let line_box_state = &mut self.stack[0];
    let text_fit = line_style.style.misc_inherited1_data.text_fit;

    if is_first_line
      || !ptr::eq(line_box_state.style, line_style)
      || line_height_mode == LineHeightMode::LineClampDisplacedEllipsis
      || (text_fit.mode != TextFitMode::None && text_fit.target != TextFitTarget::Consistent)
    {
      line_box_state.reset_style(line_style, fonts);

      let text_scale = TextFitBlockScale {
        paint_scale: line_info.text_fit_scale,
        scaled_font: None,
      };

      line_box_state.text_fit_scale = text_scale.total_scale(&line_box_state.font);
      line_box_state.compute_text_metrics(Some(&text_scale));
    }
  }

  /// Blink's `OnOpenTag`, pushing a box state.
  pub(crate) fn on_open_tag(
    &mut self,
    item: &'a InlineItem<'c>,
    style: &'c RenderContext,
    item_result: &InlineItemResult<'a, 'c>,
    line_box: &LogicalLineItems<'a, 'c>,
    fonts: &StyleFonts,
  ) -> &mut InlineBoxState<'a, 'c> {
    let text_fit_scale = self
      .stack
      .last()
      .map_or(1.0, |parent| parent.text_fit_scale);

    self.stack.push(InlineBoxState {
      item: Some(item),
      has_start_edge: true,
      margins: item_result.margins,
      borders: item_result.borders,
      padding: item_result.padding,
      text_fit_scale,
      ..InlineBoxState::new(line_box.len(), style, fonts)
    });
    self.current()
  }

  /// Blink's `OnOpenTag` that adds a box placeholder to `line_box`.
  pub(crate) fn on_open_tag_with_placeholder(
    &mut self,
    item: &'a InlineItem<'c>,
    style: &'c RenderContext,
    item_result: &InlineItemResult<'a, 'c>,
    text_scale: &TextFitBlockScale,
    line_box: &mut LogicalLineItems<'a, 'c>,
    fonts: &StyleFonts,
  ) -> &mut InlineBoxState<'a, 'c> {
    self.on_open_tag(item, style, item_result, line_box, fonts);

    let index = self.stack.len() - 1;
    let box_state = &mut self.stack[index];

    box_state.text_fit_scale = text_scale.total_scale(&box_state.font);
    box_state.needs_box_fragment = item.should_create_box_fragment;

    if box_state.needs_box_fragment {
      self.add_box_fragment_placeholder(index, text_scale, line_box, !item.is_empty_item);
    }

    &mut self.stack[index]
  }

  /// Blink's `OnCloseTag`, popping the box state.
  pub(crate) fn on_close_tag(
    &mut self,
    space: &ConstraintSpace,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    let index = self.stack.len() - 1;

    self.stack[index].has_end_edge = true;
    self.end_box_state(space, index, line_box);
    self.stack.pop();
  }

  /// Blink's `OnEndPlaceItems`: every pending position, once the line is placed.
  pub(crate) fn on_end_place_items(
    &mut self,
    space: &ConstraintSpace,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    for index in (0..self.stack.len()).rev() {
      let box_state = &mut self.stack[index];

      if !box_state.has_end_edge
        && box_state.needs_box_fragment
        && box_state.style.style.box_data.box_decoration_break == BoxDecorationBreak::Clone
      {
        box_state.has_end_edge = true;
      }

      self.end_box_state(space, index, line_box);
    }

    // The placeholders held the boxes' offsets for `apply_baseline_shift`.
    for box_data in &mut self.box_data_list {
      let placeholder = &line_box[box_data.fragment_start];

      debug_assert!(placeholder.is_placeholder());

      box_data.rect.offset = placeholder.rect.offset;
    }
  }

  /// Blink's `EndBoxState`: a box ends at its close tag or at the line's end.
  fn end_box_state(
    &mut self,
    space: &ConstraintSpace,
    index: usize,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    if self.stack[index].needs_box_fragment {
      self.add_box_data(space, index, line_box);
    }

    let position_pending = self.apply_baseline_shift(index, line_box);

    if index == 0 {
      return;
    }

    if position_pending == PositionPending::NotPending {
      let metrics = self.stack[index].metrics;

      self.stack[index - 1].metrics.unite(&metrics);
    }
  }

  /// Blink's `AddBoxFragmentPlaceholder`: a flat placeholder for the box, which
  /// `create_box_fragments` turns into the box's fragment.
  fn add_box_fragment_placeholder(
    &mut self,
    index: usize,
    text_scale: &TextFitBlockScale,
    line_box: &mut LogicalLineItems<'a, 'c>,
    has_non_empty_inline_box_start: bool,
  ) {
    let box_state = &self.stack[index];

    debug_assert!(
      box_state
        .item
        .is_some_and(|item| item.item_type != InlineItemType::AtomicInline)
    );

    let mut block_offset = LayoutUnit::ZERO;
    let mut block_size = LayoutUnit::ZERO;

    if !self.is_empty_line {
      // The font's height, without `line-height`.
      let font = text_scale.scaled_font.as_deref().unwrap_or(&box_state.font);
      let font_metrics = font.primary_font_metrics();
      let scale = text_scale.paint_scale;
      let mut metrics = if scale == 1.0 {
        font_metrics.map_or_else(FontHeight::default, PrimaryFontMetrics::font_height)
      } else {
        font_metrics.map_or_else(FontHeight::default, PrimaryFontMetrics::float_font_height)
      };

      if scale != 1.0 {
        metrics.ascent *= scale;
        metrics.descent *= scale;
      }

      // Borders and padding grow the box in the block direction; the line breaker counted their
      // inline sides.
      block_offset = -metrics.ascent - (box_state.borders.line_over + box_state.padding.line_over);
      block_size = metrics.line_height()
        + box_state.borders.line_over
        + box_state.borders.line_under
        + box_state.padding.line_over
        + box_state.padding.line_under;
    }

    let mut placeholder = LogicalLineItem::placeholder(block_offset, block_size);

    placeholder.has_non_empty_inline_box_start = has_non_empty_inline_box_start;
    debug_assert!(placeholder.is_placeholder());
    line_box.add_child(placeholder);
  }

  /// Blink's `AddBoxData`, for a box that needs a fragment.
  fn add_box_data(
    &mut self,
    space: &ConstraintSpace,
    index: usize,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    let box_state = &self.stack[index];

    debug_assert!(box_state.needs_box_fragment);

    let fragment_start = box_state.fragment_start;
    let fragment_end = line_box.len();
    let Some((item, node)) = box_state
      .item
      .and_then(|item| Some((item, item.layout_object?.node())))
    else {
      return;
    };
    let mut box_data = BoxData::new(
      fragment_start,
      fragment_end,
      item,
      node,
      Rc::clone(&box_state.font),
      line_box[fragment_start].rect.size,
    );

    box_data.borders = box_state.borders;
    box_data.padding = box_state.padding;
    box_data.margin_line_over = box_state.margins.line_over;
    box_data.margin_line_under = box_state.margins.line_under;

    if box_state.has_start_edge {
      box_data.has_line_left_edge = true;
      box_data.margin_line_left = box_state.margins.inline_start;
      box_data.margin_border_padding_line_left = box_state.margins.inline_start
        + box_state.borders.inline_start
        + box_state.padding.inline_start;
    } else {
      box_data.borders.inline_start = LayoutUnit::ZERO;
      box_data.padding.inline_start = LayoutUnit::ZERO;
    }

    if box_state.has_end_edge {
      box_data.has_line_right_edge = true;
      box_data.margin_line_right = box_state.margins.inline_end;
      box_data.margin_border_padding_line_right =
        box_state.margins.inline_end + box_state.borders.inline_end + box_state.padding.inline_end;
    } else {
      box_data.borders.inline_end = LayoutUnit::ZERO;
      box_data.padding.inline_end = LayoutUnit::ZERO;
    }

    if box_state.style.style.inherited_data.direction == TextDirection::Rtl {
      mem::swap(
        &mut box_data.has_line_left_edge,
        &mut box_data.has_line_right_edge,
      );
      mem::swap(
        &mut box_data.margin_line_left,
        &mut box_data.margin_line_right,
      );
      mem::swap(
        &mut box_data.margin_border_padding_line_left,
        &mut box_data.margin_border_padding_line_right,
      );
    }

    debug_assert!(line_box[fragment_start].is_placeholder());
    debug_assert!(fragment_end > fragment_start);

    // An empty box is flat, so its fragment needs no deferring, and its placeholder cannot reorder.
    if fragment_end > fragment_start + 1 {
      self.box_data_list.push(box_data);

      return;
    }

    let relative_offset = space.compute_relative_offset_for_inline(box_data.node);
    let placeholder = &mut line_box[fragment_start];

    placeholder.rect.offset.inline_offset += box_data.margin_line_left;
    placeholder.rect.offset += relative_offset;

    let advance =
      box_data.margin_border_padding_line_left + box_data.margin_border_padding_line_right;

    box_data.rect.size.inline_size =
      advance - box_data.margin_line_left - box_data.margin_line_right;

    // A placeholder's line box line height, for the static position, is zero.
    let fragment = box_data.create_box_fragment(line_box, LayoutUnit::ZERO);
    let placeholder = &mut line_box[fragment_start];

    placeholder.layout_result = Some(fragment);
    placeholder.inline_size = advance;
    debug_assert_eq!(placeholder.children_count, 0);
  }

  /// Blink's `ChildInserted`: indices after `index` move by one.
  pub(crate) fn child_inserted(&mut self, index: usize) {
    for state in &mut self.stack {
      if state.fragment_start >= index {
        state.fragment_start += 1;
      }

      debug_assert!(state.pending_descendants.is_empty());
    }

    for box_data in &mut self.box_data_list {
      if box_data.fragment_start >= index {
        box_data.fragment_start += 1;
      }

      if box_data.fragment_end >= index {
        box_data.fragment_end += 1;
      }
    }
  }

  /// Blink's `PrepareForReorder`: marks each child with its box, so the boxes survive bidi
  /// reordering.
  pub(crate) fn prepare_for_reorder(&mut self, line_box: &mut LogicalLineItems<'a, 'c>) {
    for box_data_index in 1..=self.box_data_list.len() {
      let (start, end) = {
        let box_data = &self.box_data_list[box_data_index - 1];

        (box_data.fragment_start, box_data.fragment_end)
      };

      debug_assert!(line_box[start].is_placeholder());

      for i in start..end {
        let child_box_data_index = line_box[i].box_data_index;

        if child_box_data_index == 0 {
          line_box[i].box_data_index = box_data_index;

          continue;
        }

        // A child box: record the nesting.
        let mut child_box_data_index = child_box_data_index;

        while child_box_data_index != box_data_index {
          let child_box_data = &mut self.box_data_list[child_box_data_index - 1];

          child_box_data_index = child_box_data.parent_box_data_index;

          if child_box_data_index == 0 {
            child_box_data.parent_box_data_index = box_data_index;

            break;
          }
        }
      }
    }
  }

  /// Blink's `UpdateAfterReorder`: the boxes' ranges from their reordered children.
  pub(crate) fn update_after_reorder(&mut self, line_box: &mut LogicalLineItems<'a, 'c>) {
    if self.box_data_list.is_empty() {
      return;
    }

    for box_data in &mut self.box_data_list {
      box_data.fragment_start = 0;
      box_data.fragment_end = 0;
    }

    let mut fragmented_boxes = Vec::new();
    let mut index = 0;

    while index < line_box.len() {
      index = self.update_box_data_fragment_range(line_box, index, &mut fragmented_boxes);
    }

    // Bidi reordering split some boxes.
    if !fragmented_boxes.is_empty() {
      self.update_fragmented_box_data_edges(fragmented_boxes);
    }
  }

  /// Blink's `UpdateBoxDataFragmentRange`: the range of the first box at `index`, adding a box for
  /// another range of a box bidi split. Returns the index to continue from.
  fn update_box_data_fragment_range(
    &mut self,
    line_box: &mut LogicalLineItems<'a, 'c>,
    mut index: usize,
    fragmented_boxes: &mut Vec<BoxData<'a, 'c>>,
  ) -> usize {
    while index < line_box.len() {
      let box_data_index = line_box[index].box_data_index;

      if box_data_index == 0 {
        index += 1;

        continue;
      }

      // The child moves to its parent box, so a nested box can include it.
      line_box[index].box_data_index = self.box_data_list[box_data_index - 1].parent_box_data_index;

      let start_index = index;

      index += 1;

      while index < line_box.len() {
        // A box inside this one updates first, which may move its children into this one.
        while line_box[index].box_data_index != 0 && line_box[index].box_data_index < box_data_index
        {
          self.update_box_data_fragment_range(line_box, index, fragmented_boxes);
        }

        if box_data_index != line_box[index].box_data_index {
          break;
        }

        line_box[index].box_data_index =
          self.box_data_list[box_data_index - 1].parent_box_data_index;
        index += 1;
      }

      let box_data = &mut self.box_data_list[box_data_index - 1];

      if box_data.fragment_end == 0 {
        box_data.fragment_start = start_index;
        box_data.fragment_end = index;
      } else {
        // Bidi reordering split this box.
        let mut fragmented_box = BoxData::fragmented(box_data, start_index, index);

        fragmented_box.fragmented_box_data_index = box_data_index;
        fragmented_boxes.push(fragmented_box);
      }

      // A box with parents goes again.
      if self.box_data_list[box_data_index - 1].parent_box_data_index != 0 {
        return start_index;
      }

      return index;
    }

    index
  }

  /// Blink's `UpdateFragmentedBoxDataEdges`: inserts the split boxes after the ones they split
  /// from, and moves the line-right edges to the last parts.
  fn update_fragmented_box_data_edges(&mut self, mut fragmented_boxes: Vec<BoxData<'a, 'c>>) {
    fragmented_boxes.sort_by(|a, b| {
      a.fragmented_box_data_index
        .cmp(&b.fragmented_box_data_index)
        .then(a.fragment_start.cmp(&b.fragment_start))
    });

    for mut fragmented_box in fragmented_boxes.into_iter().rev() {
      // Right after the box it split from, as propagating to ancestors needs this order.
      let insert_at = fragmented_box.fragmented_box_data_index;

      debug_assert!(insert_at > 0);

      fragmented_box.fragmented_box_data_index = 0;
      self.box_data_list.insert(insert_at, fragmented_box);

      for box_data in &mut self.box_data_list {
        if box_data.fragmented_box_data_index >= insert_at {
          box_data.fragmented_box_data_index += 1;
        }
      }

      // The original box points at its last part, for the edges.
      let fragmented_from = insert_at - 1;

      if self.box_data_list[fragmented_from].fragmented_box_data_index == 0 {
        self.box_data_list[fragmented_from].fragmented_box_data_index = insert_at;
      }
    }

    for index in 0..self.box_data_list.len() {
      if self.box_data_list[index].fragmented_box_data_index != 0 {
        BoxData::update_fragment_edges(&mut self.box_data_list, index);
      }
    }
  }

  /// Blink's `ComputeInlinePositions`: the children's inline offsets from `position`, in visual
  /// order, and the boxes' extents. Returns the end position.
  pub(crate) fn compute_inline_positions(
    &mut self,
    line_box: &mut LogicalLineItems<'a, 'c>,
    mut position: LayoutUnit,
    ignore_box_margin_border_padding: bool,
  ) -> LayoutUnit {
    // Children start at zero, in visual order.
    for child in &mut line_box.children {
      child.margin_line_left = child.rect.offset.inline_offset;
      child.rect.offset.inline_offset += position;

      // Box edges come later.
      if !child.has_fragment() {
        continue;
      }

      position += child.inline_size;
    }

    if self.box_data_list.is_empty() {
      return position;
    }

    if !ignore_box_margin_border_padding {
      // The boxes' edges move the children after them.
      for box_data in &self.box_data_list {
        let start = box_data.fragment_start;
        let end = box_data.fragment_end;

        debug_assert!(end > start);

        if box_data.margin_border_padding_line_left != LayoutUnit::ZERO {
          line_box.move_in_inline_direction_range(
            box_data.margin_border_padding_line_left,
            start,
            line_box.len(),
          );
          position += box_data.margin_border_padding_line_left;
        }

        if box_data.margin_border_padding_line_right != LayoutUnit::ZERO {
          line_box.move_in_inline_direction_range(
            box_data.margin_border_padding_line_right,
            end,
            line_box.len(),
          );
          position += box_data.margin_border_padding_line_right;
        }
      }
    }

    // The boxes' extents, nested boxes' edges accumulated on the leaves.
    #[derive(Clone, Copy, Default)]
    struct LinePadding {
      line_left: LayoutUnit,
      line_right: LayoutUnit,
    }

    let mut accumulated_padding = vec![LinePadding::default(); line_box.len()];

    for box_data in &mut self.box_data_list {
      let start = box_data.fragment_start;
      let start_child = &line_box[start];
      let mut line_left_offset =
        start_child.rect.offset.inline_offset - start_child.margin_line_left;

      debug_assert!(box_data.fragment_end > start);

      let last = box_data.fragment_end - 1;
      let last_child = &line_box[last];
      let mut line_right_offset =
        last_child.rect.offset.inline_offset - last_child.margin_line_left + last_child.inline_size;

      if !ignore_box_margin_border_padding {
        accumulated_padding[start].line_left += box_data.margin_border_padding_line_left;
        accumulated_padding[last].line_right += box_data.margin_border_padding_line_right;
        line_left_offset += box_data.margin_line_left;
        line_right_offset -= box_data.margin_line_right;
      }

      line_left_offset -= accumulated_padding[start].line_left;
      line_right_offset += accumulated_padding[last].line_right;
      box_data.rect.offset.inline_offset = line_left_offset;
      box_data.rect.size.inline_size = line_right_offset - line_left_offset;
    }

    position
  }

  /// Blink's `ApplyRelativePositioning`: `position: relative` moves the boxes and their children.
  pub(crate) fn apply_relative_positioning(
    &mut self,
    space: &ConstraintSpace,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    if self.box_data_list.is_empty() {
      return;
    }

    // The boxes' own offsets come from their first children's.
    let mut accumulated_offsets = vec![LogicalOffset::default(); line_box.len()];

    for box_data in &self.box_data_list {
      let relative_offset = space.compute_relative_offset_for_inline(box_data.node);

      for index in box_data.fragment_start..box_data.fragment_end {
        line_box[index].rect.offset += relative_offset;
        accumulated_offsets[index] += relative_offset;
      }
    }

    for box_data in &mut self.box_data_list {
      box_data.rect.offset += accumulated_offsets[box_data.fragment_start];
    }
  }

  /// Blink's `CreateBoxFragments`: the boxes' fragments, in place of their placeholders.
  pub(crate) fn create_box_fragments(
    &mut self,
    line_box: &mut LogicalLineItems<'a, 'c>,
    line_box_line_height: LayoutUnit,
  ) {
    if !self.has_box_fragments() {
      return;
    }

    let mut index = 0;

    while index < self.box_data_list.len() {
      let box_data = self.box_data_list[index].clone();
      let start = box_data.fragment_start;
      let end = box_data.fragment_end;

      debug_assert!(end > start);
      debug_assert!(box_data.item.should_create_box_fragment);

      let fragment = box_data.create_box_fragment(line_box, line_box_line_height);

      if line_box[start].is_placeholder() {
        let child = &mut line_box[start];

        child.layout_result = Some(fragment);
        child.rect = box_data.rect;
        child.children_count = end - start;
      } else {
        // Bidi reordering moved the placeholder.
        line_box.insert_child(
          start,
          LogicalLineItem {
            layout_result: Some(fragment),
            rect: box_data.rect,
            children_count: end - start + 1,
            bidi_level: 0,
            ..LogicalLineItem::default()
          },
        );
        self.child_inserted(start + 1);
      }

      index += 1;
    }

    self.box_data_list.clear();
  }

  /// Blink's `ApplyBaselineShift`: aligns the box by `vertical-align`, or queues it until the box
  /// it aligns to has its size.
  fn apply_baseline_shift(
    &mut self,
    index: usize,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) -> PositionPending {
    // Descendants waiting for this box's size align now.
    if !self.stack[index].pending_descendants.is_empty() {
      let mut pending_descendants = mem::take(&mut self.stack[index].pending_descendants);
      let mut has_top_or_bottom = false;
      let mut baseline_shift = LayoutUnit::ZERO;

      for child in &mut pending_descendants {
        if child.metrics.is_empty() {
          child.metrics = FontHeight::default();
        }

        match child.vertical_align {
          VerticalAlignKeyword::TextTop => {
            baseline_shift = child.metrics.ascent + self.stack[index].text_top();
          }
          VerticalAlignKeyword::TextBottom => {
            if let Some(metrics) = self.stack[index].font.primary_font_metrics() {
              baseline_shift = metrics.float_font_height().descent - child.metrics.descent;
            }
          }
          VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom => {
            has_top_or_bottom = true;

            continue;
          }
          _ => unreachable!("a pending alignment of another value"),
        }

        child.metrics.move_by(baseline_shift);
        self.stack[index].metrics.unite(&child.metrics);
        line_box.move_in_block_direction_range(
          baseline_shift,
          child.fragment_start,
          child.fragment_end,
        );
      }

      // `top` and `bottom` align to the box's size, which other pending ones may change.
      if has_top_or_bottom {
        self.stack[index].pending_descendants = pending_descendants.clone();

        let max = self.metrics_for_top_and_bottom_align(index, line_box);

        for child in &mut pending_descendants {
          baseline_shift = match child.vertical_align {
            VerticalAlignKeyword::Top => child.metrics.ascent - max.ascent,
            VerticalAlignKeyword::Bottom => max.descent - child.metrics.descent,
            _ => continue,
          };

          child.metrics.move_by(baseline_shift);
          self.stack[index].metrics.unite(&child.metrics);
          line_box.move_in_block_direction_range(
            baseline_shift,
            child.fragment_start,
            child.fragment_end,
          );
        }
      }

      self.stack[index].pending_descendants.clear();
    }

    let box_state = &self.stack[index];
    let vertical_align = box_state.style.style.base_data.vertical_align;

    if vertical_align == VerticalAlign::default() {
      return PositionPending::NotPending;
    }

    // Nothing to move.
    let fragment_end = line_box.len();

    if box_state.fragment_start == fragment_end {
      return PositionPending::NotPending;
    }

    // The line box itself has nothing to align to.
    if index == 0 {
      return PositionPending::NotPending;
    }

    let parent_box = &self.stack[index - 1];
    let baseline_shift = match vertical_align {
      VerticalAlign::Keyword(VerticalAlignKeyword::Sub) => {
        let font_size = parent_box.scaled_font_size_as_fixed();

        font_size / 5 + LayoutUnit::from_int(1)
      }
      VerticalAlign::Keyword(VerticalAlignKeyword::Super) => {
        let font_size = parent_box.scaled_font_size_as_fixed();

        -(font_size / 3 + LayoutUnit::from_int(1))
      }
      VerticalAlign::Length(length) => {
        // A percentage of the box's own `line-height`.
        // <https://www.w3.org/TR/CSS22/visudet.html#propdef-vertical-align>
        let line_height = if length.has_percentage() {
          computed_line_height_as_fixed(box_state.style, &box_state.font)
        } else {
          box_state.text_metrics.line_height()
        };

        -length.value_for(&box_state.style.sizing, line_height)
      }
      VerticalAlign::Keyword(VerticalAlignKeyword::Middle) => {
        let mut baseline_shift = (box_state.metrics.ascent - box_state.metrics.descent) / 2;

        if let Some(metrics) = parent_box.font.primary_font_metrics() {
          baseline_shift -= LayoutUnit::from_float_round(metrics.x_height.unwrap_or(0.0) / 2.0);
        }

        baseline_shift
      }
      VerticalAlign::Keyword(
        keyword @ (VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom),
      ) => {
        // The nearest ancestor aligned `top` or `bottom`, or the line box.
        let mut ancestor_index = index - 1;

        while ancestor_index > 0 {
          if matches!(
            self.stack[ancestor_index]
              .style
              .style
              .base_data
              .vertical_align,
            VerticalAlign::Keyword(VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom)
          ) {
            break;
          }

          ancestor_index -= 1;
        }

        let pending = PendingPositions {
          fragment_start: box_state.fragment_start,
          fragment_end,
          metrics: box_state.metrics,
          vertical_align: keyword,
        };

        self.stack[ancestor_index].pending_descendants.push(pending);

        return PositionPending::Pending;
      }
      VerticalAlign::Keyword(keyword) => {
        // The parent's size decides the rest.
        let pending = PendingPositions {
          fragment_start: box_state.fragment_start,
          fragment_end,
          metrics: box_state.metrics,
          vertical_align: keyword,
        };

        self.stack[index - 1].pending_descendants.push(pending);

        return PositionPending::Pending;
      }
    };
    let box_state = &mut self.stack[index];

    if !box_state.metrics.is_empty() {
      box_state.metrics.move_by(baseline_shift);
    }

    line_box.move_in_block_direction_range(baseline_shift, box_state.fragment_start, fragment_end);
    PositionPending::NotPending
  }

  /// Blink's `MetricsForTopAndBottomAlign`: the aligned subtree, which `top` and `bottom` align to,
  /// grown by a taller box aligned so.
  fn metrics_for_top_and_bottom_align(
    &self,
    index: usize,
    line_box: &LogicalLineItems<'a, 'c>,
  ) -> FontHeight {
    let box_state = &self.stack[index];

    debug_assert!(!box_state.pending_descendants.is_empty());

    // The descendants not aligned `top` or `bottom`.
    let mut metrics = box_state.metrics;

    // The boxes whose fragments come later count too.
    for box_data in &self.box_data_list {
      let context = &box_data.node.context;

      if matches!(
        context.style.base_data.vertical_align,
        VerticalAlign::Keyword(VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom)
      ) {
        continue;
      }

      // The top, with the baseline at zero.
      let placeholder = &line_box[box_data.fragment_start];

      debug_assert!(placeholder.is_placeholder());

      let box_ascent = -placeholder.rect.offset.block_offset;
      let mut box_metrics = FontHeight::new(box_ascent, box_data.rect.size.block_size - box_ascent);

      // Without padding, but with `line-height`.
      box_metrics.ascent -= box_data.padding.line_over;
      box_metrics.descent -= box_data.padding.line_under;

      let leading_space = calculate_leading_space(
        computed_line_height_as_fixed(context, &box_data.font),
        &box_metrics,
      );

      box_metrics.add_leading(leading_space);
      metrics.unite(&box_metrics);
    }

    if metrics.is_empty() {
      metrics = FontHeight::default();
    }

    // A taller box aligned `top` or `bottom` keeps one edge and extends the other.
    let mut max = metrics;

    for child in &box_state.pending_descendants {
      if matches!(
        child.vertical_align,
        VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom
      ) && child.metrics.line_height() > max.line_height()
      {
        max = if child.vertical_align == VerticalAlignKeyword::Top {
          FontHeight::new(metrics.ascent, child.metrics.line_height() - metrics.ascent)
        } else {
          FontHeight::new(
            child.metrics.line_height() - metrics.descent,
            metrics.descent,
          )
        };
      }
    }

    max
  }
}

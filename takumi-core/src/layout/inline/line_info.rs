//! Blink's [`LineInfo`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_info.cc),
//! without ruby, initial letters, or block fragmentation.

use std::rc::Rc;

use super::{
  inline_break_token::InlineBreakToken,
  inline_item::{CollapseType, InlineItemType},
  inline_item_result::{InlineItemResults, LayoutResult},
  inline_item_text_index::InlineItemTextIndex,
  inline_node_data::InlineNodeData,
};
use crate::{
  context::RenderContext,
  fonts::shaping::shape_result_run::AdjustMidCluster,
  layout::{bfc_rect::BfcOffset, layout_result::LayoutResult as BoxLayoutResult},
  layout_unit::LayoutUnit,
  style::{ComputedStyle, Direction as TextDirection, TextAlign, WhiteSpaceCollapse},
  text::character::{LINE_FEED, is_other_space_separator},
};

/// Blink's `IsHangingSpace`.
fn is_hanging_space(c: char) -> bool {
  c == ' ' || is_other_space_separator(c)
}

/// `text-align`, or on the last line and the one before a forced break `text-align-last`, per
/// css-text-3 §6.3. Written from the spec, as Blink's `ComputedStyle::GetTextAlign` is LGPL.
/// <https://drafts.csswg.org/css-text-3/#text-align-last-property>
pub(crate) fn text_align(style: &ComputedStyle, is_last_line: bool) -> TextAlign {
  let text_align = style.inherited_data.text_align;

  if is_last_line {
    style.inherited_data.text_align_last.resolve(text_align)
  } else {
    text_align
  }
}

/// Blink's `LineInfo`: the items one line holds, and how it measures.
pub(crate) struct LineInfo<'a, 'c> {
  /// Blink's `ItemsData`.
  pub(crate) items_data: &'a InlineNodeData<'c>,
  /// Blink's `LineStyle`.
  pub(crate) line_style: &'c RenderContext,
  pub(crate) results: InlineItemResults<'a, 'c>,
  /// Blink's `BfcOffset`: the line-left and block offsets of the line opportunity.
  pub(crate) bfc_offset: BfcOffset,
  pub(crate) break_token: Option<InlineBreakToken<'c>>,
  available_width: LayoutUnit,
  width: LayoutUnit,
  hang_width: LayoutUnit,
  pub(crate) text_indent: LayoutUnit,
  /// Blink's `TextFitScale`: the scale `text-fit` gives the line.
  pub(crate) text_fit_scale: f32,
  pub(crate) start: InlineItemTextIndex,
  pub(crate) end_item_index: usize,
  end_offset_for_justify: usize,
  text_align: TextAlign,
  pub(crate) base_direction: TextDirection,
  pub(crate) is_first_formatted_line: bool,
  pub(crate) is_start_of_paragraph: bool,
  pub(crate) is_last_line: bool,
  pub(crate) has_forced_break: bool,
  /// Blink's `IsBlockInInline`: the line holds a block-in-inline alone.
  pub(crate) is_block_in_inline: bool,
  pub(crate) is_empty_line: bool,
  pub(crate) has_line_even_if_empty: bool,
  pub(crate) has_overflow: bool,
  pub(crate) has_trailing_spaces: bool,
  needs_accurate_end_position: bool,
  allow_hang_for_alignment: bool,
}

impl<'a, 'c> LineInfo<'a, 'c> {
  pub(crate) fn new(items_data: &'a InlineNodeData<'c>, line_style: &'c RenderContext) -> Self {
    Self {
      items_data,
      line_style,
      results: InlineItemResults::new(),
      bfc_offset: BfcOffset::default(),
      break_token: None,
      available_width: LayoutUnit::ZERO,
      width: LayoutUnit::ZERO,
      hang_width: LayoutUnit::ZERO,
      text_indent: LayoutUnit::ZERO,
      text_fit_scale: 1.0,
      start: InlineItemTextIndex::default(),
      end_item_index: 0,
      end_offset_for_justify: 0,
      text_align: TextAlign::default(),
      base_direction: TextDirection::default(),
      is_first_formatted_line: false,
      is_start_of_paragraph: false,
      is_last_line: false,
      has_forced_break: false,
      is_block_in_inline: false,
      is_empty_line: false,
      has_line_even_if_empty: false,
      has_overflow: false,
      has_trailing_spaces: false,
      needs_accurate_end_position: false,
      allow_hang_for_alignment: false,
    }
  }

  /// Blink's `Reset`.
  pub(crate) fn reset(&mut self) {
    *self = Self::new(self.items_data, self.line_style);
  }

  /// Blink's `SetLineStyle`.
  pub(crate) fn set_line_style(&mut self, block: &'c RenderContext) {
    self.line_style = block;
    self.needs_accurate_end_position = self.compute_needs_accurate_end_position();
  }

  /// Blink's `BlockInInlineLayoutResult`.
  pub(crate) fn block_in_inline_layout_result(&self) -> Option<LayoutResult> {
    self
      .results
      .iter()
      .find(|result| result.item.item_type == InlineItemType::BlockInInline)
      .and_then(|result| result.layout_result)
  }

  /// Blink's `BlockInInlineLayoutResult`: the layout of the line's block-in-inline.
  pub(crate) fn block_in_inline_result(&self) -> Option<&Rc<BoxLayoutResult>> {
    self
      .results
      .iter()
      .find_map(|result| result.block_in_inline_result.as_ref())
  }

  /// Blink's `IsEndParagraph`.
  pub(crate) fn is_end_paragraph(&self) -> bool {
    self.break_token.is_none() || self.has_forced_break
  }

  pub(crate) fn text_align(&self) -> TextAlign {
    self.text_align
  }

  /// Blink's `UpdateTextAlign`.
  pub(crate) fn update_text_align(&mut self) {
    self.text_align = text_align(&self.line_style.style, self.is_last_line);
    self.allow_hang_for_alignment = true;

    if self.has_trailing_spaces {
      let (width, end_offset) = self.compute_trailing_space_width();

      self.hang_width = width;
      self.end_offset_for_justify = end_offset;

      return;
    }

    self.hang_width = LayoutUnit::ZERO;

    if self.text_align == TextAlign::Justify {
      self.end_offset_for_justify = self.inflow_end_offset();
    }
  }

  /// Blink's `ComputeNeedsAccurateEndPosition`: whether the line's end is where it aligns.
  fn compute_needs_accurate_end_position(&self) -> bool {
    match text_align(&self.line_style.style, false) {
      TextAlign::Start => false,
      TextAlign::End | TextAlign::Center | TextAlign::Justify => true,
      TextAlign::Left => self.base_direction == TextDirection::Rtl,
      TextAlign::Right => self.base_direction == TextDirection::Ltr,
    }
  }

  pub(crate) fn needs_accurate_end_position(&self) -> bool {
    self.needs_accurate_end_position
  }

  /// Blink's `AvailableWidth`.
  pub(crate) fn available_width(&self) -> LayoutUnit {
    self.available_width
  }

  /// Blink's `Width`.
  pub(crate) fn width(&self) -> LayoutUnit {
    self.width.clamp_negative_to_zero()
  }

  /// Blink's `WidthForAlignment`.
  pub(crate) fn width_for_alignment(&self) -> LayoutUnit {
    self.width - self.hang_width_for_alignment()
  }

  /// Blink's `HangWidth`.
  pub(crate) fn hang_width(&self) -> LayoutUnit {
    self.hang_width
  }

  /// Blink's `HangWidthForAlignment`.
  pub(crate) fn hang_width_for_alignment(&self) -> LayoutUnit {
    if self.allow_hang_for_alignment {
      self.hang_width
    } else {
      LayoutUnit::ZERO
    }
  }

  /// Blink's `SetWidth`.
  pub(crate) fn set_width(&mut self, available_width: LayoutUnit, width: LayoutUnit) {
    self.available_width = available_width;
    self.width = width;
  }

  /// Blink's `StartOffset`.
  pub(crate) fn start_offset(&self) -> usize {
    self.start.text_offset
  }

  /// Blink's `EndOffsetForJustify`.
  pub(crate) fn end_offset_for_justify(&self) -> usize {
    debug_assert_eq!(self.text_align, TextAlign::Justify);

    self.end_offset_for_justify
  }

  /// Blink's `End`.
  pub(crate) fn end(&self) -> InlineItemTextIndex {
    if let Some(break_token) = &self.break_token {
      return break_token.start;
    }

    let items = &self.items_data.items;

    if self.end_item_index != 0 && self.end_item_index < items.len() {
      return InlineItemTextIndex::new(
        self.end_item_index,
        items[self.end_item_index].start_offset(),
      );
    }

    InlineItemTextIndex::new(items.len(), self.items_data.text_content.len())
  }

  /// Blink's `EndTextOffset`.
  pub(crate) fn end_text_offset(&self) -> usize {
    self.end().text_offset
  }

  /// Blink's `InflowEndOffset`.
  pub(crate) fn inflow_end_offset(&self) -> usize {
    self.inflow_end_offset_internal(false)
  }

  /// Blink's `InflowEndOffsetInternal`: where the last item that takes space ends.
  fn inflow_end_offset_internal(&self, skip_forced_break: bool) -> usize {
    let text = &self.items_data.text_content;

    for result in self.results.iter().rev() {
      let item = result.item;

      if skip_forced_break
        && ((item.item_type == InlineItemType::Control
          && text[item.start_offset()..].starts_with(LINE_FEED))
          || (item.item_type == InlineItemType::Text && item.length() == 0))
      {
        continue;
      }

      if matches!(
        item.item_type,
        InlineItemType::Text | InlineItemType::Control | InlineItemType::AtomicInline
      ) {
        return result.end_offset();
      }
    }

    self.start_offset()
  }

  /// Blink's `IsHyphenated`.
  pub(crate) fn is_hyphenated(&self) -> bool {
    self
      .results
      .iter()
      .rev()
      .find(|result| result.length() > 0)
      .is_some_and(|result| result.is_hyphenated)
  }

  /// Blink's `ComputeTrailingSpaceWidth`: the width trailing spaces hang, and where the line
  /// ends without them.
  fn compute_trailing_space_width(&self) -> (LayoutUnit, usize) {
    if !self.has_trailing_spaces {
      return (LayoutUnit::ZERO, self.inflow_end_offset());
    }

    let text = &self.items_data.text_content;
    let mut trailing_spaces_width = LayoutUnit::ZERO;

    for result in self.results.iter().rev() {
      let item = result.item;

      if item.end_collapse_type == CollapseType::OpaqueToCollapsing {
        continue;
      }

      debug_assert!(!matches!(
        item.item_type,
        InlineItemType::Floating
          | InlineItemType::OutOfFlowPositioned
          | InlineItemType::BidiControl
      ));

      let mut trailing_item_width = LayoutUnit::ZERO;
      let mut will_continue = false;
      let mut end_offset = result.end_offset();

      if item.item_type == InlineItemType::Control || result.has_only_pre_wrap_trailing_spaces {
        trailing_item_width = result.inline_size;
        will_continue = true;
      } else if item.item_type == InlineItemType::Text {
        if result.length() == 0 {
          debug_assert_eq!(result.inline_size, LayoutUnit::ZERO);

          continue;
        }

        if text[..end_offset].ends_with(is_hanging_space) {
          while end_offset > result.start_offset() && text[..end_offset].ends_with(is_hanging_space)
          {
            end_offset -= text[..end_offset]
              .chars()
              .next_back()
              .map_or(1, char::len_utf8);
          }

          if end_offset == result.start_offset() {
            trailing_item_width = result.inline_size;
            will_continue = true;
          } else if let Some(shape_result_view) = &result.shape_result {
            debug_assert_eq!(item.direction(), self.base_direction);

            let shape_result = shape_result_view.create_shape_result();
            let end_position = shape_result.position_for_offset(
              (end_offset - shape_result.start_index as usize) as u32,
              AdjustMidCluster::ToEnd,
            );

            trailing_item_width = match self.base_direction {
              TextDirection::Rtl => LayoutUnit::from_float(end_position),
              TextDirection::Ltr => LayoutUnit::from_float(shape_result.width() - end_position),
            };
          }
        }
      }

      if trailing_item_width != LayoutUnit::ZERO {
        let style = item.style().map(|style| &style.inherited_data);

        match style.map(|style| style.white_space_collapse) {
          Some(WhiteSpaceCollapse::Collapse | WhiteSpaceCollapse::PreserveBreaks) => {
            trailing_spaces_width += trailing_item_width;
          }
          Some(WhiteSpaceCollapse::Preserve | WhiteSpaceCollapse::PreserveSpaces)
            if style.is_some_and(|style| style.text_wrap_mode.should_wrap_line()) =>
          {
            if trailing_spaces_width == LayoutUnit::ZERO
              && (self.has_forced_break || self.is_last_line)
            {
              let item_end = self.width - trailing_spaces_width;
              let actual_hang_width = trailing_item_width
                .min(item_end - self.available_width)
                .clamp_negative_to_zero();

              if actual_hang_width != trailing_item_width {
                will_continue = false;
              }

              trailing_spaces_width += actual_hang_width;
            } else {
              trailing_spaces_width += trailing_item_width;
            }
          }
          _ => {
            if will_continue {
              end_offset = item.end_offset();
              will_continue = false;
            }
          }
        }
      } else {
        trailing_spaces_width += trailing_item_width;
      }

      if !will_continue {
        return (trailing_spaces_width, end_offset);
      }
    }

    (trailing_spaces_width, self.start_offset())
  }

  /// Blink's `ComputeWidth`.
  pub(crate) fn compute_width(&self) -> LayoutUnit {
    self
      .results
      .iter()
      .fold(self.text_indent, |width, result| width + result.inline_size)
  }
}

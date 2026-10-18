//! Blink's [`LineTruncator`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_truncator.cc),
//! without `TruncateLineInTheMiddle`, which only `<input type=file>` uses.

use std::rc::Rc;

use super::{
  inline_box_state::InlineLayoutStateStack,
  inline_item::StyleVariant,
  line_info::LineInfo,
  logical_line_item::{LogicalLineItem, LogicalLineItems},
};
use crate::{
  context::RenderContext,
  fonts::{
    font::Font,
    shaping::{
      harfbuzz_shaper::HarfBuzzShaper, shape_result::ShapeResult,
      shape_result_view::ShapeResultView,
    },
  },
  layout::logical_rect::LogicalRect,
  layout_unit::LayoutUnit,
  resources::font::PrimaryFontMetrics,
  style::{BlockEllipsis, Direction as TextDirection, TextOverflow},
  text::{
    bidi_paragraph::{BidiParagraph, direction_from_level},
    character::{FORM_FEED, LINE_FEED, maybe_bidi_rtl},
  },
};

const HORIZONTAL_ELLIPSIS: char = '\u{2026}';

/// Blink's `IsForcedLineBreak`: a preserved segment break, or a UAX #14 BK or NL character.
/// <https://drafts.csswg.org/css-text-3/#forced-line-break>
fn is_forced_line_break(c: char) -> bool {
  matches!(
    c,
    LINE_FEED | '\u{B}' | FORM_FEED | '\u{85}' | '\u{2028}' | '\u{2029}'
  )
}

/// Blink's `SuppressLineBreaks`: each run of forced line breaks becomes one space.
fn suppress_line_breaks(text: &str) -> String {
  let mut suppressed = String::with_capacity(text.len());
  let mut after_break = false;

  for c in text.chars() {
    if is_forced_line_break(c) {
      if !after_break {
        suppressed.push(' ');
      }

      after_break = true;

      continue;
    }

    after_break = false;
    suppressed.push(c);
  }

  suppressed
}

/// Blink's `EllipsisShapeResult`: a bidi run of the ellipsis.
struct EllipsisShapeResult {
  shape_result: Rc<ShapeResultView>,
  text: String,
  bidi_level: u8,
}

/// Blink's `LineTruncator`: ellipsizes a line that overflows its box.
pub(crate) struct LineTruncator<'a, 'c> {
  /// The paragraph's text, which the line's offsets index.
  text: &'a str,
  line_style: &'c RenderContext,
  font: Rc<Font>,
  /// The width the line may take, without `text-indent`.
  available_width: LayoutUnit,
  line_direction: TextDirection,
  is_ellipsis_caused_by_line_clamp: bool,
  ellipsis_shape_results: Vec<EllipsisShapeResult>,
  ellipsis_width: LayoutUnit,
}

impl<'a, 'c> LineTruncator<'a, 'c> {
  pub(crate) fn new(line_info: &LineInfo<'a, 'c>, is_ellipsis_caused_by_line_clamp: bool) -> Self {
    let items_data = line_info.items_data;

    Self {
      text: &items_data.text_content,
      line_style: line_info.line_style,
      font: items_data.fonts.get(line_info.line_style),
      available_width: line_info.available_width() - line_info.text_indent,
      line_direction: line_info.base_direction,
      is_ellipsis_caused_by_line_clamp,
      ellipsis_shape_results: Vec::new(),
      ellipsis_width: LayoutUnit::ZERO,
    }
  }

  /// Blink's `ComputeEllipsisText`: `text-overflow`'s string, or an ellipsis character, or three
  /// periods when the font has no ellipsis. The ellipsis takes the line's style.
  /// <https://drafts.csswg.org/css-ui/#ellipsing-details>
  /// A `line-clamp` takes its `block-ellipsis` string, as css-overflow-4 says and Blink's
  /// `-webkit-line-clamp` does not. <https://drafts.csswg.org/css-overflow-4/#block-ellipsis>
  fn compute_ellipsis_text(&self) -> String {
    let style = &self.line_style.style;

    match (
      &style.misc4_data.text_overflow,
      &style.misc_inherited1_data.block_ellipsis,
    ) {
      (TextOverflow::Custom(text), _) if !self.is_ellipsis_caused_by_line_clamp => {
        return suppress_line_breaks(text);
      }
      (_, BlockEllipsis::String(text)) if self.is_ellipsis_caused_by_line_clamp => {
        return suppress_line_breaks(text);
      }
      _ => {}
    }

    let has_ellipsis = self
      .font
      .primary_font()
      .is_some_and(|font_data| font_data.has_glyph(HORIZONTAL_ELLIPSIS));

    if has_ellipsis {
      HORIZONTAL_ELLIPSIS.to_string()
    } else {
      "...".to_owned()
    }
  }

  /// Blink's `SetupEllipsis`: shapes the ellipsis, a run per bidi level when it mixes directions.
  fn setup_ellipsis(&mut self) {
    let ellipsis_text = self.compute_ellipsis_text();
    let bidi = maybe_bidi_rtl(&ellipsis_text)
      .then(|| BidiParagraph::new(&ellipsis_text, Some(self.line_direction)))
      .filter(|bidi| !bidi.is_unidirectional() || bidi.base_direction() == TextDirection::Rtl);

    let Some(bidi) = bidi else {
      let shape_result = self.shape(ellipsis_text.clone(), self.line_direction);

      self.ellipsis_width = shape_result.snapped_width();
      self.ellipsis_shape_results.push(EllipsisShapeResult {
        shape_result,
        text: ellipsis_text,
        bidi_level: 0,
      });

      return;
    };

    for run in bidi.get_visual_runs() {
      let run_text = ellipsis_text[run.start..run.end].to_owned();
      let shape_result = self.shape(run_text.clone(), direction_from_level(run.level));

      self.ellipsis_width += shape_result.snapped_width();
      self.ellipsis_shape_results.push(EllipsisShapeResult {
        shape_result,
        text: run_text,
        bidi_level: run.level,
      });
    }
  }

  fn shape(&self, text: String, direction: TextDirection) -> Rc<ShapeResultView> {
    let shape_result = HarfBuzzShaper::new(text).shape_all(&self.font, direction);

    Rc::new(ShapeResultView::create_from_result(&Rc::new(shape_result)))
  }

  /// Blink's `PlaceEllipsisNextTo`: the ellipsis after `line_box[ellipsized_index]`, which it
  /// belongs to. Returns its inline offset.
  fn place_ellipsis_next_to(
    &self,
    line_box: &mut LogicalLineItems<'a, 'c>,
    ellipsized_index: usize,
  ) -> LayoutUnit {
    let ellipsized_child = &line_box[ellipsized_index];

    debug_assert!(ellipsized_child.has_in_flow_fragment());

    let Some(layout_object) = ellipsized_child.layout_object() else {
      return LayoutUnit::ZERO;
    };
    let ellipsis_inline_offset = match self.line_direction {
      TextDirection::Ltr => {
        ellipsized_child.rect.offset.inline_offset + ellipsized_child.inline_size
      }
      TextDirection::Rtl => ellipsized_child.rect.offset.inline_offset - self.ellipsis_width,
    };
    let ellipsis_metrics = self
      .font
      .primary_font_metrics()
      .map(PrimaryFontMetrics::font_height)
      .unwrap_or_default();
    let mut current_offset = ellipsis_inline_offset;

    for ellipsis in &self.ellipsis_shape_results {
      let inline_size = ellipsis.shape_result.snapped_width();

      line_box.add_child(LogicalLineItem::ellipsis(
        layout_object,
        StyleVariant::StandardEllipsis,
        Rc::clone(&ellipsis.shape_result),
        ellipsis.text.clone(),
        LogicalRect::new(
          current_offset,
          -ellipsis_metrics.ascent,
          inline_size,
          ellipsis_metrics.line_height(),
        ),
        ellipsis.bidi_level,
      ));
      current_offset += inline_size;
    }

    ellipsis_inline_offset
  }

  /// Blink's `TruncateLine`: the line `line_width` wide, ellipsized at its end. Returns its new
  /// width.
  pub(crate) fn truncate_line(
    &mut self,
    line_width: LayoutUnit,
    line_box: &mut LogicalLineItems<'a, 'c>,
    box_states: &mut InlineLayoutStateStack<'a, 'c>,
  ) -> LayoutUnit {
    self.setup_ellipsis();

    // From the logical last child to the first, the child to place the ellipsis after; children
    // past it truncate or hide.
    let mut truncated_child = None;
    let indices: Vec<usize> = match self.line_direction {
      TextDirection::Ltr => (0..line_box.len()).rev().collect(),
      TextDirection::Rtl => (0..line_box.len()).collect(),
    };
    let first_child = match self.line_direction {
      TextDirection::Ltr => line_box
        .children
        .iter()
        .position(LogicalLineItem::has_in_flow_fragment),
      TextDirection::Rtl => line_box
        .children
        .iter()
        .rposition(LogicalLineItem::has_in_flow_fragment),
    };
    let ellipsized_index = indices.into_iter().find(|&index| {
      self.ellipsize_child(
        line_width,
        Some(index) == first_child,
        &mut line_box[index],
        &mut truncated_child,
      )
    });
    let Some(mut ellipsized_index) = ellipsized_index else {
      return line_width;
    };

    // The original stays, hidden, for the layout it holds; its truncated copy follows it.
    if let Some(truncated_child) = truncated_child {
      line_box.insert_child(ellipsized_index + 1, truncated_child);
      box_states.child_inserted(ellipsized_index + 1);
      hide_child(&mut line_box[ellipsized_index]);

      let child_to_truncate_inline_size = line_box[ellipsized_index].inline_size;

      ellipsized_index += 1;

      let ellipsized_child = &mut line_box[ellipsized_index];

      debug_assert!(ellipsized_child.inline_size <= child_to_truncate_inline_size);

      if self.line_direction == TextDirection::Rtl {
        ellipsized_child.rect.offset.inline_offset +=
          child_to_truncate_inline_size - ellipsized_child.inline_size;
      }
    }

    let ellipsis_inline_offset = self.place_ellipsis_next_to(line_box, ellipsized_index);

    (ellipsis_inline_offset + self.ellipsis_width).max(line_width)
  }

  /// Blink's `EllipsizeChild`: whether the ellipsis goes after `child`, which may truncate to make
  /// room for it.
  fn ellipsize_child(
    &self,
    line_width: LayoutUnit,
    is_first_child: bool,
    child: &mut LogicalLineItem<'a, 'c>,
    truncated_child: &mut Option<LogicalLineItem<'a, 'c>>,
  ) -> bool {
    debug_assert!(truncated_child.is_none());

    // Out-of-flow children stay; an inline box is not ellipsized, though an empty one already
    // has its fragment.
    if !child.has_in_flow_fragment() || child.is_inline_box() {
      return false;
    }

    let child_inline_offset = match self.line_direction {
      TextDirection::Ltr => child.rect.offset.inline_offset,
      TextDirection::Rtl => line_width - (child.rect.offset.inline_offset + child.inline_size),
    };
    let mut space_for_child = self.available_width - child_inline_offset;

    // A child outside the content box still hides: padding may leave it inside the clip.
    if space_for_child <= LayoutUnit::ZERO {
      if !is_first_child {
        hide_child(child);
      }

      return false;
    }

    space_for_child -= self.ellipsis_width;

    if space_for_child >= child.inline_size {
      return true;
    }

    if self.truncate_child(space_for_child, is_first_child, child, truncated_child) {
      return true;
    }

    // Partly in the box but too wide to truncate, it hides behind an earlier sibling's ellipsis.
    if !is_first_child {
      hide_child(child);
    }

    false
  }

  /// Blink's `TruncateChild`: `child`'s text cut to `space_for_child`. A first child keeps at
  /// least its first character, which css-ui clips rather than ellipsizes.
  /// <https://drafts.csswg.org/css-ui/#text-overflow>
  fn truncate_child(
    &self,
    space_for_child: LayoutUnit,
    is_first_child: bool,
    child: &LogicalLineItem<'a, 'c>,
    truncated_child: &mut Option<LogicalLineItem<'a, 'c>>,
  ) -> bool {
    debug_assert!(truncated_child.is_none());

    if space_for_child <= LayoutUnit::ZERO && !is_first_child {
      return false;
    }

    // Only text truncates.
    let Some(shape_result) = &child.shape_result else {
      return is_first_child;
    };
    let shape_result = shape_result.create_shape_result();
    let text_length = child.text_offset.len();
    let position = match self.line_direction {
      TextDirection::Ltr => space_for_child.to_float(),
      TextDirection::Rtl => shape_result.width() - space_for_child.to_float(),
    };
    let mut offset_to_fit = shape_result.offset_to_fit(position, self.line_direction) as usize;

    debug_assert!(offset_to_fit <= text_length);

    if offset_to_fit == 0 || offset_to_fit == text_length {
      if !is_first_child {
        return false;
      }

      // Blink moves a UTF-16 code unit in; these offsets are bytes, so move a character in.
      let text = &self.text[child.text_offset.clone()];

      offset_to_fit = if offset_to_fit == 0 {
        text.chars().next().map_or(0, char::len_utf8)
      } else {
        text_length - text.chars().next_back().map_or(0, char::len_utf8)
      };
    }

    *truncated_child = Some(truncate_text(
      child,
      shape_result,
      offset_to_fit,
      self.line_direction,
    ));

    true
  }
}

/// Blink's `HideChild`: a text child hides, an atomic inline marks itself truncated.
fn hide_child(child: &mut LogicalLineItem<'_, '_>) {
  debug_assert!(child.has_in_flow_fragment());

  if child.layout_result.is_some() {
    child.is_truncated = true;
  } else if child.inline_item.is_some() {
    child.is_hidden_due_to_layout = true;
  }
}

/// Blink's `TruncateText`: `item`'s text up to `offset_to_fit` from its start in `direction`.
fn truncate_text<'a, 'c>(
  item: &LogicalLineItem<'a, 'c>,
  shape_result: ShapeResult,
  offset_to_fit: usize,
  direction: TextDirection,
) -> LogicalLineItem<'a, 'c> {
  let start = item.text_offset.start;
  let new_text_offset = if direction == shape_result.direction() {
    start..start + offset_to_fit
  } else {
    start + offset_to_fit..item.text_offset.end
  };
  let new_shape_result = ShapeResultView::create_from_result_range(
    &Rc::new(shape_result),
    new_text_offset.start as u32,
    new_text_offset.end as u32,
  );

  LogicalLineItem::sub_range(item, Rc::new(new_shape_result), new_text_offset)
}

#[cfg(test)]
mod tests {
  use super::suppress_line_breaks;
  use crate::{
    layout::{
      inline::{
        inline_node::InlineNode,
        testing::{TestAtomicInline, render_tree},
      },
      node::Node,
      tree::RenderNode,
    },
    layout_unit::LayoutUnit,
  };

  const ELLIPSIS: &str =
    ".container { overflow: hidden; text-overflow: ellipsis; white-space: nowrap }";

  /// The visible items of the line at `line_index`: each text, or ellipsis, with its inline
  /// offset and size.
  fn line_items(block: &RenderNode, available_width: f32, line_index: usize) -> Vec<String> {
    let node = InlineNode::new(block);
    let data = node.prepare_layout();
    let mut atomic_inline = TestAtomicInline;
    let lines = node.layout_lines(
      &data,
      LayoutUnit::from_float(available_width),
      &mut atomic_inline,
    );

    lines[line_index]
      .items
      .children
      .iter()
      .filter(|item| item.shape_result.is_some() && !item.is_hidden_due_to_layout)
      .map(|item| {
        let text = item
          .text_content
          .clone()
          .unwrap_or_else(|| data.text_content[item.text_offset.clone()].to_owned());

        format!(
          "{text}@{}+{}",
          item.rect.offset.inline_offset.to_float(),
          item.inline_size.to_float()
        )
      })
      .collect()
  }

  fn visible_items(block: &RenderNode, available_width: f32) -> Vec<String> {
    line_items(block, available_width, 0)
  }

  /// `line-clamp` ellipsizes its last line whether it overflows or not, and leaves the lines
  /// before it.
  #[test]
  fn line_clamp_ellipsizes_its_last_line() {
    let block = render_tree(
      [Node::text("aaa bbb ccc ddd eee")],
      ".container { line-clamp: 2 }",
    );

    assert_eq!(line_items(&block, 42.0, 0), ["aaa bbb@0+42"]);
    assert_eq!(
      line_items(&block, 42.0, 1),
      ["ccc dd@0+36", "\u{2026}@36+6"]
    );
  }

  /// A clamp line with no line after it keeps its text.
  #[test]
  fn line_clamp_leaves_a_last_line_whole() {
    let block = render_tree([Node::text("aaa bbb ccc")], ".container { line-clamp: 2 }");

    assert_eq!(line_items(&block, 42.0, 1), ["ccc@0+18"]);
  }

  /// A balanced paragraph still ellipsizes its clamp line; these two lines score a tie, which keeps
  /// the greedy breaks.
  #[test]
  fn a_balanced_clamp_line_ends_in_an_ellipsis() {
    let block = render_tree(
      [Node::text("aaa bbb ccc ddd eee fff ggg")],
      ".container { line-clamp: 2; text-wrap: balance }",
    );

    assert_eq!(line_items(&block, 66.0, 0), ["aaa bbb ccc@0+66"]);
    assert_eq!(
      line_items(&block, 66.0, 1),
      ["ddd eee ff@0+60", "\u{2026}@60+6"]
    );
  }

  /// css-overflow-4's `block-ellipsis` string replaces the ellipsis.
  #[test]
  fn line_clamp_takes_its_block_ellipsis_string() {
    let block = render_tree(
      [Node::text("aaa bbb ccc ddd eee")],
      ".container { line-clamp: 2 \" >\" }",
    );

    assert_eq!(line_items(&block, 42.0, 1), ["ccc d@0+30", " >@30+12"]);
  }

  #[test]
  fn an_overflowing_line_ends_in_an_ellipsis() {
    let block = render_tree([Node::text("abcdefghij")], ELLIPSIS);

    assert_eq!(visible_items(&block, 36.0), ["abcde@0+30", "\u{2026}@30+6"]);
  }

  /// An RTL line overflows to the left, where its ellipsis goes; LTR text in it keeps its end.
  #[test]
  fn an_rtl_line_ellipsizes_on_its_left() {
    let block = render_tree(
      [Node::text("abcdefghij")],
      &format!("{ELLIPSIS} .container {{ direction: rtl }}"),
    );

    assert_eq!(
      visible_items(&block, 36.0),
      ["fghij@30+30", "\u{2026}@24+6"]
    );
  }

  /// With no room for even one character beside the ellipsis, the first character stays, clipped.
  #[test]
  fn the_first_character_stays() {
    let block = render_tree([Node::text("\u{E9}bc")], ELLIPSIS);

    assert_eq!(visible_items(&block, 6.0), ["\u{E9}@0+6", "\u{2026}@6+6"]);
  }

  #[test]
  fn a_line_that_fits_keeps_its_text() {
    let block = render_tree([Node::text("abc")], ELLIPSIS);

    assert_eq!(visible_items(&block, 36.0), ["abc@0+18"]);
  }

  #[test]
  fn forced_breaks_in_the_ellipsis_string_become_one_space() {
    assert_eq!(suppress_line_breaks("a\n\u{2028}b\nc"), "a b c");
  }
}

//! Blink's [`InlineNodeData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_node_data.h),
//! with `InlineItemsData` folded in.

use std::{
  cell::{OnceCell, RefCell},
  collections::HashMap,
  fmt::{self, Debug, Formatter},
  ops::Range,
  ptr,
  rc::Rc,
};

use rustc_hash::FxHashMap;

use super::{
  inline_item::{InlineItem, InlineItemType, InlineItems, LayoutObject},
  inline_item_segment::InlineItemSegments,
};
use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  fonts::{font::Font, font_description::FontDescription},
  style::Direction as TextDirection,
  text::{
    bidi_paragraph::BidiParagraph,
    character::{LINE_FEED, is_8bit},
    line_break_iterator::{BreakOptions, LineBreakIterator},
  },
};

/// Blink's `InlineNodeData`, without first-line items, SVG text, ruby, initial letters, or text
/// emphasis.
#[derive(Debug, Default)]
pub(crate) struct InlineNodeData<'c> {
  pub(crate) text_content: String,
  pub(crate) items: InlineItems<'c>,
  /// The runs of `text_content`, when it has more than one.
  pub(crate) segments: Option<InlineItemSegments>,
  pub(crate) has_non_orc_16bit: bool,
  pub(crate) is_bidi_enabled: bool,
  pub(crate) base_direction: TextDirection,
  pub(crate) has_floats: bool,
  pub(crate) has_out_of_flow_positioned: bool,
  pub(crate) is_bisect_line_break_disabled: bool,
  pub(crate) is_score_line_break_disabled: bool,
  pub(crate) fonts: StyleFonts,
  /// The soft wrap opportunities of `text_content` per break type, which every line breaking pass
  /// shares.
  pub(crate) break_opportunities: RefCell<FxHashMap<BreakOptions, Rc<[bool]>>>,
  /// Where each paragraph after a line feed starts, and the direction its first strong character
  /// gives it, for `unicode-bidi: plaintext`.
  pub(crate) plaintext_paragraphs: OnceCell<Vec<(usize, TextDirection)>>,
}

impl<'c> InlineNodeData<'c> {
  /// The base direction `unicode-bidi: plaintext` gives the paragraph `offset` is in, or none in
  /// the first paragraph, whose direction the block's bidi paragraph resolved. Computed once, as
  /// each line's line breaker asks again.
  pub(crate) fn plaintext_paragraph_direction(&self, offset: usize) -> Option<TextDirection> {
    let paragraphs = self.plaintext_paragraphs.get_or_init(|| {
      // Latin-1 text has no right-to-left character.
      if is_8bit(&self.text_content) {
        return Vec::new();
      }

      self
        .text_content
        .match_indices(LINE_FEED)
        .map(|(line_feed, _)| {
          let start = line_feed + LINE_FEED.len_utf8();
          let direction = BidiParagraph::base_direction_for_string(
            &self.text_content[start..],
            Some(|c| c == LINE_FEED),
          );

          // UAX #9 rules P2 and P3 make a paragraph without a strong character left-to-right.
          (start, direction.unwrap_or(TextDirection::Ltr))
        })
        .collect()
    });
    let index = paragraphs.partition_point(|&(start, _)| start <= offset);

    index.checked_sub(1).map(|index| paragraphs[index].1)
  }

  /// An iterator over the soft wrap opportunities of `text_content` under `options`, computed
  /// once.
  pub(crate) fn break_iterator(&self, options: BreakOptions) -> LineBreakIterator<'_> {
    let breakable = Rc::clone(
      self
        .break_opportunities
        .borrow_mut()
        .entry(options)
        .or_insert_with(|| LineBreakIterator::opportunities(&self.text_content, options)),
    );

    LineBreakIterator::with_opportunities(&self.text_content, breakable, options)
  }

  /// Blink's `HasFloatingOrOutOfFlowPositioned`.
  pub(crate) fn has_floating_or_out_of_flow_positioned(&self) -> bool {
    self.has_floats || self.has_out_of_flow_positioned
  }

  /// The style an item after the close tag at `index` takes: the box enclosing that tag's box,
  /// else `block`, as Blink reads `LayoutObject::Parent`.
  pub(crate) fn parent_context(&self, index: usize, block: &'c RenderContext) -> &'c RenderContext {
    let mut depth = 0_isize;

    for item in self.items[..=index].iter().rev() {
      match item.item_type {
        InlineItemType::CloseTag => depth += 1,
        InlineItemType::OpenTag => {
          depth -= 1;

          if depth < 0 {
            return item.layout_object.map_or(block, LayoutObject::context);
          }
        }
        _ => {}
      }
    }

    block
  }

  /// Blink's `GetOpenTagItems`: the tags `items[range]` leaves open.
  pub(crate) fn open_tag_items(&self, range: Range<usize>) -> Vec<&InlineItem<'c>> {
    let mut open_items = Vec::new();

    for item in &self.items[range] {
      match item.item_type {
        InlineItemType::OpenTag => open_items.push(item),
        InlineItemType::CloseTag => {
          open_items.pop();
        }
        _ => {}
      }
    }

    open_items
  }
}

/// The font of each style the context shapes with: Blink's `ComputedStyle::GetFont`.
#[derive(Default)]
pub(crate) struct StyleFonts {
  fonts: RefCell<HashMap<*const RenderContext, Rc<Font>>>,
}

impl StyleFonts {
  pub(crate) fn get(&self, context: &RenderContext) -> Rc<Font> {
    let mut fonts = self.fonts.borrow_mut();
    let font = fonts.entry(ptr::from_ref(context)).or_insert_with(|| {
      let description = FontDescription::new(&SizedFontStyle::from_style(&context.style, context));

      Rc::new(Font::new(context.fonts(), description))
    });

    Rc::clone(font)
  }
}

impl Debug for StyleFonts {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("StyleFonts")
      .field("len", &self.fonts.borrow().len())
      .finish()
  }
}

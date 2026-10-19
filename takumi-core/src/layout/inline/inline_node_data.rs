//! Blink's [`InlineNodeData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_node_data.h),
//! with `InlineItemsData` folded in.

use std::{
  cell::RefCell,
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
  text::line_break_iterator::{BreakSpaceType, LineBreakIterator, LineBreakType},
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
  pub(crate) break_opportunities: RefCell<FxHashMap<BreakKind, Rc<[bool]>>>,
}

/// The break type and break space a line breaker asks for opportunities under.
type BreakKind = (LineBreakType, BreakSpaceType);

impl<'c> InlineNodeData<'c> {
  /// An iterator over the soft wrap opportunities of `text_content` under `break_type` and
  /// `break_space`, computed once.
  pub(crate) fn break_iterator(
    &self,
    break_type: LineBreakType,
    break_space: BreakSpaceType,
  ) -> LineBreakIterator<'_> {
    let breakable = Rc::clone(
      self
        .break_opportunities
        .borrow_mut()
        .entry((break_type, break_space))
        .or_insert_with(|| {
          LineBreakIterator::opportunities(&self.text_content, break_type, break_space)
        }),
    );

    LineBreakIterator::with_opportunities(&self.text_content, breakable, break_type, break_space)
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

      Rc::new(
        context
          .fonts()
          .with_context(|fonts| Font::new(fonts, description)),
      )
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

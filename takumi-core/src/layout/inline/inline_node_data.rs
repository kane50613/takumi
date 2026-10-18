//! Blink's [`InlineNodeData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_node_data.h),
//! with `InlineItemsData` folded in.

use std::{
  cell::RefCell,
  collections::HashMap,
  fmt::{self, Debug, Formatter},
  ptr,
  rc::Rc,
};

use super::{inline_item::InlineItems, inline_item_segment::InlineItemSegments};
use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  fonts::{font::Font, font_description::FontDescription},
  style::Direction as TextDirection,
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
  pub(crate) is_block_level: bool,
  pub(crate) is_bisect_line_break_disabled: bool,
  pub(crate) is_score_line_break_disabled: bool,
  pub(crate) fonts: StyleFonts,
}

impl InlineNodeData<'_> {
  /// Blink's `HasFloatingOrOutOfFlowPositioned`.
  pub(crate) fn has_floating_or_out_of_flow_positioned(&self) -> bool {
    self.has_floats || self.has_out_of_flow_positioned
  }

  /// Blink's `DisableBidi`.
  pub(crate) fn disable_bidi(&mut self) {
    self.is_bidi_enabled = false;
    self.base_direction = TextDirection::Ltr;
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

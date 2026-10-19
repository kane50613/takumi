//! Approximate stand-in for the fallback half of Blink's `FontCache`, searching registered fonts; see #2001.

use std::rc::Rc;

use super::{font_fallback_priority::FontFallbackPriority, simple_font_data::SimpleFontData};

/// The registered faces at one size, in fallback order.
#[derive(Debug)]
pub(crate) struct FontCache {
  fallback_fonts: Vec<Rc<SimpleFontData>>,
}

impl FontCache {
  pub(crate) fn new(fallback_fonts: Vec<Rc<SimpleFontData>>) -> Self {
    Self { fallback_fonts }
  }

  /// Blink's `FallbackFontForCharacter`.
  pub(crate) fn fallback_font_for_character(
    &self,
    hint: char,
    font_fallback_priority: FontFallbackPriority,
  ) -> Option<Rc<SimpleFontData>> {
    let with_glyph = |is_color: bool| {
      self
        .fallback_fonts
        .iter()
        .find(|font| font.face().is_color == is_color && font.has_glyph(hint))
    };
    let (preferred, other) = match font_fallback_priority {
      FontFallbackPriority::Emoji | FontFallbackPriority::EmojiBySelector => (true, false),
      _ => (false, true),
    };

    with_glyph(preferred).or_else(|| with_glyph(other)).cloned()
  }

  /// Blink's `GetLastResortFallbackFont`.
  pub(crate) fn last_resort_fallback_font(&self) -> Option<Rc<SimpleFontData>> {
    self.fallback_fonts.first().cloned()
  }
}

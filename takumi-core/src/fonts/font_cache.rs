//! takumi's stand-in for the fallback half of Blink's `FontCache`, which asks the system for a font
//! with a character. takumi has no system fonts, so it searches the registered ones.
//!
//! Approximate: fontconfig ranks system fonts by their own rules, where this takes the registered
//! fallback order, color families first for emoji presentation and last for text presentation.

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

  /// Blink's `FallbackFontForCharacter`: a face with a glyph for `hint`.
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
      FontFallbackPriority::EmojiEmoji | FontFallbackPriority::EmojiEmojiWithVs => (true, false),
      _ => (false, true),
    };

    with_glyph(preferred).or_else(|| with_glyph(other)).cloned()
  }

  /// Blink's `GetLastResortFallbackFont`: the first registered face.
  pub(crate) fn last_resort_fallback_font(&self) -> Option<Rc<SimpleFontData>> {
    self.fallback_fonts.first().cloned()
  }
}

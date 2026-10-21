//! Approximate stand-in for the fallback half of Blink's `FontCache`, searching registered fonts; see #2001.

use std::{cell::RefCell, collections::hash_map::Entry, rc::Rc};

use rustc_hash::FxHashMap;

use super::{font_fallback_priority::FontFallbackPriority, simple_font_data::SimpleFontData};

/// A fallback answer by character and whether color faces come first.
type FallbackAnswers = FxHashMap<(char, bool), Option<Rc<SimpleFontData>>>;

/// The registered faces at one size, in fallback order.
#[derive(Debug)]
pub(crate) struct FontCache {
  fallback_fonts: Vec<Rc<SimpleFontData>>,
  fallbacks: RefCell<FallbackAnswers>,
}

impl FontCache {
  pub(crate) fn new(fallback_fonts: Vec<Rc<SimpleFontData>>) -> Self {
    Self {
      fallback_fonts,
      fallbacks: RefCell::default(),
    }
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
        .find(|font| font.face.typeface.is_color == is_color && font.has_glyph(hint))
    };
    let (preferred, other) = match font_fallback_priority {
      FontFallbackPriority::Emoji | FontFallbackPriority::EmojiBySelector => (true, false),
      _ => (false, true),
    };

    match self.fallbacks.borrow_mut().entry((hint, preferred)) {
      Entry::Occupied(entry) => entry.get().clone(),
      Entry::Vacant(entry) => entry
        .insert(with_glyph(preferred).or_else(|| with_glyph(other)).cloned())
        .clone(),
    }
  }

  /// Blink's `GetLastResortFallbackFont`.
  pub(crate) fn last_resort_fallback_font(&self) -> Option<Rc<SimpleFontData>> {
    self.fallback_fonts.first().cloned()
  }
}

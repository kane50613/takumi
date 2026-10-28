//! Approximate stand-in for the fallback half of Blink's `FontCache`, searching registered fonts; see #2001.
//!
//! Pending faces of the fallback families take part as their registered faces would, though Blink's
//! system fallback never picks a web font: a pending face counts as a text face that has every
//! codepoint of its `unicode-range`, since it has no glyphs to read yet.

use std::{cell::RefCell, collections::hash_map::Entry, rc::Rc};

use rustc_hash::FxHashMap;

use super::{
  font_face::{FaceData, PendingFace},
  font_fallback_priority::FontFallbackPriority,
  simple_font_data::SimpleFontData,
};

/// A fallback answer by character and whether color faces come first.
type FallbackAnswers = FxHashMap<(char, bool), Option<Rc<SimpleFontData>>>;

/// The registered faces at one size, in fallback order.
#[derive(Debug)]
pub(crate) struct FontCache {
  fallback_fonts: Vec<FaceData<Rc<SimpleFontData>>>,
  fallbacks: RefCell<FallbackAnswers>,
}

impl FontCache {
  pub(crate) fn new(fallback_fonts: Vec<FaceData<Rc<SimpleFontData>>>) -> Self {
    Self {
      fallback_fonts,
      fallbacks: RefCell::default(),
    }
  }

  /// The loaded faces, in fallback order.
  fn loaded_fonts(&self) -> impl Iterator<Item = &Rc<SimpleFontData>> {
    self.fallback_fonts.iter().filter_map(FaceData::loaded)
  }

  /// Whether color faces come first for `font_fallback_priority`.
  fn prefers_color(font_fallback_priority: FontFallbackPriority) -> bool {
    matches!(
      font_fallback_priority,
      FontFallbackPriority::Emoji | FontFallbackPriority::EmojiBySelector
    )
  }

  /// Blink's `FallbackFontForCharacter`.
  pub(crate) fn fallback_font_for_character(
    &self,
    hint: char,
    font_fallback_priority: FontFallbackPriority,
  ) -> Option<Rc<SimpleFontData>> {
    let with_glyph = |is_color: bool| {
      self
        .loaded_fonts()
        .find(|font| font.face.typeface.is_color == is_color && font.has_glyph(hint))
    };
    let preferred = Self::prefers_color(font_fallback_priority);

    match self.fallbacks.borrow_mut().entry((hint, preferred)) {
      Entry::Occupied(entry) => entry.get().clone(),
      Entry::Vacant(entry) => entry
        .insert(
          with_glyph(preferred)
            .or_else(|| with_glyph(!preferred))
            .cloned(),
        )
        .clone(),
    }
  }

  /// The pending face [`Self::fallback_font_for_character`] would pick for `hint` once it loads,
  /// none when a loaded face comes first.
  pub(crate) fn pending_fallback_font_for_character(
    &self,
    hint: char,
    font_fallback_priority: FontFallbackPriority,
  ) -> Option<&PendingFace> {
    let first_with_glyph = |is_color: bool| {
      self.fallback_fonts.iter().find(|font| match font {
        FaceData::Loaded(font) => font.face.typeface.is_color == is_color && font.has_glyph(hint),
        FaceData::Pending(face) => !is_color && face.range_set.contains(u32::from(hint)),
      })
    };
    let preferred = Self::prefers_color(font_fallback_priority);

    match first_with_glyph(preferred).or_else(|| first_with_glyph(!preferred))? {
      FaceData::Loaded(_) => None,
      FaceData::Pending(face) => Some(face),
    }
  }

  /// Blink's `GetLastResortFallbackFont`.
  pub(crate) fn last_resort_fallback_font(&self) -> Option<Rc<SimpleFontData>> {
    self.loaded_fonts().next().cloned()
  }
}

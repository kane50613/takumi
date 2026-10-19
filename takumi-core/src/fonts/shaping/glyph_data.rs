//! Blink's [`HarfBuzzRunGlyphData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/glyph_data.h).

use crate::{geometry::Point, layout_unit::TextRunLayoutUnit};

/// Whether a line may break before a glyph without shaping again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SafeToBreak {
  #[default]
  Safe,
  Unsafe,
}

/// A glyph of a [`ShapeResultRun`](super::shape_result_run::ShapeResultRun), with the character it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct HarfBuzzRunGlyphData {
  /// The glyph id.
  pub(crate) glyph: u16,
  /// The offset from the run's start of the character this glyph is for, always at a cluster boundary, in UTF-8 bytes where Blink counts UTF-16 code units.
  pub(crate) character_index: u16,
  unsafe_to_break_before: bool,
  pub(crate) advance: TextRunLayoutUnit,
}

impl HarfBuzzRunGlyphData {
  /// Blink's `kCharacterIndexBits`.
  pub(crate) const CHARACTER_INDEX_BITS: u32 = 15;
  /// Blink's `kMaxCharacters`.
  pub(crate) const MAX_CHARACTERS: u32 = 1 << Self::CHARACTER_INDEX_BITS;
  /// Blink's `kMaxCharacterIndex`.
  pub(crate) const MAX_CHARACTER_INDEX: u32 = Self::MAX_CHARACTERS - 1;
  /// Blink's `kMaxGlyphs`.
  pub(crate) const MAX_GLYPHS: u32 = Self::MAX_CHARACTERS;

  pub(crate) fn new(
    glyph: u16,
    character_index: u32,
    safe_to_break_before: SafeToBreak,
    advance: TextRunLayoutUnit,
  ) -> Self {
    debug_assert!(character_index <= Self::MAX_CHARACTER_INDEX);

    Self {
      glyph,
      character_index: character_index as u16,
      unsafe_to_break_before: safe_to_break_before == SafeToBreak::Unsafe,
      advance,
    }
  }

  /// Blink's `IsSafeToBreakBefore`.
  pub(crate) fn is_safe_to_break_before(self) -> bool {
    !self.unsafe_to_break_before
  }

  #[cfg(test)]
  /// Blink's `SetSafeToBreakBefore`.
  pub(crate) fn set_safe_to_break_before(&mut self, value: SafeToBreak) {
    self.unsafe_to_break_before = value == SafeToBreak::Unsafe;
  }
}

/// Blink's `GlyphOffset`.
pub(crate) type GlyphOffset = Point<f32>;

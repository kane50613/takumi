//! Blink's `HarfBuzzRunGlyphData` and `GlyphOffset`, after
//! [`glyph_data.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/glyph_data.h).
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use crate::{geometry::Point, layout_unit::TextRunLayoutUnit};

/// Whether a line may break before a glyph without shaping again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SafeToBreak {
  #[default]
  Safe,
  Unsafe,
}

/// A glyph of a [`ShapeResultRun`](super::shape_result_run::ShapeResultRun), with the character
/// it is for. Offsets of glyphs, often all zero, live apart in the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct HarfBuzzRunGlyphData {
  /// The glyph id.
  pub(crate) glyph: u16,
  /// The offset from the run's start of the character this glyph is for, always at a cluster
  /// boundary, in UTF-8 bytes where Blink counts UTF-16 code units.
  pub(crate) character_index: u16,
  unsafe_to_break_before: bool,
  pub(crate) advance: TextRunLayoutUnit,
}

impl HarfBuzzRunGlyphData {
  /// Blink's `kCharacterIndexBits`: the bits `character_index` packs into.
  pub(crate) const CHARACTER_INDEX_BITS: u32 = 15;
  /// Blink's `kMaxCharacters`: the most characters a run holds, limited by `character_index`.
  pub(crate) const MAX_CHARACTERS: u32 = 1 << Self::CHARACTER_INDEX_BITS;
  /// Blink's `kMaxCharacterIndex`.
  pub(crate) const MAX_CHARACTER_INDEX: u32 = Self::MAX_CHARACTERS - 1;
  /// Blink's `kMaxGlyphs`: the most glyphs a run holds, which keeps its buffer predictable.
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

  /// Blink's `SafeToBreakBefore`.
  pub(crate) fn safe_to_break_before(self) -> SafeToBreak {
    if self.unsafe_to_break_before {
      SafeToBreak::Unsafe
    } else {
      SafeToBreak::Safe
    }
  }

  /// Blink's `IsSafeToBreakBefore`.
  pub(crate) fn is_safe_to_break_before(self) -> bool {
    !self.unsafe_to_break_before
  }

  /// Blink's `SetSafeToBreakBefore`.
  pub(crate) fn set_safe_to_break_before(&mut self, value: SafeToBreak) {
    self.unsafe_to_break_before = value == SafeToBreak::Unsafe;
  }
}

/// Blink's `GlyphOffset`, a `gfx::Vector2dF`.
pub(crate) type GlyphOffset = Point<f32>;

//! Blink's `GlyphDataRange`, a run's glyphs in one range of glyph indices, after
//! [`glyph_data_range.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/glyph_data_range.cc).
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use super::{
  glyph_data::{GlyphOffset, HarfBuzzRunGlyphData},
  shape_result_run::ShapeResultRun,
};

/// `size` glyphs of a run from glyph `index` on, or no run at all.
#[derive(Clone, Copy, Default)]
pub(crate) struct GlyphDataRange<'a> {
  run: Option<&'a ShapeResultRun>,
  index: usize,
  size: usize,
}

impl<'a> GlyphDataRange<'a> {
  /// Every glyph of `run`.
  pub(crate) fn new(run: &'a ShapeResultRun) -> Self {
    Self {
      run: Some(run),
      index: 0,
      size: run.glyph_data.size(),
    }
  }

  pub(crate) fn is_empty(self) -> bool {
    self.size == 0
  }

  pub(crate) fn glyphs(self) -> &'a [HarfBuzzRunGlyphData] {
    self.run.map_or(&[], |run| {
      &run.glyph_data.glyphs()[self.index..self.index + self.size]
    })
  }

  pub(crate) fn has_offsets(self) -> bool {
    self
      .run
      .is_some_and(|run| run.glyph_data.has_non_zero_offsets())
  }

  /// The offsets of the glyphs when [`Self::has_offsets`], or none.
  pub(crate) fn offsets(self) -> &'a [GlyphOffset] {
    match self.run {
      Some(run) if self.has_offsets() => {
        &run.glyph_data.offsets()[self.index..self.index + self.size]
      }
      _ => &[],
    }
  }

  /// Blink's `FindGlyphDataRange`: the glyphs for the characters from `start_character_index` to
  /// `end_character_index`, in the order the run stores them.
  pub(crate) fn find_glyph_data_range(
    self,
    is_rtl: bool,
    start_character_index: u32,
    end_character_index: u32,
  ) -> Self {
    let Some(run) = self.run else {
      return Self::default();
    };

    if self.size == 0 {
      return self;
    }

    let glyphs = self.glyphs();
    let lower_bound = |glyphs: &[HarfBuzzRunGlyphData], index: u32| {
      glyphs.partition_point(|glyph| u32::from(glyph.character_index) < index)
    };

    if !is_rtl {
      let start_glyph = lower_bound(glyphs, start_character_index);

      if start_glyph == glyphs.len() {
        // No glyph matches; an empty range that still keeps the run.
        return Self {
          run: Some(run),
          index: self.index + self.size,
          size: 0,
        };
      }

      let end_glyph = start_glyph + lower_bound(&glyphs[start_glyph..], end_character_index);

      return Self {
        run: Some(run),
        index: self.index + start_glyph,
        size: end_glyph - start_glyph,
      };
    }

    // RTL searches from the end, as there may be several glyphs for a character and the first in
    // logical order is wanted.
    let reverse_lower_bound = |end: usize, index: u32| {
      end
        - glyphs[..end]
          .iter()
          .rev()
          .position(|glyph| u32::from(glyph.character_index) >= index)
          .unwrap_or(end)
    };
    let start_glyph_end = reverse_lower_bound(glyphs.len(), start_character_index);

    if start_glyph_end == 0 {
      // No glyph matches; an empty range that still keeps the run.
      return Self {
        run: Some(run),
        index: self.index,
        size: 0,
      };
    }

    let end_glyph_end = reverse_lower_bound(start_glyph_end, end_character_index);

    Self {
      run: Some(run),
      index: self.index + end_glyph_end,
      size: start_glyph_end - end_glyph_end,
    }
  }
}

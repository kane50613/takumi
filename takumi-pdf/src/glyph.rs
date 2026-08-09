//! The krilla [`Glyph`] implementation and its ToUnicode span mapping.

use std::ops::Range;

use takumi_core::layout::inline::ShapedRun;

use crate::krilla::{
  surface::Location,
  text::{Glyph, GlyphId},
};

/// Per-glyph byte ranges into `run_text` for ToUnicode, from the shaper's
/// cluster segmentation (correct for ligatures and complex scripts).
pub(crate) fn glyph_text_spans(shaped: &ShapedRun, run_text: &str) -> Vec<Range<usize>> {
  let base = shaped.text_range.start;

  if shaped.cluster_ranges.len() == shaped.glyphs.len() {
    return shaped
      .cluster_ranges
      .iter()
      .map(|range| {
        let start = range.start.saturating_sub(base).min(run_text.len());
        let end = range.end.saturating_sub(base).min(run_text.len());

        if start <= end { start..end } else { 0..0 }
      })
      .collect();
  }

  // Alignment unknown: map every glyph to the whole run.
  vec![0..run_text.len(); shaped.glyphs.len()]
}

/// A positioned glyph adapter. Offsets are stored em-normalized (position ÷ font
/// size): krilla calls the accessors with `size = 1.0` for text-space math and
/// with the real font size for cursor movement, so returning `stored × size`
/// satisfies both. Advances stay zero — glyphs carry absolute offsets instead.
pub(crate) struct PdfGlyph {
  pub(crate) id: GlyphId,
  pub(crate) x_offset: f32,
  pub(crate) y_offset: f32,
  pub(crate) range: Range<usize>,
}

impl Glyph for PdfGlyph {
  fn glyph_id(&self) -> GlyphId {
    self.id
  }

  fn text_range(&self) -> Range<usize> {
    self.range.clone()
  }

  fn x_advance(&self, _size: f32) -> f32 {
    0.0
  }

  fn x_offset(&self, size: f32) -> f32 {
    self.x_offset * size
  }

  fn y_offset(&self, size: f32) -> f32 {
    self.y_offset * size
  }

  fn y_advance(&self, _size: f32) -> f32 {
    0.0
  }

  fn location(&self) -> Option<Location> {
    None
  }
}

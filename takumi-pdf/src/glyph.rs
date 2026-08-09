//! The krilla [`Glyph`] implementation and its ToUnicode span mapping.

use std::ops::Range;

use takumi_core::layout::inline::ShapedRun;

use crate::{
  krilla::{
    surface::Location,
    text::{Glyph, GlyphId},
  },
  options::PdfError,
};

/// The run's glyphs, each carrying the source text it maps to.
///
/// A character no registered font covers shapes to `.notdef`. It draws nothing
/// and leaves nothing behind in the text layer, so the page would come out
/// looking finished with the character quietly gone. Every such character lands
/// in `uncovered` for the caller to report once the page is done.
pub(crate) fn run_glyphs(
  shaped: &ShapedRun,
  run_text: &str,
  uncovered: &mut String,
) -> Vec<PdfGlyph> {
  let clusters = cluster_spans(shaped, run_text);

  if let Some(clusters) = &clusters {
    collect_uncovered(shaped, run_text, clusters, uncovered);
  }

  let mut spans = clusters
    // Alignment unknown: map every glyph to the whole run.
    .unwrap_or_else(|| vec![0..run_text.len(); shaped.glyphs.len()]);

  merge_overlapping_spans(&mut spans);
  shaped
    .glyphs
    .iter()
    .zip(spans)
    .map(|(glyph, range)| PdfGlyph {
      id: GlyphId::new(glyph.id),
      x_offset: glyph.x / shaped.font_size,
      y_offset: -glyph.y / shaped.font_size,
      range,
    })
    .collect()
}

/// Adds the characters that came out as `.notdef`.
///
/// Reads the shaper's own cluster spans, before they are merged for ToUnicode:
/// a merged span covers its neighbour's text too, and the fallback span covers
/// the whole run, either of which would blame characters that shaped fine.
fn collect_uncovered(
  shaped: &ShapedRun,
  run_text: &str,
  clusters: &[Range<usize>],
  uncovered: &mut String,
) {
  for (glyph, cluster) in shaped.glyphs.iter().zip(clusters) {
    if glyph.id != 0 {
      continue;
    }
    for character in run_text.get(cluster.clone()).unwrap_or_default().chars() {
      if !uncovered.contains(character) {
        uncovered.push(character);
      }
    }
  }
}

/// The error naming what no font covered, or `None` when everything mapped.
pub(crate) fn uncovered_error(uncovered: &str) -> Option<PdfError> {
  if uncovered.is_empty() {
    return None;
  }
  let named = uncovered
    .chars()
    .map(|character| format!("{character} (U+{:04X})", character as u32))
    .collect::<Vec<_>>()
    .join(", ");

  Some(PdfError::MissingGlyphs(named))
}

/// Per-glyph byte ranges into `run_text`, from the shaper's cluster
/// segmentation (correct for ligatures and complex scripts). `None` when the
/// shaper's ranges do not line up with the glyphs, which leaves every glyph's
/// text unknown rather than wrong.
fn cluster_spans(shaped: &ShapedRun, run_text: &str) -> Option<Vec<Range<usize>>> {
  if shaped.cluster_ranges.len() != shaped.glyphs.len() {
    return None;
  }
  let base = shaped.text_range.start;

  Some(
    shaped
      .cluster_ranges
      .iter()
      .map(|range| {
        let start = range.start.saturating_sub(base).min(run_text.len());
        let end = range.end.saturating_sub(base).min(run_text.len());

        if start <= end { start..end } else { 0..0 }
      })
      .collect(),
  )
}

/// Gives every glyph whose text overlaps its neighbour's the union of the two.
///
/// A consonant and its matra come back as two clusters over the same source
/// text rather than a partition of it, so each glyph would map to the base
/// character again. An identical range instead marks them as one cluster, which
/// is what krilla needs to emit them under a single `/ActualText`.
fn merge_overlapping_spans(spans: &mut [Range<usize>]) {
  let mut group = 0;

  for index in 1..spans.len() {
    if !spans_overlap(&spans[group], &spans[index]) {
      group = index;
      continue;
    }
    let union = spans[group].start.min(spans[index].start)..spans[group].end.max(spans[index].end);

    for span in &mut spans[group..=index] {
      *span = union.clone();
    }
  }
}

fn spans_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
  left.start < right.end && right.start < left.end
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn overlapping_spans_share_one_range() {
    // A consonant and its matra, twice, around a space.
    let mut spans = vec![0..3, 0..6, 6..9, 6..12, 12..13];

    merge_overlapping_spans(&mut spans);
    assert_eq!(spans, vec![0..6, 0..6, 6..12, 6..12, 12..13]);
  }

  #[test]
  fn disjoint_spans_stay_apart() {
    let ascending = vec![0..1, 1..2, 2..3];
    let descending = vec![4..6, 2..4, 0..2];

    for original in [ascending, descending] {
      let mut spans = original.clone();

      merge_overlapping_spans(&mut spans);
      assert_eq!(spans, original);
    }
  }
}

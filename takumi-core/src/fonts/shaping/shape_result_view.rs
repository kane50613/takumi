//! Blink's [`ShapeResultView`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/shape_result_view.cc).

use std::rc::Rc;

use icu_segmenter::GraphemeClusterSegmenter;

use super::{
  glyph_data::{GlyphOffset, HarfBuzzRunGlyphData},
  glyph_data_range::GlyphDataRange,
  shape_result::ShapeResult,
  shape_result_run::ShapeResultRun,
};
use crate::{
  fonts::simple_font_data::SimpleFontData,
  layout_unit::{InlineLayoutUnit, LayoutUnit},
  style::Direction as TextDirection,
};

/// Blink's `Segment`.
#[derive(Clone, Copy)]
pub(crate) struct Segment<'a> {
  pub(crate) source: SegmentSource<'a>,
  pub(crate) start_index: u32,
  pub(crate) end_index: u32,
}

/// What a [`Segment`] reads.
#[derive(Clone, Copy)]
pub(crate) enum SegmentSource<'a> {
  Result(&'a Rc<ShapeResult>),
  View(&'a ShapeResultView),
}

/// Blink's `RunInfoPart`.
#[derive(Debug, Clone)]
pub(crate) struct RunInfoPart {
  /// The result holding the run, and the run's index in it; none for a part without a run.
  run: Option<(Rc<ShapeResult>, usize)>,
  glyph_index: usize,
  num_glyphs: usize,
  /// Start index for a partial run, adjusted so runs are continuous.
  start_index: u32,
  /// Offset from the start index of the original run.
  offset: u32,
  num_characters: u32,
  width: f32,
}

impl RunInfoPart {
  /// The whole of run `run_index` of `result`.
  fn whole_run(result: &Rc<ShapeResult>, run_index: usize) -> Self {
    let run = &result.runs[run_index];

    Self {
      run: Some((result.clone(), run_index)),
      glyph_index: 0,
      num_glyphs: run.glyph_data.size(),
      start_index: run.start_index,
      offset: 0,
      num_characters: run.num_characters,
      width: run.width,
    }
  }

  /// Blink's `GetRunInfo`.
  pub(crate) fn run_info(&self) -> Option<&ShapeResultRun> {
    self
      .run
      .as_ref()
      .map(|(result, run_index)| &result.runs[*run_index])
  }

  /// Blink's `GetGlyphDataRange`.
  pub(crate) fn glyph_data_range(&self) -> GlyphDataRange<'_> {
    GlyphDataRange::with_range(self.run_info(), self.glyph_index, self.num_glyphs)
  }

  /// Blink's `HasGlyphOffsets`.
  pub(crate) fn has_glyph_offsets(&self) -> bool {
    self.glyph_data_range().has_offsets()
  }

  /// The offset of each glyph, zero for a part without offsets.
  fn glyph_offsets(&self) -> impl Iterator<Item = GlyphOffset> + '_ {
    let offsets = self.glyph_data_range().offsets();

    (0..self.num_glyphs).map(move |i| offsets.get(i).copied().unwrap_or_default())
  }

  /// Blink's `CharacterIndexOfEndGlyph`: the end character index, ignoring the view's offsets.
  pub(crate) fn character_index_of_end_glyph(&self) -> u32 {
    self.num_characters + self.offset
  }

  pub(crate) fn num_characters(&self) -> u32 {
    self.num_characters
  }

  pub(crate) fn num_glyphs(&self) -> usize {
    self.num_glyphs
  }

  pub(crate) fn width(&self) -> f32 {
    self.width
  }

  /// Blink's `OffsetToRunStartIndex`.
  pub(crate) fn offset_to_run_start_index(&self) -> u32 {
    self.offset
  }

  /// Blink's `PreviousSafeToBreakOffset`.
  pub(crate) fn previous_safe_to_break_offset(&self, offset: u32) -> u32 {
    if offset >= self.num_characters {
      return self.num_characters;
    }

    let offset = offset + self.offset;
    let glyphs = self.glyph_data_range().glyphs();
    let is_safe = |glyph: &&HarfBuzzRunGlyphData| {
      glyph.is_safe_to_break_before() && u32::from(glyph.character_index) <= offset
    };
    let found = if self.run_info().is_some_and(ShapeResultRun::is_ltr) {
      glyphs.iter().rev().find(is_safe)
    } else {
      glyphs.iter().find(is_safe)
    };

    // Previous safe break is at the start of the run.
    found.map_or(0, |glyph| u32::from(glyph.character_index) - self.offset)
  }

  /// Blink's `FindGlyphDataRange`.
  fn find_glyph_data_range(&self, start_character_index: u32, end_character_index: u32) -> Self {
    let is_rtl = self.run_info().is_some_and(ShapeResultRun::is_rtl);
    let range = self.glyph_data_range().find_glyph_data_range(
      is_rtl,
      start_character_index,
      end_character_index,
    );
    let mut part = self.clone();

    if range.run().is_none() {
      part.run = None;
    }

    part.glyph_index = range.index();
    part.num_glyphs = range.size();
    part
  }

  /// Blink's `ComputeStart`, with `offset_for_run` the source's `StartIndexOffsetForRun`.
  fn compute_start(&self, offset_for_run: u32, is_ltr: bool) -> u32 {
    let part_start = self.start_index + offset_for_run;

    if is_ltr {
      return part_start;
    }

    // In RTL with several parts, a part's offset may pass its start index.
    part_start.max(self.offset)
  }

  /// Blink's `ComputeStartEnd`.
  fn compute_start_end(
    &self,
    offset_for_run: u32,
    is_ltr: bool,
    segment: &Segment<'_>,
  ) -> Option<(u32, u32)> {
    self.run_info()?;

    let part_start = self.compute_start(offset_for_run, is_ltr);

    if segment.end_index <= part_start {
      return None;
    }

    if self.num_characters == 0 {
      return Some((part_start, part_start));
    }

    let part_end = part_start + self.num_characters;

    if segment.start_index >= part_end {
      return None;
    }

    Some((part_start, part_end))
  }
}

/// A result or a view as `PopulateRunInfoParts` reads it.
struct PartSource {
  parts: Vec<RunInfoPart>,
  start_index_offset_for_run: u32,
  start_index: u32,
  end_index: u32,
  direction: TextDirection,
  has_vertical_offsets: bool,
}

impl PartSource {
  fn of(source: SegmentSource<'_>) -> Self {
    match source {
      SegmentSource::Result(result) => Self {
        parts: (0..result.runs.len())
          .map(|run_index| RunInfoPart::whole_run(result, run_index))
          .collect(),
        start_index_offset_for_run: 0,
        start_index: result.start_index(),
        end_index: result.end_index(),
        direction: result.direction(),
        has_vertical_offsets: result.has_vertical_offsets(),
      },
      SegmentSource::View(view) => Self {
        parts: view.parts.clone(),
        start_index_offset_for_run: view.char_index_offset,
        start_index: view.start_index(),
        end_index: view.end_index(),
        direction: view.direction,
        has_vertical_offsets: view.has_vertical_offsets,
      },
    }
  }
}

/// Blink's `ShapeResultView`.
#[derive(Debug, Clone)]
pub(crate) struct ShapeResultView {
  parts: Vec<RunInfoPart>,
  start_index: u32,
  width: f32,
  num_characters: u32,
  direction: TextDirection,
  has_vertical_offsets: bool,
  /// The start offset of the first segment, as `ShapeResult::sub_range` keeps it.
  char_index_offset: u32,
}

impl ShapeResultView {
  /// Blink's `Create` from segments in logical order.
  pub(crate) fn create(segments: &[Segment<'_>]) -> Self {
    debug_assert!(!segments.is_empty());

    let sources: Vec<PartSource> = segments
      .iter()
      .map(|segment| PartSource::of(segment.source))
      .collect();
    let first = &sources[0];
    let direction = first.direction;
    let mut start_index = 0;
    let mut char_index_offset = 0;

    if direction == TextDirection::Ltr {
      char_index_offset = first.start_index.max(segments[0].start_index);
    } else {
      start_index = first.start_index.max(segments[0].start_index);
    }

    let has_vertical_offsets = sources.iter().any(|source| source.has_vertical_offsets);
    let mut view = Self {
      parts: Vec::new(),
      start_index,
      width: 0.0,
      num_characters: 0,
      direction,
      has_vertical_offsets,
      char_index_offset,
    };

    // Segments are logical, parts visual: RTL takes the segments back to front.
    if view.is_ltr() {
      for (segment, source) in segments.iter().zip(&sources) {
        view.populate_run_info_parts(source, segment);
      }
    } else {
      for (segment, source) in segments.iter().zip(&sources).rev() {
        view.populate_run_info_parts(source, segment);
      }
    }

    view
  }

  /// Blink's `Create` of a whole result.
  pub(crate) fn create_from_result(result: &Rc<ShapeResult>) -> Self {
    Self::create(&[Segment {
      source: SegmentSource::Result(result),
      start_index: 0,
      end_index: u32::MAX,
    }])
  }

  /// Blink's `Create` of a range of a result.
  pub(crate) fn create_from_result_range(
    result: &Rc<ShapeResult>,
    start_index: u32,
    end_index: u32,
  ) -> Self {
    Self::create(&[Segment {
      source: SegmentSource::Result(result),
      start_index,
      end_index,
    }])
  }

  /// Blink's `Create` of a range of a view.
  pub(crate) fn create_from_view_range(view: &Self, start_index: u32, end_index: u32) -> Self {
    Self::create(&[Segment {
      source: SegmentSource::View(view),
      start_index,
      end_index,
    }])
  }

  /// Blink's `PopulateRunInfoParts`.
  fn populate_run_info_parts(&mut self, source: &PartSource, segment: &Segment<'_>) {
    let source_start = segment.start_index.max(source.start_index);
    let index_diff = i64::from(self.start_index + self.num_characters) - i64::from(source_start);

    self.num_characters += segment.end_index.min(source.end_index) - source_start;

    let is_ltr = source.direction == TextDirection::Ltr;

    for run in &source.parts {
      let Some((part_start, part_end)) =
        run.compute_start_end(source.start_index_offset_for_run, is_ltr, segment)
      else {
        continue;
      };

      debug_assert!(part_start >= run.offset_to_run_start_index());

      let run_start = part_start - run.offset_to_run_start_index();
      let range_start = if segment.start_index > run_start {
        segment.start_index.max(part_start) - run_start
      } else {
        0
      };
      let range_end = segment.end_index.min(part_end) - run_start;

      debug_assert!(range_end > range_start);

      let part_characters = range_end - range_start;

      // A run the segment covers whole needs no search.
      let mut part = if part_start >= segment.start_index && part_end <= segment.end_index {
        run.clone()
      } else {
        let mut part = run.find_glyph_data_range(range_start, range_end);
        let width = part
          .glyph_data_range()
          .glyphs()
          .iter()
          .fold(InlineLayoutUnit::ZERO, |sum, glyph| {
            sum + glyph.advance.into()
          });

        part.width = width.to_float();
        part
      };

      self.width += part.width;
      // Start indices continue from the parts before.
      part.start_index = (i64::from(run_start + range_start) + index_diff) as u32;
      part.offset = range_start;
      part.num_characters = part_characters;
      self.parts.push(part);
    }
  }

  /// Blink's `CreateShapeResult`.
  pub(crate) fn create_shape_result(&self) -> ShapeResult {
    let runs = self
      .parts
      .iter()
      .filter_map(|part| {
        let part_run = part.run_info()?;
        let mut new_run = ShapeResultRun::new(
          part_run.font_data.clone(),
          part_run.direction,
          part_run.script,
          part.start_index + self.char_index_offset,
          0,
          part.num_characters,
        );

        new_run
          .glyph_data
          .copy_from_range(part.glyph_data_range(), part.offset);
        new_run.width = part.width;

        Some(new_run)
      })
      .collect();

    ShapeResult::from_runs(
      self.start_index(),
      self.num_characters,
      self.direction,
      runs,
      self.width,
      self.has_vertical_offsets,
    )
  }

  pub(crate) fn start_index(&self) -> u32 {
    self.start_index + self.char_index_offset
  }

  pub(crate) fn end_index(&self) -> u32 {
    self.start_index() + self.num_characters
  }

  pub(crate) fn num_characters(&self) -> u32 {
    self.num_characters
  }

  pub(crate) fn width(&self) -> f32 {
    self.width
  }

  /// Blink's `SnappedWidth`.
  pub(crate) fn snapped_width(&self) -> LayoutUnit {
    LayoutUnit::from_float_ceil(self.width)
  }

  pub(crate) fn direction(&self) -> TextDirection {
    self.direction
  }

  pub(crate) fn is_ltr(&self) -> bool {
    self.direction == TextDirection::Ltr
  }

  pub(crate) fn is_rtl(&self) -> bool {
    self.direction == TextDirection::Rtl
  }

  pub(crate) fn has_vertical_offsets(&self) -> bool {
    self.has_vertical_offsets
  }

  pub(crate) fn parts(&self) -> &[RunInfoPart] {
    &self.parts
  }

  /// Blink's `NumGlyphs`.
  pub(crate) fn num_glyphs(&self) -> usize {
    self.parts.iter().map(RunInfoPart::num_glyphs).sum()
  }

  /// Blink's `UsedFonts`: each font once.
  pub(crate) fn used_fonts(&self) -> Vec<Rc<SimpleFontData>> {
    let mut used_fonts: Vec<Rc<SimpleFontData>> = Vec::new();

    for font_data in self
      .parts
      .iter()
      .filter_map(|part| part.run_info()?.font_data.clone())
    {
      if !used_fonts.iter().any(|used| Rc::ptr_eq(used, &font_data)) {
        used_fonts.push(font_data);
      }
    }

    used_fonts
  }

  /// Blink's `PreviousSafeToBreakOffset`.
  pub(crate) fn previous_safe_to_break_offset(&self, index: u32) -> u32 {
    for (position, part) in self.parts.iter().enumerate().rev() {
      let run_start = part.start_index + self.char_index_offset;

      if index >= run_start {
        let offset = index - run_start;

        if offset <= part.num_characters {
          return part.previous_safe_to_break_offset(offset) + run_start;
        }

        if self.is_ltr() {
          return run_start + part.num_characters;
        }
      } else if self.is_rtl() {
        if position == self.parts.len() - 1 {
          return part.start_index;
        }

        let previous_run = &self.parts[position + 1];

        return previous_run.start_index + previous_run.num_characters;
      }
    }

    self.start_index()
  }

  /// The offset `HarfBuzzRunGlyphData::character_index` takes to index the source string. It may
  /// wrap below zero, as Blink's unsigned one does, until a glyph's index is added.
  fn character_index_offset_for_glyph_data(&self, part: &RunInfoPart) -> u32 {
    (part.start_index + self.char_index_offset).wrapping_sub(part.offset)
  }

  /// Blink's `ForEachGlyph`.
  pub(crate) fn for_each_glyph(
    &self,
    initial_advance: f32,
    mut glyph_callback: impl FnMut(u32, u16, GlyphOffset, f32, Option<&Rc<SimpleFontData>>),
  ) -> f32 {
    let mut total_advance = initial_advance;

    for part in &self.parts {
      let Some(run) = part.run_info() else {
        continue;
      };
      let mut advance = InlineLayoutUnit::from_float_round(total_advance);
      let character_index_offset = self.character_index_offset_for_glyph_data(part);

      for (glyph_data, offset) in part
        .glyph_data_range()
        .glyphs()
        .iter()
        .zip(part.glyph_offsets())
      {
        glyph_callback(
          u32::from(glyph_data.character_index).wrapping_add(character_index_offset),
          glyph_data.glyph,
          offset,
          advance.to_float(),
          run.font_data.as_ref(),
        );
        advance += glyph_data.advance.into();
      }

      total_advance = advance.to_float();
    }

    total_advance
  }

  /// Blink's `ForEachGlyph` over the characters from `from` to `to`.
  pub(crate) fn for_each_glyph_in_range(
    &self,
    initial_advance: f32,
    from: u32,
    to: u32,
    mut glyph_callback: impl FnMut(u32, u16, GlyphOffset, f32, Option<&Rc<SimpleFontData>>),
  ) -> f32 {
    let mut total_advance = initial_advance;

    for part in &self.parts {
      let Some(run) = part.run_info() else {
        continue;
      };
      let mut advance = InlineLayoutUnit::from_float_round(total_advance);
      let character_index_offset = self.character_index_offset_for_glyph_data(part);

      for (glyph_data, offset) in part
        .glyph_data_range()
        .glyphs()
        .iter()
        .zip(part.glyph_offsets())
      {
        let character_index =
          u32::from(glyph_data.character_index).wrapping_add(character_index_offset);

        // LTR glyphs ascend in character index, RTL ones descend.
        if run.is_ltr() {
          if character_index >= to {
            break;
          }
        } else if character_index < from {
          break;
        }

        if character_index >= from && character_index < to {
          glyph_callback(
            character_index,
            glyph_data.glyph,
            offset,
            advance.to_float(),
            run.font_data.as_ref(),
          );
        }

        advance += glyph_data.advance.into();
      }

      total_advance = advance.to_float();
    }

    total_advance
  }

  /// Blink's `ForEachGraphemeClusters`, counting graphemes with ICU4X.
  pub(crate) fn for_each_grapheme_clusters(
    &self,
    text: &str,
    initial_advance: f32,
    from: u32,
    to: u32,
    index_offset: u32,
    mut callback: impl FnMut(u32, f32, u32, f32),
  ) -> f32 {
    let segmenter = GraphemeClusterSegmenter::new();
    let num_grapheme_clusters =
      |text: &str| segmenter.segment_str(text).count().saturating_sub(1) as u32;
    // Blink keeps text that fits Latin-1 in 8 bits, where every character is a grapheme.
    let is_8bit = text.chars().all(|ch| u32::from(ch) <= 0xFF);
    let rtl = self.is_rtl();
    let mut advance_so_far = initial_advance;

    for part in &self.parts {
      if part.num_glyphs == 0 {
        continue;
      }

      let glyphs = part.glyph_data_range().glyphs();
      let mut cluster_advance = 0.0;
      let character_index_offset = self
        .character_index_offset_for_glyph_data(part)
        .wrapping_add(index_offset);
      let mut cluster_start = if rtl {
        part
          .character_index_of_end_glyph()
          .wrapping_add(character_index_offset)
      } else {
        u32::from(glyphs[0].character_index).wrapping_add(character_index_offset)
      };

      for (i, glyph_data) in glyphs.iter().enumerate() {
        let current_character_index =
          u32::from(glyph_data.character_index).wrapping_add(character_index_offset);

        assert!((current_character_index as usize) < text.len());

        if current_character_index < from || current_character_index >= to {
          advance_so_far += glyph_data.advance.to_float();

          cluster_start = if rtl {
            cluster_start.wrapping_sub(1)
          } else {
            cluster_start.wrapping_add(1)
          };

          continue;
        }

        let is_run_end = i + 1 == glyphs.len();
        let is_cluster_end = is_run_end
          || u32::from(glyphs[i + 1].character_index).wrapping_add(character_index_offset)
            != current_character_index;

        cluster_advance += glyph_data.advance.to_float();

        if is_8bit {
          callback(
            current_character_index,
            advance_so_far,
            1,
            glyph_data.advance.to_float(),
          );
          advance_so_far += glyph_data.advance.to_float();
        } else if is_cluster_end {
          let cluster_end = if rtl {
            current_character_index
          } else if is_run_end {
            part
              .character_index_of_end_glyph()
              .wrapping_add(character_index_offset)
          } else {
            u32::from(glyphs[i + 1].character_index).wrapping_add(character_index_offset)
          };
          let (start, end) = if cluster_end >= cluster_start {
            (cluster_start, cluster_end)
          } else {
            (cluster_end, cluster_start)
          };
          let graphemes_in_cluster = num_grapheme_clusters(&text[start as usize..end as usize]);

          if graphemes_in_cluster == 0 || cluster_advance == 0.0 {
            continue;
          }

          callback(
            current_character_index,
            advance_so_far,
            graphemes_in_cluster,
            cluster_advance,
          );
          advance_so_far += cluster_advance;
          cluster_start = cluster_end;
          cluster_advance = 0.0;
        }
      }
    }

    advance_so_far
  }

  /// Blink's `ExpandRangeToIncludePartialGlyphs`.
  pub(crate) fn expand_range_to_include_partial_glyphs(&self, from: &mut i32, to: &mut i32) {
    for part in &self.parts {
      let Some(run) = part.run_info() else {
        continue;
      };
      let part_offset = (self.char_index_offset + part.start_index) as i32 - part.offset as i32;

      run.expand_range_to_include_partial_glyphs(part_offset, from, to);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    fonts::{
      font::Font,
      shaping::{
        harfbuzz_shaper::HarfBuzzShaper,
        testing::{GEIST, NOTO_SANS_TC, font, geist, register},
      },
    },
    resources::font::Fonts,
  };

  /// Blink's `ShapeResultTestGlyphInfo`: a glyph's character index, id, and position.
  type GlyphInfo = (u32, u16, f32);

  fn shape(font: &Font, text: &str) -> Rc<ShapeResult> {
    Rc::new(HarfBuzzShaper::new(text.into()).shape_all(font, TextDirection::Ltr))
  }

  /// Blink's `AddGlyphInfo` over every glyph.
  fn glyphs(view: &ShapeResultView) -> Vec<GlyphInfo> {
    let mut glyphs = Vec::new();

    view.for_each_glyph(0.0, |character_index, glyph, _, advance, _| {
      glyphs.push((character_index, glyph, advance));
    });

    glyphs
  }

  /// Blink's `CompareResultGlyphs`: `test` against `reference` from `reference_start`, positions
  /// relative to it.
  fn assert_glyphs_match(test: &[GlyphInfo], reference: &[GlyphInfo], reference_start: usize) {
    let advance_offset = reference[reference_start].2;
    let expected = &reference[reference_start..reference_start + test.len()];

    for (&(index, glyph, advance), &(expected_index, expected_glyph, expected_advance)) in
      test.iter().zip(expected)
    {
      assert_eq!((index, glyph), (expected_index, expected_glyph));
      // Positions differ in the last bits once the reference's start is taken off.
      assert!((advance - (expected_advance - advance_offset)).abs() < 1e-3);
    }
  }

  fn segment(result: &Rc<ShapeResult>, start_index: u32, end_index: u32) -> Segment<'_> {
    Segment {
      source: SegmentSource::Result(result),
      start_index,
      end_index,
    }
  }

  #[test]
  fn latin_single_view() {
    let (_fonts, font) = geist();
    let result = shape(
      &font,
      "Test run with multiple words and breaking opportunities.",
    );
    let all = glyphs(&ShapeResultView::create_from_result(&result));

    // "Test run with multiple", "multiple words and breaking", "breaking opportunities."
    for (start, end) in [(0, 22), (14, 41), (33, 56)] {
      let view = ShapeResultView::create(&[segment(&result, start, end)]);
      let view_glyphs = glyphs(&view);

      assert_eq!(view.start_index(), start);
      assert_eq!(view.num_characters(), end - start);
      assert_eq!(view.num_glyphs(), (end - start) as usize);
      assert_glyphs_match(&view_glyphs, &all, start as usize);
    }
  }

  #[test]
  fn latin_multi_run() {
    let (_fonts, font) = geist();
    let mut result = ShapeResult::new(0, 0, TextDirection::Ltr);

    // "hello world!" in four runs.
    for text in ["hello", " w", "orld", "!"] {
      shape(&font, text).copy_range(0, text.len() as u32, &mut result);
    }

    let result = Rc::new(result);
    // "hello wood wold!"
    let ranges = [(0, 8), (7, 8), (10, 11), (5, 8), (9, 12)];
    let composite_view =
      ShapeResultView::create(&ranges.map(|(start, end)| segment(&result, start, end)));
    let view_glyphs = glyphs(&composite_view);

    assert_eq!(composite_view.start_index(), 0);
    assert_eq!(composite_view.num_characters(), 16);
    assert_eq!(composite_view.num_glyphs(), 16);

    let mut composite_copy = ShapeResult::new(0, 0, TextDirection::Ltr);

    for (start, end) in ranges {
      result.copy_range(start, end, &mut composite_copy);
    }

    let composite_copy = Rc::new(composite_copy);
    let copy_glyphs = glyphs(&ShapeResultView::create_from_result(&composite_copy));

    assert_glyphs_match(&view_glyphs, &copy_glyphs, 0);
    assert_eq!(composite_view.width(), composite_copy.width());
  }

  #[test]
  fn latin_composite_view() {
    let (_fonts, font) = geist();
    let result = shape(
      &font,
      "Test run with multiple words and breaking opportunities.",
    );
    let reference_result = shape(&font, "multiple breaking opportunities Test");
    // "multiple ", "breaking opportunities", " ", "Test"
    let ranges = [(14, 23), (33, 55), (4, 5), (0, 4)];
    let mut composite_copy = ShapeResult::new(0, 0, TextDirection::Ltr);

    for (start, end) in ranges {
      result.copy_range(start, end, &mut composite_copy);
    }

    assert_eq!(
      composite_copy.num_characters(),
      reference_result.num_characters()
    );
    assert_eq!(composite_copy.num_glyphs(), reference_result.num_glyphs());

    let composite_copy = Rc::new(composite_copy);
    let composite_view =
      ShapeResultView::create(&ranges.map(|(start, end)| segment(&result, start, end)));
    let composite_glyphs = glyphs(&composite_view);

    assert_eq!(composite_view.start_index(), composite_copy.start_index());
    assert_eq!(
      composite_view.num_characters(),
      reference_result.num_characters()
    );
    assert_eq!(
      composite_view.num_glyphs() as u32,
      reference_result.num_glyphs()
    );
    assert_eq!(composite_glyphs.len(), 36);
    assert_glyphs_match(
      &composite_glyphs[..22],
      &glyphs(&ShapeResultView::create_from_result(&composite_copy)),
      0,
    );
    assert_eq!(composite_view.width(), composite_copy.width());
  }

  #[test]
  fn mixed_scripts_composite_view() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");
    register(&mut fonts, NOTO_SANS_TC, "Noto Sans TC");

    let font = font(&mut fonts, "Geist, 'Noto Sans TC'");
    let string_a = "Test with multiple 字体 ";
    let string_b = "and 本書.";
    let result_a = shape(&font, string_a);
    let result_b = shape(&font, string_b);
    let reference_result = shape(&font, &format!("{string_a}{string_b}"));
    let mut composite_copy = ShapeResult::new(0, 0, TextDirection::Ltr);

    result_a.copy_range(0, string_a.len() as u32, &mut composite_copy);
    result_b.copy_range(0, string_b.len() as u32, &mut composite_copy);

    assert_eq!(
      composite_copy.num_characters(),
      reference_result.num_characters()
    );
    assert_eq!(composite_copy.num_glyphs(), reference_result.num_glyphs());

    let composite_copy = Rc::new(composite_copy);
    let composite_view = ShapeResultView::create(&[
      segment(&result_a, 0, string_a.len() as u32),
      segment(&result_b, 0, string_b.len() as u32),
    ]);

    assert_eq!(composite_view.start_index(), 0);
    assert_eq!(
      composite_view.num_characters(),
      reference_result.num_characters()
    );
    assert_eq!(
      composite_view.num_glyphs() as u32,
      reference_result.num_glyphs()
    );
    assert_glyphs_match(
      &glyphs(&composite_view),
      &glyphs(&ShapeResultView::create_from_result(&composite_copy)),
      0,
    );
    assert_eq!(composite_view.width(), composite_copy.width());
  }

  #[test]
  fn trim_end_of_view() {
    let (_fonts, font) = geist();
    let result = shape(&font, "12345678901234567890");
    let view1 = ShapeResultView::create_from_result_range(&result, 5, 20);

    assert_eq!((view1.num_characters(), view1.num_glyphs()), (15, 15));

    let view2 = ShapeResultView::create_from_view_range(&view1, 5, 19);

    assert_eq!((view2.num_characters(), view2.num_glyphs()), (14, 14));
  }

  #[test]
  fn previous_safe_offset_inside_view() {
    let (_fonts, font) = geist();
    let result = shape(&font, "Blah bla test something. ");

    // Once 14 - 9 = 5, before the start of the view.
    let view1 = ShapeResultView::create_from_result_range(&result, 9, 14);

    assert_eq!(view1.previous_safe_to_break_offset(14), 14);

    // Once 25 - 9 = 16, inside the view but not its last safe offset.
    let view2 = ShapeResultView::create_from_result_range(&result, 9, 25);

    assert_eq!(view2.previous_safe_to_break_offset(24), 24);
  }

  #[test]
  fn create_shape_result_matches_the_view() {
    let (_fonts, font) = geist();
    let result = shape(&font, "Test run with multiple words.");
    let view = ShapeResultView::create_from_result_range(&result, 5, 22);
    let copy = Rc::new(view.create_shape_result());

    assert_eq!(copy.start_index(), view.start_index());
    assert_eq!(copy.num_characters(), view.num_characters());
    assert_eq!(copy.width(), view.width());
    assert_eq!(
      glyphs(&ShapeResultView::create_from_result(&copy)),
      glyphs(&view)
    );
  }
}

//! Blink's [`ShapeResultView`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/shape_result_view.cc).

use std::rc::Rc;

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

  /// The offset of each glyph, zero for a part without offsets.
  fn glyph_offsets(&self) -> impl Iterator<Item = GlyphOffset> + '_ {
    let offsets = self.glyph_data_range().offsets();

    (0..self.num_glyphs).map(move |i| offsets.get(i).copied().unwrap_or_default())
  }

  pub(crate) fn num_glyphs(&self) -> usize {
    self.num_glyphs
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

  /// Blink's `ComputeStart`.
  fn compute_start(&self, offset_for_run: u32, is_ltr: bool) -> u32 {
    let part_start = self.start_index + offset_for_run;

    if is_ltr {
      return part_start;
    }

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
        start_index: result.start_index,
        end_index: result.end_index(),
        direction: result.direction,
        has_vertical_offsets: result.has_vertical_offsets,
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
  pub(crate) width: f32,
  pub(crate) num_characters: u32,
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

  /// Blink's `SnappedWidth`.
  pub(crate) fn snapped_width(&self) -> LayoutUnit {
    LayoutUnit::from_float_ceil(self.width)
  }

  pub(crate) fn is_ltr(&self) -> bool {
    self.direction == TextDirection::Ltr
  }

  pub(crate) fn is_rtl(&self) -> bool {
    self.direction == TextDirection::Rtl
  }

  /// Blink's `NumGlyphs`.
  pub(crate) fn num_glyphs(&self) -> usize {
    self.parts.iter().map(RunInfoPart::num_glyphs).sum()
  }

  /// Blink's `UsedFonts`.
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

  const SCHEHERAZADE: &str = "../assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2";

  /// Blink's `PreviousSafeToBreak` text: Hebrew with points, between Latin punctuation.
  const HEBREW: &str = concat!(
    "(\u{5D1}) \u{5D4}\u{5D1}\u{5DC} \u{5D0}\u{5DE}\u{5E8} \u{5E2}\u{5DC} \u{5D3}",
    "\u{5D1}\u{5E8}\u{5D9} \u{5D4}\u{5D1}\u{5DC}\u{5D9} \u{5D4}\u{5E2}\u{5D5}",
    "\u{5DC}\u{5DD}, \u{5D5}\u{5E1}\u{5DE}\u{5DA} \u{5D4}\u{5B2}\u{5D1}\u{5B5}",
    "\u{5DC} \u{5D0}\u{5DC} \u{5D4}\u{5D1}\u{5DC}\u{5D9}\u{5DD} \u{5D5}\u{5D0}",
    "\u{5DD} \u{5DC}\u{5D0} \u{5D9}\u{5DE}\u{5E6}\u{5D0} \u{5DE}\u{5D4}\u{5E9}",
    "\u{5DE}\u{5D5}\u{5EA} \u{5E9}\u{5D4}\u{5DD} \u{5E2}\u{5DC} \u{5DE}\u{5E9}",
    "\u{5E7}\u{5DC} \u{5D0}\u{5E8}\u{5E5} \u{5E9}\u{5D9}\u{5E9}\u{5EA}\u{5E0}",
    "\u{5D4} \u{5D7}\u{5D5}\u{5E5} \u{5DE}\u{5B5}\u{5D7}\u{5B2}\u{5D3}\u{5B7}",
    "\u{5E8} \u{5DE}\u{5B4}\u{5E9}\u{5B0}\u{5C1}\u{5DB}\u{5B8}\u{5D1}\u{5B0}\u{5DA}",
    "\u{5B8} (\u{5E9}\u{5DE}\u{5D5}\u{5EA} \u{5D6}\u{5F3}:\u{5DB}\u{5F4}\u{5D7})"
  );

  /// Blink's `ShapeResultTestGlyphInfo`.
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

  /// Blink's `CompareResultGlyphs`.
  fn assert_glyphs_match(test: &[GlyphInfo], reference: &[GlyphInfo], reference_start: usize) {
    let advance_offset = reference[reference_start].2;
    let expected = &reference[reference_start..reference_start + test.len()];

    for (&(index, glyph, advance), &(expected_index, expected_glyph, expected_advance)) in
      test.iter().zip(expected)
    {
      assert_eq!((index, glyph), (expected_index, expected_glyph));
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

    for (start, end) in [(0, 22), (14, 41), (33, 56)] {
      let view = ShapeResultView::create(&[segment(&result, start, end)]);
      let view_glyphs = glyphs(&view);

      assert_eq!(view.start_index(), start);
      assert_eq!(view.num_characters, end - start);
      assert_eq!(view.num_glyphs(), (end - start) as usize);
      assert_glyphs_match(&view_glyphs, &all, start as usize);
    }
  }

  #[test]
  fn latin_multi_run() {
    let (_fonts, font) = geist();
    let mut result = ShapeResult::new(0, 0, TextDirection::Ltr);

    for text in ["hello", " w", "orld", "!"] {
      shape(&font, text).copy_range(0, text.len() as u32, &mut result);
    }

    let result = Rc::new(result);
    let ranges = [(0, 8), (7, 8), (10, 11), (5, 8), (9, 12)];
    let composite_view =
      ShapeResultView::create(&ranges.map(|(start, end)| segment(&result, start, end)));
    let view_glyphs = glyphs(&composite_view);

    assert_eq!(composite_view.start_index(), 0);
    assert_eq!(composite_view.num_characters, 16);
    assert_eq!(composite_view.num_glyphs(), 16);

    let mut composite_copy = ShapeResult::new(0, 0, TextDirection::Ltr);

    for (start, end) in ranges {
      result.copy_range(start, end, &mut composite_copy);
    }

    let composite_copy = Rc::new(composite_copy);
    let copy_glyphs = glyphs(&ShapeResultView::create_from_result(&composite_copy));

    assert_glyphs_match(&view_glyphs, &copy_glyphs, 0);
    assert_eq!(composite_view.width, composite_copy.width());
  }

  #[test]
  fn latin_composite_view() {
    let (_fonts, font) = geist();
    let result = shape(
      &font,
      "Test run with multiple words and breaking opportunities.",
    );
    let reference_result = shape(&font, "multiple breaking opportunities Test");
    let ranges = [(14, 23), (33, 55), (4, 5), (0, 4)];
    let mut composite_copy = ShapeResult::new(0, 0, TextDirection::Ltr);

    for (start, end) in ranges {
      result.copy_range(start, end, &mut composite_copy);
    }

    assert_eq!(
      composite_copy.num_characters,
      reference_result.num_characters
    );
    assert_eq!(composite_copy.num_glyphs(), reference_result.num_glyphs());

    let composite_copy = Rc::new(composite_copy);
    let composite_view =
      ShapeResultView::create(&ranges.map(|(start, end)| segment(&result, start, end)));
    let composite_glyphs = glyphs(&composite_view);

    assert_eq!(composite_view.start_index(), composite_copy.start_index);
    assert_eq!(
      composite_view.num_characters,
      reference_result.num_characters
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
    assert_eq!(composite_view.width, composite_copy.width());
  }

  #[test]
  fn mixed_scripts_composite_view() {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");
    register(&mut fonts, NOTO_SANS_TC, "Noto Sans TC");

    let font = font(&fonts, "Geist, 'Noto Sans TC'");
    let string_a = "Test with multiple 字体 ";
    let string_b = "and 本書.";
    let result_a = shape(&font, string_a);
    let result_b = shape(&font, string_b);
    let reference_result = shape(&font, &format!("{string_a}{string_b}"));
    let mut composite_copy = ShapeResult::new(0, 0, TextDirection::Ltr);

    result_a.copy_range(0, string_a.len() as u32, &mut composite_copy);
    result_b.copy_range(0, string_b.len() as u32, &mut composite_copy);

    assert_eq!(
      composite_copy.num_characters,
      reference_result.num_characters
    );
    assert_eq!(composite_copy.num_glyphs(), reference_result.num_glyphs());

    let composite_copy = Rc::new(composite_copy);
    let composite_view = ShapeResultView::create(&[
      segment(&result_a, 0, string_a.len() as u32),
      segment(&result_b, 0, string_b.len() as u32),
    ]);

    assert_eq!(composite_view.start_index(), 0);
    assert_eq!(
      composite_view.num_characters,
      reference_result.num_characters
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
    assert_eq!(composite_view.width, composite_copy.width());
  }

  #[test]
  fn trim_end_of_view() {
    let (_fonts, font) = geist();
    let result = shape(&font, "12345678901234567890");
    let view1 = ShapeResultView::create_from_result_range(&result, 5, 20);

    assert_eq!((view1.num_characters, view1.num_glyphs()), (15, 15));

    let view2 = ShapeResultView::create_from_view_range(&view1, 5, 19);

    assert_eq!((view2.num_characters, view2.num_glyphs()), (14, 14));
  }

  #[test]
  fn previous_safe_offset_inside_view() {
    let (_fonts, font) = geist();
    let result = shape(&font, "Blah bla test something. ");

    let view1 = ShapeResultView::create_from_result_range(&result, 9, 14);

    assert_eq!(view1.previous_safe_to_break_offset(14), 14);

    let view2 = ShapeResultView::create_from_result_range(&result, 9, 25);

    assert_eq!(view2.previous_safe_to_break_offset(24), 24);
  }

  /// Blink's `ArabicSingleView`, in UTF-8 bytes: each Arabic letter takes two.
  #[test]
  fn arabic_single_view() {
    let mut fonts = Fonts::default();

    register(&mut fonts, SCHEHERAZADE, "Scheherazade");

    let font = font(&fonts, "Scheherazade");
    let shape_rtl =
      |text: &str| Rc::new(HarfBuzzShaper::new(text.into()).shape_all(&font, TextDirection::Rtl));
    let result = shape_rtl("\u{639}\u{631}\u{628}\u{649} \u{646}\u{635}");
    let all = glyphs(&ShapeResultView::create_from_result(&result));
    let first_word = ShapeResultView::create(&[segment(&result, 0, 8)]);
    let first_glyphs = glyphs(&first_word);
    let reference = glyphs(&ShapeResultView::create_from_result(&shape_rtl(
      "\u{639}\u{631}\u{628}\u{649}",
    )));

    assert_eq!(
      (first_word.start_index(), first_word.num_characters),
      (0, 8)
    );
    assert_eq!((first_word.num_glyphs(), first_glyphs.len()), (4, 4));
    assert_glyphs_match(&first_glyphs, &reference, 0);
    assert_glyphs_match(&first_glyphs, &all, 3);

    let last_word = ShapeResultView::create(&[segment(&result, 8, 13)]);

    assert_eq!((last_word.start_index(), last_word.num_characters), (8, 5));
    assert_eq!((last_word.num_glyphs(), glyphs(&last_word).len()), (3, 3));
  }

  /// Blink's `PreviousSafeToBreak`: a view of Hebrew with points finds the safe offsets the
  /// result it creates caches. Blink's offsets are characters; these are their UTF-8 bytes.
  #[test]
  fn previous_safe_to_break() {
    let (_fonts, font) = geist();
    let byte_of = |character: usize| {
      HEBREW
        .char_indices()
        .nth(character)
        .map_or(HEBREW.len(), |(byte, _)| byte)
    };
    let shaper = HarfBuzzShaper::new(HEBREW.into());
    let shape_result = Rc::new(shaper.shape(&font, TextDirection::Rtl, byte_of(51), byte_of(131)));
    let (start_offset, end_offset) = (byte_of(59) as u32, byte_of(118) as u32);
    let view = ShapeResultView::create_from_result_range(&shape_result, start_offset, end_offset);
    let result = view.create_shape_result();
    let offsets =
      (start_offset + 1..=end_offset).filter(|&offset| HEBREW.is_char_boundary(offset as usize));

    for offset in offsets {
      assert_eq!(
        view.previous_safe_to_break_offset(offset),
        result.previous_safe_to_break_offset(offset)
      );
    }
  }

  #[test]
  fn create_shape_result_matches_the_view() {
    let (_fonts, font) = geist();
    let result = shape(&font, "Test run with multiple words.");
    let view = ShapeResultView::create_from_result_range(&result, 5, 22);
    let copy = Rc::new(view.create_shape_result());

    assert_eq!(copy.start_index, view.start_index());
    assert_eq!(copy.num_characters, view.num_characters);
    assert_eq!(copy.width(), view.width);
    assert_eq!(
      glyphs(&ShapeResultView::create_from_result(&copy)),
      glyphs(&view)
    );
  }
}

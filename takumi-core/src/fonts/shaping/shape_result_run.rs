//! Blink's [`ShapeResultRun`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/shape_result_run.h).

use std::rc::Rc;

use harfrust::{Direction, GlyphInfo, Script};

use super::{
  glyph_data::{GlyphOffset, HarfBuzzRunGlyphData},
  glyph_data_range::GlyphDataRange,
};
use crate::{fonts::simple_font_data::SimpleFontData, layout_unit::InlineLayoutUnit};

/// Blink's `AdjustMidCluster`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdjustMidCluster {
  /// The logical end boundary.
  ToEnd,
  /// The logical start boundary.
  ToStart,
}

/// Blink's `BreakGlyphsOption`.
pub(crate) type BreakGlyphsOption = bool;

/// Blink's `GlyphIndexResult`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct GlyphIndexResult {
  pub(crate) left_character_index: u32,
  pub(crate) right_character_index: u32,
  pub(crate) origin_x: f32,
  pub(crate) advance: f32,
}

/// Blink's `ShapeResultRun::GlyphDataCollection`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GlyphDataCollection {
  data: Vec<HarfBuzzRunGlyphData>,
  /// `offsets[i]` is the offset of `data[i]`.
  offsets: Option<Vec<GlyphOffset>>,
  /// `graphemes[i]` is the index of the grapheme cluster holding byte `i` of the run.
  graphemes: Option<Vec<u32>>,
}

impl GlyphDataCollection {
  pub(crate) fn new(num_glyphs: usize) -> Self {
    Self {
      data: vec![HarfBuzzRunGlyphData::default(); num_glyphs],
      offsets: None,
      graphemes: None,
    }
  }

  pub(crate) fn size(&self) -> usize {
    self.data.len()
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.data.is_empty()
  }

  /// Blink's `NonCompactGlyphs`.
  pub(crate) fn glyphs(&self) -> &[HarfBuzzRunGlyphData] {
    &self.data
  }

  /// Blink's `MutableGlyphs`.
  pub(crate) fn glyphs_mut(&mut self) -> &mut [HarfBuzzRunGlyphData] {
    &mut self.data
  }

  pub(crate) fn has_non_zero_offsets(&self) -> bool {
    self.offsets.is_some()
  }

  pub(crate) fn has_graphemes(&self) -> bool {
    self.graphemes.is_some()
  }

  pub(crate) fn graphemes(&self) -> Option<&[u32]> {
    self.graphemes.as_deref()
  }

  pub(crate) fn set_graphemes(&mut self, graphemes: Vec<u32>) {
    self.graphemes = Some(graphemes);
  }

  /// The offsets when [`Self::has_non_zero_offsets`], or none.
  pub(crate) fn offsets(&self) -> &[GlyphOffset] {
    self.offsets.as_deref().unwrap_or_default()
  }

  /// Blink's `CopyFrom`.
  pub(crate) fn copy_from(&mut self, first: &Self, second: &Self) {
    let first_size = first.size();
    let second_size = second.size();

    assert!(first_size <= HarfBuzzRunGlyphData::MAX_GLYPHS as usize);
    assert!(second_size <= HarfBuzzRunGlyphData::MAX_GLYPHS as usize - first_size);
    debug_assert!(!first.is_empty());
    debug_assert!(!second.is_empty());

    self.data = first.data.iter().chain(&second.data).copied().collect();
    self.offsets = None;

    if first.has_non_zero_offsets() || second.has_non_zero_offsets() {
      let mut offsets = vec![GlyphOffset::default(); first_size + second_size];

      if first.has_non_zero_offsets() {
        offsets[..first_size].copy_from_slice(first.offsets());
      }

      if second.has_non_zero_offsets() {
        offsets[first_size..].copy_from_slice(second.offsets());
      }

      self.offsets = Some(offsets);
    }
  }

  /// Blink's `CopyFromRange`.
  pub(crate) fn copy_from_range(&mut self, range: GlyphDataRange<'_>, character_index_offset: u32) {
    self.data = range.glyphs().to_vec();
    self.offsets = None;

    for glyph in &mut self.data {
      assert!(u32::from(glyph.character_index) >= character_index_offset);
      glyph.character_index -= character_index_offset as u16;
    }

    if range.has_offsets() && !range.is_empty() {
      self.offsets = Some(range.offsets().to_vec());
    }
  }

  /// Blink's `AddOffsetHeightAt`.
  pub(crate) fn add_offset_height_at(&mut self, index: usize, delta: f32) {
    debug_assert!(delta != 0.0);

    self.allocate_offsets_if_needed()[index].y += delta;
  }

  /// Blink's `AddOffsetWidthAt`.
  pub(crate) fn add_offset_width_at(&mut self, index: usize, delta: f32) {
    debug_assert!(delta != 0.0);

    self.allocate_offsets_if_needed()[index].x += delta;
  }

  /// Blink's `SetOffsetAt`.
  pub(crate) fn set_offset_at(&mut self, index: usize, offset: GlyphOffset) {
    if !self.has_non_zero_offsets() && offset == GlyphOffset::default() {
      return;
    }

    self.allocate_offsets_if_needed()[index] = offset;
  }

  /// Blink's `Reverse`.
  pub(crate) fn reverse(&mut self) {
    self.data.reverse();

    if let Some(offsets) = &mut self.offsets {
      offsets.reverse();
    }
  }

  /// Blink's `Shrink`.
  pub(crate) fn shrink(&mut self, new_size: usize) {
    debug_assert!(new_size >= 1);

    if new_size == self.size() {
      return;
    }

    debug_assert!(new_size < self.size());

    self.data.truncate(new_size);

    if let Some(offsets) = &mut self.offsets {
      offsets.truncate(new_size);
    }
  }

  fn allocate_offsets_if_needed(&mut self) -> &mut [GlyphOffset] {
    let size = self.size();

    self
      .offsets
      .get_or_insert_with(|| vec![GlyphOffset::default(); size])
  }
}

/// Blink's `ShapeResultRun`.
#[derive(Debug, Clone)]
pub(crate) struct ShapeResultRun {
  pub(crate) glyph_data: GlyphDataCollection,
  pub(crate) font_data: Option<Rc<SimpleFontData>>,
  pub(crate) start_index: u32,
  pub(crate) num_characters: u32,
  pub(crate) width: f32,
  pub(crate) script: Script,
  pub(crate) direction: Direction,
}

impl ShapeResultRun {
  pub(crate) fn new(
    font: Option<Rc<SimpleFontData>>,
    direction: Direction,
    script: Script,
    start_index: u32,
    num_glyphs: u32,
    num_characters: u32,
  ) -> Self {
    Self {
      glyph_data: GlyphDataCollection::new(
        num_glyphs.min(HarfBuzzRunGlyphData::MAX_GLYPHS) as usize
      ),
      font_data: font,
      start_index,
      num_characters,
      width: 0.0,
      script,
      direction,
    }
  }

  pub(crate) fn num_glyphs(&self) -> u32 {
    self.glyph_data.size() as u32
  }

  pub(crate) fn has_ligatures(&self) -> bool {
    self.num_glyphs() < self.num_characters
  }

  pub(crate) fn is_ltr(&self) -> bool {
    matches!(
      self.direction,
      Direction::LeftToRight | Direction::TopToBottom
    )
  }

  pub(crate) fn is_rtl(&self) -> bool {
    matches!(
      self.direction,
      Direction::RightToLeft | Direction::BottomToTop
    )
  }

  pub(crate) fn is_horizontal(&self) -> bool {
    matches!(
      self.direction,
      Direction::LeftToRight | Direction::RightToLeft
    )
  }

  /// Blink's `NextSafeToBreakOffset`.
  pub(crate) fn next_safe_to_break_offset(&self, offset: u32) -> u32 {
    debug_assert!(offset <= self.num_characters);

    let is_safe = |glyph: &&HarfBuzzRunGlyphData| {
      glyph.is_safe_to_break_before() && u32::from(glyph.character_index) >= offset
    };
    let glyphs = self.glyph_data.glyphs();
    let found = if self.is_ltr() {
      glyphs.iter().find(is_safe)
    } else {
      glyphs.iter().rev().find(is_safe)
    };

    found.map_or(self.num_characters, |glyph| {
      u32::from(glyph.character_index)
    })
  }

  /// Blink's `PreviousSafeToBreakOffset`.
  pub(crate) fn previous_safe_to_break_offset(&self, offset: u32) -> u32 {
    if offset >= self.num_characters {
      return self.num_characters;
    }

    let is_safe = |glyph: &&HarfBuzzRunGlyphData| {
      glyph.is_safe_to_break_before() && u32::from(glyph.character_index) <= offset
    };
    let glyphs = self.glyph_data.glyphs();
    let found = if self.is_ltr() {
      glyphs.iter().rev().find(is_safe)
    } else {
      glyphs.iter().find(is_safe)
    };

    found.map_or(0, |glyph| u32::from(glyph.character_index))
  }

  /// Blink's `XPositionForVisualOffset`.
  pub(crate) fn x_position_for_visual_offset(
    &self,
    offset: u32,
    adjust_mid_cluster: AdjustMidCluster,
  ) -> f32 {
    debug_assert!(offset < self.num_characters);

    let offset = if self.is_rtl() {
      self.num_characters - offset - 1
    } else {
      offset
    };

    self.x_position_for_offset(offset, adjust_mid_cluster)
  }

  /// Blink's `NumGraphemes`.
  pub(crate) fn num_graphemes(&self, start: u32, end: u32) -> u32 {
    let Some(graphemes) = self.glyph_data.graphemes() else {
      return 0;
    };

    if start >= self.num_characters {
      return 0;
    }

    assert!(start < end);
    assert!(end <= self.num_characters);
    assert_eq!(self.num_characters as usize, graphemes.len());

    graphemes[end as usize - 1] - graphemes[start as usize] + 1
  }

  /// Blink's `XPositionForOffset`.
  pub(crate) fn x_position_for_offset(
    &self,
    offset: u32,
    adjust_mid_cluster: AdjustMidCluster,
  ) -> f32 {
    debug_assert!(offset <= self.num_characters);

    let glyphs = self.glyph_data.glyphs();

    let mut glyph_sequence_start = 0;
    let mut glyph_sequence_end = self.num_characters;
    let mut glyph_sequence_advance = InlineLayoutUnit::ZERO;
    let mut accumulated_position = InlineLayoutUnit::ZERO;

    if self.is_ltr() {
      for glyph in glyphs {
        let current_glyph_char_index = u32::from(glyph.character_index);

        if glyph_sequence_start == current_glyph_char_index {
          glyph_sequence_advance += glyph.advance.into();
          continue;
        }

        if glyph_sequence_start <= offset && offset < current_glyph_char_index {
          glyph_sequence_end = current_glyph_char_index;
          break;
        }

        glyph_sequence_start = current_glyph_char_index;
        glyph_sequence_end = self.num_characters;
        accumulated_position += glyph_sequence_advance;
        glyph_sequence_advance = glyph.advance.into();
      }
    } else {
      glyph_sequence_start = self.num_characters;
      glyph_sequence_end = self.num_characters;

      for glyph in glyphs {
        let current_glyph_char_index = u32::from(glyph.character_index);

        if glyph_sequence_start == current_glyph_char_index {
          glyph_sequence_advance += glyph.advance.into();
          continue;
        }

        if glyph_sequence_start <= offset && offset < glyph_sequence_end {
          break;
        }

        glyph_sequence_end = glyph_sequence_start;
        glyph_sequence_start = current_glyph_char_index;
        accumulated_position += glyph_sequence_advance;
        glyph_sequence_advance = glyph.advance.into();
      }

      if glyph_sequence_start > offset {
        glyph_sequence_start = 0;
      }
    }

    let mut is_offset_at_glyph_sequence_start = offset == glyph_sequence_start;
    let graphemes = self.num_graphemes(glyph_sequence_start, glyph_sequence_end);

    if graphemes > 1 {
      debug_assert!(glyph_sequence_end >= glyph_sequence_start);

      let next_offset = offset + u32::from(offset != self.num_characters);
      let num_graphemes_to_offset = self.num_graphemes(glyph_sequence_start, next_offset) - 1;

      if offset > 0 {
        is_offset_at_glyph_sequence_start = self.num_graphemes(offset - 1, next_offset) != 1;
      }

      glyph_sequence_advance = glyph_sequence_advance / graphemes as i32;

      let num_graphemes_from_left = if self.is_ltr() {
        num_graphemes_to_offset
      } else {
        graphemes - num_graphemes_to_offset - 1
      };

      accumulated_position += glyph_sequence_advance * num_graphemes_from_left as i32;
    }

    if adjust_mid_cluster == AdjustMidCluster::ToEnd && !is_offset_at_glyph_sequence_start {
      if self.is_ltr() {
        accumulated_position += glyph_sequence_advance;
      } else {
        accumulated_position -= glyph_sequence_advance;
      }
    }

    if self.is_rtl() {
      accumulated_position += glyph_sequence_advance;
    }

    accumulated_position.to_float()
  }

  /// Blink's `CharacterIndexForXPosition`.
  pub(crate) fn character_index_for_x_position(
    &self,
    target_x: f32,
    break_glyphs: BreakGlyphsOption,
  ) -> GlyphIndexResult {
    debug_assert!(target_x >= 0.0 && target_x <= self.width);

    let mut result = GlyphIndexResult::default();
    let mut glyph_sequence_start = 0;
    let mut glyph_sequence_end = self.num_characters;

    if self.is_rtl() {
      glyph_sequence_start = self.num_characters;
      glyph_sequence_end = self.num_characters;
    }

    for glyph in self.glyph_data.glyphs() {
      let current_glyph_char_index = u32::from(glyph.character_index);

      if glyph_sequence_start == current_glyph_char_index {
        result.advance += glyph.advance.to_float();
        continue;
      }

      if result.origin_x + result.advance > target_x {
        if self.is_ltr() {
          glyph_sequence_end = current_glyph_char_index;
        }

        break;
      }

      if self.is_rtl() {
        glyph_sequence_end = glyph_sequence_start;
      }

      glyph_sequence_start = current_glyph_char_index;
      result.origin_x += result.advance;
      result.advance = glyph.advance.to_float();
    }

    if break_glyphs && glyph_sequence_end > glyph_sequence_start {
      let graphemes = self.num_graphemes(glyph_sequence_start, glyph_sequence_end);

      if graphemes > 1 {
        let unit_size = result.advance / graphemes as f32;
        let step = ((target_x - result.origin_x) / unit_size).floor() as u32;
        let glyph_length = glyph_sequence_end - glyph_sequence_start;
        let final_size = glyph_length / graphemes;

        result.origin_x += unit_size * step as f32;

        if self.is_ltr() {
          glyph_sequence_start += step;
          glyph_sequence_end = glyph_sequence_start + final_size;
        } else {
          glyph_sequence_end -= step;
          glyph_sequence_start = glyph_sequence_end - final_size;
        }

        result.advance = unit_size;
      }
    }

    if self.is_ltr() {
      result.left_character_index = glyph_sequence_start;
      result.right_character_index = glyph_sequence_end;
    } else {
      result.left_character_index = glyph_sequence_end;
      result.right_character_index = glyph_sequence_start;
    }

    result
  }

  /// Blink's `LimitNumGlyphs`, returning the glyphs kept and the glyphs dropped.
  pub(crate) fn limit_num_glyphs(
    &mut self,
    start_glyph: usize,
    mut num_glyphs: usize,
    is_ltr: bool,
    glyph_infos: &[GlyphInfo],
  ) -> (usize, usize) {
    assert!(num_glyphs > 0);

    let num_glyphs_in = num_glyphs;
    let max_character_index = HarfBuzzRunGlyphData::MAX_CHARACTER_INDEX;
    let left = start_glyph;
    let mut right = start_glyph + num_glyphs - 1;
    let mut start_cluster;

    if is_ltr {
      start_cluster = glyph_infos[left].cluster;

      let last_cluster = glyph_infos[right].cluster;
      let max_cluster = start_cluster + max_character_index;

      if last_cluster > max_cluster {
        let limit =
          left + glyph_infos[left..=right].partition_point(|info| info.cluster <= max_cluster) - 1;

        assert!(limit > left);
        assert!(limit < right);

        right = limit;
        num_glyphs = right - left + 1;
        self.num_characters = glyph_infos[right + 1].cluster - start_cluster;
      }
    } else {
      start_cluster = glyph_infos[right].cluster;

      let last_cluster = glyph_infos[left].cluster;
      let max_cluster = start_cluster + max_character_index;

      if last_cluster > max_cluster {
        let min_cluster = last_cluster - max_character_index;

        debug_assert!(start_cluster < min_cluster);

        let limit =
          left + glyph_infos[left..=right].partition_point(|info| info.cluster >= min_cluster) - 1;

        assert!(limit > left);
        assert!(limit < right);

        right = limit;
        start_cluster = glyph_infos[right].cluster;
        num_glyphs = right - left + 1;
        self.start_index = start_cluster;
        self.num_characters = last_cluster - glyph_infos[right + 1].cluster;
      }
    }

    let mut num_glyphs_removed = 0;
    let max_glyphs = HarfBuzzRunGlyphData::MAX_GLYPHS as usize;

    if num_glyphs > max_glyphs {
      let old_num_glyphs = num_glyphs;

      num_glyphs = max_glyphs;

      let end_cluster = glyph_infos[start_glyph + num_glyphs].cluster;

      while num_glyphs > 0 && glyph_infos[start_glyph + num_glyphs - 1].cluster == end_cluster {
        num_glyphs -= 1;
      }

      if num_glyphs == 0 {
        num_glyphs = max_glyphs;
        num_glyphs_removed = old_num_glyphs - num_glyphs;
      } else if is_ltr {
        self.num_characters = end_cluster - start_cluster;
        debug_assert!(self.num_characters > 0);
      } else {
        self.num_characters = glyph_infos[start_glyph].cluster - end_cluster;
        self.start_index = glyph_infos[start_glyph + num_glyphs - 1].cluster;
        debug_assert!(self.num_characters > 0);
      }
    }

    debug_assert!(num_glyphs <= max_glyphs);

    if num_glyphs != num_glyphs_in {
      self.glyph_data.shrink(num_glyphs);
    }

    (num_glyphs, num_glyphs_removed)
  }

  /// Blink's `FindGlyphDataRange`.
  pub(crate) fn find_glyph_data_range(
    &self,
    start_character_index: u32,
    end_character_index: u32,
  ) -> GlyphDataRange<'_> {
    GlyphDataRange::new(self).find_glyph_data_range(
      self.is_rtl(),
      start_character_index,
      end_character_index,
    )
  }

  /// Blink's `CreateSubRun`.
  pub(crate) fn create_sub_run(&self, start: u32, end: u32) -> Option<Self> {
    debug_assert!(end > start);

    let number_of_characters = (end - start).min(self.num_characters);
    let glyphs = self.find_glyph_data_range(start, end);

    if glyphs.is_empty() {
      return None;
    }

    let mut run = Self::new(
      self.font_data.clone(),
      self.direction,
      self.script,
      self.start_index + start,
      0,
      number_of_characters,
    );

    run.glyph_data.copy_from_range(glyphs, start);

    let mut total_advance = InlineLayoutUnit::ZERO;

    for glyph in run.glyph_data.glyphs() {
      total_advance += glyph.advance.into();
    }

    run.width = total_advance.to_float();

    Some(run)
  }

  /// Blink's `MergeIfPossible`.
  pub(crate) fn merge_if_possible(&self, other: &Self) -> Option<Self> {
    if !self.can_merge(other) {
      return None;
    }

    debug_assert!(self.start_index < other.start_index);

    let mut run = Self::new(
      self.font_data.clone(),
      self.direction,
      self.script,
      self.start_index,
      0,
      self.num_characters + other.num_characters,
    );
    let index_adjust = (other.start_index - self.start_index) as u16;

    if self.is_rtl() {
      run
        .glyph_data
        .copy_from(&other.glyph_data, &self.glyph_data);

      for glyph in &mut run.glyph_data.glyphs_mut()[..other.glyph_data.size()] {
        glyph.character_index += index_adjust;
      }
    } else {
      run
        .glyph_data
        .copy_from(&self.glyph_data, &other.glyph_data);

      for glyph in &mut run.glyph_data.glyphs_mut()[self.glyph_data.size()..] {
        glyph.character_index += index_adjust;
      }
    }

    run.width = self.width + other.width;

    Some(run)
  }

  /// Blink's `CanMerge`.
  pub(crate) fn can_merge(&self, other: &Self) -> bool {
    let same_font = match (&self.font_data, &other.font_data) {
      (Some(a), Some(b)) => Rc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    };

    self.start_index + self.num_characters == other.start_index
      && same_font
      && self.direction == other.direction
      && self.script == other.script
      && self.glyph_data.size() + other.glyph_data.size()
        <= HarfBuzzRunGlyphData::MAX_CHARACTER_INDEX as usize
  }

  /// Blink's `ExpandRangeToIncludePartialGlyphs`.
  pub(crate) fn expand_range_to_include_partial_glyphs(
    &self,
    offset: i32,
    from: &mut i32,
    to: &mut i32,
  ) {
    let num_characters = self.num_characters as i32;
    let mut include = |start: i32, end: i32| {
      if end > *from && start < *to {
        *from = (*from).min(start);
        *to = (*to).max(end);
      }
    };
    let mut end = offset + num_characters;
    let mut start = offset + num_characters;

    if self.is_ltr() {
      for glyph in self.glyph_data.glyphs() {
        let index = offset + i32::from(glyph.character_index);

        if start == index {
          continue;
        }

        include(start, index);
        start = index;
      }
    } else {
      for glyph in self.glyph_data.glyphs() {
        let index = offset + i32::from(glyph.character_index);

        if start == index {
          continue;
        }

        include(start, end);
        end = start;
        start = index;
      }
    }

    include(start, end);
  }
}

#[cfg(test)]
mod tests {
  use harfrust::script;

  use super::*;

  /// Blink's `CreateTestShapeResultRun`.
  fn create_test_shape_result_run(num_glyphs: u32, num_characters: u32) -> ShapeResultRun {
    ShapeResultRun::new(
      None,
      Direction::LeftToRight,
      script::LATIN,
      0,
      num_glyphs,
      num_characters,
    )
  }

  #[test]
  fn glyph_data_copy_constructor() {
    let mut run = create_test_shape_result_run(2, 2);

    run.glyph_data.set_graphemes(vec![0; 2]);

    let run2 = run.clone();

    assert!(!run2.glyph_data.has_non_zero_offsets());
    assert_eq!(run2.glyph_data.graphemes(), run.glyph_data.graphemes());

    run.glyph_data.set_offset_at(0, GlyphOffset::new(1.0, 1.0));

    let run3 = run.clone();

    assert!(run3.glyph_data.has_non_zero_offsets());
    assert_eq!(run3.glyph_data.offsets()[0], GlyphOffset::new(1.0, 1.0));
    assert_eq!(run3.glyph_data.graphemes(), run.glyph_data.graphemes());

    run.glyph_data.set_offset_at(0, GlyphOffset::new(2.0, 2.0));

    assert_eq!(run3.glyph_data.offsets()[0], GlyphOffset::new(1.0, 1.0));
  }

  #[test]
  fn large_num_characters() {
    let run = create_test_shape_result_run(2, 131_072);

    assert_eq!(run.num_characters, 131_072);
  }

  #[test]
  fn glyph_data_copy_from_range() {
    let mut run = create_test_shape_result_run(2, 2);
    let mut run2 = create_test_shape_result_run(2, 2);

    run2
      .glyph_data
      .copy_from_range(GlyphDataRange::new(&run), 0);

    assert!(!run2.glyph_data.has_non_zero_offsets());

    run.glyph_data.set_offset_at(0, GlyphOffset::new(1.0, 1.0));

    assert!(run.glyph_data.has_non_zero_offsets());

    let mut run3 = create_test_shape_result_run(2, 2);

    run3
      .glyph_data
      .copy_from_range(GlyphDataRange::new(&run), 0);

    assert!(run3.glyph_data.has_non_zero_offsets());
    assert_eq!(run3.glyph_data.offsets()[0], GlyphOffset::new(1.0, 1.0));
  }

  #[test]
  fn glyph_data_reverse() {
    let mut run = create_test_shape_result_run(2, 2);

    run.glyph_data.reverse();

    assert!(!run.glyph_data.has_non_zero_offsets());

    run.glyph_data.set_offset_at(0, GlyphOffset::new(1.0, 1.0));

    assert!(run.glyph_data.has_non_zero_offsets());

    run.glyph_data.reverse();

    assert_eq!(run.glyph_data.offsets()[0], GlyphOffset::default());
    assert_eq!(run.glyph_data.offsets()[1], GlyphOffset::new(1.0, 1.0));
  }

  #[test]
  fn glyph_data_add_offset_height_at() {
    let mut run = create_test_shape_result_run(2, 2);

    run.glyph_data.add_offset_height_at(1, 1.5);

    assert!(run.glyph_data.has_non_zero_offsets());
    assert_eq!(run.glyph_data.offsets()[1], GlyphOffset::new(0.0, 1.5));

    run.glyph_data.add_offset_height_at(1, 2.0);

    assert_eq!(run.glyph_data.offsets()[1], GlyphOffset::new(0.0, 3.5));
  }

  #[test]
  fn glyph_data_add_offset_width_at() {
    let mut run = create_test_shape_result_run(2, 2);

    run.glyph_data.add_offset_width_at(1, 1.5);

    assert!(run.glyph_data.has_non_zero_offsets());
    assert_eq!(run.glyph_data.offsets()[1], GlyphOffset::new(1.5, 0.0));

    run.glyph_data.add_offset_width_at(1, 2.0);

    assert_eq!(run.glyph_data.offsets()[1], GlyphOffset::new(3.5, 0.0));
  }

  #[test]
  fn glyph_data_set_at() {
    let mut run = create_test_shape_result_run(2, 2);

    run.glyph_data.set_offset_at(0, GlyphOffset::default());

    assert!(!run.glyph_data.has_non_zero_offsets());

    run.glyph_data.set_offset_at(1, GlyphOffset::new(1.0, 1.0));

    assert!(run.glyph_data.has_non_zero_offsets());
    assert_eq!(run.glyph_data.offsets()[1], GlyphOffset::new(1.0, 1.0));
  }

  #[test]
  fn glyph_data_shrink() {
    let mut run_no_offsets = create_test_shape_result_run(3, 3);

    run_no_offsets.glyph_data.shrink(2);

    assert!(!run_no_offsets.glyph_data.has_non_zero_offsets());
    assert_eq!(run_no_offsets.glyph_data.size(), 2);

    run_no_offsets.glyph_data.shrink(1);

    assert!(!run_no_offsets.glyph_data.has_non_zero_offsets());
    assert_eq!(run_no_offsets.glyph_data.size(), 1);

    let mut run_with_offsets = create_test_shape_result_run(3, 3);

    run_with_offsets
      .glyph_data
      .set_offset_at(0, GlyphOffset::new(1.0, 0.0));
    run_with_offsets
      .glyph_data
      .set_offset_at(1, GlyphOffset::new(2.0, 0.0));
    run_with_offsets
      .glyph_data
      .set_offset_at(2, GlyphOffset::new(3.0, 0.0));
    run_with_offsets.glyph_data.shrink(2);

    assert_eq!(
      run_with_offsets.glyph_data.offsets(),
      [GlyphOffset::new(1.0, 0.0), GlyphOffset::new(2.0, 0.0)]
    );

    run_with_offsets.glyph_data.shrink(1);

    assert_eq!(
      run_with_offsets.glyph_data.offsets(),
      [GlyphOffset::new(1.0, 0.0)]
    );

    let mut run_shrink_same_size = create_test_shape_result_run(2, 2);

    run_shrink_same_size
      .glyph_data
      .set_offset_at(0, GlyphOffset::new(5.0, 0.0));
    run_shrink_same_size.glyph_data.shrink(2);

    assert_eq!(run_shrink_same_size.glyph_data.size(), 2);
    assert_eq!(
      run_shrink_same_size.glyph_data.offsets()[0],
      GlyphOffset::new(5.0, 0.0)
    );
  }
}

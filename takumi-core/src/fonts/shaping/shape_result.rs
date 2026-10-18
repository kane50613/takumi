//! Blink's [`ShapeResult`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/shape_result.cc).

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

#[cfg(test)]
use harfrust::{Direction as HbDirection, script};
use icu_segmenter::GraphemeClusterSegmenter;

#[cfg(test)]
use super::glyph_data::HarfBuzzRunGlyphData;
use super::{
  glyph_data::SafeToBreak,
  shape_result_run::{AdjustMidCluster, BreakGlyphsOption, GlyphIndexResult, ShapeResultRun},
};
#[cfg(test)]
use crate::layout_unit::TextRunLayoutUnit;
use crate::{
  fonts::simple_font_data::SimpleFontData,
  layout_unit::{FixedPoint, InlineLayoutUnit, LayoutUnit},
  style::Direction as TextDirection,
};

/// Blink's `ShapeResultCharacterData`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ShapeResultCharacterData {
  pub(crate) x_position: LayoutUnit,
  pub(crate) is_cluster_base: bool,
  pub(crate) safe_to_break_before: bool,
}

impl ShapeResultCharacterData {
  /// Blink's `SetCachedData`.
  fn set_cached_data(
    &mut self,
    x_position: LayoutUnit,
    is_cluster_base: bool,
    safe_to_break_before: bool,
  ) {
    self.x_position = x_position;
    self.is_cluster_base = is_cluster_base;
    self.safe_to_break_before = safe_to_break_before;
  }
}

/// Blink's `ShapeResult::ShapeRange`.
pub(crate) struct ShapeRange<'a> {
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) target: &'a mut ShapeResult,
}

/// Blink's `ShapeResult`.
#[derive(Debug, Clone)]
pub(crate) struct ShapeResult {
  character_position: RefCell<Vec<ShapeResultCharacterData>>,
  /// Runs in visual order.
  pub(crate) runs: Vec<ShapeResultRun>,
  width: Cell<f32>,
  start_index: u32,
  num_characters: u32,
  direction: TextDirection,
  has_vertical_offsets: bool,
  is_applied_spacing: bool,
}

impl ShapeResult {
  pub(crate) fn new(start_index: u32, num_characters: u32, direction: TextDirection) -> Self {
    Self {
      character_position: RefCell::new(Vec::new()),
      runs: Vec::new(),
      width: Cell::new(0.0),
      start_index,
      num_characters,
      direction,
      has_vertical_offsets: false,
      is_applied_spacing: false,
    }
  }

  /// Blink's `CreateEmpty`.
  pub(crate) fn create_empty(other: &Self) -> Self {
    Self::new(0, 0, other.direction())
  }

  pub(crate) fn width(&self) -> f32 {
    self.width.get()
  }

  /// Blink's `SnappedWidth`.
  pub(crate) fn snapped_width(&self) -> LayoutUnit {
    LayoutUnit::from_float_ceil(self.width.get())
  }

  pub(crate) fn num_characters(&self) -> u32 {
    self.num_characters
  }

  /// Blink's `HasLigatures`.
  pub(crate) fn has_ligatures(&self) -> bool {
    self.runs.iter().any(ShapeResultRun::has_ligatures)
  }

  /// Blink's `NumGlyphs`.
  pub(crate) fn num_glyphs(&self) -> u32 {
    self.runs.iter().map(ShapeResultRun::num_glyphs).sum()
  }

  /// Blink's `HasFallbackFonts`.
  pub(crate) fn has_fallback_fonts(&self, primary_font: &Rc<SimpleFontData>) -> bool {
    self.runs.iter().any(|run| {
      run
        .font_data
        .as_ref()
        .is_none_or(|font| !Rc::ptr_eq(font, primary_font))
    })
  }

  pub(crate) fn start_index(&self) -> u32 {
    self.start_index
  }

  pub(crate) fn end_index(&self) -> u32 {
    self.start_index + self.num_characters
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

  pub(crate) fn is_applied_spacing(&self) -> bool {
    self.is_applied_spacing
  }

  /// Blink's `CharacterData`.
  pub(crate) fn character_data(&self, offset: u32) -> ShapeResultCharacterData {
    debug_assert!(offset >= self.start_index());
    debug_assert!(offset < self.end_index());

    let character_position = self.character_position.borrow();

    debug_assert!(!character_position.is_empty());

    character_position[(offset - self.start_index) as usize]
  }

  /// Blink's `IsStartSafeToBreak`.
  pub(crate) fn is_start_safe_to_break(&self) -> bool {
    let run = if self.is_ltr() {
      self.runs.first()
    } else {
      self.runs.last()
    };
    let Some(run) = run else {
      return false;
    };
    let glyph = if self.is_ltr() {
      run.glyph_data.glyphs().first()
    } else {
      run.glyph_data.glyphs().last()
    };

    glyph.is_some_and(|glyph| {
      glyph.is_safe_to_break_before()
        && self.start_index() == run.start_index + u32::from(glyph.character_index)
    })
  }

  /// Blink's `NextSafeToBreakOffset`.
  pub(crate) fn next_safe_to_break_offset(&self, index: u32) -> u32 {
    for (position, run) in self.runs.iter().enumerate() {
      let run_start = run.start_index;

      if index >= run_start {
        let offset = index - run_start;

        if offset < run.num_characters {
          return run.next_safe_to_break_offset(offset) + run_start;
        }

        if self.is_rtl() {
          if position == 0 {
            return run_start + run.num_characters;
          }

          return self.runs[position - 1].start_index;
        }
      } else if self.is_ltr() {
        return run_start;
      }
    }

    self.end_index()
  }

  /// Blink's `PreviousSafeToBreakOffset`.
  pub(crate) fn previous_safe_to_break_offset(&self, index: u32) -> u32 {
    for (position, run) in self.runs.iter().enumerate().rev() {
      let run_start = run.start_index;

      if index >= run_start {
        let offset = index - run_start;

        if offset <= run.num_characters {
          return run.previous_safe_to_break_offset(offset) + run_start;
        }

        if self.is_ltr() {
          return run_start + run.num_characters;
        }
      } else if self.is_rtl() {
        if position == self.runs.len() - 1 {
          return run.start_index;
        }

        let previous_run = &self.runs[position + 1];

        return previous_run.start_index + previous_run.num_characters;
      }
    }

    self.start_index()
  }

  /// Blink's `AddUnsafeToBreak`.
  pub(crate) fn add_unsafe_to_break(&mut self, offsets: &[u32]) {
    if self.is_ltr() {
      self.add_unsafe_to_break_in_order(offsets.iter().copied());
    } else {
      self.add_unsafe_to_break_in_order(offsets.iter().rev().copied());
    }
  }

  fn add_unsafe_to_break_in_order(&mut self, mut offsets: impl Iterator<Item = u32>) {
    let Some(mut offset) = offsets.next() else {
      debug_assert!(false, "AddUnsafeToBreak needs an offset");

      return;
    };

    debug_assert!(self.character_position.borrow().is_empty());
    debug_assert!(offset >= self.start_index());

    for run in &mut self.runs {
      let mut run_offset = offset.wrapping_sub(run.start_index);

      if run_offset >= run.num_characters {
        continue;
      }

      for glyph in run.glyph_data.glyphs_mut() {
        if u32::from(glyph.character_index) == run_offset {
          glyph.set_safe_to_break_before(SafeToBreak::Unsafe);

          let Some(next) = offsets.next() else {
            return;
          };

          offset = next;
          run_offset = offset.wrapping_sub(run.start_index);

          if run_offset >= run.num_characters {
            break;
          }
        }
      }
    }
  }

  /// Blink's `OffsetForPosition` with a `GlyphIndexResult`.
  pub(crate) fn offset_for_position_result(
    &self,
    target_x: f32,
    break_glyphs: BreakGlyphsOption,
  ) -> GlyphIndexResult {
    let mut result = GlyphIndexResult::default();

    if target_x <= 0.0 {
      if self.is_rtl() {
        result.left_character_index = self.num_characters();
        result.right_character_index = self.num_characters();
      }

      return result;
    }

    let mut characters_so_far = if self.is_rtl() {
      self.num_characters()
    } else {
      0
    };
    let mut current_x = 0.0;

    for run in &self.runs {
      if self.is_rtl() {
        characters_so_far -= run.num_characters;
      }

      let offset_for_run = target_x - current_x;

      if offset_for_run >= 0.0 && offset_for_run < run.width {
        result = run.character_index_for_x_position(offset_for_run, break_glyphs);
        result.left_character_index += characters_so_far;
        result.right_character_index += characters_so_far;
        result.origin_x += current_x;

        return result;
      }

      if self.is_ltr() {
        characters_so_far += run.num_characters;
      }

      current_x += run.width;
    }

    if self.is_rtl() {
      result.left_character_index = 0;
      result.right_character_index = 0;
    } else {
      result.left_character_index += characters_so_far;
      result.right_character_index += characters_so_far;
    }

    result
  }

  /// Blink's `OffsetForPosition`.
  pub(crate) fn offset_for_position(&self, x: f32) -> u32 {
    let result = self.offset_for_position_result(x, true);

    if self.is_ltr() {
      return result.left_character_index;
    }

    if x == result.origin_x {
      return result.left_character_index;
    }

    result.right_character_index
  }

  /// Blink's `OffsetToFit`.
  pub(crate) fn offset_to_fit(&self, x: f32, line_direction: TextDirection) -> u32 {
    let result = self.offset_for_position_result(x, false);

    if line_direction == TextDirection::Ltr {
      return result.left_character_index;
    }

    if x == result.origin_x {
      return result.left_character_index;
    }

    result.right_character_index
  }

  /// Blink's `PositionForOffset`.
  pub(crate) fn position_for_offset(
    &self,
    absolute_offset: u32,
    adjust_mid_cluster: AdjustMidCluster,
  ) -> f32 {
    let mut x = 0.0;
    let mut offset = absolute_offset;

    if self.is_rtl() && offset < self.num_characters() {
      offset = self.num_characters() - offset - 1;
    }

    for run in &self.runs {
      debug_assert_eq!(self.is_rtl(), run.is_rtl());

      if offset < run.num_characters {
        return run.x_position_for_visual_offset(offset, adjust_mid_cluster) + x;
      }

      offset -= run.num_characters;
      x += run.width;
    }

    if absolute_offset == self.num_characters() {
      return if self.is_rtl() { 0.0 } else { self.width() };
    }

    0.0
  }

  /// Blink's `EnsureGraphemes`, counting graphemes with ICU4X instead of Blink's LGPL code.
  pub(crate) fn ensure_graphemes(&mut self, text: &str) {
    assert_eq!(self.num_characters() as usize, text.len());

    if self.runs.is_empty() || self.runs[0].glyph_data.has_graphemes() {
      return;
    }

    let result_start_index = self.start_index();
    let segmenter = GraphemeClusterSegmenter::new();

    for run in &mut self.runs {
      debug_assert!(run.start_index >= result_start_index);

      let start = (run.start_index - result_start_index) as usize;
      let run_text = &text[start..start + run.num_characters as usize];
      let mut graphemes = vec![0; run_text.len()];
      let mut position = 0;

      for (count, boundary) in segmenter.segment_str(run_text).skip(1).enumerate() {
        graphemes[position..boundary].fill(count as u32);
        position = boundary;
      }

      run.glyph_data.set_graphemes(graphemes);
    }
  }

  /// Blink's `EnsurePositionData`.
  pub(crate) fn ensure_position_data(&self) {
    if !self.character_position.borrow().is_empty() {
      return;
    }

    *self.character_position.borrow_mut() =
      vec![ShapeResultCharacterData::default(); self.num_characters as usize];
    self.compute_position_data();
  }

  /// Blink's `ComputePositionData`.
  fn compute_position_data(&self) {
    let is_rtl = self.is_rtl();
    let mut character_position = self.character_position.borrow_mut();
    let num_characters = self.num_characters;
    let mut next_character_index = 0;
    let mut total_advance = InlineLayoutUnit::ZERO;
    let mut last_x_position = LayoutUnit::ZERO;

    for run in &self.runs {
      debug_assert_eq!(self.is_rtl(), run.is_rtl());

      for glyph_data in run.glyph_data.glyphs() {
        debug_assert!(run.start_index >= self.start_index);

        let logical_index =
          run.start_index + u32::from(glyph_data.character_index) - self.start_index;

        debug_assert!(logical_index < num_characters);

        let character_index = if is_rtl {
          num_characters - logical_index - 1
        } else {
          logical_index
        };

        assert!(character_index < num_characters);

        if next_character_index <= character_index {
          if next_character_index < character_index {
            let x_position = if is_rtl {
              total_advance.to_ceil::<LayoutUnit>()
            } else {
              last_x_position
            };

            for data in
              &mut character_position[next_character_index as usize..character_index as usize]
            {
              data.set_cached_data(x_position, false, false);
            }
          }

          last_x_position = total_advance.to_ceil::<LayoutUnit>();
          character_position[character_index as usize].set_cached_data(
            last_x_position,
            true,
            glyph_data.is_safe_to_break_before(),
          );
        }

        total_advance += glyph_data.advance.into();
        next_character_index = character_index + 1;
      }
    }

    if next_character_index < num_characters {
      let x_position = if is_rtl {
        total_advance.to_ceil::<LayoutUnit>()
      } else {
        last_x_position
      };

      for data in &mut character_position[next_character_index as usize..] {
        data.set_cached_data(x_position, false, false);
      }
    }

    self.width.set(total_advance.to_float());
  }

  /// Blink's `CachedOffsetForPosition`.
  pub(crate) fn cached_offset_for_position(&self, x: LayoutUnit) -> u32 {
    let character_position = self.character_position.borrow();

    debug_assert!(!character_position.is_empty());

    let rtl = self.is_rtl();
    let length = self.num_characters();

    if x <= LayoutUnit::ZERO {
      return if rtl { length } else { 0 };
    }

    if x.to_float() >= self.width() {
      return if rtl { 0 } else { length };
    }

    let mut low: i64 = 0;
    let mut high: i64 = i64::from(length) - 1;

    while low <= high {
      let midpoint = low + (high - low) / 2;
      let x_position = character_position[midpoint as usize].x_position;

      if x_position <= x
        && (midpoint + 1 == i64::from(length)
          || character_position[midpoint as usize + 1].x_position > x)
      {
        if !rtl {
          return midpoint as u32;
        }

        return if x_position == x {
          length - midpoint as u32
        } else {
          length - midpoint as u32 - 1
        };
      }

      if x < x_position {
        high = midpoint - 1;
      } else {
        low = midpoint + 1;
      }
    }

    0
  }

  /// Blink's `CachedPositionForOffset`.
  pub(crate) fn cached_position_for_offset(&self, offset: u32) -> LayoutUnit {
    let character_position = self.character_position.borrow();

    debug_assert!(offset <= self.num_characters);
    debug_assert!(!character_position.is_empty());

    let length = self.num_characters();

    if !self.is_rtl() {
      if offset < length {
        return character_position[offset as usize].x_position;
      }
    } else {
      if offset >= length {
        return LayoutUnit::ZERO;
      }

      for visual_offset in (length - offset - 1)..length {
        if character_position[visual_offset as usize].is_cluster_base {
          return if visual_offset + 1 < length {
            character_position[visual_offset as usize + 1].x_position
          } else {
            LayoutUnit::from_float_ceil(self.width())
          };
        }
      }
    }

    LayoutUnit::from_float_ceil(self.width())
  }

  /// Blink's `CachedWidth`.
  pub(crate) fn cached_width(&self, start_offset: u32, end_offset: u32) -> LayoutUnit {
    let start_position = self.cached_position_for_offset(start_offset - self.start_index);
    let end_position = self.cached_position_for_offset(end_offset - self.start_index);

    if self.is_ltr() {
      end_position - start_position
    } else {
      start_position - end_position
    }
  }

  /// Blink's `CachedNextSafeToBreakOffset`.
  pub(crate) fn cached_next_safe_to_break_offset(&self, offset: u32) -> u32 {
    if self.is_rtl() {
      return self.next_safe_to_break_offset(offset);
    }

    let character_position = self.character_position.borrow();

    debug_assert!(!character_position.is_empty());
    debug_assert!(self.start_index <= offset);

    let adjusted_offset = offset - self.start_index;
    let length = self.num_characters();

    debug_assert!(adjusted_offset < length);

    (adjusted_offset..length)
      .find(|&i| character_position[i as usize].safe_to_break_before)
      .map_or(self.start_index + length, |i| self.start_index + i)
  }

  /// Blink's `CachedPreviousSafeToBreakOffset`.
  pub(crate) fn cached_previous_safe_to_break_offset(&self, offset: u32) -> u32 {
    if self.is_rtl() {
      return self.previous_safe_to_break_offset(offset);
    }

    let character_position = self.character_position.borrow();

    debug_assert!(!character_position.is_empty());
    debug_assert!(self.start_index <= offset);

    let adjusted_offset = offset - self.start_index;
    let length = self.num_characters();

    debug_assert!(adjusted_offset <= length);

    if adjusted_offset >= length {
      return self.start_index + length;
    }

    (0..=adjusted_offset)
      .rev()
      .find(|&i| character_position[i as usize].safe_to_break_before)
      .map_or(self.start_index, |i| self.start_index + i)
  }

  /// Blink's `ReorderRtlRuns`.
  fn reorder_rtl_runs(&mut self, run_size_before: usize) {
    debug_assert!(self.is_rtl());
    debug_assert!(self.runs.len() > run_size_before);

    self.runs.rotate_left(run_size_before);
  }

  /// Blink's `CopyRange`.
  pub(crate) fn copy_range(&self, start_offset: u32, end_offset: u32, target: &mut Self) {
    self.copy_range_internal(0, start_offset, end_offset, target);
  }

  /// Blink's `CopyRanges`.
  pub(crate) fn copy_ranges(&self, ranges: &mut [ShapeRange<'_>]) {
    debug_assert!(!ranges.is_empty());

    let mut run_index = 0;

    if self.is_rtl() {
      for range in ranges.iter_mut().rev() {
        debug_assert!(range.end >= range.start);

        run_index = self.copy_range_internal(run_index, range.start, range.end, range.target);
      }

      return;
    }

    for range in ranges.iter_mut() {
      debug_assert!(range.end >= range.start);

      run_index = self.copy_range_internal(run_index, range.start, range.end, range.target);
    }
  }

  /// Blink's `CopyRangeInternal`.
  fn copy_range_internal(
    &self,
    mut run_index: usize,
    start_offset: u32,
    end_offset: u32,
    target: &mut Self,
  ) -> usize {
    target.is_applied_spacing |= self.is_applied_spacing;

    let index_diff: i64 = if target.num_characters == 0 {
      target.start_index = start_offset;
      0
    } else {
      i64::from(target.end_index()) - i64::from(start_offset.max(self.start_index()))
    };

    target.num_characters +=
      end_offset.min(self.end_index()) - start_offset.max(self.start_index());

    let target_run_size_before = target.runs.len();
    let mut should_merge = !target.runs.is_empty();
    let mut has_glyphs = false;

    while run_index < self.runs.len() {
      let run = &self.runs[run_index];
      let run_start = run.start_index;
      let run_end = run_start + run.num_characters;

      if start_offset < run_end && end_offset > run_start {
        let start = start_offset.saturating_sub(run_start);
        let end = end_offset.min(run_end) - run_start;

        debug_assert!(end > start);

        if let Some(mut sub_run) = run.create_sub_run(start, end) {
          sub_run.start_index = (i64::from(sub_run.start_index) + index_diff) as u32;
          target.width.set(target.width() + sub_run.width);
          has_glyphs |= sub_run.glyph_data.size() > 0;

          let last = target.runs.last_mut().filter(|_| should_merge);

          match last.and_then(|last| Some((last.merge_if_possible(&sub_run)?, last))) {
            Some((merged, last)) => *last = merged,
            None => target.runs.push(sub_run),
          }
        }

        should_merge = false;

        if (self.is_ltr() && end_offset <= run_end) || (self.is_rtl() && start_offset >= run_start)
        {
          break;
        }
      }

      run_index += 1;
    }

    if !has_glyphs {
      return run_index;
    }

    debug_assert_eq!(self.is_rtl(), target.is_rtl());

    if self.is_rtl() && target.runs.len() != target_run_size_before {
      target.reorder_rtl_runs(target_run_size_before);
    }

    target.has_vertical_offsets |= self.has_vertical_offsets;

    run_index
  }

  /// Blink's `SubRange`.
  pub(crate) fn sub_range(&self, start_offset: u32, end_offset: u32) -> Self {
    let mut sub_range = Self::new(0, 0, self.direction());

    self.copy_range(start_offset, end_offset, &mut sub_range);

    sub_range
  }

  /// Blink's `CopyAdjustedOffset`.
  pub(crate) fn copy_adjusted_offset(&self, start_index: u32) -> Self {
    let mut result = self.clone();

    *result.character_position.get_mut() = Vec::new();

    if start_index > result.start_index() {
      let delta = start_index - result.start_index();

      for run in &mut result.runs {
        run.start_index += delta;
      }
    } else {
      let delta = result.start_index() - start_index;

      for run in &mut result.runs {
        debug_assert!(run.start_index >= delta);

        run.start_index -= delta;
      }
    }

    result.start_index = start_index;

    result
  }

  /// Blink's `InsertRun` for a run already filled.
  pub(crate) fn insert_run(&mut self, run: ShapeResultRun) {
    let position = if run.is_ltr() {
      self
        .runs
        .partition_point(|existing| existing.start_index < run.start_index)
    } else {
      self
        .runs
        .partition_point(|existing| existing.start_index > run.start_index)
    };

    self.runs.insert(position, run);
  }

  /// Blink's `InsertRunForTesting`.
  #[cfg(test)]
  pub(crate) fn insert_run_for_testing(
    &mut self,
    start_index: u32,
    num_characters: u32,
    direction: TextDirection,
    safe_break_offsets: &[u16],
  ) -> &mut ShapeResultRun {
    let hb_direction = if direction == TextDirection::Ltr {
      HbDirection::LeftToRight
    } else {
      HbDirection::RightToLeft
    };
    let mut run = ShapeResultRun::new(
      None,
      hb_direction,
      script::COMMON,
      start_index,
      num_characters,
      num_characters,
    );

    for (i, glyph) in run.glyph_data.glyphs_mut().iter_mut().enumerate() {
      *glyph = HarfBuzzRunGlyphData::new(0, i as u32, SafeToBreak::Unsafe, TextRunLayoutUnit::ZERO);
    }

    for &offset in safe_break_offsets {
      run.glyph_data.glyphs_mut()[usize::from(offset)].set_safe_to_break_before(SafeToBreak::Safe);
    }

    if self.is_rtl() {
      run.glyph_data.reverse();
    }

    self.insert_run(run);

    let position = self
      .runs
      .iter()
      .position(|run| run.start_index == start_index);

    &mut self.runs[position.unwrap_or_default()]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Blink's `InsertConstantAdvanceRun`.
  fn insert_constant_advance_run(
    result: &mut ShapeResult,
    start_index: u32,
    num_characters: u32,
    advance: TextRunLayoutUnit,
  ) {
    let direction = result.direction();
    let run = result.insert_run_for_testing(start_index, num_characters, direction, &[]);

    for glyph in run.glyph_data.glyphs_mut() {
      glyph.advance = advance;
      glyph.set_safe_to_break_before(SafeToBreak::Safe);
    }

    run.width =
      InlineLayoutUnit::from_raw_value(i64::from(advance.raw_value()) * i64::from(num_characters))
        .to_float();
  }

  fn all_safe(num_characters: u16) -> Vec<u16> {
    (0..num_characters).collect()
  }

  #[test]
  fn add_unsafe_to_break_ltr() {
    let mut result = ShapeResult::new(0, 8, TextDirection::Ltr);

    result.insert_run_for_testing(0, 8, TextDirection::Ltr, &all_safe(8));

    let offsets = [2, 5];

    for offset in offsets {
      assert_eq!(result.next_safe_to_break_offset(offset), offset);
    }

    result.add_unsafe_to_break(&offsets);
    result.ensure_position_data();

    for offset in offsets {
      assert_ne!(result.next_safe_to_break_offset(offset), offset);
      assert_ne!(result.cached_next_safe_to_break_offset(offset), offset);
    }
  }

  #[test]
  fn add_unsafe_to_break_rtl() {
    let mut result = ShapeResult::new(0, 6, TextDirection::Rtl);

    result.insert_run_for_testing(0, 6, TextDirection::Rtl, &all_safe(6));

    let offsets = [2, 5];

    for offset in offsets {
      assert_eq!(result.next_safe_to_break_offset(offset), offset);
    }

    result.add_unsafe_to_break(&offsets);
    result.ensure_position_data();

    for offset in offsets {
      assert_ne!(result.next_safe_to_break_offset(offset), offset);
      assert_ne!(result.cached_next_safe_to_break_offset(offset), offset);
    }
  }

  #[test]
  fn add_unsafe_to_break_range() {
    let mut result = ShapeResult::new(1, 8, TextDirection::Ltr);

    result.insert_run_for_testing(1, 8, TextDirection::Ltr, &all_safe(8));

    let offsets = [2, 5, 7];

    for offset in offsets {
      assert_eq!(result.next_safe_to_break_offset(offset), offset);
    }

    result.add_unsafe_to_break(&offsets);
    result.ensure_position_data();

    for offset in offsets {
      assert_ne!(result.next_safe_to_break_offset(offset), offset);
      assert_ne!(result.cached_next_safe_to_break_offset(offset), offset);
    }
  }

  #[test]
  fn unsafe_breaks_prevent_position_compaction() {
    let mut result = ShapeResult::new(0, 4, TextDirection::Ltr);

    result.insert_run_for_testing(0, 4, TextDirection::Ltr, &[0, 2]);
    result.ensure_position_data();

    assert_eq!(result.cached_next_safe_to_break_offset(1), 2);
    assert_eq!(result.cached_previous_safe_to_break_offset(1), 0);

    let mut result = ShapeResult::new(0, 4, TextDirection::Ltr);

    result.insert_run_for_testing(0, 4, TextDirection::Ltr, &[1, 2, 3]);
    result.ensure_position_data();

    assert_eq!(result.cached_next_safe_to_break_offset(0), 1);
  }

  #[test]
  fn compact_position_data_rebuilds_full_table() {
    let mut result = ShapeResult::new(0, 8, TextDirection::Ltr);

    result.insert_run_for_testing(0, 8, TextDirection::Ltr, &all_safe(8));
    result.ensure_position_data();

    assert_eq!(result.character_position.borrow().len(), 8);

    for offset in 0..8 {
      let data = result.character_data(offset);

      assert_eq!(data.x_position, LayoutUnit::ZERO);
      assert!(data.is_cluster_base);
      assert!(data.safe_to_break_before);
    }
  }

  #[test]
  fn compact_runs_force_full_position_data() {
    let mut result = ShapeResult::new(0, 16, TextDirection::Ltr);

    insert_constant_advance_run(&mut result, 0, 16, TextRunLayoutUnit::from_int(10));
    result.ensure_position_data();

    assert_eq!(result.character_position.borrow().len(), 16);

    for offset in 0..16 {
      let data = result.character_data(offset);

      assert_eq!(data.x_position, LayoutUnit::from_int(offset as i32 * 10));
      assert!(data.is_cluster_base);
      assert!(data.safe_to_break_before);
    }
  }

  /// Two runs of four characters, ten pixels each.
  fn two_runs(direction: TextDirection) -> ShapeResult {
    let mut result = ShapeResult::new(0, 8, direction);

    insert_constant_advance_run(&mut result, 0, 4, TextRunLayoutUnit::from_int(10));
    insert_constant_advance_run(&mut result, 4, 4, TextRunLayoutUnit::from_int(10));
    result.width.set(80.0);

    result
  }

  fn run_ranges(result: &ShapeResult) -> Vec<(u32, u32)> {
    result
      .runs
      .iter()
      .map(|run| (run.start_index, run.num_characters))
      .collect()
  }

  #[test]
  fn sub_range_spans_runs_in_visual_order() {
    let ltr = two_runs(TextDirection::Ltr).sub_range(2, 6);
    let rtl = two_runs(TextDirection::Rtl).sub_range(2, 6);

    assert_eq!(
      (ltr.start_index(), ltr.num_characters(), ltr.width()),
      (2, 4, 40.0)
    );
    assert_eq!(run_ranges(&ltr), [(2, 2), (4, 2)]);
    assert_eq!(
      (rtl.start_index(), rtl.num_characters(), rtl.width()),
      (2, 4, 40.0)
    );
    assert_eq!(run_ranges(&rtl), [(4, 2), (2, 2)]);
  }

  #[test]
  fn copy_ranges_fill_each_target() {
    let result = two_runs(TextDirection::Rtl);
    let mut first = ShapeResult::new(0, 0, TextDirection::Rtl);
    let mut second = ShapeResult::new(0, 0, TextDirection::Rtl);

    result.copy_ranges(&mut [
      ShapeRange {
        start: 0,
        end: 3,
        target: &mut first,
      },
      ShapeRange {
        start: 3,
        end: 8,
        target: &mut second,
      },
    ]);

    assert_eq!(run_ranges(&first), [(0, 3)]);
    assert_eq!(run_ranges(&second), [(4, 4), (3, 1)]);
    assert_eq!(run_ranges(&first), run_ranges(&result.sub_range(0, 3)));
    assert_eq!(run_ranges(&second), run_ranges(&result.sub_range(3, 8)));
  }

  #[test]
  fn positions_and_offsets_agree_in_ltr() {
    let result = two_runs(TextDirection::Ltr);

    assert_eq!(
      result.position_for_offset(3, AdjustMidCluster::ToStart),
      30.0
    );
    assert_eq!(
      result.position_for_offset(8, AdjustMidCluster::ToStart),
      80.0
    );
    assert_eq!(result.offset_for_position(25.0), 2);
    assert_eq!(result.offset_for_position(45.0), 4);
    assert_eq!(result.offset_for_position(-1.0), 0);
    assert_eq!(result.offset_for_position(90.0), 8);

    result.ensure_position_data();

    assert_eq!(
      result.cached_offset_for_position(LayoutUnit::from_int(45)),
      4
    );
    assert_eq!(
      result.cached_position_for_offset(5),
      LayoutUnit::from_int(50)
    );
    assert_eq!(result.cached_width(2, 6), LayoutUnit::from_int(40));
  }

  #[test]
  fn previous_safe_to_break_offset_crosses_runs() {
    let mut result = ShapeResult::new(0, 8, TextDirection::Ltr);

    result.insert_run_for_testing(0, 4, TextDirection::Ltr, &[0]);
    result.insert_run_for_testing(4, 4, TextDirection::Ltr, &[2]);

    assert_eq!(result.previous_safe_to_break_offset(5), 4);
    assert_eq!(result.previous_safe_to_break_offset(7), 6);
    assert_eq!(result.next_safe_to_break_offset(1), 4);
    assert_eq!(result.next_safe_to_break_offset(5), 6);
  }

  #[test]
  fn graphemes_count_clusters_per_byte() {
    let text = "e\u{301}x";
    let mut result = ShapeResult::new(0, text.len() as u32, TextDirection::Ltr);

    result.insert_run_for_testing(0, text.len() as u32, TextDirection::Ltr, &[]);
    result.ensure_graphemes(text);

    assert_eq!(
      result.runs[0].glyph_data.graphemes(),
      Some(&[0, 0, 0, 1][..])
    );
  }
}

//! Blink's [`ShapingLineBreaker`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/shaping_line_breaker.cc).

use std::rc::Rc;

use super::{
  shape_result::ShapeResult,
  shape_result_view::{Segment, SegmentSource, ShapeResultView},
};
use crate::{
  layout_unit::LayoutUnit,
  text::{
    character::{SOFT_HYPHEN, char_at, char_before, is_other_space_separator},
    line_break_iterator::{BreakSpaceType, LineBreakIterator},
  },
};

/// Blink's `ShapingLineBreaker::Result`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ShapingLineBreakerResult {
  pub(crate) break_offset: usize,
  pub(crate) has_trailing_spaces: bool,
  /// No break opportunity fits.
  pub(crate) is_overflow: bool,
  /// The break is at a soft hyphen, whose glyph the result leaves out.
  pub(crate) is_hyphenated: bool,
}

/// Blink's `BreakOpportunity`.
#[derive(Debug, Clone, Copy, Default)]
struct BreakOpportunity {
  offset: usize,
  non_hangable_run_end: Option<usize>,
  is_hyphenated: bool,
}

impl BreakOpportunity {
  fn new(offset: usize, non_hangable_run_end: Option<usize>) -> Self {
    Self {
      offset,
      non_hangable_run_end,
      is_hyphenated: false,
    }
  }
}

/// Blink's `IsBreakableSpace`, with ideographic space.
fn is_breakable_space(ch: char) -> bool {
  LineBreakIterator::is_breakable_space(ch) || is_other_space_separator(ch)
}

/// Blink's `FindNonHangableEnd`: the start of the run of spaces ending with the one at
/// `candidate`.
fn find_non_hangable_end(text: &str, candidate: usize) -> usize {
  debug_assert!(is_breakable_space(char_at(text, candidate)));

  let mut non_hangable_end = candidate;

  while non_hangable_end > 0 {
    let previous = char_before(text, non_hangable_end);

    if !is_breakable_space(previous) {
      return non_hangable_end;
    }

    non_hangable_end -= previous.len_utf8();
  }

  non_hangable_end
}

/// Blink's `IsAllSpaces`.
fn is_all_spaces(text: &str, start: usize, end: usize) -> bool {
  text[start..end].chars().all(is_breakable_space)
}

/// Blink's `CheckBreakOffset`: a break must move forward, within the range.
fn check_break_offset(offset: usize, start: usize, end: usize) {
  assert!(offset > start);
  assert!(offset <= end);
}

/// Blink's `FlipRtl`: logical advances to visual ones, and back.
fn flip_rtl(is_ltr: bool, value: LayoutUnit) -> LayoutUnit {
  if is_ltr { value } else { -value }
}

/// Blink's `ShapingLineBreaker`, shaping again with `shape` where a line's edge is not safe to break.
pub(crate) struct ShapingLineBreaker<'a, S: FnMut(usize, usize) -> Rc<ShapeResult>> {
  result: &'a Rc<ShapeResult>,
  break_iterator: &'a LineBreakIterator<'a>,
  shape: S,
  line_start: usize,
  dont_reshape_end_if_at_space: bool,
  no_result_if_overflow: bool,
  is_after_forced_break: bool,
}

impl<'a, S: FnMut(usize, usize) -> Rc<ShapeResult>> ShapingLineBreaker<'a, S> {
  pub(crate) fn new(
    result: &'a Rc<ShapeResult>,
    break_iterator: &'a LineBreakIterator<'a>,
    shape: S,
  ) -> Self {
    Self {
      result,
      break_iterator,
      shape,
      line_start: 0,
      dont_reshape_end_if_at_space: false,
      no_result_if_overflow: false,
      is_after_forced_break: false,
    }
  }

  pub(crate) fn set_line_start(&mut self, offset: usize) {
    self.line_start = offset;
  }

  pub(crate) fn set_dont_reshape_end_if_at_space(&mut self) {
    self.dont_reshape_end_if_at_space = true;
  }

  pub(crate) fn no_result_if_overflow(&self) -> bool {
    self.no_result_if_overflow
  }

  pub(crate) fn set_no_result_if_overflow(&mut self) {
    self.no_result_if_overflow = true;
  }

  pub(crate) fn set_is_after_forced_break(&mut self, value: bool) {
    self.is_after_forced_break = value;
  }

  fn text(&self) -> &'a str {
    self.break_iterator.text()
  }

  /// Blink's `IsStartOfWrappedLine`.
  fn is_start_of_wrapped_line(&self, offset: usize) -> bool {
    offset != 0 && offset == self.line_start && !self.is_after_forced_break
  }

  /// Blink's `FirstSafeOffset`: reshaping starts a wrapped line up to its first safe break.
  fn first_safe_offset(&self, start: usize) -> usize {
    if !self.is_start_of_wrapped_line(start) {
      return start;
    }

    self.result.cached_next_safe_to_break_offset(start as u32) as usize
  }

  /// Blink's `PreviousBreakOpportunity`.
  fn previous_break_opportunity(&self, offset: usize, start: usize) -> BreakOpportunity {
    let text = self.text();
    let break_offset = self
      .break_iterator
      .previous_break_opportunity(offset, start);

    if break_offset > 0 && is_breakable_space(char_before(text, break_offset)) {
      let space = break_offset - char_before(text, break_offset).len_utf8();

      return BreakOpportunity::new(break_offset, Some(find_non_hangable_end(text, space)));
    }

    BreakOpportunity::new(break_offset, None)
  }

  /// Blink's `NextBreakOpportunity`.
  fn next_break_opportunity(&self, offset: usize, len: usize) -> BreakOpportunity {
    let text = self.text();
    let break_offset = self.break_iterator.next_break_opportunity(offset, len);

    if break_offset > 0 && is_breakable_space(char_before(text, break_offset)) {
      let space = break_offset - char_before(text, break_offset).len_utf8();

      return BreakOpportunity::new(break_offset, Some(find_non_hangable_end(text, space)));
    }

    BreakOpportunity::new(break_offset, None)
  }

  /// Blink's `SetBreakOffset`.
  fn set_break_offset(&self, break_offset: usize, result: &mut ShapingLineBreakerResult) {
    result.break_offset = break_offset;
    result.is_hyphenated = char_before(self.text(), break_offset) == SOFT_HYPHEN;
  }

  /// Blink's `SetBreakOffset` for a break opportunity.
  fn set_break_opportunity(
    &self,
    break_opportunity: &BreakOpportunity,
    result: &mut ShapingLineBreakerResult,
  ) {
    result.break_offset = break_opportunity.offset;
    result.is_hyphenated = break_opportunity.is_hyphenated
      || char_before(self.text(), break_opportunity.offset) == SOFT_HYPHEN;
  }

  /// Blink's `ShapeLine`: the line from `start` that fits `available_space`, breaking at the last
  /// opportunity that fits, and shaping its edges again where they are not safe to break. None
  /// when the line overflows and `no_result_if_overflow` is set.
  pub(crate) fn shape_line(
    &mut self,
    start: usize,
    mut available_space: LayoutUnit,
    result_out: &mut ShapingLineBreakerResult,
  ) -> Option<ShapeResultView> {
    debug_assert!(available_space >= LayoutUnit::ZERO);

    let result = self.result;
    let is_ltr = result.is_ltr();
    let range_start = result.start_index as usize;
    let range_end = result.end_index() as usize;

    debug_assert!(start >= range_start);
    debug_assert!(start < range_end);

    result_out.is_overflow = false;
    result_out.is_hyphenated = false;
    result_out.has_trailing_spaces = false;

    let text = self.text();

    // No break needed, before `ensure_position_data`.
    if start == range_start
      && available_space >= result.snapped_width()
      && result.is_start_safe_to_break()
    {
      self.set_break_offset(range_end, result_out);

      return Some(ShapeResultView::create_from_result(result));
    }

    // Breaking looks up offsets by position, from the position cache.
    result.ensure_position_data();

    let start_position = result.cached_position_for_offset((start - range_start) as u32);
    // The start up to the next safe break shapes again, if it is not safe itself.
    let mut line_start_result = None;
    let first_safe = self.first_safe_offset(start);

    debug_assert!(first_safe >= start);

    if first_safe != start {
      let first_safe_position =
        result.cached_position_for_offset((first_safe - range_start) as u32);
      let reshaped = (self.shape)(start, first_safe);
      let old_width = flip_rtl(is_ltr, first_safe_position - start_position);
      let diff = old_width - reshaped.snapped_width();

      if diff != LayoutUnit::ZERO {
        available_space = (available_space + diff).max(LayoutUnit::ZERO);
      }

      line_start_result = Some(reshaped);
    }

    // The candidate break is the last offset before the available space runs out; the break is the
    // last valid one before it.
    let end_position = start_position + flip_rtl(is_ltr, available_space);

    debug_assert!(flip_rtl(is_ltr, end_position - start_position) >= LayoutUnit::ZERO);

    let mut candidate_break =
      result.cached_offset_for_position(end_position) as usize + range_start;
    let mut last_safe = start;
    let mut line_end_result: Option<Rc<ShapeResult>> = None;

    if candidate_break >= range_end {
      // Too few glyphs to fill the available space.
      debug_assert_eq!(candidate_break, range_end);

      self.set_break_offset(range_end, result_out);

      return Some(self.shape_to_end(
        start,
        line_start_result.as_ref(),
        first_safe,
        range_start,
        range_end,
      ));
    }

    // Float rounding can put the candidate before the start.
    candidate_break = candidate_break.max(start);

    // In a run of trailing spaces, UAX #14 class BA, the break is after the run:
    // https://www.unicode.org/reports/tr14/#BA
    let mut break_opportunity;
    let is_break_after_any_space =
      self.break_iterator.break_space() == BreakSpaceType::AfterEverySpace;
    let use_previous_break_opportunity =
      !is_breakable_space(char_at(text, candidate_break)) || is_break_after_any_space;

    if use_previous_break_opportunity {
      break_opportunity = self.previous_break_opportunity(candidate_break, start);

      // Overflow without a break opportunity before the candidate: take the next one.
      result_out.is_overflow = break_opportunity.offset <= start;

      if result_out.is_overflow {
        if self.no_result_if_overflow {
          return None;
        }

        break_opportunity = self.next_break_opportunity(
          candidate_break.max(next_character_offset(self.text(), start)),
          range_end,
        );
      }
    } else {
      break_opportunity = self.next_break_opportunity(
        candidate_break.max(next_character_offset(self.text(), start)),
        range_end,
      );

      debug_assert!(break_opportunity.offset > start);

      // A next opportunity whose spaces do not reach the candidate overflows, unless one before
      // the candidate fits.
      if break_opportunity.offset > candidate_break
        && break_opportunity
          .non_hangable_run_end
          .is_none_or(|end| end > candidate_break)
      {
        let previous_opportunity = self.previous_break_opportunity(candidate_break, start);

        if previous_opportunity.offset > start {
          break_opportunity = previous_opportunity;
        } else {
          result_out.is_overflow = true;

          if self.no_result_if_overflow {
            return None;
          }
        }
      }

      debug_assert!(!is_break_after_any_space);

      if break_opportunity
        .non_hangable_run_end
        .is_some_and(|end| end <= start)
      {
        result_out.has_trailing_spaces = true;
        result_out.break_offset = range_end.min(break_opportunity.offset);
        result_out.is_hyphenated = false;

        debug_assert!(is_all_spaces(text, start, result_out.break_offset));

        return Some(ShapeResultView::create_from_result_range(
          result,
          start as u32,
          result_out.break_offset as u32,
        ));
      }
    }

    let mut reshape_line_end = true;

    // Nothing past `range_end` can be measured.
    if break_opportunity.offset >= range_end {
      self.set_break_offset(range_end, result_out);

      if result_out.is_overflow {
        return Some(self.shape_to_end(
          start,
          line_start_result.as_ref(),
          first_safe,
          range_start,
          range_end,
        ));
      }

      break_opportunity.offset = range_end;
      // `range_end` may end a `<span>` before the opportunity, so the end needs no reshape.
      reshape_line_end = false;

      if break_opportunity
        .non_hangable_run_end
        .is_some_and(|end| range_end < end)
      {
        break_opportunity.non_hangable_run_end = None;
      }

      let last = char_before(text, range_end);

      if is_breakable_space(last) {
        break_opportunity.non_hangable_run_end =
          Some(find_non_hangable_end(text, range_end - last.len_utf8()));
      }
    }

    // Decided before the non-hangable run end moves the offset.
    if self.dont_reshape_end_if_at_space && reshape_line_end {
      reshape_line_end = !is_breakable_space(char_before(text, break_opportunity.offset));
    }

    if !is_break_after_any_space && let Some(end) = break_opportunity.non_hangable_run_end {
      break_opportunity.offset = next_character_offset(text, start).max(end);
    }

    check_break_offset(break_opportunity.offset, start, range_end);

    // Without a safe break between the start and the break, the whole range shapes again.
    if first_safe >= break_opportunity.offset {
      debug_assert_ne!(first_safe, start);

      self.set_break_opportunity(&break_opportunity, result_out);
      check_break_offset(result_out.break_offset, start, range_end);

      return Some(ShapeResultView::create_from_result(&(self.shape)(
        start,
        break_opportunity.offset,
      )));
    }

    if reshape_line_end {
      // From the last safe break, the line end shapes again; a line that no longer fits tries
      // the break before.
      loop {
        if !is_break_after_any_space && let Some(end) = break_opportunity.non_hangable_run_end {
          break_opportunity.offset = next_character_offset(text, start).max(end);
        }

        last_safe =
          result.cached_previous_safe_to_break_offset(break_opportunity.offset as u32) as usize;

        if last_safe == break_opportunity.offset {
          break;
        }

        debug_assert!(last_safe < break_opportunity.offset);

        // So far back the whole line shapes again.
        if last_safe < first_safe {
          last_safe = start;
          line_start_result = None;
        }

        if result_out.is_overflow {
          line_end_result = Some((self.shape)(last_safe, break_opportunity.offset));

          break;
        }

        let safe_position = result.cached_position_for_offset((last_safe - range_start) as u32);
        let reshaped = (self.shape)(last_safe, break_opportunity.offset);

        if reshaped.width() <= flip_rtl(is_ltr, end_position - safe_position).to_float() {
          line_end_result = Some(reshaped);

          break;
        }

        break_opportunity = self.previous_break_opportunity(
          break_opportunity.offset - char_before(self.text(), break_opportunity.offset).len_utf8(),
          start,
        );

        if break_opportunity.offset > start {
          continue;
        }

        // Nothing fits, so the line overflows; past the candidate suits ligatures, before it
        // kerning.
        result_out.is_overflow = true;
        break_opportunity = self.previous_break_opportunity(candidate_break, start);

        if break_opportunity.offset <= start {
          break_opportunity = self.next_break_opportunity(
            candidate_break.max(next_character_offset(self.text(), start)),
            range_end,
          );

          if break_opportunity.offset >= range_end {
            self.set_break_offset(range_end, result_out);

            return Some(self.shape_to_end(
              start,
              line_start_result.as_ref(),
              first_safe,
              range_start,
              range_end,
            ));
          }
        }
      }
    }

    if line_end_result.is_none() {
      last_safe = break_opportunity.offset;

      debug_assert!(last_safe > start);
    }

    // Moving forward keeps callers out of an infinite loop.
    check_break_offset(break_opportunity.offset, start, range_end);
    self.set_break_opportunity(&break_opportunity, result_out);

    Some(Self::concat_shape_results(
      result,
      start,
      break_opportunity.offset,
      first_safe,
      last_safe,
      line_start_result.as_ref(),
      line_end_result.as_ref(),
    ))
  }

  /// Blink's `ConcatShapeResults`: the reshaped start, the result between the safe breaks, and
  /// the reshaped end.
  fn concat_shape_results(
    result: &Rc<ShapeResult>,
    start: usize,
    end: usize,
    first_safe: usize,
    last_safe: usize,
    line_start_result: Option<&Rc<ShapeResult>>,
    line_end_result: Option<&Rc<ShapeResult>>,
  ) -> ShapeResultView {
    let mut segments = Vec::with_capacity(3);

    if let Some(line_start_result) = line_start_result {
      segments.push(Segment {
        source: SegmentSource::Result(line_start_result),
        start_index: 0,
        end_index: u32::MAX,
      });
    }

    if last_safe > first_safe {
      segments.push(Segment {
        source: SegmentSource::Result(result),
        start_index: first_safe as u32,
        end_index: last_safe as u32,
      });
    }

    if let Some(line_end_result) = line_end_result {
      segments.push(Segment {
        source: SegmentSource::Result(line_end_result),
        start_index: last_safe as u32,
        end_index: u32::MAX,
      });
    }

    let line_result = ShapeResultView::create(&segments);

    debug_assert_eq!((end - start) as u32, line_result.num_characters);

    line_result
  }

  /// Blink's `ShapeToEnd`: the result from `start` on, its start shaped again if not safe.
  fn shape_to_end(
    &self,
    start: usize,
    line_start_result: Option<&Rc<ShapeResult>>,
    first_safe: usize,
    range_start: usize,
    range_end: usize,
  ) -> ShapeResultView {
    let result = self.result;

    let Some(line_start_result) = line_start_result else {
      debug_assert_eq!(first_safe, start);

      if start == range_start {
        return ShapeResultView::create_from_result(result);
      }

      return ShapeResultView::create_from_result_range(result, start as u32, range_end as u32);
    };

    // No safe break in range: the whole range shapes again.
    if first_safe >= range_end {
      return ShapeResultView::create_from_result_range(
        line_start_result,
        start as u32,
        range_end as u32,
      );
    }

    ShapeResultView::create(&[
      Segment {
        source: SegmentSource::Result(line_start_result),
        start_index: 0,
        end_index: u32::MAX,
      },
      Segment {
        source: SegmentSource::Result(result),
        start_index: first_safe as u32,
        end_index: range_end as u32,
      },
    ])
  }

  /// Blink's `ShapeLineAt`: the line from `start` to `end`, its edges shaped again if not safe.
  pub(crate) fn shape_line_at(&mut self, start: usize, end: usize) -> ShapeResultView {
    debug_assert!(end > start);

    let result = self.result;

    result.ensure_position_data();

    let first_safe = self.first_safe_offset(start);

    debug_assert!(first_safe >= start);

    let mut line_start_result = None;

    if first_safe != start {
      if first_safe >= end {
        return ShapeResultView::create_from_result(&(self.shape)(start, end));
      }

      line_start_result = Some((self.shape)(start, first_safe));
    }

    let last_safe;
    let mut line_end_result = None;

    if self.dont_reshape_end_if_at_space && is_breakable_space(char_before(self.text(), end)) {
      last_safe = end;
    } else {
      last_safe = result.cached_previous_safe_to_break_offset(end as u32) as usize;

      debug_assert!(last_safe >= first_safe);

      if last_safe != end {
        line_end_result = Some((self.shape)(last_safe, end));
      }
    }

    Self::concat_shape_results(
      result,
      start,
      end,
      first_safe,
      last_safe,
      line_start_result.as_ref(),
      line_end_result.as_ref(),
    )
  }
}

/// The offset past the character at `offset`: Blink's `offset + 1` for a UTF-16 code unit.
fn next_character_offset(text: &str, offset: usize) -> usize {
  offset + char_at(text, offset).len_utf8().max(1)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    fonts::{
      font::Font,
      shaping::{
        harfbuzz_shaper::HarfBuzzShaper,
        testing::{geist, geist_mono},
      },
    },
    resources::font::Fonts,
    style::Direction as TextDirection,
    text::line_break_iterator::LineBreakType,
  };

  struct Fixture<'t> {
    shaper: HarfBuzzShaper<'t>,
    font: Font,
    result: Rc<ShapeResult>,
  }

  impl<'t> Fixture<'t> {
    fn new(text: &'t str) -> (Fonts, Self) {
      Self::with_font(text, geist())
    }

    fn with_font(text: &'t str, (fonts, font): (Fonts, Font)) -> (Fonts, Self) {
      let shaper = HarfBuzzShaper::new(text);
      let result = Rc::new(shaper.shape_all(&font, TextDirection::Ltr));

      (
        fonts,
        Self {
          shaper,
          font,
          result,
        },
      )
    }

    fn shape(&self, start: usize, end: usize) -> ShapeResult {
      self
        .shaper
        .shape(&self.font, TextDirection::Ltr, start, end)
    }

    fn breaker<'a>(
      &'a self,
      result: &'a Rc<ShapeResult>,
      break_iterator: &'a LineBreakIterator<'a>,
    ) -> ShapingLineBreaker<'a, impl FnMut(usize, usize) -> Rc<ShapeResult> + 'a> {
      ShapingLineBreaker::new(result, break_iterator, move |start, end| {
        Rc::new(self.shape(start, end))
      })
    }
  }

  fn iterator(text: &str, break_type: LineBreakType) -> LineBreakIterator<'_> {
    LineBreakIterator::new(text, break_type, BreakSpaceType::AfterSpaceRun)
  }

  /// The break offset and the line, from `start` in `available_space`.
  fn shape_line<S: FnMut(usize, usize) -> Rc<ShapeResult>>(
    breaker: &mut ShapingLineBreaker<'_, S>,
    start: usize,
    available_space: LayoutUnit,
  ) -> (usize, ShapeResultView) {
    let mut result = ShapingLineBreakerResult::default();
    let line = breaker
      .shape_line(start, available_space, &mut result)
      .unwrap();

    (result.break_offset, line)
  }

  #[test]
  fn shape_line_latin() {
    let text = "Test run with multiple words and breaking opportunities.";
    let (_fonts, fixture) = Fixture::new(text);
    let break_iterator = iterator(text, LineBreakType::Normal);
    // "Test run with multiple", "Test run with", "Test run", "Test"
    let first4 = fixture.shape(0, 22).snapped_width();
    let first3 = fixture.shape(0, 13).snapped_width();
    let first2 = fixture.shape(0, 8).snapped_width();
    let first1 = fixture.shape(0, 4).snapped_width();
    let full = fixture.result.snapped_width();

    assert!(first4 < full && first3 < first4 && first2 < first3 && first1 < first2);

    let mut breaker = fixture.breaker(&fixture.result, &break_iterator);
    let one = LayoutUnit::from_int(1);
    let cases = [
      (full, 56, full),
      (first4, 22, first4),
      (first4 + LayoutUnit::from_int(10), 22, first4),
      (first4 - one, 13, first3),
      (first3, 13, first3),
      (first3 - one, 8, first2),
      (first2, 8, first2),
      (first2 - one, 4, first1),
      (first1, 4, first1),
      // No earlier break.
      (first1 - one, 4, first1),
    ];

    for (available_space, break_offset, width) in cases {
      let (offset, line) = shape_line(&mut breaker, 0, available_space);

      assert_eq!((offset, line.snapped_width()), (break_offset, width));
    }
  }

  /// A line too narrow for its first character breaks after it, a whole multi-byte character.
  #[test]
  fn shape_line_breaks_after_a_whole_character() {
    let text = "\u{3000}\u{3000}a b";
    let (_fonts, fixture) = Fixture::new(text);
    let break_iterator = iterator(text, LineBreakType::Normal);
    let mut breaker = fixture.breaker(&fixture.result, &break_iterator);

    for available in [0, 1, 4] {
      let (offset, _) = shape_line(&mut breaker, 0, LayoutUnit::from_int(available));

      assert!(text.is_char_boundary(offset), "{available}: {offset}");
    }
  }

  #[test]
  fn shape_line_latin_multi_line() {
    let text = "Line breaking test case.";
    let (_fonts, fixture) = Fixture::new(text);
    let break_iterator = iterator(text, LineBreakType::Normal);
    let first = fixture.shape(0, 4).snapped_width();
    let mid_third = fixture.shape(0, 16).snapped_width();
    let full = fixture.result.snapped_width();
    let mut breaker = fixture.breaker(&fixture.result, &break_iterator);

    assert_eq!(
      shape_line(&mut breaker, 0, full - LayoutUnit::from_int(1)).0,
      18
    );
    assert_eq!(shape_line(&mut breaker, 0, first).0, 4);
    assert_eq!(shape_line(&mut breaker, 0, mid_third).0, 13);
    assert_eq!(shape_line(&mut breaker, 13, mid_third).0, 24);
  }

  #[test]
  fn shape_line_latin_break_all() {
    let text = "Testing break type-break all.";
    // Geist kerns "y" before "p", which Blink's test font does not.
    let (_fonts, fixture) = Fixture::with_font(text, geist_mono());
    let break_iterator = iterator(text, LineBreakType::BreakAll);
    let midpoint = fixture.shape(0, 16).snapped_width();
    let full = fixture.result.snapped_width();
    let mut breaker = fixture.breaker(&fixture.result, &break_iterator);
    let (offset, line) = shape_line(&mut breaker, 0, midpoint);

    assert_eq!((offset, line.snapped_width()), (16, midpoint));

    let (offset, line) = shape_line(&mut breaker, 16, full);

    assert_eq!(offset, 29);
    assert!(midpoint >= line.snapped_width());
  }

  #[test]
  fn shape_line_zero_available_width() {
    let text = "Testing overflow line break.";
    let (_fonts, fixture) = Fixture::new(text);
    let break_iterator = iterator(text, LineBreakType::Normal);
    let mut breaker = fixture.breaker(&fixture.result, &break_iterator);

    for (start, break_offset) in [(0, 7), (8, 16), (17, 21), (22, 28)] {
      assert_eq!(
        shape_line(&mut breaker, start, LayoutUnit::ZERO).0,
        break_offset
      );
    }
  }

  #[test]
  fn shape_line_range_end_mid_word() {
    let text = "Mid word";
    let (_fonts, fixture) = Fixture::new(text);
    let break_iterator = iterator(text, LineBreakType::Normal);
    let result = Rc::new(fixture.shape(0, 2));
    let mut breaker = fixture.breaker(&result, &break_iterator);
    let (offset, line) = shape_line(&mut breaker, 0, LayoutUnit::MAX);

    assert_eq!((offset, line.width), (2, result.width()));
  }

  #[test]
  fn break_opportunities() {
    // Blink's positions in UTF-16 code units, here in bytes: U+00AD takes two.
    let cases: [(&str, &[usize]); 5] = [
      ("x y z", &[2, 4, 5]),
      ("y\u{AD}z", &[3, 4]),
      ("\u{AD}z", &[2, 3]),
      ("y\u{AD}", &[3]),
      ("\u{AD}\u{AD}z", &[4, 5]),
    ];

    for (text, expected) in cases {
      let (_fonts, fixture) = Fixture::new(text);
      let break_iterator = iterator(text, LineBreakType::Normal);
      let breaker = fixture.breaker(&fixture.result, &break_iterator);
      let mut next = Vec::new();

      for i in 0..=text.len() {
        let offset = breaker.next_break_opportunity(i, text.len()).offset;

        if next.last() != Some(&offset) {
          next.push(offset);
        }
      }

      let mut previous = Vec::new();

      for i in (1..=text.len()).rev() {
        let offset = breaker.previous_break_opportunity(i, 0).offset;

        if offset != 0 && previous.last() != Some(&offset) {
          previous.push(offset);
        }
      }

      previous.reverse();

      assert_eq!(next, expected, "next {text:?}");
      assert_eq!(previous, expected, "previous {text:?}");
    }
  }
}

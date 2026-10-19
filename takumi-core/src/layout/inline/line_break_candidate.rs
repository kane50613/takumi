//! Blink's [`LineBreakCandidate`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_break_candidate.cc).

use super::{
  inline_item::{InlineItem, InlineItemType},
  inline_item_result::InlineItemResult,
  inline_item_text_index::InlineItemTextIndex,
  line_break_point::LineBreakPoint,
  line_breaker::LineBreaker,
  line_info::LineInfo,
};
use crate::layout_unit::LayoutUnit;

/// Blink's `LineBreakCandidate`: a break opportunity, with the line's end position if it breaks
/// there and if it does not.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LineBreakCandidate {
  pub(crate) point: LineBreakPoint,
  pub(crate) pos_no_break: f32,
  pub(crate) pos_if_break: f32,
  pub(crate) penalty: f32,
}

impl LineBreakCandidate {
  fn new(
    offset: InlineItemTextIndex,
    end: InlineItemTextIndex,
    pos_no_break: f32,
    pos_if_break: f32,
    penalty: f32,
    is_hyphenated: bool,
  ) -> Self {
    Self {
      point: LineBreakPoint::new(offset, end, is_hyphenated),
      pos_no_break,
      pos_if_break,
      penalty,
    }
  }
}

/// Blink's `LineBreakCandidateContext::State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateState {
  Break,
  MidWord,
}

/// Blink's `LineBreakCandidateContext`: collects a paragraph's break candidates from its lines.
pub(crate) struct LineBreakCandidateContext<'r, 'a, 'c> {
  pub(crate) candidates: &'r mut Vec<LineBreakCandidate>,
  position_no_snap: f32,
  pub(crate) state: CandidateState,
  pub(crate) last_item: Option<&'a InlineItem<'c>>,
  pub(crate) last_end_offset: usize,
  pub(crate) hyphen_penalty: f32,
}

impl<'r, 'a, 'c> LineBreakCandidateContext<'r, 'a, 'c> {
  pub(crate) fn new(candidates: &'r mut Vec<LineBreakCandidate>) -> Self {
    Self {
      candidates,
      position_no_snap: 0.0,
      state: CandidateState::Break,
      last_item: None,
      last_end_offset: 0,
      hyphen_penalty: 0.0,
    }
  }

  /// Blink's `Position`.
  pub(crate) fn position(&self) -> f32 {
    self.position_no_snap
  }

  /// Blink's `SnappedPosition`.
  fn snapped_position(&self) -> LayoutUnit {
    LayoutUnit::from_float_ceil(self.position_no_snap)
  }

  /// Blink's `SetLast`.
  pub(crate) fn set_last(&mut self, item: &'a InlineItem<'c>, offset: usize) {
    self.last_item = Some(item);
    self.last_end_offset = offset;
  }

  /// Blink's `Append`: a candidate, or, mid-word, a change to the last one.
  #[expect(clippy::too_many_arguments, reason = "Blink's `Append` takes these")]
  pub(crate) fn append(
    &mut self,
    new_state: CandidateState,
    offset: InlineItemTextIndex,
    end: InlineItemTextIndex,
    pos_no_break: f32,
    pos_if_break: f32,
    penalty: f32,
    is_hyphenated: bool,
  ) {
    let candidate = LineBreakCandidate::new(
      offset,
      end,
      pos_no_break,
      pos_if_break,
      penalty,
      is_hyphenated,
    );

    match (self.state, self.candidates.last_mut()) {
      (CandidateState::MidWord, Some(last_candidate)) => *last_candidate = candidate,
      _ => self.candidates.push(candidate),
    }

    self.position_no_snap = pos_no_break;
    self.state = new_state;
  }

  /// Blink's `Append` of a candidate at `position` whether it breaks or not.
  pub(crate) fn append_at(
    &mut self,
    new_state: CandidateState,
    offset: InlineItemTextIndex,
    position: f32,
  ) {
    self.append(new_state, offset, offset, position, position, 0.0, false);
  }

  /// Blink's `AppendTrailingSpaces`: trailing spaces move the last candidate's offset and its
  /// position without a break, not its position with one.
  pub(crate) fn append_trailing_spaces(
    &mut self,
    new_state: CandidateState,
    offset: InlineItemTextIndex,
    pos_no_break: f32,
  ) {
    if let Some(last_candidate) = self.candidates.last_mut() {
      debug_assert!(offset.is_at_least(last_candidate.point.offset));

      last_candidate.point.offset = offset;
      last_candidate.pos_no_break = pos_no_break;
    }

    self.position_no_snap = pos_no_break;
    self.state = new_state;
  }

  /// Blink's `AppendLine`: the candidates of a line's results. Returns false for a negative inline
  /// size, which breaks the candidates' monotonic positions.
  pub(crate) fn append_line(
    &mut self,
    line_info: &mut LineInfo<'a, 'c>,
    line_breaker: &mut LineBreaker<'a, '_, 'c>,
  ) -> bool {
    // The line breaker leaves some lines' last `can_break_after` unset, as only rewinding reads
    // it.
    if let Some(last_item_result) = line_info
      .results
      .iter_mut()
      .rev()
      .find(|result| result.item.item_type != InlineItemType::OutOfFlowPositioned)
    {
      last_item_result.can_break_after = true;
    }

    for index in 0..line_info.results.len() {
      let item_result: &InlineItemResult<'a, 'c> = &line_info.results[index];

      if item_result.inline_size < LayoutUnit::ZERO {
        return false;
      }

      let item = item_result.item;

      match item.item_type {
        InlineItemType::Text => line_breaker.append_candidates(index, line_info, self),
        InlineItemType::Control => {
          let new_state = if item_result.can_break_after {
            CandidateState::Break
          } else {
            CandidateState::MidWord
          };

          self.append_trailing_spaces(
            new_state,
            InlineItemTextIndex::new(item_result.item_index, item_result.end_offset()),
            (self.snapped_position() + item_result.inline_size).to_float(),
          );
          self.set_last(item, item_result.end_offset());
        }
        InlineItemType::OutOfFlowPositioned => {}
        _ => {
          let new_state = if item_result.can_break_after {
            CandidateState::Break
          } else if self.state == CandidateState::Break {
            CandidateState::MidWord
          } else {
            self.state
          };
          let offset =
            InlineItemTextIndex::new(item_result.item_index + 1, item_result.end_offset());
          let end_position = (self.snapped_position() + item_result.inline_size).to_float();

          // Opaque items, such as tags, leave the position with a break, as trailing spaces do.
          match self
            .candidates
            .last()
            .copied()
            .filter(|_| item.length() == 0)
          {
            Some(last_candidate) => self.append(
              new_state,
              offset,
              last_candidate.point.end,
              end_position,
              last_candidate.pos_if_break,
              0.0,
              false,
            ),
            None => self.append_at(new_state, offset, end_position),
          }

          self.set_last(item, item_result.end_offset());
        }
      }
    }

    true
  }

  /// Blink's `EnsureFirstSentinel`.
  pub(crate) fn ensure_first_sentinel(&mut self, first_line_info: &LineInfo<'_, '_>) {
    debug_assert!(self.candidates.is_empty());

    if let Some(first_item_result) = first_line_info.results.first() {
      let start = first_item_result.start();

      self
        .candidates
        .push(LineBreakCandidate::new(start, start, 0.0, 0.0, 0.0, false));
    }
  }

  /// Blink's `EnsureLastSentinel`, which only checks the candidates.
  pub(crate) fn ensure_last_sentinel(&self) {
    debug_assert_eq!(self.state, CandidateState::Break);
    debug_assert!(self.candidates.len() >= 2);
  }
}

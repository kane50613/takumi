//! Blink's [`ScoreLineBreaker`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/score_line_breaker.cc)
//! and [`ScoreLineBreakContext`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/score_line_break_context.h),
//! for `text-wrap: pretty` and `balance`, without hyphenation dictionaries, `line-clamp`, or
//! reusing the lines it breaks; layout breaks them again at the points it finds.

use super::{
  inline_break_token::InlineBreakToken,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  leading_floats::LeadingFloats,
  line_break_candidate::{LineBreakCandidate, LineBreakCandidateContext},
  line_break_point::LineBreakPoint,
  line_breaker::{AtomicInlineLayout, LineBreaker, LineBreakerMode},
  line_info::LineInfo,
  line_widths::LineWidths,
};
use crate::{
  context::RenderContext,
  layout::{
    constraint_space::ConstraintSpace,
    exclusions::{exclusion_space::ExclusionSpace, layout_opportunity::LineLayoutOpportunity},
  },
  layout_unit::LayoutUnit,
  style::TextAlign,
};

/// Blink's `kMaxLinesForBalance`.
pub(crate) const MAX_LINES_FOR_BALANCE: usize = 6;
/// Blink's `kMaxLinesForOptimal`.
pub(crate) const MAX_LINES_FOR_OPTIMAL: usize = 4;

const SCORE_INFINITY: f32 = f32::MAX;
const SCORE_OVERFULL: f32 = 1e12;
const LAST_LINE_PENALTY_MULTIPLIER: f32 = 4.0;

/// Blink's `ScoreLineBreakContext`: the break points a paragraph's lines take, one per line.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScoreLineBreakContext {
  pub(crate) line_break_points: Vec<LineBreakPoint>,
  line_break_points_index: usize,
  is_suspended: bool,
}

impl ScoreLineBreakContext {
  /// Blink's `CurrentLineBreakPoint`.
  pub(crate) fn current_line_break_point(&self) -> Option<&LineBreakPoint> {
    self.line_break_points.get(self.line_break_points_index)
  }

  /// Blink's `IsActive`: no break points yet, and not waiting for the paragraph's end.
  pub(crate) fn is_active(&self) -> bool {
    self.line_break_points.is_empty() && !self.is_suspended
  }

  /// Blink's `SuspendUntilEndParagraph`.
  pub(crate) fn suspend_until_end_paragraph(&mut self) {
    self.is_suspended = true;
  }

  /// Blink's `DidCreateLine`.
  pub(crate) fn did_create_line(&mut self, is_end_paragraph: bool) {
    if self.is_suspended && is_end_paragraph {
      self.is_suspended = false;
    }

    if self.line_break_points.is_empty() {
      return;
    }

    self.line_break_points_index += 1;

    if self.line_break_points_index >= self.line_break_points.len() {
      self.line_break_points.clear();
      self.line_break_points_index = 0;
    }
  }
}

/// Blink's `LineBreakScore`.
#[derive(Debug, Clone, Copy)]
struct LineBreakScore {
  /// The best score found for this break.
  score: f32,
  /// The break before it.
  prev_index: usize,
  /// The line it ends.
  line_index: usize,
}

/// Blink's `ScoreLineBreaker`: line breaks by penalties and scores, as Knuth's TeX does, after
/// Android's `LineBreak.Strategy.HighQuality`.
/// <https://cs.android.com/android/platform/superproject/+/master:frameworks/minikin/libs/minikin/OptimalLineBreaker.cpp>
pub(crate) struct ScoreLineBreaker<'s, 'a, 'c> {
  node: InlineNode<'c>,
  items_data: &'a InlineNodeData<'c>,
  space: &'s ConstraintSpace,
  break_token: Option<InlineBreakToken<'c>>,
  exclusion_space: &'s mut ExclusionSpace,
  atomic_inline: &'s mut dyn AtomicInlineLayout<'c>,
  parameters: ScoreParameters,
}

/// What Blink's `ScoreLineBreaker` scores the break candidates by.
#[derive(Debug, Clone, Copy)]
struct ScoreParameters {
  line_widths: LineWidths,
  is_balanced: bool,
  first_line_indent: LayoutUnit,
  hyphen_penalty: f32,
  line_penalty: f32,
  zoom: f32,
  is_justified: bool,
}

impl<'s, 'a, 'c> ScoreLineBreaker<'s, 'a, 'c> {
  pub(crate) fn new(
    node: InlineNode<'c>,
    items_data: &'a InlineNodeData<'c>,
    space: &'s ConstraintSpace,
    line_widths: LineWidths,
    break_token: Option<InlineBreakToken<'c>>,
    exclusion_space: &'s mut ExclusionSpace,
    atomic_inline: &'s mut dyn AtomicInlineLayout<'c>,
  ) -> Self {
    debug_assert!(!items_data.is_score_line_break_disabled);

    Self {
      node,
      items_data,
      space,
      break_token,
      exclusion_space,
      atomic_inline,
      parameters: ScoreParameters {
        line_widths,
        is_balanced: false,
        first_line_indent: LayoutUnit::ZERO,
        hyphen_penalty: 0.0,
        line_penalty: 0.0,
        zoom: 1.0,
        is_justified: false,
      },
    }
  }

  /// Blink's `MaxLines`.
  fn max_lines(&self) -> usize {
    if self.parameters.is_balanced {
      MAX_LINES_FOR_BALANCE
    } else {
      MAX_LINES_FOR_OPTIMAL
    }
  }

  /// Blink's `BalanceBreakPoints`.
  pub(crate) fn balance_break_points(
    &mut self,
    leading_floats: &LeadingFloats,
    context: &mut ScoreLineBreakContext,
  ) {
    self.parameters.is_balanced = true;
    self.optimal_break_points(leading_floats, context);
  }

  /// Blink's `OptimalBreakPoints`: breaks the paragraph greedily, then, when that leaves its last
  /// line short, finds the break points with the best score.
  pub(crate) fn optimal_break_points(
    &mut self,
    leading_floats: &LeadingFloats,
    context: &mut ScoreLineBreakContext,
  ) {
    debug_assert!(context.line_break_points.is_empty());
    debug_assert!(context.is_active());

    let max_lines = self.max_lines();
    let mut line_index = 0;
    let mut line_width = self.parameters.line_widths[line_index];
    let mut line_info_list: Vec<LineInfo<'a, 'c>> = Vec::new();
    let mut line_breaker = LineBreaker::new(
      self.node,
      self.items_data,
      LineBreakerMode::Content,
      self.space.clone(),
      LineLayoutOpportunity::new(line_width),
      leading_floats,
      self.break_token,
      &mut *self.atomic_inline,
      &mut *self.exclusion_space,
    );

    loop {
      let mut line_info = LineInfo::new(self.items_data, &self.node.block.context);

      line_breaker.next_line(&mut line_info);

      if line_breaker.disable_score_line_break {
        context.suspend_until_end_paragraph();

        return;
      }

      let is_end_paragraph = line_info.is_end_paragraph();

      line_info_list.push(line_info);

      // The clamp ends the paragraph too.
      if is_end_paragraph
        || self
          .space
          .lines_until_clamp
          .is_some_and(|lines| lines > 0 && line_info_list.len() == lines as usize)
      {
        context.suspend_until_end_paragraph();

        break;
      }

      if line_info_list.len() >= max_lines {
        return;
      }

      line_index += 1;

      let next_line_width = self.parameters.line_widths[line_index];

      if next_line_width != line_width {
        line_width = next_line_width;
        line_breaker.set_line_opportunity(LineLayoutOpportunity::new(line_width));
      }
    }

    // Now `line_info_list` holds a paragraph: the block, or a part of it a forced break ends.
    if line_info_list.len() <= 1 {
      return;
    }

    if !self.parameters.is_balanced && !should_optimize(&line_info_list, &mut line_breaker) {
      return;
    }

    let mut break_points = Vec::new();

    self.parameters.setup(&self.node.block.context);

    if self
      .parameters
      .optimize(&mut line_info_list, &mut line_breaker, &mut break_points)
    {
      context.line_break_points = break_points;
    }
  }
}

impl ScoreParameters {
  /// Blink's `Optimize`.
  fn optimize<'a, 'c>(
    &mut self,
    line_info_list: &mut [LineInfo<'a, 'c>],
    line_breaker: &mut LineBreaker<'a, '_, 'c>,
    break_points: &mut Vec<LineBreakPoint>,
  ) -> bool {
    debug_assert!(break_points.is_empty());

    let mut candidates = Vec::new();

    if !self.compute_candidates(line_info_list, line_breaker, &mut candidates) {
      return false;
    }

    // Sentinels at the front and the back leave fewer than three break opportunities nothing to
    // optimize.
    const MIN_CANDIDATES: usize = 3;

    if candidates.len() < MIN_CANDIDATES + 2 {
      return false;
    }

    // Typographic orphans cost more: the last opportunity, and the hyphenated ones before it.
    const ORPHANS_PENALTY: f32 = 10000.0;

    let orphans_penalty = ORPHANS_PENALTY * self.zoom;
    let last = candidates.len() - 1;

    for candidate in candidates[..last].iter_mut().rev() {
      candidate.penalty += orphans_penalty;

      if !candidate.point.is_hyphenated {
        break;
      }
    }

    self.first_line_indent = line_info_list[0].text_indent;

    let scores = self.compute_scores(&candidates);

    compute_break_points(&candidates, &scores, break_points);

    // css-text-4 allows another number of lines for `pretty`, and for `balance` past five lines,
    // but layout takes the greedy number. <https://drafts.csswg.org/css-text-4/#text-wrap-style>
    if break_points.len() != line_info_list.len() {
      break_points.clear();

      return false;
    }

    true
  }

  /// Blink's `ComputeCandidates`: every break opportunity of the paragraph, between sentinels.
  fn compute_candidates<'a, 'c>(
    &self,
    line_info_list: &mut [LineInfo<'a, 'c>],
    line_breaker: &mut LineBreaker<'a, '_, 'c>,
    candidates: &mut Vec<LineBreakCandidate>,
  ) -> bool {
    let mut context = LineBreakCandidateContext::new(candidates);

    context.hyphen_penalty = self.hyphen_penalty;
    context.ensure_first_sentinel(&line_info_list[0]);

    for line_info in line_info_list.iter_mut() {
      if !context.append_line(line_info, line_breaker) {
        context.candidates.clear();

        return false;
      }
    }

    context.ensure_last_sentinel();
    true
  }

  /// Blink's `AvailableWidth`.
  fn available_width(&self, line_index: usize) -> LayoutUnit {
    let mut available_width = self.line_widths[line_index];

    if line_index == 0 {
      available_width -= self.first_line_indent;
    }

    available_width.clamp_negative_to_zero()
  }

  /// Blink's `AvailableWidthToFit`, with the epsilon `LineBreaker` allows.
  fn available_width_to_fit(&self, line_index: usize) -> LayoutUnit {
    self.available_width(line_index).add_epsilon()
  }

  /// Blink's `SetupParameters`: Minikin's `computePenalties` heuristics. Justified text
  /// hyphenates more and has no line penalty.
  fn setup(&mut self, block_context: &RenderContext) {
    let available_width = self.line_widths.default_width.clamp_negative_to_zero();
    let font_size = block_context.sizing.font_size;
    let width_times_font_size = available_width.to_float() * font_size / self.zoom;

    self.is_justified = block_context.style.inherited_data.text_align == TextAlign::Justify;

    if self.is_justified {
      self.hyphen_penalty = width_times_font_size / 2.0;
      self.line_penalty = 0.0;
    } else {
      self.hyphen_penalty = width_times_font_size * 2.0;
      self.line_penalty = self.hyphen_penalty * 2.0;
    }
  }

  /// Blink's `ComputeScores`: each candidate's best score as a line's end, and the line start it
  /// takes.
  fn compute_scores(&self, candidates: &[LineBreakCandidate]) -> Vec<LineBreakScore> {
    debug_assert!(candidates.len() >= 2);

    let mut scores = Vec::with_capacity(candidates.len());
    let mut active = 0;

    scores.push(LineBreakScore {
      score: 0.0,
      prev_index: 0,
      line_index: 0,
    });

    for (end, end_candidate) in candidates.iter().enumerate().skip(1) {
      let is_end_last_candidate = end == candidates.len() - 1;
      let mut best = SCORE_INFINITY;
      let mut best_prev_index = 0;
      let scores_so_far: &[LineBreakScore] = &scores;
      let mut last_line_index = scores_so_far[active].line_index;
      let mut available_width = self.available_width_to_fit(last_line_index);
      let mut start_edge = end_candidate.pos_if_break - available_width.to_float();
      let mut best_hope = 0.0;

      for (start, start_score) in scores_so_far.iter().enumerate().take(end).skip(active) {
        let line_index = start_score.line_index;

        if line_index != last_line_index {
          last_line_index = line_index;

          let new_available_width = self.available_width_to_fit(line_index);

          if new_available_width != available_width {
            available_width = new_available_width;
            start_edge = end_candidate.pos_if_break - available_width.to_float();
            best_hope = 0.0;
          }
        }

        let start_score_value = start_score.score;

        if start_score_value + best_hope >= best {
          continue;
        }

        let delta = candidates[start].pos_no_break - start_edge;
        // `best_hope` assumes a non-negative `delta`'s width score grows with each later break.
        let mut width_score = 0.0;
        let mut additional_penalty = 0.0;

        if (is_end_last_candidate || !self.is_justified) && delta < 0.0 {
          width_score = SCORE_OVERFULL;
        } else if is_end_last_candidate && !self.is_balanced {
          // A hyphen on the last line costs more.
          additional_penalty = LAST_LINE_PENALTY_MULTIPLIER * candidates[start].penalty;
        } else if delta < 0.0 {
          width_score = SCORE_OVERFULL;
        } else {
          width_score = delta * delta / self.zoom;
        }

        if delta < 0.0 {
          active = start + 1;
        } else {
          best_hope = width_score;
        }

        let score = start_score_value + width_score + additional_penalty;

        if score <= best {
          best = score;
          best_prev_index = start;
        }
      }

      let line_index = scores[best_prev_index].line_index + 1;

      scores.push(LineBreakScore {
        score: best + end_candidate.penalty + self.line_penalty,
        prev_index: best_prev_index,
        line_index,
      });
    }

    scores
  }
}

/// Blink's `ShouldOptimize`: the score is worth its cost when the greedy last line is short and
/// one word, or when the two lines before it both end hyphenated.
fn should_optimize<'a, 'c>(
  line_info_list: &[LineInfo<'a, 'c>],
  line_breaker: &mut LineBreaker<'a, '_, 'c>,
) -> bool {
  const SHORT_LINE_DENOMINATOR: i32 = 3;
  const NUM_LAST_HYPHENATED_LINES: usize = 2;

  let Some(last_line) = line_info_list.last() else {
    return false;
  };

  if last_line.width() < last_line.available_width() / SHORT_LINE_DENOMINATOR
    && !line_breaker.can_break_inside(last_line)
  {
    return true;
  }

  let num_lines = line_info_list.len();

  num_lines > NUM_LAST_HYPHENATED_LINES
    && line_info_list[num_lines - 2].is_hyphenated()
    && line_info_list[num_lines - 3].is_hyphenated()
}

/// Blink's `ComputeBreakPoints`: the best scores' breaks, from the paragraph's end back.
fn compute_break_points(
  candidates: &[LineBreakCandidate],
  scores: &[LineBreakScore],
  break_points: &mut Vec<LineBreakPoint>,
) {
  debug_assert!(candidates.len() >= 3);
  debug_assert_eq!(candidates.len(), scores.len());
  debug_assert!(break_points.is_empty());

  let mut index = scores.len() - 1;

  while index > 0 {
    break_points.push(candidates[index].point);
    index = scores[index].prev_index;
  }

  break_points.reverse();
}

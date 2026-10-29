//! Blink's [`flex_line_breaker`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_line_breaker.cc):
//! flex items broken into lines, greedily or balanced for `flex-wrap: balance`.

use crate::{layout::flex::flex_item::FlexItem, layout_unit::LayoutUnit};

/// Blink's `InitialFlexLine`: how many items a line holds, and their hypothetical main sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InitialFlexLine {
  pub(crate) count: usize,
  pub(crate) sum_hypothetical_main_size: LayoutUnit,
}

/// Blink's `FlexLineBreakerResult`.
#[derive(Debug, Default)]
pub(crate) struct FlexLineBreakerResult {
  pub(crate) flex_lines: Vec<InitialFlexLine>,
  pub(crate) max_sum_hypothetical_main_size: LayoutUnit,
}

/// Blink's `BreakFlexItemsIntoLines`.
pub(crate) fn break_flex_items_into_lines(
  all_items: &[FlexItem],
  line_break_size: LayoutUnit,
  gap_between_items: LayoutUnit,
  is_multi_line: bool,
  balance_min_line_count: Option<usize>,
) -> FlexLineBreakerResult {
  if all_items.is_empty() {
    return FlexLineBreakerResult::default();
  }

  match balance_min_line_count {
    Some(min_line_count) => balance_break_flex_items_into_lines(
      all_items,
      line_break_size,
      gap_between_items,
      min_line_count,
    ),
    None => {
      // Greedily break once the line exceeds the break size.
      break_into_lines(all_items, gap_between_items, |count, line_size| {
        is_multi_line && count > 0 && line_size > line_break_size
      })
    }
  }
}

/// Scores add squares of layout units, which saturate rather than overflow.
type ScoreUnit = u64;

const INFINITY: ScoreUnit = u64::MAX;

/// Blink's `ScoreData`: the best break found so far for one line end.
#[derive(Clone, Copy)]
struct ScoreData {
  limit: ScoreUnit,
  best_score: ScoreUnit,
  best_break: Option<usize>,
  start: Option<usize>,
  initial_start: usize,
}

impl Default for ScoreData {
  fn default() -> Self {
    Self {
      limit: INFINITY,
      best_score: INFINITY,
      best_break: None,
      start: None,
      initial_start: 0,
    }
  }
}

/// Blink's `ScoreContext`: the items' prefix sums, each with the gap after it.
struct ScoreContext {
  gap_between_items: ScoreUnit,
  line_break_size: ScoreUnit,
  sums: Vec<ScoreUnit>,
  scores: Vec<ScoreData>,
}

impl ScoreContext {
  /// The size of the items `start..=end` on one line.
  fn length(&self, start: usize, end: usize) -> ScoreUnit {
    let before = if start == 0 { 0 } else { self.sums[start - 1] };

    self.sums[end]
      .saturating_sub(before)
      .saturating_sub(self.gap_between_items)
  }

  /// Blink's `Score`, a Knuth-Plass breaking that resumes when asked again with a larger `limit`.
  fn score(&mut self, end: usize, limit: ScoreUnit) -> ScoreUnit {
    let data = self.scores[end];
    let mut best_score = INFINITY;
    let mut best_break = None;
    let mut start = data.initial_start;

    if data.limit == INFINITY {
      if data.best_score != INFINITY {
        return data.best_score;
      }
    } else if limit <= data.limit {
      return data.best_score;
    } else {
      best_score = data.best_score;
      best_break = data.best_break;
      start = data.start.unwrap_or(data.initial_start);
    }

    while start <= end {
      // A single item longer than the line scores zero.
      let length = self.length(start, end);
      let line_score = if length > self.line_break_size {
        0
      } else {
        let free = self.line_break_size - length;

        free.saturating_mul(free)
      };

      // Line scores only grow from here, so nothing later beats the best total.
      if line_score > best_score {
        break;
      }

      // Nothing later fits under the limit either; keep the state to continue from.
      if line_score > limit {
        self.scores[end] = ScoreData {
          best_score,
          best_break,
          limit,
          start: Some(start),
          ..data
        };
        return best_score;
      }

      let new_limit = if best_score == INFINITY {
        INFINITY
      } else {
        best_score - line_score
      };
      let score = line_score.saturating_add(if start == 0 {
        0
      } else {
        self.score(start - 1, new_limit)
      });

      if score <= best_score {
        best_score = score;
        best_break = start.checked_sub(1);
      }
      start += 1;
    }

    self.scores[end] = ScoreData {
      best_score,
      best_break,
      limit: INFINITY,
      start: None,
      ..data
    };
    best_score
  }

  /// Blink's `GreedyLineCount`: how many lines greedy packing makes at `line_break_size`.
  fn greedy_line_count(&self, line_break_size: ScoreUnit) -> usize {
    let mut it = 0;
    let mut previous_sum: ScoreUnit = 0;
    let mut line_count = 1;

    loop {
      // The first sum past the break.
      let bound = previous_sum
        .saturating_add(line_break_size)
        .saturating_add(self.gap_between_items);
      let next = it + self.sums[it..].partition_point(|&sum| sum <= bound);

      if next == self.sums.len() {
        break;
      }

      // A single item past the break stays; otherwise the line backs off one item.
      let is_single_item = usize::from(line_count == 1) + (next - it) <= 1;

      it = if is_single_item { next } else { next - 1 };
      previous_sum = self.sums[it];

      if it + 1 != self.sums.len() {
        line_count += 1;
      }
    }

    line_count
  }

  /// Blink's `ApplyMinLineCount`: bisects the line break size toward `min_line_count` lines, then
  /// grows items that fit a line exactly so they move to the next one. Returns the line count.
  fn apply_min_line_count(&mut self, min_line_count: usize) -> usize {
    let mut low: ScoreUnit = 0;
    let mut high = self.line_break_size.min(
      self
        .sums
        .last()
        .copied()
        .unwrap_or(0)
        .saturating_sub(self.gap_between_items),
    );

    while low < high {
      let midpoint = low + (high - low) / 2;

      if self.greedy_line_count(midpoint) > min_line_count {
        low = midpoint + 1;
      } else {
        high = midpoint;
      }
    }
    self.line_break_size = high;

    let mut line_count = self.greedy_line_count(self.line_break_size);

    if line_count >= min_line_count {
      return line_count;
    }

    // No break size gives some counts, such as 3 lines of 4 equal items: push items that fit
    // exactly onto the next line.
    let mut perfect_fit_indices = Vec::new();
    let mut previous: ScoreUnit = 0;
    let mut line_has_content = false;

    for index in 0..self.sums.len() {
      let line_size = self.sums[index]
        .saturating_sub(previous)
        .saturating_sub(self.gap_between_items);

      if line_size == self.line_break_size {
        perfect_fit_indices.push(index);
      }

      if line_size > self.line_break_size {
        previous = if line_has_content {
          self.sums[index - 1]
        } else {
          self.sums[index]
        };
        line_has_content = false;
        continue;
      }

      line_has_content = true;
    }

    for &index in perfect_fit_indices.iter().rev() {
      for sum in &mut self.sums[index..] {
        *sum += 1;
      }

      line_count = self.greedy_line_count(self.line_break_size);

      if line_count >= min_line_count {
        break;
      }
    }

    line_count
  }
}

/// Blink's `BreakIntoLines`: `all_items` in lines, a line ending where `should_break` says
/// given the items so far and the size the next one would bring them to.
fn break_into_lines(
  all_items: &[FlexItem],
  gap_between_items: LayoutUnit,
  mut should_break: impl FnMut(usize, LayoutUnit) -> bool,
) -> FlexLineBreakerResult {
  let mut result = FlexLineBreakerResult::default();
  let mut items = all_items;

  while !items.is_empty() {
    let mut sum_hypothetical_main_size = LayoutUnit::ZERO;
    let mut count = 0;

    for item in items {
      if should_break(
        count,
        sum_hypothetical_main_size + item.hypothetical_main_axis_margin_box_size(),
      ) {
        break;
      }

      sum_hypothetical_main_size +=
        item.hypothetical_main_axis_margin_box_size() + gap_between_items;
      count += 1;
    }

    // A line always holds an item, and the gap after its last one goes.
    sum_hypothetical_main_size -= gap_between_items;
    result.flex_lines.push(InitialFlexLine {
      count,
      sum_hypothetical_main_size,
    });
    result.max_sum_hypothetical_main_size = result
      .max_sum_hypothetical_main_size
      .max(sum_hypothetical_main_size);
    items = &items[count..];
  }

  result
}

/// Blink's `BalanceBreakFlexItemsIntoLines`.
fn balance_break_flex_items_into_lines(
  all_items: &[FlexItem],
  line_break_size: LayoutUnit,
  gap_between_items: LayoutUnit,
  min_line_count: usize,
) -> FlexLineBreakerResult {
  let raw = |size: LayoutUnit| size.raw_value().max(0) as ScoreUnit;
  let min_line_count = min_line_count.min(all_items.len()).max(1);
  let mut ctx = ScoreContext {
    gap_between_items: raw(gap_between_items),
    line_break_size: raw(line_break_size),
    sums: Vec::with_capacity(all_items.len()),
    scores: vec![ScoreData::default(); all_items.len()],
  };

  // The prefix sums only grow, so a negative margin box counts as zero.
  for item in all_items {
    let item_size = item
      .hypothetical_main_axis_margin_box_size()
      .clamp_negative_to_zero();
    let previous = ctx.sums.last().copied().unwrap_or(0);

    ctx.sums.push(previous + raw(item_size + gap_between_items));
  }

  let mut line_count = ctx.greedy_line_count(ctx.line_break_size);

  if line_count < min_line_count {
    line_count = ctx.apply_min_line_count(min_line_count);
  }

  // The earliest start that fits on a line with each end.
  let mut start = 0;

  for end in 0..all_items.len() {
    while start < end && ctx.length(start, end) > ctx.line_break_size {
      start += 1;
    }
    ctx.scores[end].initial_start = start;

    // A single item longer than the line ends a line of its own.
    if start == end && ctx.length(start, end) > ctx.line_break_size {
      start += 1;
    }
  }

  ctx.score(all_items.len() - 1, INFINITY);

  // The item count of each line, last line first.
  let mut item_counts = Vec::with_capacity(line_count);
  let mut previous_index = all_items.len() - 1;
  let mut index = ctx.scores[previous_index].best_break;

  while let Some(break_index) = index {
    item_counts.push(previous_index - break_index);
    previous_index = break_index;
    index = ctx.scores[break_index].best_break;
  }
  item_counts.push(previous_index + 1);

  let mut counts = item_counts.into_iter().rev().peekable();

  break_into_lines(all_items, gap_between_items, |count, _| {
    if counts.peek() == Some(&count) {
      counts.next();
      return true;
    }

    false
  })
}

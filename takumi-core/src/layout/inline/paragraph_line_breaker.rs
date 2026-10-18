//! Blink's [`ParagraphLineBreaker`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/paragraph_line_breaker.cc),
//! the bisection `text-wrap: balance` falls back to, without `line-clamp`.

use super::{
  inline_break_token::InlineBreakToken,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  leading_floats::LeadingFloats,
  line_breaker::{AtomicInlineLayout, LineBreaker, LineBreakerMode},
  line_info::LineInfo,
  score_line_breaker::MAX_LINES_FOR_BALANCE,
};
use crate::{
  layout::{
    constraint_space::ConstraintSpace,
    exclusions::{exclusion_space::ExclusionSpace, layout_opportunity::LineLayoutOpportunity},
  },
  layout_unit::LayoutUnit,
};

/// Blink's `LineBreakResults::Status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
  /// Broken to the end, or to `stop_at`.
  Finished,
  /// The paragraph cannot bisect.
  NotApplicable,
  /// Past `max_lines` lines.
  MaxLinesExceeded,
}

/// Blink's `LineBreakResults`: a paragraph's lines at one available width.
struct LineBreakResults<'s, 'a, 'c> {
  node: InlineNode<'c>,
  items_data: &'a InlineNodeData<'c>,
  space: &'s ConstraintSpace,
  atomic_inline: &'s mut dyn AtomicInlineLayout<'c>,
  line_widths: Vec<LayoutUnit>,
  break_token: Option<InlineBreakToken<'c>>,
}

impl<'s, 'a, 'c> LineBreakResults<'s, 'a, 'c> {
  fn new(
    node: InlineNode<'c>,
    items_data: &'a InlineNodeData<'c>,
    space: &'s ConstraintSpace,
    atomic_inline: &'s mut dyn AtomicInlineLayout<'c>,
  ) -> Self {
    Self {
      node,
      items_data,
      space,
      atomic_inline,
      line_widths: Vec::new(),
      break_token: None,
    }
  }

  /// Blink's `LineWidthSum`.
  fn line_width_sum(&self) -> LayoutUnit {
    self
      .line_widths
      .iter()
      .fold(LayoutUnit::ZERO, |sum, &width| sum + width)
  }

  /// Blink's `Clear`.
  fn clear(&mut self) {
    self.break_token = None;
    self.line_widths.clear();
  }

  /// Blink's `BreakLines`: the lines at `available_width`, up to `max_lines`, or to `stop_at`.
  fn break_lines(
    &mut self,
    available_width: LayoutUnit,
    mut max_lines: usize,
    stop_at: Option<InlineBreakToken<'c>>,
  ) -> Status {
    debug_assert!(self.line_widths.is_empty());

    let line_opportunity = LineLayoutOpportunity::new(available_width);
    let leading_floats = LeadingFloats::default();
    let mut exclusion_space = ExclusionSpace::default();

    loop {
      let mut line_info = LineInfo::new(self.items_data, &self.node.block.context);
      let mut line_breaker = LineBreaker::new(
        self.node,
        self.items_data,
        LineBreakerMode::Content,
        *self.space,
        line_opportunity,
        &leading_floats,
        self.break_token,
        &mut *self.atomic_inline,
        &mut exclusion_space,
      );

      line_breaker.next_line(&mut line_info);

      // Forced breaks keep bisection from finding the width.
      debug_assert!(!line_info.has_forced_break);

      if line_breaker.disable_bisect_line_break {
        return Status::NotApplicable;
      }

      self.break_token = line_info.break_token;
      self.line_widths.push(line_info.width());

      let Some(break_token) = self.break_token else {
        return Status::Finished;
      };

      if stop_at.is_some_and(|stop_at| break_token.start.is_at_least(stop_at.start)) {
        return Status::Finished;
      }

      max_lines -= 1;

      if max_lines == 0 {
        return Status::MaxLinesExceeded;
      }
    }
  }

  /// Blink's `BisectAvailableWidth`: the narrowest width, to `epsilon`, that keeps `num_lines`.
  fn bisect_available_width(
    &mut self,
    max_available_width: LayoutUnit,
    min_available_width: LayoutUnit,
    epsilon: LayoutUnit,
    num_lines: usize,
    stop_at: Option<InlineBreakToken<'c>>,
  ) -> LayoutUnit {
    debug_assert!(epsilon > LayoutUnit::ZERO);
    debug_assert!(num_lines > 0);

    let mut upper = max_available_width;
    let mut lower = min_available_width;

    while lower + epsilon < upper {
      let middle = (upper + lower) / 2;

      if self.break_lines(middle, num_lines, stop_at) == Status::Finished {
        upper = middle;
      } else {
        lower = middle;
      }

      self.clear();
    }

    upper
  }
}

/// Blink's `EstimateNumLines`: lines of space-wide characters, zero when it cannot tell, and the
/// most there can be when no character fits.
fn estimate_num_lines(text_content: &str, space_width: f32, available_width: LayoutUnit) -> usize {
  if space_width <= 0.0 {
    return 0;
  }

  let num_line_chars = (available_width.to_float() / space_width) as usize;

  if num_line_chars == 0 {
    return usize::MAX;
  }

  // Blink counts UTF-16 code units.
  text_content.encode_utf16().count().div_ceil(num_line_chars)
}

impl<'c> InlineNode<'c> {
  /// Blink's `AttemptParagraphBalancing`: the available width that balances the paragraph's
  /// lines, or none when it needs no balancing or cannot have it.
  pub(crate) fn attempt_paragraph_balancing(
    self,
    items_data: &InlineNodeData<'c>,
    space: &ConstraintSpace,
    line_opportunity: &LineLayoutOpportunity,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> Option<LayoutUnit> {
    if items_data.is_bisect_line_break_disabled {
      return None;
    }

    let available_width = line_opportunity.available_inline_size();
    let block_context = &self.block.context;
    let mut normal_lines = LineBreakResults::new(self, items_data, space, &mut *atomic_inline);
    let lines_until_clamp = space
      .lines_until_clamp
      .and_then(|lines| usize::try_from(lines).ok())
      .filter(|&lines| lines > 0 && lines <= MAX_LINES_FOR_BALANCE);

    if let Some(lines_until_clamp) = lines_until_clamp {
      // A clamp of one line leaves nothing to balance; the lines past the clamp do not count.
      if lines_until_clamp == 1
        || normal_lines.break_lines(available_width, lines_until_clamp, None)
          == Status::NotApplicable
      {
        return None;
      }
    } else {
      // An estimate, so it may be twice the most lines.
      let space_width = items_data
        .fonts
        .get(block_context)
        .primary_font()
        .map_or(0.0, |font_data| font_data.space_width());

      if estimate_num_lines(&items_data.text_content, space_width, available_width)
        > MAX_LINES_FOR_BALANCE * 2
        || normal_lines.break_lines(available_width, MAX_LINES_FOR_BALANCE, None)
          != Status::Finished
      {
        return None;
      }
    }

    let num_lines = normal_lines.line_widths.len();

    if num_lines <= 1 {
      return None;
    }

    // Bisecting below a CSS pixel is worthless.
    let epsilon = LayoutUnit::from_float_ceil(1.0);
    // From the average line width, less a fifth for edge cases.
    let avg_line_width = normal_lines.line_width_sum() / num_lines as i32;
    let min_available_width = LayoutUnit::from_float_round(avg_line_width.to_float() * 0.8);
    let stop_at = normal_lines.break_token;

    drop(normal_lines);

    Some(
      LineBreakResults::new(self, items_data, space, atomic_inline).bisect_available_width(
        available_width,
        min_available_width,
        epsilon,
        num_lines,
        stop_at,
      ),
    )
  }
}

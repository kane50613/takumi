//! `text-overflow: ellipsis` and line-clamp truncation.

use crate::{context::RenderContext, font_style::SizedFontStyle, text_processing::RebreakOptions};
use parley::{InlineBoxKind, PositionedInlineBox, PositionedLayoutItem};

use super::{
  InlineLayout, break_into_lines, chromium_line_breaks, items::ProcessedInlineSpan,
  push_presentation_text, push_spans_into_builder, refresh_text_span_ranges,
};

fn tail_text_span<'a, 'c>(
  spans: &'a [ProcessedInlineSpan<'c>],
) -> Option<(&'a SizedFontStyle<'c>, u64)> {
  spans
    .iter()
    .enumerate()
    .rev()
    .find_map(|(span_id, span)| match span {
      ProcessedInlineSpan::Text { style, .. } => Some((style.as_ref(), span_id as u64)),
      ProcessedInlineSpan::DirectionMark { .. }
      | ProcessedInlineSpan::Box(_)
      | ProcessedInlineSpan::Spacer { .. } => None,
    })
}

fn measure_ellipsis_width(
  context: &RenderContext,
  ellipsis_style: &SizedFontStyle,
  ellipsis_char: &str,
) -> f32 {
  let (mut ellipsis_layout, _) = context.tree_builder(ellipsis_style.into(), true, |builder| {
    push_presentation_text(
      builder,
      ellipsis_style,
      None,
      ellipsis_char,
      &context.fonts().classes,
    );
  });
  ellipsis_layout.break_all_lines(None);
  ellipsis_layout
    .lines()
    .next()
    .map(|line| line.runs().map(|run| run.advance()).sum::<f32>())
    .unwrap_or(0.0)
}

struct TruncationCheckpoint {
  cumulative_width: f32,
  byte_end: usize,
}

fn collect_truncation_checkpoints(layout: &InlineLayout) -> Vec<TruncationCheckpoint> {
  let Some(last_line) = layout.lines().last() else {
    return Vec::new();
  };

  let mut checkpoints = Vec::new();
  let mut cumulative_width = 0.0_f32;
  let mut last_run_index: Option<usize> = None;

  for item in last_line.items() {
    match item {
      PositionedLayoutItem::InlineBox(inline_box) => {
        if inline_box.kind != InlineBoxKind::InFlow {
          continue;
        }
        cumulative_width += inline_box.width;
      }
      PositionedLayoutItem::GlyphRun(glyph_run) => {
        let run = glyph_run.run();
        if last_run_index == Some(run.index()) {
          continue;
        }
        last_run_index = Some(run.index());

        for cluster in run.visual_clusters() {
          cumulative_width += cluster.advance();
          checkpoints.push(TruncationCheckpoint {
            cumulative_width,
            byte_end: cluster.text_range().end,
          });
        }
      }
    }
  }

  checkpoints
}

/// Where truncation cuts the spans: every span from `span_cut` on goes, and `text_cut` shortens one
/// text span to a byte length.
struct TruncationPlan {
  span_cut: usize,
  text_cut: Option<(usize, usize)>,
}

impl TruncationPlan {
  fn new(
    checkpoints: &[TruncationCheckpoint],
    spans: &[ProcessedInlineSpan<'_>],
    available_w: f32,
  ) -> Self {
    let mut remaining = checkpoints
      .partition_point(|checkpoint| checkpoint.cumulative_width <= available_w)
      .checked_sub(1)
      .map_or(0, |index| checkpoints[index].byte_end);
    let mut span_cut = spans.len();
    let mut text_cut = None;

    for (index, span) in spans.iter().enumerate() {
      match span {
        // The mark forces the paragraph's base direction, so truncation
        // shortens the text around it but never cuts it.
        ProcessedInlineSpan::DirectionMark { direction, .. } => {
          remaining = remaining.saturating_sub(direction.bidi_mark().len());
        }
        ProcessedInlineSpan::Text { text, .. } => {
          let len = text.len();
          if remaining <= len {
            let safe_cut = text.floor_char_boundary(remaining.min(len));
            text_cut = Some((index, safe_cut));
            span_cut = index + 1;
            break;
          }
          remaining -= len;
        }
        ProcessedInlineSpan::Box(_) | ProcessedInlineSpan::Spacer { .. } => {
          if remaining == 0 {
            span_cut = index;
            break;
          }
        }
      }
    }

    Self { span_cut, text_cut }
  }

  fn apply(self, spans: &mut Vec<ProcessedInlineSpan<'_>>) {
    if let Some((text_index, safe_cut)) = self.text_cut
      && let Some(ProcessedInlineSpan::Text { text, .. }) = spans.get_mut(text_index)
    {
      text.truncate(safe_cut);
    }
    spans.truncate(self.span_cut);
  }
}

fn text_span_style_by_id<'a, 'c>(
  spans: &'a [ProcessedInlineSpan<'c>],
  span_id: u64,
) -> Option<&'a SizedFontStyle<'c>> {
  match spans.get(span_id as usize)? {
    ProcessedInlineSpan::Text { style, .. } => Some(style.as_ref()),
    ProcessedInlineSpan::DirectionMark { .. }
    | ProcessedInlineSpan::Box(_)
    | ProcessedInlineSpan::Spacer { .. } => None,
  }
}

/// Truncates text in the layout to fit within `max_width` and appends an ellipsis.
pub(super) fn make_ellipsis_layout<'c>(
  layout: &mut InlineLayout,
  spans: &mut Vec<ProcessedInlineSpan<'c>>,
  options: RebreakOptions,
  root_style: &'c SizedFontStyle,
  context: &RenderContext,
  positioned_floats: &mut Vec<PositionedInlineBox>,
) {
  let ellipsis_char = root_style.parent.ellipsis_char();
  let checkpoints = collect_truncation_checkpoints(layout);
  let mut ellipsis_span_id = tail_text_span(spans).map(|(_, span_id)| span_id);

  let mut iterations = 0;
  let final_plan = loop {
    iterations += 1;
    let ellipsis_style = ellipsis_span_id
      .and_then(|span_id| text_span_style_by_id(spans, span_id))
      .unwrap_or(root_style);
    let ellipsis_w = measure_ellipsis_width(context, ellipsis_style, ellipsis_char);

    let plan = TruncationPlan::new(
      &checkpoints,
      spans,
      (options.max_width - ellipsis_w).max(0.0),
    );
    let next_ellipsis_span_id = tail_text_span(&spans[..plan.span_cut]).map(|(_, span_id)| span_id);

    if next_ellipsis_span_id == ellipsis_span_id || iterations > 3 {
      break plan;
    }
    ellipsis_span_id = next_ellipsis_span_id;
  };

  final_plan.apply(spans);
  refresh_text_span_ranges(spans);

  let ellipsis_style = tail_text_span(spans).map_or(root_style, |(style, _)| style);

  let (mut final_layout, _) =
    context.tree_builder(root_style.into(), chromium_line_breaks(spans), |builder| {
      push_spans_into_builder(builder, spans, &context.fonts().classes);
      push_presentation_text(
        builder,
        ellipsis_style,
        None,
        ellipsis_char,
        &context.fonts().classes,
      );
    });

  positioned_floats.clear();
  break_into_lines(
    &mut final_layout,
    options,
    root_style,
    spans,
    positioned_floats,
  );
  *layout = final_layout;
}

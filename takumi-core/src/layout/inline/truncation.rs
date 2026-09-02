//! `text-overflow: ellipsis` and line-clamp truncation.

use crate::{context::RenderContext, font_style::SizedFontStyle, text_processing::MaxHeight};
use parley::{InlineBoxKind, PositionedInlineBox, PositionedLayoutItem};

use super::{
  InlineLayout, apply_text_indent, breaking::break_lines, chromium_line_breaks,
  inline_line_height_hint, items::ProcessedInlineSpan, push_presentation_text,
  push_spans_into_builder, refresh_text_span_ranges,
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

fn truncation_plan<'c>(
  checkpoints: &[TruncationCheckpoint],
  spans: &[ProcessedInlineSpan<'c>],
  available_w: f32,
) -> (Option<usize>, Option<(usize, usize)>) {
  let truncate_at = checkpoints
    .partition_point(|checkpoint| checkpoint.cumulative_width <= available_w)
    .checked_sub(1)
    .map(|index| checkpoints[index].byte_end)
    .or(Some(0));

  if let Some(cut) = truncate_at {
    let mut remaining = cut;
    let mut span_cut_idx = spans.len();
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
            span_cut_idx = index + 1;
            break;
          }
          remaining -= len;
        }
        ProcessedInlineSpan::Box(_) | ProcessedInlineSpan::Spacer { .. } => {
          if remaining == 0 {
            span_cut_idx = index;
            break;
          }
        }
      }
    }

    (Some(span_cut_idx), text_cut)
  } else {
    (None, None)
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

fn truncated_tail_text_span_id<'c>(
  spans: &[ProcessedInlineSpan<'c>],
  span_cut_idx: Option<usize>,
) -> Option<u64> {
  span_cut_idx.and_then(|cut_idx| {
    spans[..cut_idx]
      .iter()
      .enumerate()
      .rev()
      .find_map(|(span_id, span)| match span {
        ProcessedInlineSpan::Text { .. } => Some(span_id as u64),
        ProcessedInlineSpan::DirectionMark { .. }
        | ProcessedInlineSpan::Box(_)
        | ProcessedInlineSpan::Spacer { .. } => None,
      })
  })
}

fn apply_truncation_plan<'c>(
  spans: &mut Vec<ProcessedInlineSpan<'c>>,
  plan: (Option<usize>, Option<(usize, usize)>),
) {
  let (span_cut_idx, text_cut) = plan;
  if let Some(span_cut_idx) = span_cut_idx {
    if let Some((text_index, safe_cut)) = text_cut
      && let Some(ProcessedInlineSpan::Text { text, .. }) = spans.get_mut(text_index)
    {
      text.truncate(safe_cut);
    }
    spans.truncate(span_cut_idx);
  } else {
    spans.clear();
  }
}

/// Truncates text in the layout to fit within `max_width` and appends an ellipsis.
pub(super) fn make_ellipsis_layout<'c>(
  layout: &mut InlineLayout,
  spans: &mut Vec<ProcessedInlineSpan<'c>>,
  max_width: f32,
  max_height: Option<MaxHeight>,
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

    let plan = truncation_plan(&checkpoints, spans, (max_width - ellipsis_w).max(0.0));
    let next_ellipsis_span_id = truncated_tail_text_span_id(spans, plan.0);

    if next_ellipsis_span_id == ellipsis_span_id || iterations > 3 {
      break plan;
    }
    ellipsis_span_id = next_ellipsis_span_id;
  };

  apply_truncation_plan(spans, final_plan);
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

  apply_text_indent(&mut final_layout, root_style, max_width);
  let text_wrap_mode = root_style.parent.resolved_text_wrap_mode();
  positioned_floats.clear();
  break_lines(
    &mut final_layout,
    max_width,
    max_height,
    inline_line_height_hint(root_style),
    text_wrap_mode,
    spans,
    positioned_floats,
  );
  *layout = final_layout;
}

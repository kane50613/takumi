//! Line breaking against the layout constraint.

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, Size},
  style::{Length, TextWrapMode},
  text_processing::MaxHeight,
};
use parley::{InlineBoxKind, Line, PositionedInlineBox, PositionedLayoutItem, YieldData};

use super::{
  InlineBrush, InlineLayout,
  decoration_break::{ClonedSpans, LineEdges},
  floats::FloatLayoutState,
  items::ProcessedInlineSpan,
  runs::HangingWhitespace,
};

/// Splits a line's trailing-whitespace advance over its trailing glyph runs, walking back from the
/// line's logical end so a run keeps at most its own advance.
pub(super) fn distribute_trailing_whitespace(
  items: &[PositionedLayoutItem<'_, InlineBrush>],
  line: &Line<'_, InlineBrush>,
  rtl_paragraph: bool,
) -> Vec<HangingWhitespace> {
  let mut shares = vec![HangingWhitespace::default(); items.len()];
  let mut remaining = line.metrics().trailing_whitespace;
  let count = items.len();

  // The line's logical end sits at its visual left in a right-to-left paragraph, where parley
  // measures the whitespace from.
  for step in 0..count {
    let index = if rtl_paragraph {
      step
    } else {
      count - 1 - step
    };

    if remaining <= 0.0 {
      break;
    }
    let PositionedLayoutItem::GlyphRun(glyph_run) = &items[index] else {
      break;
    };
    let share = remaining.min(glyph_run.advance());

    shares[index] = HangingWhitespace::of(share, glyph_run.run().is_rtl(), rtl_paragraph);
    remaining -= share;
  }

  shares
}

/// Resolve the inline layout's max width and optional max height from available space and known
/// dimensions.
pub(crate) fn create_inline_constraint(
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
) -> (f32, Option<MaxHeight>) {
  let known_width = known_dimensions.width;
  let available_width = match available_space.width {
    AvailableSpace::MinContent => Some(0.0),
    AvailableSpace::MaxContent => None,
    AvailableSpace::Definite(width) => Some(width),
  };
  // taffy subtracts the content-box inset without a floor, so a box narrower
  // than its own padding arrives here negative. parley asserts on that.
  let mut width_constraint = known_width
    .or(available_width)
    .unwrap_or(f32::INFINITY)
    .max(0.0);

  // taffy hands the measure function a border-box width whatever `box-sizing`
  // says, so the insets always come off.
  if known_width.is_some() && width_constraint.is_finite() {
    let sizing = &context.sizing;
    let horizontal_insets = context.style.padding_left.to_px(sizing, 0.0)
      + context.style.padding_right.to_px(sizing, 0.0)
      + if !context.style.border_left_style.is_rendered() {
        0.0
      } else {
        Length::from(context.style.border_left_width).to_px(sizing, 0.0)
      }
      + if !context.style.border_right_style.is_rendered() {
        0.0
      } else {
        Length::from(context.style.border_right_width).to_px(sizing, 0.0)
      };
    width_constraint = (width_constraint - horizontal_insets).max(0.0);
  }

  let max_height = match (
    context.sizing.viewport.size.height,
    context.style.clamp_lines(),
  ) {
    (Some(height), Some(lines)) => Some(MaxHeight::HeightAndLines(height as f32, lines)),
    (Some(height), None) => Some(MaxHeight::Absolute(height as f32)),
    (None, Some(lines)) => Some(MaxHeight::Lines(lines)),
    (None, None) => None,
  };

  (width_constraint, max_height)
}

/// The width lines break at and the width they align within.
///
/// `text-wrap: balance` narrows the breaking width while alignment keeps the container width,
/// as Blink clears its overridden available width before aligning
/// (blink/renderer/core/layout/inline/line_breaker.cc, `LineBreaker::NextLine`).
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct LineWidths {
  pub(crate) breaking: f32,
  pub(crate) alignment: f32,
}

impl LineWidths {
  pub(crate) fn uniform(width: f32) -> Self {
    Self {
      breaking: width,
      alignment: width,
    }
  }
}

pub(crate) fn has_custom_out_of_flow(layout: &InlineLayout) -> bool {
  layout
    .inline_boxes()
    .iter()
    .any(|inline_box| inline_box.kind == InlineBoxKind::CustomOutOfFlow)
}

/// How many times lines break again to settle the edges `box-decoration-break: clone` spans
/// repeat on them.
const CLONED_EDGE_PASSES: usize = 4;

/// Breaks `layout` into lines; true when `max_height` may have dropped lines.
///
/// Each line reserves the start edges of the `box-decoration-break: clone` spans open where it
/// starts, as Blink's line breaker does. Parley reports where a line broke only once the paragraph
/// is laid out, so the lines break again with the edges of the previous breaks until they settle.
/// Approximate: lines still moving after [`CLONED_EDGE_PASSES`] keep the last breaks.
pub(crate) fn break_lines(
  layout: &mut InlineLayout,
  widths: LineWidths,
  max_height: Option<MaxHeight>,
  line_height_hint: f32,
  text_wrap_mode: TextWrapMode,
  spans: &[ProcessedInlineSpan<'_>],
  positioned_floats: &mut Vec<PositionedInlineBox>,
) -> bool {
  let breaks = LineBreaks {
    widths,
    max_height,
    line_height_hint,
    text_wrap_mode,
    spans,
  };
  let Some(cloned) = ClonedSpans::of(spans) else {
    return breaks.apply(layout, &[], positioned_floats);
  };
  let mut edges: Vec<LineEdges> = Vec::new();
  let mut clamped = false;

  for _ in 0..CLONED_EDGE_PASSES {
    positioned_floats.clear();
    clamped = breaks.apply(layout, &edges, positioned_floats);

    let settled: Vec<LineEdges> = layout
      .lines()
      .map(|line| cloned.line_edges(line.text_range()))
      .collect();

    if settled == edges {
      break;
    }
    edges = settled;
  }

  clamped
}

/// What lines break against.
#[derive(Clone, Copy)]
struct LineBreaks<'s, 'c> {
  widths: LineWidths,
  max_height: Option<MaxHeight>,
  line_height_hint: f32,
  text_wrap_mode: TextWrapMode,
  spans: &'s [ProcessedInlineSpan<'c>],
}

impl LineBreaks<'_, '_> {
  /// Breaks `layout` into lines, each reserving its cloned `edges`; true when `max_height` may
  /// have dropped lines.
  fn apply(
    self,
    layout: &mut InlineLayout,
    edges: &[LineEdges],
    positioned_floats: &mut Vec<PositionedInlineBox>,
  ) -> bool {
    let LineBreaks {
      widths,
      max_height,
      line_height_hint,
      text_wrap_mode,
      spans,
    } = self;
    let inline_boxes = layout.inline_boxes().to_vec();
    let mut float_layout = FloatLayoutState::new(widths, line_height_hint);
    let has_custom_out_of_flow = has_custom_out_of_flow(layout);
    let is_uniform = widths.breaking == widths.alignment && edges.is_empty();
    let rtl = layout.is_rtl();

    if text_wrap_mode == TextWrapMode::NoWrap && !has_custom_out_of_flow && is_uniform {
      layout.break_all_lines(Some(widths.breaking));
      return false;
    }

    if max_height.is_none() && !has_custom_out_of_flow && is_uniform {
      layout.break_all_lines(Some(widths.breaking));
      return false;
    }

    let (limit_height, limit_lines) = match max_height {
      Some(MaxHeight::Lines(lines)) => (f32::MAX, lines),
      Some(MaxHeight::Absolute(height)) => (height, u32::MAX),
      Some(MaxHeight::HeightAndLines(height, lines)) => (height, lines),
      None => (f32::MAX, u32::MAX),
    };

    let mut total_height = 0.0;
    let mut line_count = 0;
    let mut line_y = 0.0;
    let mut breaker = layout.break_lines();
    let mut exhausted = false;
    float_layout.update_breaker_line(&mut breaker, line_y);
    LineEdges::reserve_on(edges, line_count, &mut breaker, rtl);

    loop {
      let Some(yield_data) = breaker.break_next() else {
        exhausted = true;
        break;
      };
      if line_count >= limit_lines {
        breaker.revert();
        break;
      }
      let height = match yield_data {
        YieldData::LineBreak(data) => data.line_height,
        YieldData::MaxHeightExceeded(data) => data.line_height,
        YieldData::InlineBoxBreak(data) => {
          breaker
            .state_mut()
            .append_inline_box_to_line(data.advance, 0.0);

          let Some(inline_box) = inline_boxes.get(data.inline_box_index).cloned() else {
            continue;
          };
          let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box.id as usize) else {
            continue;
          };
          let Some(side) = item.float_side() else {
            continue;
          };
          let clear = item.clear();
          let start_y = breaker.state().line_y() as f32;
          let positioned_float = float_layout.push_float(side, clear, start_y, &inline_box);
          line_y = float_layout.find_line_y_for_advance(start_y, data.advance);
          float_layout.update_breaker_line(&mut breaker, line_y);
          LineEdges::reserve_on(edges, line_count, &mut breaker, rtl);
          positioned_floats.push(positioned_float);
          continue;
        }
      };

      if !can_commit_line_candidate(total_height, height, line_count, limit_height) {
        breaker.revert();
        break;
      }

      let reserved = edges
        .get(line_count as usize)
        .map_or(0.0, |edges| edges.start_width + edges.end_width);

      breaker.set_prior_line_width(float_layout.line_width(line_y) - reserved);
      total_height += height;
      line_count += 1;
      line_y = breaker.state().line_y() as f32;
      float_layout.update_breaker_line(&mut breaker, line_y);
      LineEdges::reserve_on(edges, line_count, &mut breaker, rtl);
    }

    breaker.finish();
    !exhausted
  }
}

fn can_commit_line_candidate(
  current_height: f32,
  candidate_line_height: f32,
  committed_lines: u32,
  limit_height: f32,
) -> bool {
  committed_lines == 0 || current_height + candidate_line_height <= limit_height
}

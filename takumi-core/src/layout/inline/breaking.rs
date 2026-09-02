//! Line breaking against the layout constraint.

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, Size},
  style::{Length, TextWrapMode},
  text_processing::MaxHeight,
};
use parley::{InlineBoxKind, Line, PositionedInlineBox, PositionedLayoutItem, YieldData};

use super::{InlineBrush, InlineLayout, floats::FloatLayoutState, items::ProcessedInlineSpan};

/// Splits a line's trailing-whitespace advance over its trailing glyph runs,
/// walking back from the line end so a run keeps at most its own advance.
pub(super) fn distribute_trailing_whitespace(
  items: &[PositionedLayoutItem<'_, InlineBrush>],
  line: &Line<'_, InlineBrush>,
) -> Vec<f32> {
  let mut shares = vec![0.0_f32; items.len()];
  let mut remaining = line.metrics().trailing_whitespace;

  for (index, item) in items.iter().enumerate().rev() {
    if remaining <= 0.0 {
      break;
    }
    let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
      break;
    };
    let share = remaining.min(glyph_run.advance());

    shares[index] = share;
    remaining -= share;
  }

  shares
}

/// Resolve the inline layout's max width and optional max height from available space and known dimensions.
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

  // applies a maximum height to reduce unnecessary calculation.
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

pub(crate) fn break_lines(
  layout: &mut InlineLayout,
  max_width: f32,
  max_height: Option<MaxHeight>,
  line_height_hint: f32,
  text_wrap_mode: TextWrapMode,
  spans: &[ProcessedInlineSpan<'_>],
  positioned_floats: &mut Vec<PositionedInlineBox>,
) {
  let inline_boxes = layout.inline_boxes().to_vec();
  let mut float_layout = FloatLayoutState::new(max_width, line_height_hint);
  let has_custom_out_of_flow = inline_boxes
    .iter()
    .any(|inline_box| inline_box.kind == InlineBoxKind::CustomOutOfFlow);

  if text_wrap_mode == TextWrapMode::NoWrap && !has_custom_out_of_flow {
    return layout.break_all_lines(Some(max_width));
  }

  if max_height.is_none() && !has_custom_out_of_flow {
    return layout.break_all_lines(Some(max_width));
  }

  let (limit_height, limit_lines) = match max_height {
    Some(MaxHeight::Lines(lines)) => (f32::MAX, lines),
    Some(MaxHeight::Absolute(height)) => (height, u32::MAX),
    Some(MaxHeight::HeightAndLines(height, lines)) => (height, lines),
    None => (f32::MAX, u32::MAX),
  };

  let mut total_height = 0.0;
  let mut line_count = 0;
  let mut breaker = layout.break_lines();
  float_layout.update_breaker_line(&mut breaker, 0.0);

  while line_count < limit_lines {
    let Some(yield_data) = breaker.break_next() else {
      break;
    };
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
        let Some(side) = float_layout.side_for_inline_box(spans, inline_box.id) else {
          continue;
        };
        let clear = float_layout.clear_for_inline_box(spans, inline_box.id);
        let start_y = breaker.state().line_y() as f32;
        let positioned_float = float_layout.push_float(side, clear, start_y, &inline_box);
        let line_y = float_layout.find_line_y_for_advance(start_y, data.advance);
        float_layout.update_breaker_line(&mut breaker, line_y);
        positioned_floats.push(positioned_float);
        continue;
      }
    };

    if !can_commit_line_candidate(total_height, height, line_count, limit_height) {
      breaker.revert();
      break;
    }

    total_height += height;
    line_count += 1;
    let next_line_y = breaker.state().line_y() as f32;
    float_layout.update_breaker_line(&mut breaker, next_line_y);

    if total_height >= limit_height {
      break;
    }
  }

  breaker.finish();
}

fn can_commit_line_candidate(
  current_height: f32,
  candidate_line_height: f32,
  committed_lines: u32,
  limit_height: f32,
) -> bool {
  committed_lines == 0 || current_height + candidate_line_height <= limit_height
}

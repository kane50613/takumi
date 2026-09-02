//! Vertical line metrics: line-height, baselines and vertical-align.

use crate::style::{ResolvedVerticalAlign, VerticalAlignKeyword};
use parley::{InlineBoxKind, Line, LineMetrics, PositionedInlineBox, PositionedLayoutItem};

use super::{
  InlineBrush, InlineLayout,
  items::{InlineBoxItem, ProcessedInlineSpan},
};

#[derive(Clone, Copy, Debug)]
/// x-height and ascent/descent of the parent font.
pub(crate) struct ParentFontMetrics {
  pub(crate) x_height: Option<f32>,
  pub(crate) text_metrics: (f32, f32),
}

/// Font metrics of the first run, used as the parent reference.
pub(crate) fn get_parent_font_metrics(layout: &InlineLayout) -> Option<ParentFontMetrics> {
  let run = layout.lines().find_map(|line| line.runs().next())?;
  let metrics = run.metrics();
  Some((metrics.x_height, metrics.ascent, metrics.descent)).map(|(x_height, ascent, descent)| {
    ParentFontMetrics {
      x_height,
      text_metrics: (ascent, descent),
    }
  })
}

#[derive(Clone, Copy, Debug)]
/// Final vertical metrics computed for one inline line.
pub(crate) struct ResolvedLineMetrics {
  pub(crate) resolved_ascent: f32,
  pub(crate) resolved_descent: f32,
  pub(crate) resolved_leading: f32,
  pub(crate) resolved_line_height: f32,
  /// Baseline position within the line.
  pub resolved_baseline: f32,
  pub(crate) resolved_line_top: f32,
  pub(crate) resolved_line_bottom: f32,
  pub(crate) baseline_shift: f32,
}

fn quantized_baseline(line_height: f32, ascent: f32, descent: f32) -> f32 {
  let rounded_ascent = ascent.round();
  let rounded_descent = descent.round();
  let leading = line_height - (rounded_ascent + rounded_descent);
  let leading_above = (leading * 0.5).floor();
  rounded_ascent + leading_above
}

pub(super) fn text_line_box_contribution(
  line_height: f32,
  ascent: f32,
  descent: f32,
) -> (f32, f32) {
  let above = quantized_baseline(line_height, ascent, descent);
  (above, line_height - above)
}

fn parent_baseline_offset_for_box(
  line: &Line<'_, InlineBrush>,
  item: &InlineBoxItem<'_>,
  inline_box: &PositionedInlineBox,
  effective_parent_x_height: Option<f32>,
  effective_parent_text_metrics: Option<(f32, f32)>,
) -> f32 {
  let baseline_in_item = item
    .baseline_offset
    .unwrap_or(inline_box.height)
    .clamp(0.0, inline_box.height);
  let mut top = 0.0;
  item.vertical_align.apply(
    &mut top,
    line.metrics(),
    inline_box.height,
    Some(baseline_in_item),
    effective_parent_x_height,
    effective_parent_text_metrics,
  );
  top - (line.metrics().baseline - baseline_in_item)
}

pub(crate) fn effective_parent_x_height_for_line(
  line: &Line<'_, InlineBrush>,
  parent_font_metrics: Option<ParentFontMetrics>,
) -> Option<f32> {
  let parent_x_height = parent_font_metrics.and_then(|metrics| metrics.x_height);
  if parent_x_height.is_some() {
    return parent_x_height;
  }

  let mut text_ascent_max = 0.0_f32;
  for item in line.items() {
    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
      text_ascent_max = text_ascent_max.max(glyph_run.run().metrics().ascent);
    }
  }

  (text_ascent_max > 0.0).then_some(text_ascent_max * 0.5)
}

pub(crate) fn effective_parent_text_metrics_for_line(
  line: &Line<'_, InlineBrush>,
  parent_font_metrics: Option<ParentFontMetrics>,
) -> Option<(f32, f32)> {
  let parent_text_metrics = parent_font_metrics.map(|metrics| metrics.text_metrics);
  if parent_text_metrics.is_some() {
    return parent_text_metrics;
  }

  let mut has_glyph = false;
  for item in line.items() {
    if matches!(item, PositionedLayoutItem::GlyphRun(_)) {
      has_glyph = true;
      break;
    }
  }

  has_glyph.then_some((line.metrics().ascent, line.metrics().descent))
}

/// Resolve per-line metrics from the laid-out lines and spans.
pub(crate) fn resolve_inline_line_metrics(
  inline_layout: &InlineLayout,
  spans: &[ProcessedInlineSpan<'_>],
  parent_font_metrics: Option<ParentFontMetrics>,
  line_scales: &[f32],
) -> Vec<ResolvedLineMetrics> {
  let mut result = Vec::with_capacity(inline_layout.lines().count());
  let mut previous_parley_bottom = 0.0_f32;
  let mut previous_resolved_bottom = 0.0_f32;
  let preserve_first_line_top = spans.iter().any(|span| match span {
    ProcessedInlineSpan::Box(item) => {
      matches!(
        item.inline_box.kind,
        InlineBoxKind::CustomOutOfFlow | InlineBoxKind::OutOfFlow
      )
    }
    ProcessedInlineSpan::DirectionMark { .. }
    | ProcessedInlineSpan::Text { .. }
    | ProcessedInlineSpan::Spacer { .. } => false,
  });

  for (line_index, line) in inline_layout.lines().enumerate() {
    let line_scale = line_scales.get(line_index).copied().unwrap_or(1.0);
    let effective_parent_x_height = effective_parent_x_height_for_line(&line, parent_font_metrics);
    let effective_parent_text_metrics =
      effective_parent_text_metrics_for_line(&line, parent_font_metrics);

    let line_metrics = line.metrics();
    let mut resolved_above = 0.0_f32;
    let mut resolved_below = f32::NEG_INFINITY;
    let mut top_box_heights: Vec<f32> = Vec::new();
    let mut bottom_box_heights: Vec<f32> = Vec::new();
    let mut has_contribution = false;

    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let metrics = glyph_run.run().metrics();
          let (base_above, base_below) = glyph_run.style().brush.line_box_contribution(
            metrics.line_height,
            metrics.ascent,
            metrics.descent,
          );
          if (line_scale - 1.0).abs() <= f32::EPSILON {
            resolved_above = resolved_above.max(base_above);
            resolved_below = resolved_below.max(base_below);
          } else if glyph_run.style().brush.line_height_scales_with_text_fit {
            resolved_above = resolved_above.max(base_above * line_scale);
            resolved_below = resolved_below.max(base_below * line_scale);
          } else {
            resolved_above = resolved_above.max(base_above);
            resolved_below = resolved_below.max(base_below);
          }
          has_contribution = true;
        }
        PositionedLayoutItem::InlineBox(inline_box) => {
          if inline_box.kind != InlineBoxKind::InFlow {
            continue;
          }
          let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box.id as usize) else {
            continue;
          };
          has_contribution = true;
          // `top`/`bottom` boxes attach to the line-box edges, not the baseline, so
          // they grow only the opposite edge after baseline content is measured.
          match item.vertical_align {
            ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Top) => {
              top_box_heights.push(inline_box.height);
              continue;
            }
            ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => {
              bottom_box_heights.push(inline_box.height);
              continue;
            }
            _ => {}
          }
          let baseline_in_item = item
            .baseline_offset
            .unwrap_or(inline_box.height)
            .clamp(0.0, inline_box.height);
          let parent_baseline_offset = parent_baseline_offset_for_box(
            &line,
            item,
            &inline_box,
            effective_parent_x_height,
            effective_parent_text_metrics,
          );
          let ascent_contrib = (baseline_in_item - parent_baseline_offset).max(0.0);
          let descent_contrib =
            (inline_box.height - baseline_in_item + parent_baseline_offset).max(0.0);
          resolved_above = resolved_above.max(ascent_contrib);
          resolved_below = resolved_below.max(descent_contrib);
        }
      }
    }

    if !top_box_heights.is_empty() || !bottom_box_heights.is_empty() {
      let mut above = resolved_above.max(0.0);
      let mut below = if resolved_below.is_finite() {
        resolved_below.max(0.0)
      } else {
        0.0
      };
      for height in top_box_heights {
        below = below.max(height - above);
      }
      for height in bottom_box_heights {
        above = above.max(height - below);
      }
      resolved_above = above;
      resolved_below = below;
    }

    if !has_contribution {
      let (above, below) = text_line_box_contribution(
        line_metrics.line_height,
        line_metrics.ascent.max(0.0),
        line_metrics.descent.max(0.0),
      );
      resolved_above = above;
      resolved_below = below;
    }

    let resolved_line_height = resolved_above + resolved_below;
    let resolved_ascent = resolved_above.max(0.0);
    let resolved_descent = resolved_below.max(0.0);
    let resolved_leading = resolved_line_height - (resolved_ascent + resolved_descent);
    let interline_gap = if result.is_empty() {
      if preserve_first_line_top {
        line_metrics.block_min_coord.max(0.0)
      } else {
        0.0
      }
    } else {
      (line_metrics.block_min_coord - previous_parley_bottom).max(0.0)
    };
    let resolved_line_top = previous_resolved_bottom + interline_gap;
    let resolved_baseline = resolved_line_top + resolved_above;
    let resolved_line_bottom = resolved_line_top + resolved_line_height;
    let baseline_shift = if (resolved_baseline - line_metrics.baseline).is_finite() {
      resolved_baseline - line_metrics.baseline
    } else {
      0.0
    };

    result.push(ResolvedLineMetrics {
      resolved_ascent,
      resolved_descent,
      resolved_leading,
      resolved_line_height,
      resolved_baseline,
      resolved_line_top,
      resolved_line_bottom,
      baseline_shift,
    });

    previous_parley_bottom = line_metrics.block_max_coord;
    previous_resolved_bottom = resolved_line_bottom;
  }

  result
}

pub(crate) fn resolved_line_metrics_for_apply(
  line_metrics: &LineMetrics,
  resolved: ResolvedLineMetrics,
) -> LineMetrics {
  let mut adjusted = *line_metrics;
  adjusted.ascent = resolved.resolved_ascent;
  adjusted.descent = resolved.resolved_descent;
  adjusted.leading = resolved.resolved_leading;
  adjusted.baseline = resolved.resolved_baseline;
  adjusted.block_min_coord = resolved.resolved_line_top;
  adjusted.block_max_coord = resolved.resolved_line_bottom;
  adjusted.line_height = resolved.resolved_line_height;
  adjusted
}

#[derive(Clone, Copy, Debug)]
/// Resolved metrics and parent context for a single inline line.
pub(crate) struct ResolvedInlineLineState {
  pub(crate) adjusted_metrics: LineMetrics,
  pub(crate) parent_x_height: Option<f32>,
  pub(crate) parent_text_metrics: Option<(f32, f32)>,
}

/// Resolve per-line state used when placing inline boxes and glyphs.
pub(crate) fn resolve_inline_line_states(
  inline_layout: &InlineLayout,
  spans: &[ProcessedInlineSpan<'_>],
  parent_font_metrics: Option<ParentFontMetrics>,
  line_scales: &[f32],
) -> Vec<ResolvedInlineLineState> {
  inline_layout
    .lines()
    .zip(resolve_inline_line_metrics(
      inline_layout,
      spans,
      parent_font_metrics,
      line_scales,
    ))
    .map(|(line, resolved)| ResolvedInlineLineState {
      adjusted_metrics: resolved_line_metrics_for_apply(line.metrics(), resolved),
      parent_x_height: effective_parent_x_height_for_line(&line, parent_font_metrics),
      parent_text_metrics: effective_parent_text_metrics_for_line(&line, parent_font_metrics),
    })
    .collect()
}

pub(crate) fn normalize_inline_box(
  mut inline_box: PositionedInlineBox,
  line_state: ResolvedInlineLineState,
  spans: &[ProcessedInlineSpan<'_>],
) -> Option<PositionedInlineBox> {
  if inline_box.kind == InlineBoxKind::CustomOutOfFlow
    || inline_box.kind == InlineBoxKind::OutOfFlow
  {
    return None;
  }

  if inline_box.kind == InlineBoxKind::InFlow
    && let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box.id as usize)
  {
    item.vertical_align.apply(
      &mut inline_box.y,
      &line_state.adjusted_metrics,
      inline_box.height,
      item.baseline_offset,
      line_state.parent_x_height,
      line_state.parent_text_metrics,
    );
  }

  Some(inline_box)
}

#[derive(Clone, Copy, Debug)]
/// An inline box resolved to its painted position and size.
pub struct VisualInlineBox {
  /// Index into the span list.
  pub id: u64,
  /// Left edge.
  pub x: f32,
  /// Top edge.
  pub y: f32,
  /// Box width.
  pub width: f32,
  /// Box height.
  pub height: f32,
  /// Baseline of the in-flow line that owns this box, relative to the inline formatting context's
  /// content-box top edge.
  pub line_baseline: Option<f32>,
}

/// Resolve a positioned inline box into its painted geometry.
pub(crate) fn resolve_visual_inline_box(
  inline_box: PositionedInlineBox,
  line_state: Option<ResolvedInlineLineState>,
  spans: &[ProcessedInlineSpan<'_>],
) -> Option<VisualInlineBox> {
  let item = match spans.get(inline_box.id as usize) {
    Some(ProcessedInlineSpan::Box(item)) => item,
    // A spacer only advances the line; it keeps its layout position so
    // text-fit prefix accounting stays exact, and paints nothing (backends
    // paint boxes by matching `Box`).
    Some(ProcessedInlineSpan::Spacer { .. }) => {
      return Some(VisualInlineBox {
        id: inline_box.id,
        x: inline_box.x,
        y: inline_box.y,
        width: inline_box.width,
        height: 0.0,
        line_baseline: line_state.map(|state| state.adjusted_metrics.baseline),
      });
    }
    _ => return None,
  };

  let line_baseline = line_state.map(|state| state.adjusted_metrics.baseline);
  let positioned = if inline_box.kind == InlineBoxKind::InFlow {
    normalize_inline_box(inline_box, line_state?, spans)?
  } else {
    inline_box
  };

  Some(VisualInlineBox {
    id: positioned.id,
    x: positioned.x,
    y: positioned.y,
    width: item.paint_width,
    height: item.paint_height,
    line_baseline,
  })
}

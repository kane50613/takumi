//! `text-fit` scaling of lines to the available width.

use crate::{
  font_style::SizedFontStyle,
  geometry::Point,
  style::{Affine, TextFitMode, TextFitTarget},
};
use parley::{BreakReason, InlineBoxKind, Line, PositionedInlineBox, PositionedLayoutItem};

use super::{InlineBrush, InlineLayout};

fn text_fit_line_is_scalable(
  line: &Line<'_, InlineBrush>,
  line_index: usize,
  line_count: usize,
  target: TextFitTarget,
) -> bool {
  if target != TextFitTarget::PerLine {
    return true;
  }

  line_index + 1 != line_count && line.break_reason() != BreakReason::Explicit
}

fn clamp_text_fit_scale(style: &SizedFontStyle, scale: f32) -> f32 {
  match (style.parent.text_fit.mode, style.parent.text_fit.limit) {
    (TextFitMode::Grow, Some(limit)) if limit >= 1.0 => scale.min(limit),
    (TextFitMode::Shrink, Some(limit)) if limit <= 1.0 => scale.max(limit),
    _ => scale,
  }
}

/// Blink's float carve-out from `text_fit_utils.cc`; in-flow inline boxes scale.
pub(super) fn text_fit_is_applicable(positioned_floats: &[PositionedInlineBox]) -> bool {
  positioned_floats.is_empty()
}

/// Returns `(text_advance, static_advance)` for a line.
fn text_fit_line_advance(line: &Line<'_, InlineBrush>) -> (f32, f32) {
  let metrics = line.metrics();
  let static_advance: f32 = line
    .items()
    .filter_map(|item| match item {
      PositionedLayoutItem::InlineBox(b) if b.kind == InlineBoxKind::InFlow => Some(b.width),
      _ => None,
    })
    .sum();
  let text_advance = (metrics.advance - metrics.trailing_whitespace - static_advance).max(0.0);
  (text_advance, static_advance)
}

/// Naive next to Blink's `text_fit_utils.cc`: fixed letter/word-spacing scales
/// with the glyphs instead of staying constant, though the fitted line width
/// matches.
pub(super) fn text_fit_line_scales(
  layout: &InlineLayout,
  max_width: f32,
  style: &SizedFontStyle,
) -> Vec<f32> {
  let text_fit = style.parent.text_fit;
  if text_fit.mode == TextFitMode::None || !max_width.is_finite() {
    return Vec::new();
  }

  let line_count = layout.lines().count();
  if line_count == 0 {
    return Vec::new();
  }

  let mut scales: Vec<(usize, f32)> = Vec::with_capacity(line_count);
  for (index, line) in layout.lines().enumerate() {
    if !text_fit_line_is_scalable(&line, index, line_count, text_fit.target) {
      continue;
    }

    let (text_advance, static_advance) = text_fit_line_advance(&line);
    let flexible_fit_width =
      (max_width - line.metrics().inline_min_coord - static_advance).max(0.0);

    if text_advance <= 0.0 {
      continue;
    }
    if flexible_fit_width <= 0.0 && text_fit.mode != TextFitMode::Shrink {
      continue;
    }

    let scale = match text_fit.mode {
      TextFitMode::Grow if text_advance < flexible_fit_width => flexible_fit_width / text_advance,
      TextFitMode::Shrink if text_advance > flexible_fit_width => flexible_fit_width / text_advance,
      _ => 1.0,
    };
    scales.push((index, clamp_text_fit_scale(style, scale)));
  }

  if text_fit.target == TextFitTarget::Consistent {
    let raw = match text_fit.mode {
      TextFitMode::Grow => scales.iter().map(|(_, s)| *s).fold(f32::INFINITY, f32::min),
      TextFitMode::Shrink => scales
        .iter()
        .map(|(_, s)| *s)
        .filter(|s| *s < 1.0)
        .fold(1.0_f32, f32::min),
      TextFitMode::None => 1.0,
    };
    let consistent_scale = if raw.is_finite() {
      clamp_text_fit_scale(style, raw)
    } else {
      1.0
    };
    return vec![consistent_scale; line_count];
  }

  let mut result = vec![1.0; line_count];
  for (index, scale) in scales {
    result[index] = scale;
  }
  result
}

/// Line start and offset correction for a scaled text-fit line.
pub(crate) fn text_fit_line_alignment_correction(
  line: &Line<'_, InlineBrush>,
  line_scale: f32,
  container_width: f32,
) -> (f32, f32) {
  let metrics = line.metrics();
  let line_start = metrics.inline_min_coord + metrics.offset;

  if (line_scale - 1.0).abs() <= f32::EPSILON {
    return (line_start, 0.0);
  }

  let (text_advance, static_advance) = text_fit_line_advance(line);
  let scaled_line_width = static_advance + text_advance * line_scale;

  // free_space_pre_scale = room left for alignment before text-fit scaling.
  // metrics.offset encodes alignment shift (LTR start = 0, center = 0.5×free, end = free).
  // For RTL, offset is negative (−trailing_whitespace); clamping ratio to [0,1] handles it.
  let line_width = metrics.inline_max_coord - metrics.inline_min_coord;
  let free_space_pre_scale = (line_width - static_advance - text_advance).max(0.0);
  let align_ratio = if free_space_pre_scale > 0.0 {
    (metrics.offset / free_space_pre_scale).clamp(0.0, 1.0)
  } else {
    if metrics.offset < 0.0 { 1.0 } else { 0.0 }
  };

  let free_space_post_scale = (container_width - scaled_line_width).max(0.0);
  let aligned_line_start = metrics.inline_min_coord + free_space_post_scale * align_ratio;

  (line_start, aligned_line_start - line_start)
}

/// Per-line text-fit scaling state: `scale` applied about `layout_origin`, plus the horizontal
/// `alignment_correction` for a scaled-down line.
#[derive(Clone, Copy)]
pub(crate) struct LineScaleState {
  /// Text-fit scale factor for the line.
  pub(crate) scale: f32,
  /// Horizontal correction keeping a scaled line aligned.
  pub(crate) alignment_correction: f32,
  /// The origin the scale is applied about (border/padding + baseline).
  pub(crate) layout_origin: Point<f32>,
}

/// Horizontal correction for a text-fit-scaled line: `static_inline_prefix * (1
pub(crate) fn text_fit_x_correction(
  scale: f32,
  static_inline_prefix: f32,
  alignment_correction: f32,
) -> f32 {
  static_inline_prefix * (1.0 - scale) + alignment_correction
}

impl LineScaleState {
  /// Composes the affine transform for a glyph run on this (possibly scaled) line: `base *
  /// T(x_correction) * scale-about-origin`.
  pub(crate) fn transform(self, base: Affine, static_inline_prefix: f32) -> Affine {
    let x_correction =
      text_fit_x_correction(self.scale, static_inline_prefix, self.alignment_correction);
    base
      * Affine::translation(x_correction, 0.0)
      * Affine::translation(self.layout_origin.x, self.layout_origin.y)
      * Affine::scale(self.scale, self.scale)
      * Affine::translation(-self.layout_origin.x, -self.layout_origin.y)
  }
}

/// Scales an inline box's `x` for a text-fit-scaled line, mirroring the horizontal correction in
/// [`LineScaleState::transform`].
pub(crate) fn scale_text_fit_x(
  x: f32,
  origin_x: f32,
  scale: f32,
  static_inline_prefix: f32,
  line_alignment_correction: f32,
) -> f32 {
  if (scale - 1.0).abs() <= f32::EPSILON {
    return x;
  }
  text_fit_x_correction(scale, static_inline_prefix, line_alignment_correction)
    + origin_x
    + (x - origin_x) * scale
}

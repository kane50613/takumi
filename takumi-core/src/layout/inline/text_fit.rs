//! `text-fit` scaling of lines to the available width.

use crate::{
  font_style::SizedFontStyle,
  geometry::Point,
  style::{Affine, TextFitMode, TextFitTarget},
};
use parley::{
  BreakReason, Cluster, GlyphRun, InlineBoxKind, Line, PositionedInlineBox, PositionedLayoutItem,
};

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

/// The fixed letter- and word-spacing `cluster` carries, which `text-fit` leaves unscaled as
/// Blink's `PercentageSpacingDescription` drops it before measuring.
fn cluster_fixed_spacing(cluster: &Cluster<'_, InlineBrush>) -> f32 {
  let brush = &cluster.first_style().brush;
  let word = if cluster.is_space_or_nbsp() {
    brush.fixed_word_spacing
  } else {
    0.0
  };

  brush.fixed_letter_spacing + word
}

/// How far `text-fit` moves a glyph run's glyphs and end before scaling, so the fixed spacing
/// between them stays unscaled.
#[derive(Clone, Default)]
pub(crate) struct SpacingStretch {
  /// The shift of each glyph, in visual order; empty when none moves.
  shifts: Vec<f32>,
  /// The shift of the run's end.
  pub(crate) advance: f32,
}

impl SpacingStretch {
  /// The stretch of `glyph_run`, whose glyphs start at `glyph_start` among its parley run's, on a
  /// line `text-fit` scales by `scale`, with the fixed spacing it carries.
  fn of(glyph_run: &GlyphRun<'_, InlineBrush>, glyph_start: usize, scale: f32) -> (Self, f32) {
    let brush = &glyph_run.style().brush;

    if brush.fixed_letter_spacing == 0.0 && brush.fixed_word_spacing == 0.0 {
      return (Self::default(), 0.0);
    }

    let stretch = (1.0 - scale) / scale;
    let mut shifts = Vec::new();
    let mut total = 0.0;

    for after in glyph_run
      .run()
      .visual_clusters()
      .flat_map(|cluster| {
        let spacing = cluster_fixed_spacing(&cluster);
        let count = cluster.glyphs().count();

        (0..count).map(move |index| if index + 1 == count { spacing } else { 0.0 })
      })
      .skip(glyph_start)
      .take(glyph_run.glyphs().count())
    {
      shifts.push(total * stretch);
      total += after;
    }

    (
      Self {
        shifts,
        advance: total * stretch,
      },
      total,
    )
  }

  /// The shift of the glyph at `index`.
  pub(crate) fn shift(&self, index: usize) -> f32 {
    self.shifts.get(index).copied().unwrap_or(0.0)
  }
}

/// Where each glyph run on a line starts among its parley run's glyphs, tracked as parley's
/// `GlyphRunIter` splits a run where the style changes.
#[derive(Default)]
pub(crate) struct GlyphCursor {
  run: Option<usize>,
  end: usize,
}

impl GlyphCursor {
  /// The stretch of the next glyph run, `glyph_run`, on a line `text-fit` scales by `scale`, with
  /// the fixed spacing it carries.
  pub(crate) fn stretch(
    &mut self,
    glyph_run: &GlyphRun<'_, InlineBrush>,
    scale: f32,
  ) -> (SpacingStretch, f32) {
    if (scale - 1.0).abs() <= f32::EPSILON {
      return (SpacingStretch::default(), 0.0);
    }

    let run = glyph_run.run().index();
    let start = if self.run == Some(run) { self.end } else { 0 };

    self.run = Some(run);
    self.end = start + glyph_run.glyphs().count();
    SpacingStretch::of(glyph_run, start, scale)
  }
}

/// The fixed spacing on `line` outside its trailing whitespace, whose advance `text-fit` leaves
/// out, in a paragraph that is right-to-left when `rtl`.
fn line_fixed_spacing(line: &Line<'_, InlineBrush>, rtl: bool) -> f32 {
  let mut clusters: Vec<(f32, bool)> = Vec::new();
  let mut last_run = None;

  for item in line.items() {
    let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
      continue;
    };
    let run = glyph_run.run();

    if last_run.replace(run.index()) == Some(run.index()) {
      continue;
    }
    clusters.extend(run.visual_clusters().map(|cluster| {
      (
        cluster_fixed_spacing(&cluster),
        cluster.is_space_or_nbsp() || cluster.is_hard_line_break(),
      )
    }));
  }

  let total: f32 = clusters.iter().map(|(spacing, _)| spacing).sum();
  let trailing = |clusters: &mut dyn Iterator<Item = &(f32, bool)>| -> f32 {
    clusters
      .take_while(|(_, whitespace)| *whitespace)
      .map(|(spacing, _)| spacing)
      .sum()
  };

  total
    - if rtl {
      trailing(&mut clusters.iter())
    } else {
      trailing(&mut clusters.iter().rev())
    }
}

/// Returns `(text_advance, static_advance)` for a line in a paragraph that is right-to-left when
/// `rtl`. Fixed spacing counts as static, as Blink's `LineFitter` measures text without it.
pub(super) fn text_fit_line_advance(line: &Line<'_, InlineBrush>, rtl: bool) -> (f32, f32) {
  let metrics = line.metrics();
  let boxes: f32 = line
    .items()
    .filter_map(|item| match item {
      PositionedLayoutItem::InlineBox(b) if b.kind == InlineBoxKind::InFlow => Some(b.width),
      _ => None,
    })
    .sum();
  let spacing = line_fixed_spacing(line, rtl);
  let static_advance = boxes + spacing;
  let text_advance = (metrics.advance - metrics.trailing_whitespace - static_advance).max(0.0);

  (text_advance, static_advance)
}

/// The scale `text-fit` gives each line, after Blink's `text_fit_utils.cc`.
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

    let (text_advance, static_advance) = text_fit_line_advance(&line, layout.is_rtl());
    let flexible_fit_width =
      (max_width - line.metrics().inline_min_coord - static_advance).max(0.0);

    if text_advance <= 0.0 {
      continue;
    }
    // Blink's `MeasurePerBlockScale` skips a line whose fixed parts already fill it, and Chrome
    // leaves such a line unscaled under `per-line` too.
    if flexible_fit_width <= 0.0 {
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
pub(super) fn text_fit_line_alignment_correction(
  line: &Line<'_, InlineBrush>,
  line_scale: f32,
  container_width: f32,
  rtl: bool,
) -> (f32, f32) {
  let metrics = line.metrics();
  let line_start = metrics.inline_min_coord + metrics.offset;

  if (line_scale - 1.0).abs() <= f32::EPSILON {
    return (line_start, 0.0);
  }

  let (text_advance, static_advance) = text_fit_line_advance(line, rtl);
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

/// Horizontal correction for a text-fit-scaled line:
/// `static_inline_prefix * (1 - scale) + alignment_correction`.
pub(super) fn text_fit_x_correction(
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

//! `text-fit` scaling of lines to the available width.

use crate::{
  font_style::SizedFontStyle,
  geometry::Point,
  style::{Affine, TextAlign, TextFit, TextFitMode, TextFitTarget},
};
use parley::{
  BreakReason, Cluster, GlyphRun, InlineBoxKind, Line, PositionedInlineBox, PositionedLayoutItem,
};

use super::{InlineBrush, InlineLayout, LineIndent};

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

/// How `text-fit` fits one line, as Blink's `LineFitter::FitLine` leaves it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineFit {
  /// Blink's `TextFitScale`: how much the line's text grows or shrinks.
  pub(crate) scale: f32,
  /// Whether the line's text reshapes at the scaled font size, as Blink reshapes a line holding
  /// fixed spacing, instead of scaling as it paints.
  pub(crate) reshaped: bool,
  /// Where the fitted line's content starts from the line's start, as Blink offsets it by
  /// `text-indent` and `text-align` after fitting.
  pub(crate) start: f32,
}

impl LineFit {
  /// A line `text-fit` leaves alone.
  pub(crate) const NONE: Self = Self {
    scale: 1.0,
    reshaped: false,
    start: 0.0,
  };

  /// How the line sizes the text of a box, the root inline box when `root`. Blink's root box
  /// always scales as it paints, even on a reshaped line.
  pub(crate) fn text_scale(self, root: bool) -> TextScale {
    if self.reshaped && !root {
      TextScale::Font(self.scale)
    } else {
      TextScale::Paint(self.scale)
    }
  }
}

/// How a box's text is sized on a `text-fit` line, as Blink's `TextFitBlockScale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TextScale {
  /// At the font's own size, scaled by the factor as it paints.
  Paint(f32),
  /// At the font's size times the factor.
  Font(f32),
}

impl TextScale {
  /// The factor the font's own size is multiplied by.
  pub(crate) fn font(self) -> f32 {
    match self {
      Self::Paint(_) => 1.0,
      Self::Font(scale) => scale,
    }
  }
}

/// Blink's `RestrictScale`: `scale` held to the `text-fit` limit on the side it moves toward.
fn restrict_scale(scale: f32, is_grow: bool, limit: Option<f32>) -> f32 {
  let Some(limit) = limit else {
    return scale;
  };

  if is_grow {
    scale.min(limit.max(1.0))
  } else {
    scale.max(limit.min(1.0))
  }
}

/// One line as `text-fit` measures it.
struct FitLine {
  /// The available width less the line box's width, which leaves out `text-indent`.
  remaining: f32,
  /// The line's `text-indent`.
  indent: f32,
  /// The width of the parts that scale: text without its fixed spacing.
  flexible: f32,
  /// The width of the parts that do not: inline boxes and fixed spacing.
  fixed: f32,
}

/// How `text-fit` fits each line, after Blink's `LineFitter::MeasureScale` for the per-line
/// targets and `MeasurePerBlockScale` with its caller for `consistent` (`text_fit_utils.cc`,
/// `block_layout_algorithm.cc`).
pub(super) fn text_fit_lines(
  layout: &InlineLayout,
  max_width: f32,
  style: &SizedFontStyle,
) -> Vec<LineFit> {
  let text_fit = style.parent.rare_inherited_data.text_fit;
  if text_fit.mode == TextFitMode::None || !max_width.is_finite() {
    return Vec::new();
  }

  let line_count = layout.lines().count();
  if line_count == 0 {
    return Vec::new();
  }

  let epsilon = 2.0 * style.sizing.viewport.device_pixel_ratio;
  let is_grow = text_fit.mode == TextFitMode::Grow;
  let rtl = layout.is_rtl();
  let indents = LineIndent::of(style, max_width).per_line(layout);
  let lines: Vec<FitLine> = layout
    .lines()
    .zip(indents)
    .map(|(line, indent)| {
      let (flexible, fixed) = text_fit_line_advance(&line, rtl);

      FitLine {
        remaining: max_width - line.metrics().inline_min_coord - flexible - fixed,
        indent,
        flexible,
        fixed,
      }
    })
    .collect();
  let align = style.parent.inherited_data.text_align;

  if text_fit.target == TextFitTarget::Consistent {
    let minimum = lines
      .iter()
      .filter(|line| {
        line.remaining.abs() >= epsilon
          && line.flexible > 0.0
          && line.remaining + line.flexible > 0.0
      })
      .map(|line| (line.remaining + line.flexible) / line.flexible)
      .fold(f32::INFINITY, f32::min);
    let scale = if minimum.is_finite() {
      restrict_scale(minimum, is_grow, text_fit.limit)
    } else {
      1.0
    };
    let applies = (scale < 1.0 && !is_grow) || (scale > 1.0 && is_grow);

    let scale = if applies { scale } else { 1.0 };

    return layout
      .lines()
      .zip(&lines)
      .map(|(line, fit)| LineFit {
        scale,
        reshaped: line_has_fixed_spacing(&line),
        start: fit.start(&line, scale, align, rtl),
      })
      .collect();
  }

  layout
    .lines()
    .zip(&lines)
    .enumerate()
    .map(|(index, (line, fit))| {
      let scale = per_line_scale(&line, fit, index, line_count, max_width, epsilon, text_fit);

      LineFit {
        scale,
        reshaped: line_has_fixed_spacing(&line),
        start: fit.start(&line, scale, align, rtl),
      }
    })
    .collect()
}

impl FitLine {
  /// Where the line's content starts from the line's start once its text scales by `scale`: past
  /// `text-indent` and the `text-align` offset of the space left, as Blink's
  /// `InlineLayoutAlgorithm::CreateLine` places a fitted line. Naive for `justify`, which aligns
  /// to the start where Blink would justify the space a capped scale leaves.
  fn start(&self, line: &Line<'_, InlineBrush>, scale: f32, align: TextAlign, rtl: bool) -> f32 {
    let metrics = line.metrics();
    let available = metrics.inline_max_coord - metrics.inline_min_coord;
    let space = available - self.indent - self.fixed - self.flexible * scale;
    let offset = line_offset_for_text_align(align, rtl, space);

    if rtl {
      offset - metrics.trailing_whitespace * scale
    } else {
      self.indent + offset
    }
  }
}

/// Blink's `LineOffsetForTextAlign`: how far `text-align` moves a line with `space` left over,
/// where a line too wide spills past the end its direction flows to.
fn line_offset_for_text_align(align: TextAlign, rtl: bool, space: f32) -> f32 {
  let align = match (align, rtl) {
    (TextAlign::Start | TextAlign::Justify, false) | (TextAlign::End, true) => TextAlign::Left,
    (TextAlign::Start | TextAlign::Justify, true) | (TextAlign::End, false) => TextAlign::Right,
    (align, _) => align,
  };

  match align {
    TextAlign::Right if rtl => space,
    TextAlign::Right => space.max(0.0),
    TextAlign::Center if rtl && space <= 0.0 => space,
    TextAlign::Center => (space / 2.0).max(0.0),
    _ if rtl => space.min(0.0),
    _ => 0.0,
  }
}

/// The scale `text-fit` gives the line `index` of `line_count` when each line fits by itself, as
/// Blink's `LineFitter::MeasureScale`.
fn per_line_scale(
  line: &Line<'_, InlineBrush>,
  fit: &FitLine,
  index: usize,
  line_count: usize,
  max_width: f32,
  epsilon: f32,
  text_fit: TextFit,
) -> f32 {
  let is_grow = text_fit.mode == TextFitMode::Grow;
  let remaining = fit.remaining - fit.indent;
  let applies = (remaining > 0.0 && is_grow) || (remaining < 0.0 && !is_grow);

  if remaining.abs() < epsilon
    || !applies
    || !text_fit_line_is_scalable(line, index, line_count, text_fit.target)
    || fit.flexible <= 0.0
  {
    return 1.0;
  }

  let room = max_width - fit.fixed;

  // Chrome leaves a line whose fixed parts already fill it unscaled.
  if room <= 0.0 {
    return 1.0;
  }

  restrict_scale(room / fit.flexible, is_grow, text_fit.limit)
}

/// Whether any text on `line` has fixed spacing, which makes Blink reshape the line, as its
/// `HasFixedSpacing`.
fn line_has_fixed_spacing(line: &Line<'_, InlineBrush>) -> bool {
  line.items().any(|item| match item {
    PositionedLayoutItem::GlyphRun(glyph_run) => {
      let brush = &glyph_run.style().brush;

      brush.fixed_letter_spacing != 0.0 || brush.fixed_word_spacing != 0.0
    }
    PositionedLayoutItem::InlineBox(_) => false,
  })
}

/// The line's start and how far `text-fit` moves it to where the fitted line starts.
pub(super) fn text_fit_line_alignment_correction(
  line: &Line<'_, InlineBrush>,
  fit: LineFit,
) -> (f32, f32) {
  let metrics = line.metrics();
  let line_start = metrics.inline_min_coord + metrics.offset;

  if (fit.scale - 1.0).abs() <= f32::EPSILON {
    return (line_start, 0.0);
  }

  (
    line_start,
    metrics.inline_min_coord + fit.start - line_start,
  )
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

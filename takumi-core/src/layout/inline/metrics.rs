//! Vertical line metrics: line-height, baselines and vertical-align.

use crate::{
  context::RenderContext, font_style::SizedFontStyle, resources::font::PrimaryFontMetrics,
};
use parley::{InlineBoxKind, LineMetrics, PositionedInlineBox, PositionedLayoutItem};

use super::{
  InlineBrush, InlineLayout, LineFit, TextScale,
  items::ProcessedInlineSpan,
  line_box::{BoxFont, BoxKey, FontHeight, LineBoxOffsets, LineBoxTree},
  text_style_with_span_id,
};

/// An inline box's strut: its primary font's content area with its line height's half-leading,
/// as Blink's `InlineBoxState::ComputeTextMetrics`.
#[derive(Clone)]
pub(crate) struct Strut {
  brush: InlineBrush,
  metrics: PrimaryFontMetrics,
}

impl Strut {
  /// The strut of the inline box `style` sizes in `context`.
  pub(crate) fn of(context: &RenderContext, style: &SizedFontStyle<'_>) -> Option<Self> {
    Some(Self {
      brush: text_style_with_span_id(style, None).brush,
      metrics: context.primary_font_metrics(&context.style, context.sizing.font_size)?,
    })
  }

  /// The strut with its text sized by `scale`.
  pub(super) fn height(&self, scale: TextScale) -> FontHeight {
    let exact = self.metrics.exact;

    self
      .brush
      .line_box_height(exact.ascent, exact.descent, exact.line_gap, scale)
  }
}

#[derive(Clone, Debug)]
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
  /// Where each box on the line sits below its baseline.
  pub(crate) offsets: LineBoxOffsets,
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

/// Resolve per-line metrics from the laid-out lines and spans.
pub(super) fn resolve_inline_line_metrics(
  inline_layout: &InlineLayout,
  spans: &[ProcessedInlineSpan<'_>],
  font: BoxFont,
  line_fits: &[LineFit],
  strut: Option<&Strut>,
) -> Vec<ResolvedLineMetrics> {
  let mut result = Vec::with_capacity(inline_layout.lines().count());
  let mut previous_parley_bottom = 0.0_f32;
  let mut previous_resolved_bottom = 0.0_f32;
  let has_boxes = spans.iter().any(|span| {
    matches!(
      span,
      ProcessedInlineSpan::Box(_) | ProcessedInlineSpan::Spacer { .. }
    )
  });
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
    let fit = line_fits.get(line_index).copied().unwrap_or(LineFit::NONE);
    let line_metrics = line.metrics();
    let mut tree = LineBoxTree::new(FontHeight::EMPTY, font, fit);
    let mut has_contribution = false;

    // Walking runs by cluster style skips the per-fragment glyph re-walk that
    // `line.items()` does, and boxes only exist when a span holds one.
    for run in line.runs() {
      let metrics = run.metrics();
      let mut seen = None;
      for cluster in run.clusters() {
        let Some(glyph) = cluster.glyphs().next() else {
          continue;
        };
        if seen == Some(glyph.style_index()) {
          continue;
        }
        seen = Some(glyph.style_index());
        let style = cluster.first_style();
        let chain = match style
          .brush
          .source_span_id
          .and_then(|span_id| spans.get(span_id as usize))
        {
          Some(ProcessedInlineSpan::Text { decorations, .. }) => decorations.as_ref(),
          _ => None,
        };
        let parent = tree.open_chain(chain);

        tree.add(
          parent,
          style.brush.line_box_height(
            metrics.ascent,
            metrics.descent,
            metrics.leading,
            fit.text_scale(parent == 0),
          ),
        );
        has_contribution = true;
      }
    }

    for item in has_boxes.then(|| line.items()).into_iter().flatten() {
      let PositionedLayoutItem::InlineBox(inline_box) = item else {
        continue;
      };
      let item = match spans.get(inline_box.id as usize) {
        Some(ProcessedInlineSpan::Box(item)) => item,
        Some(ProcessedInlineSpan::Spacer { decorations, .. }) => {
          tree.open_chain(decorations.as_ref());
          continue;
        }
        _ => continue,
      };

      if item.render_node.is_out_of_flow() {
        tree.open_chain(item.decorations.as_ref());
      }
      if inline_box.kind != InlineBoxKind::InFlow {
        continue;
      }

      let parent = tree.open_chain(item.decorations.as_ref());
      let baseline_in_item = item
        .baseline_offset
        .unwrap_or(inline_box.height)
        .clamp(0.0, inline_box.height);

      tree.open(
        BoxKey::Atomic(inline_box.id),
        parent,
        FontHeight::nearest(baseline_in_item, inline_box.height - baseline_in_item),
        item.vertical_align,
        None,
      );
      has_contribution = true;
    }

    // CSS 2 §10.8.1: each line box starts with the root inline box's strut, but a line with no
    // content has zero height.
    if has_contribution && let Some(strut) = strut {
      tree.add(0, strut.height(fit.text_scale(true)));
    }

    let (height, offsets) = tree.resolve();
    let (resolved_above, resolved_below) = if has_contribution {
      (height.ascent.to_f32().max(0.0), height.descent.to_f32())
    } else {
      text_line_box_contribution(
        line_metrics.line_height,
        line_metrics.ascent.max(0.0),
        line_metrics.descent.max(0.0),
      )
    };

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
      offsets,
    });

    previous_parley_bottom = line_metrics.block_max_coord;
    previous_resolved_bottom = resolved_line_bottom;
  }

  result
}

impl ResolvedLineMetrics {
  /// Parley's `line_metrics` with the vertical metrics replaced by these.
  fn apply_to(&self, line_metrics: &LineMetrics) -> LineMetrics {
    let mut adjusted = *line_metrics;
    adjusted.ascent = self.resolved_ascent;
    adjusted.descent = self.resolved_descent;
    adjusted.leading = self.resolved_leading;
    adjusted.baseline = self.resolved_baseline;
    adjusted.block_min_coord = self.resolved_line_top;
    adjusted.block_max_coord = self.resolved_line_bottom;
    adjusted.line_height = self.resolved_line_height;
    adjusted
  }
}

#[derive(Clone, Debug)]
/// Resolved metrics for a single inline line.
pub(crate) struct ResolvedInlineLineState {
  pub(crate) adjusted_metrics: LineMetrics,
  /// Where each box on the line sits below its baseline.
  pub(crate) offsets: LineBoxOffsets,
}

/// Resolve per-line state used when placing inline boxes and glyphs.
pub(super) fn resolve_inline_line_states(
  inline_layout: &InlineLayout,
  line_metrics: &[ResolvedLineMetrics],
) -> Vec<ResolvedInlineLineState> {
  inline_layout
    .lines()
    .zip(line_metrics)
    .map(|(line, resolved)| ResolvedInlineLineState {
      adjusted_metrics: resolved.apply_to(line.metrics()),
      offsets: resolved.offsets.clone(),
    })
    .collect()
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
  /// How the box sits in its line.
  pub kind: InlineBoxKind,
}

/// Which draws of an inline formatting context one pass paints: CSS 2.1 Appendix E paints the
/// floats in a phase of their own, before the line content, and out-of-flow boxes with their
/// stacking context rather than here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlinePass {
  /// The text and every box that does not float.
  Content,
  /// The floats alone.
  Floats,
}

impl InlinePass {
  /// Whether the pass paints `inline_box`.
  pub fn paints(self, inline_box: &VisualInlineBox) -> bool {
    match inline_box.kind {
      InlineBoxKind::InFlow => self == Self::Content,
      InlineBoxKind::CustomOutOfFlow => self == Self::Floats,
      InlineBoxKind::OutOfFlow => false,
    }
  }
}

/// Resolve a positioned inline box into its painted geometry.
pub(super) fn resolve_visual_inline_box(
  inline_box: PositionedInlineBox,
  line_state: Option<&ResolvedInlineLineState>,
  spans: &[ProcessedInlineSpan<'_>],
) -> Option<VisualInlineBox> {
  let line_baseline = line_state.map(|state| state.adjusted_metrics.baseline);
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
        line_baseline,
        kind: InlineBoxKind::InFlow,
      });
    }
    _ => return None,
  };
  let mut y = inline_box.y;

  if inline_box.kind == InlineBoxKind::InFlow {
    let line_state = line_state?;
    let baseline_in_item = item
      .baseline_offset
      .unwrap_or(inline_box.height)
      .clamp(0.0, inline_box.height);

    y = line_state.adjusted_metrics.baseline + line_state.offsets.of(BoxKey::Atomic(inline_box.id))
      - baseline_in_item;
  }

  Some(VisualInlineBox {
    id: inline_box.id,
    x: inline_box.x,
    y,
    width: item.paint_width,
    height: item.paint_height,
    line_baseline,
    kind: item.render_node.inline_box_kind(),
  })
}

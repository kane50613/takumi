use crate::{
  context::RenderContext,
  font_style::{BoxLineHeight, SizedFontStyle, contains_variation_selector, presentation_segments},
  geometry::{AvailableSpace, ComputedLayout, LAYOUT_UNIT_EPSILON, Point, Rect, Size},
  layout::tree::RenderNode,
  layout_unit::LayoutUnit,
  resources::font::FontClasses,
  style::{
    AppliedTextDecorations, Color, Direction, FontSynthesis, Lang, Length, TextDecorationSkipInk,
    TextFitMode, TextOverflow, TextWrapStyle, VerticalAlign, WhiteSpaceCollapse, WordBreak,
  },
  text_processing::{
    MaxHeight, RebreakOptions, apply_text_transform, apply_white_space_collapse,
    make_balanced_text, make_pretty_text,
  },
};
use parley::{
  BreakReason, GlyphRun, IndentOptions, InlineBox, InlineBoxKind, Line, PositionedInlineBox,
  PositionedLayoutItem, TextStyle, TreeBuilder,
};
use std::{
  convert::Infallible,
  hash::{Hash, Hasher},
  rc::Rc,
};
use xxhash_rust::xxh3::Xxh3;

mod background;
mod breaking;
mod cache;
mod decorations;
mod floats;
mod items;
mod line_box;
mod metrics;
mod outline;
mod runs;
mod text_fit;
mod truncation;

pub(crate) use self::{
  background::PaddingBox,
  items::InlineOutOfFlow,
  text_fit::{LineFit, TextScale},
};
pub use self::{
  background::{FragmentBackground, InlineBackgroundFragment},
  decorations::DecorationLine,
  items::{DecorationLink, InlineBoxItem, InlineItem, ProcessedInlineSpan, collect_inline_items},
  metrics::{InlinePass, VisualInlineBox},
  outline::{InlineOutline, InlineOutlineRect, OutlineIsland, RightAngleContour},
  runs::{
    HangingWhitespace, InlineRunLayout, MeasuredInlineBox, MeasuredInlineRun, PositionedGlyph,
    PositionedInlineRun, RunMetrics, ShapedRun,
  },
};
use self::{
  breaking::distribute_trailing_whitespace,
  line_box::{BoxFont, BoxKey, FontHeight},
  metrics::{
    ResolvedInlineLineState, ResolvedLineMetrics, Strut, resolve_inline_line_metrics,
    resolve_inline_line_states, resolve_visual_inline_box,
  },
  runs::measured_run_text,
  text_fit::{
    GlyphCursor, LineScaleState, SpacingStretch, text_fit_is_applicable, text_fit_line_advance,
    text_fit_line_alignment_correction, text_fit_lines, text_fit_x_correction,
  },
  truncation::make_ellipsis_layout,
};
pub(crate) use self::{
  breaking::{LineWidths, break_lines, create_inline_constraint, has_custom_out_of_flow},
  items::InlineContentKind,
};
pub(crate) use cache::{InlineLayoutCache, MeasureCache, ShapeCache};

/// Inputs for building an inline layout.
pub struct InlineLayoutRequest<'c> {
  /// Inline items to lay out.
  pub items: Vec<InlineItem<'c>>,
  /// Available space for layout.
  pub available_space: Size<AvailableSpace>,
  /// Maximum line width.
  pub max_width: f32,
  /// Optional height/line-count clamp.
  pub max_height: Option<MaxHeight>,
  /// Resolved font style.
  pub style: &'c SizedFontStyle<'c>,
  /// Render context.
  pub context: &'c RenderContext,
  /// Measure or draw.
  pub mode: InlineLayoutMode,
  /// Whether text-only shaping may use the per-render cache.
  pub shape_cacheable: bool,
}

impl<'c> InlineLayoutRequest<'c> {
  /// A request that lays `items` into a content box.
  pub fn in_content_box(
    items: Vec<InlineItem<'c>>,
    content: Size<f32>,
    style: &'c SizedFontStyle<'c>,
    context: &'c RenderContext,
    mode: InlineLayoutMode,
  ) -> Self {
    Self {
      items,
      available_space: Size {
        width: AvailableSpace::Definite(content.width),
        height: AvailableSpace::Definite(content.height),
      },
      max_width: content.width,
      max_height: resolve_inline_max_height(style, content.height),
      style,
      context,
      mode,
      shape_cacheable: true,
    }
  }

  /// A request measured against taffy's own constraint, which is what a layout
  /// pass hands down: the box may still be sizing itself, so the wrap width
  /// comes from the available space and `box-sizing` rather than from a
  /// content box that does not exist yet.
  pub fn in_available_space(
    items: Vec<InlineItem<'c>>,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &'c SizedFontStyle<'c>,
    context: &'c RenderContext,
    mode: InlineLayoutMode,
  ) -> Self {
    let (max_width, max_height) =
      create_inline_constraint(context, available_space, known_dimensions);

    Self {
      items,
      available_space,
      max_width,
      max_height,
      style,
      context,
      mode,
      shape_cacheable: true,
    }
  }
}

/// Hashes everything shaping depends on: each span's processed text and
/// style, plus the root style and language.
fn shape_fingerprint(
  spans: &[ProcessedInlineSpan<'_>],
  style: &SizedFontStyle<'_>,
  lang: Option<&str>,
) -> u64 {
  let mut hasher = Xxh3::new();

  style.hash_shaping_inputs(&mut hasher);
  lang.hash(&mut hasher);
  for (span_id, span) in spans.iter().enumerate() {
    let (text, style): (&str, _) = match span {
      ProcessedInlineSpan::DirectionMark { direction, style } => (direction.bidi_mark(), style),
      ProcessedInlineSpan::Text { text, style, .. } => (text, style),
      ProcessedInlineSpan::Box(_) | ProcessedInlineSpan::Spacer { .. } => continue,
    };

    span_id.hash(&mut hasher);
    text.hash(&mut hasher);
    style.hash_shaping_inputs(&mut hasher);
  }
  hasher.finish()
}

/// Where an out-of-flow box sits before insets move it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StaticPosition {
  /// The inline-start and block-start corner of its margin box.
  pub(crate) point: Point<f32>,
  /// Whether the inline start is the right edge, in a right-to-left paragraph.
  pub(crate) from_end: bool,
}

/// A completed inline layout with its source text, spans, and per-line scales.
pub struct BuiltInlineLayout<'c> {
  /// The parley layout.
  pub(crate) layout: InlineLayout,
  /// Concatenated laid-out text.
  pub text: String,
  /// Processed spans backing the layout.
  pub spans: Vec<ProcessedInlineSpan<'c>>,
  /// Out-of-flow inline boxes positioned separately.
  pub(crate) positioned_floats: Vec<PositionedInlineBox>,
  /// How `text-fit` fits each line, empty when it fits none.
  pub(crate) line_fits: Vec<LineFit>,
  /// Whether a height or line limit may have dropped lines.
  pub(crate) clamped: bool,
  /// The root inline box's strut, which every line holding content grows to, or `None` when the
  /// root has no primary font.
  pub(crate) strut: Option<Strut>,
  /// The root inline box's font, which its children align against.
  pub(crate) font: BoxFont,
}

impl BuiltInlineLayout<'_> {
  /// The static position of each out-of-flow box among the spans, after Blink's
  /// `LogicalLineBuilder::PlaceOutOfFlowObjects`.
  ///
  /// Naive next to Blink: a box inside a span that opens right after a line's trailing space stays
  /// on that line, where Blink's `LineBreaker` ends the trailing run at the span's open tag and
  /// carries the span and the box to the next line.
  pub(crate) fn out_of_flow_static_positions(&self, content_width: f32) -> Vec<StaticPosition> {
    let metrics = self.line_metrics();
    let from_end = self.layout.is_rtl();
    let line_start = if from_end { content_width } else { 0.0 };
    let mut positions = Vec::new();

    for (line, metrics) in self.layout.lines().zip(&metrics) {
      let top = metrics.resolved_line_top;
      let mut preceded = false;

      for item in line.items() {
        let inline_box = match item {
          PositionedLayoutItem::GlyphRun(glyph_run) => {
            preceded |= glyph_run.glyphs().next().is_some();
            continue;
          }
          PositionedLayoutItem::InlineBox(inline_box) => inline_box,
        };

        match self.box_kind(&inline_box) {
          InlineBoxKind::InFlow => preceded = true,
          InlineBoxKind::CustomOutOfFlow => {}
          InlineBoxKind::OutOfFlow => {
            let Some(ProcessedInlineSpan::Box(item)) = self.spans.get(inline_box.id as usize)
            else {
              continue;
            };
            let inline_level = item
              .render_node
              .context
              .style
              .original_display
              .is_inline_level();

            let point = if inline_level {
              Point {
                x: inline_box.x,
                y: top,
              }
            } else {
              Point {
                x: line_start,
                y: if preceded {
                  top + metrics.resolved_line_height
                } else {
                  top
                },
              }
            };

            positions.push(StaticPosition { point, from_end });
          }
        }
      }
    }

    positions
  }

  /// How `inline_box` sits in its line, which parley's kind does not tell for an out-of-flow box.
  fn box_kind(&self, inline_box: &PositionedInlineBox) -> InlineBoxKind {
    match self.spans.get(inline_box.id as usize) {
      Some(ProcessedInlineSpan::Box(item)) => item.render_node.inline_box_kind(),
      _ => inline_box.kind,
    }
  }

  /// How `text-fit` fits the line `index`.
  pub(crate) fn line_fit(&self, index: usize) -> LineFit {
    self.line_fits.get(index).copied().unwrap_or(LineFit::NONE)
  }

  /// Resolved metrics for each line.
  pub(crate) fn line_metrics(&self) -> Vec<ResolvedLineMetrics> {
    resolve_inline_line_metrics(
      &self.layout,
      &self.spans,
      self.font,
      &self.line_fits,
      self.strut.as_ref(),
    )
  }

  /// The size the layout measures at and where its first and last lines sit.
  pub(crate) fn measure(&self, options: InlineMeasureOptions) -> InlineMeasurement {
    let InlineMeasureOptions {
      max_width,
      ceil_width,
      min_content_query,
    } = options;
    let max_run_width = self
      .layout
      .lines()
      .enumerate()
      .map(|(index, line)| {
        let metrics = line.metrics();

        if !min_content_query {
          return metrics.inline_min_coord + metrics.advance;
        }

        let (text_advance, static_advance) = text_fit_line_advance(&line, self.layout.is_rtl());
        let scale = self.line_fit(index).scale;

        metrics.inline_min_coord + static_advance + text_advance * scale
      })
      .fold(0.0, f32::max);
    let line_metrics = self.line_metrics();
    let total_height = line_metrics
      .last()
      .map(|metrics| metrics.resolved_line_bottom)
      .unwrap_or(0.0);
    let float_box_width = self
      .positioned_floats
      .iter()
      .map(|inline_box| inline_box.x + inline_box.width)
      .fold(0.0, f32::max);
    let float_box_height = self
      .positioned_floats
      .iter()
      .map(|inline_box| inline_box.y + inline_box.height)
      .fold(0.0, f32::max);

    let measured_width = if ceil_width {
      layout_unit_ceil(max_run_width.max(float_box_width))
    } else {
      max_run_width.max(float_box_width)
    };

    // Blink's fit-content width, `max(min-content, min(max-content, available))`: lines that wrapped
    // mean the content is wider than `max_width`, and the widest of them past it is a word no
    // opportunity breaks, which the min-content holds; lines that did not are the max-content.
    let wrapped = self.layout.lines().any(|line| {
      matches!(
        line.break_reason(),
        BreakReason::Regular | BreakReason::Emergency
      )
    });

    InlineMeasurement {
      size: Size {
        width: if min_content_query || !wrapped {
          measured_width
        } else {
          measured_width.max(max_width)
        },
        height: total_height.max(float_box_height),
      },
      first_baseline: line_metrics.first().map(|line| line.resolved_baseline),
      last_baseline: line_metrics.last().map(|line| line.resolved_baseline),
      clamped: self.clamped,
    }
  }

  /// Measures each glyph run's text/bounding box and each inline box's position/size, with text-fit
  /// line scaling applied.
  pub fn measure_runs(
    &self,
    layout: ComputedLayout,
  ) -> (Vec<MeasuredInlineRun<'_>>, Vec<MeasuredInlineBox>) {
    let mut runs = Vec::new();
    let mut inline_boxes = Vec::new();

    let Ok(()) = self.walk_items::<Infallible>(layout, |line, item| {
      let setup = &line.setup;

      match item {
        PlacedItem::Run {
          glyph_run,
          static_inline_prefix,
          hanging,
          stretch,
        } => {
          let span_id = glyph_run.style().brush.source_span_id;
          let text = measured_run_text(&self.text, &self.spans, &glyph_run, span_id);
          if text.is_empty()
            || (glyph_run.style().brush.is_direction_mark && glyph_run.advance() == 0.0)
          {
            return Ok(());
          }

          let baseline_shift = self.run_baseline_shift(line, &glyph_run);
          let (origin, size) = glyph_run_rect(&glyph_run, hanging, &stretch, baseline_shift);
          let (origin, size) = setup.scale_rect(origin, size, static_inline_prefix, baseline_shift);

          let link = span_id.and_then(|span_id| match self.spans.get(span_id as usize) {
            Some(ProcessedInlineSpan::Text { link, .. }) => link.as_deref(),
            _ => None,
          });

          runs.push(MeasuredInlineRun {
            text,
            x: origin.x,
            y: origin.y,
            width: size.width,
            height: size.height,
            link,
          });
        }
        PlacedItem::Box(inline_box) => {
          // A padding spacer advances the line but is not a measured box.
          if matches!(
            self.spans.get(inline_box.id as usize),
            Some(ProcessedInlineSpan::Spacer { .. })
          ) {
            return Ok(());
          }
          inline_boxes.push(MeasuredInlineBox {
            x: inline_box.x,
            y: inline_box.y,
            width: inline_box.width,
            height: inline_box.height,
          });
        }
        PlacedItem::Placeholder(_) => {}
      }
      Ok(())
    });

    for positioned_box in &self.positioned_floats {
      inline_boxes.push(MeasuredInlineBox {
        x: positioned_box.x,
        y: positioned_box.y,
        width: positioned_box.width,
        height: positioned_box.height,
      });
    }

    (runs, inline_boxes)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Whether the inline layout is built for measurement or drawing.
pub enum InlineLayoutMode {
  /// Size only; skip wrapping refinements.
  Measure,
  /// Full layout for painting.
  Draw,
}

/// Parley layout specialized to [`InlineBrush`].
pub(crate) type InlineLayout = parley::Layout<InlineBrush>;

#[derive(Clone, Copy, Debug, PartialEq)]
/// The size an inline layout measures at and where its first and last lines sit.
pub struct InlineMeasurement {
  pub(crate) size: Size<f32>,
  /// Baseline of the first line from the content-box top.
  pub(crate) first_baseline: Option<f32>,
  /// Baseline of the last line from the content-box top.
  pub(crate) last_baseline: Option<f32>,
  /// Whether a height or line limit may have dropped lines.
  pub(crate) clamped: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineMeasureOptions {
  max_width: f32,
  ceil_width: bool,
  /// A min-content query wraps at every opportunity, so the width it wrapped against neither caps
  /// the answer nor counts the spaces that pushed the breaks.
  pub(crate) min_content_query: bool,
}

impl InlineMeasureOptions {
  /// Options for measuring against `max_width` under taffy's constraint.
  pub(crate) fn new(
    max_width: f32,
    ceil_width: bool,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> Self {
    Self {
      max_width,
      ceil_width,
      min_content_query: known_dimensions.width.is_none()
        && matches!(available_space.width, AvailableSpace::MinContent),
    }
  }
}

#[derive(Clone, PartialEq, Debug)]
/// Paint attributes carried per glyph run through the inline layout.
pub struct InlineBrush {
  /// Span this run originated from, if any.
  pub source_span_id: Option<u64>,
  /// Whether this run is the synthetic direction mark, which never paints.
  pub(crate) is_direction_mark: bool,
  /// Run opacity.
  pub opacity: f32,
  /// Text fill color.
  pub color: Color,
  /// The decorations the run paints.
  pub decorations: AppliedTextDecorations,
  /// Whether decorations skip over glyph ink.
  pub decoration_skip_ink: TextDecorationSkipInk,
  /// `-webkit-text-stroke` colour, which a span may set for itself.
  pub stroke_color: Color,
  /// `-webkit-text-stroke` width in pixels.
  pub stroke_width: f32,
  pub(crate) font_synthesis: FontSynthesis,
  pub(crate) line_height: BoxLineHeight,
  /// `letter-spacing` in pixels when it is fixed rather than a percentage, which `text-fit`
  /// leaves unscaled.
  pub(crate) fixed_letter_spacing: f32,
  /// `word-spacing` in pixels when it is fixed rather than a percentage.
  pub(crate) fixed_word_spacing: f32,
  pub(crate) vertical_align: VerticalAlign,
}

impl InlineBrush {
  /// The run's line-box contribution from its font's metrics. The line height comes off the
  /// brush, since parley's run metrics can carry a neighbouring span's style at run boundaries.
  /// Under `line-height: normal` each font a run uses grows the line to its own leaded box, as
  /// Blink's `InlineBoxState::AccumulateUsedFonts` does.
  fn line_box_contribution(&self, ascent: f32, descent: f32, line_gap: f32) -> (f32, f32) {
    let height = self.line_box_height(ascent, descent, line_gap, TextScale::Paint(1.0));

    (height.ascent.to_f32(), height.descent.to_f32())
  }

  /// The run's line-box contribution with its text sized by `scale`, as Blink's
  /// `InlineBoxState::ComputeTextMetrics` measures it.
  fn line_box_height(
    &self,
    ascent: f32,
    descent: f32,
    line_gap: f32,
    scale: TextScale,
  ) -> FontHeight {
    FontHeight::text(ascent, descent, scale)
      .with_leading(self.line_height.resolve(ascent, descent, line_gap, scale))
  }
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      source_span_id: None,
      is_direction_mark: false,
      opacity: 1.0,
      color: Color::black(),
      decorations: AppliedTextDecorations::default(),
      decoration_skip_ink: TextDecorationSkipInk::default(),
      stroke_color: Color::black(),
      stroke_width: 0.0,
      font_synthesis: FontSynthesis::default(),
      line_height: BoxLineHeight::Normal,
      fixed_letter_spacing: 0.0,
      fixed_word_spacing: 0.0,
      vertical_align: VerticalAlign::default(),
    }
  }
}

fn text_style_with_span_id<'s>(
  style: &'s SizedFontStyle<'s>,
  source_span_id: Option<u64>,
) -> TextStyle<'s, 's, InlineBrush> {
  let mut text_style: TextStyle<'s, 's, InlineBrush> = style.into();
  text_style.brush.source_span_id = source_span_id;
  text_style
}

fn apply_text_indent(layout: &mut InlineLayout, style: &SizedFontStyle, max_width: f32) {
  let indent_basis = if max_width.is_finite() {
    max_width
  } else {
    0.0
  };
  let amount = style
    .parent
    .text_indent
    .resolve_px(&style.sizing, indent_basis);
  let options = IndentOptions {
    each_line: style.parent.text_indent.each_line,
    hanging: style.parent.text_indent.hanging,
  };

  layout.set_text_indent(amount, options);
}

fn inline_line_height_hint(style: &SizedFontStyle) -> f32 {
  match style.line_height {
    parley::LineHeight::Absolute(value) => value,
    parley::LineHeight::FontSizeRelative(value) | parley::LineHeight::MetricsRelative(value) => {
      value * style.sizing.font_size
    }
  }
  .max(style.sizing.font_size)
  .max(1.0)
}

pub(super) fn refresh_text_span_ranges(spans: &mut [ProcessedInlineSpan<'_>]) {
  let mut byte_offset = 0;

  for span in spans {
    match span {
      ProcessedInlineSpan::Text {
        text, byte_range, ..
      } => {
        let end = byte_offset + text.len();
        *byte_range = byte_offset..end;
        byte_offset = end;
      }
      // The mark occupies bytes in the laid-out text, so it shifts later ranges.
      ProcessedInlineSpan::DirectionMark { direction, .. } => {
        byte_offset += direction.bidi_mark().len();
      }
      ProcessedInlineSpan::Box(_) | ProcessedInlineSpan::Spacer { .. } => {}
    }
  }
}

/// Chromium's break table encodes `word-break: normal` pairs, and Blink runs
/// break-all through a separate iterator.
/// <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/text_break_iterator.cc>
///
/// Parley takes the override per builder, so one break-all span costs the
/// whole paragraph its Chromium breaks.
pub(super) fn chromium_line_breaks(spans: &[ProcessedInlineSpan<'_>]) -> bool {
  !spans.iter().any(|span| {
    matches!(
      span,
      ProcessedInlineSpan::Text { style, .. } if style.parent.word_break == WordBreak::BreakAll
    )
  })
}

/// Pushes `text` under `style`, giving each variation-selector segment a presentation-reordered
/// font stack.
pub(super) fn push_presentation_text(
  builder: &mut TreeBuilder<'_, InlineBrush>,
  style: &SizedFontStyle,
  span_id: Option<u64>,
  text: &str,
  classes: &FontClasses,
) {
  builder.push_style_span(text_style_with_span_id(style, span_id));
  if contains_variation_selector(text) {
    for (range, presentation) in presentation_segments(text) {
      match presentation {
        Some(presentation) => {
          let mut segment_style = text_style_with_span_id(style, span_id);
          segment_style.font_family = style.font_family.with_presentation(presentation, classes);
          builder.push_style_span(segment_style);
          builder.push_text(&text[range]);
          builder.pop_style_span();
        }
        None => builder.push_text(&text[range]),
      }
    }
  } else {
    builder.push_text(text);
  }
  builder.pop_style_span();
}

pub(super) fn push_spans_into_builder(
  builder: &mut TreeBuilder<'_, InlineBrush>,
  spans: &[ProcessedInlineSpan<'_>],
  classes: &FontClasses,
) {
  for (span_id, span) in spans.iter().enumerate() {
    match span {
      ProcessedInlineSpan::DirectionMark { direction, style } => {
        let mut mark_style = text_style_with_span_id(style, first_text_span_id(spans));
        mark_style.brush.is_direction_mark = true;
        builder.push_style_span(mark_style);
        builder.push_text(direction.bidi_mark());
        builder.pop_style_span();
      }
      ProcessedInlineSpan::Text { text, style, .. } => {
        push_presentation_text(builder, style, Some(span_id as u64), text, classes);
      }
      ProcessedInlineSpan::Box(item) => {
        builder.push_inline_box(item.inline_box.clone());
      }
      ProcessedInlineSpan::Spacer { inline_box, .. } => {
        builder.push_inline_box(inline_box.clone());
      }
    }
  }
}

/// The span the direction mark attributes its output to: a run the mark's cluster merged into
/// (emoji sequences) paints as the first real text span.
fn first_text_span_id(spans: &[ProcessedInlineSpan<'_>]) -> Option<u64> {
  spans
    .iter()
    .position(|span| matches!(span, ProcessedInlineSpan::Text { .. }))
    .map(|span_id| span_id as u64)
}

fn build_inline_layout_tree<'c>(
  items: &[InlineItem<'c>],
  available_space: Size<AvailableSpace>,
  style: &'c SizedFontStyle,
  context: &'c RenderContext,
  shape_cacheable: bool,
) -> BuiltInlineLayout<'c> {
  // Build spans first: measuring an inline box re-enters layout, so it must run
  // before `tree_builder` holds the shared font borrow.
  let mut spans: Vec<ProcessedInlineSpan<'c>> = Vec::new();
  let mut index_pos = 0;
  // A paragraph opens as a line does, so its leading collapsible spaces go.
  let mut previous_collapsible_space = true;
  let mut previous_was_line_break = false;

  if let Some(mark) = direction_mark_span(items, context) {
    index_pos = context.style.direction.bidi_mark().len();
    spans.push(mark);
  }

  for item in items {
    match item {
      InlineItem::Text {
        text,
        context,
        link,
        decorations,
      } => {
        let span_style = SizedFontStyle::from_style(&context.style, context);
        let transformed = apply_text_transform(text, context.style.text_transform);
        let collapsed = apply_white_space_collapse(
          &transformed,
          context.style.white_space_collapse,
          context.style.tab_size.spaces(),
          &mut previous_collapsible_space,
          &mut previous_was_line_break,
        );
        let start = index_pos;
        let end = start + collapsed.len();
        index_pos = end;

        spans.push(ProcessedInlineSpan::Text {
          byte_range: start..end,
          text: collapsed.into_owned(),
          style: Box::new(span_style),
          link: link.clone(),
          decorations: decorations.clone(),
        });
      }
      InlineItem::RenderNode {
        render_node,
        decorations,
      } => {
        spans.push(inline_box_span(
          render_node,
          decorations.clone(),
          available_space,
          index_pos,
          spans.len() as u64,
        ));
        // A float or an out-of-flow box is opaque to white space collapsing, as Blink's
        // `InlineItemsBuilder::AppendOpaque` leaves the spaces around it adjacent.
        if render_node.inline_box_kind() == InlineBoxKind::InFlow {
          previous_collapsible_space = false;
          previous_was_line_break = false;
        }
      }
      // Whitespace flags stay untouched: the padding must not change how the
      // text around it collapses.
      InlineItem::Spacer { width, decorations } => {
        spans.push(ProcessedInlineSpan::Spacer {
          inline_box: InlineBox {
            index: index_pos,
            id: spans.len() as u64,
            kind: InlineBoxKind::InFlow,
            width: *width,
            height: 0.0,
          },
          decorations: decorations.clone(),
        });
      }
    }
  }

  trim_trailing_space(&mut spans);

  let (layout, text) = shape_spans(context, &spans, style, shape_cacheable);
  let strut = Strut::of(context, style);
  let font = BoxFont::of(context);

  BuiltInlineLayout {
    layout,
    text,
    spans,
    positioned_floats: Vec::new(),
    line_fits: Vec::new(),
    clamped: false,
    strut,
    font,
  }
}

/// Drops the collapsible space a paragraph ends with, as the end of its last line removes it.
/// Spacers after the space move back with the text.
fn trim_trailing_space(spans: &mut [ProcessedInlineSpan<'_>]) {
  let Some(last) = spans
    .iter()
    .rposition(|span| !matches!(span, ProcessedInlineSpan::Spacer { .. }))
  else {
    return;
  };
  let ProcessedInlineSpan::Text {
    byte_range,
    text,
    style,
    ..
  } = &mut spans[last]
  else {
    return;
  };

  if !matches!(
    style.parent.white_space_collapse,
    WhiteSpaceCollapse::Collapse | WhiteSpaceCollapse::PreserveBreaks
  ) || !text.ends_with(' ')
  {
    return;
  }

  text.pop();
  byte_range.end -= 1;
  for span in &mut spans[last + 1..] {
    if let ProcessedInlineSpan::Spacer { inline_box, .. } = span {
      inline_box.index -= 1;
    }
  }
}

/// The direction mark a paragraph leads with. Parley has no base-direction
/// API and infers the paragraph level from the first strong character, so
/// every block leads with its direction's mark; a text-less LTR paragraph
/// already has that base level, and the mark's line metrics would inflate its
/// line box.
fn direction_mark_span<'c>(
  items: &[InlineItem<'c>],
  context: &'c RenderContext,
) -> Option<ProcessedInlineSpan<'c>> {
  let text_item_context = items.iter().find_map(|item| match item {
    InlineItem::Text { context, .. } => Some(*context),
    _ => None,
  });

  if items.is_empty() || (text_item_context.is_none() && context.style.direction != Direction::Rtl)
  {
    return None;
  }

  // The mark borrows the first text span's style so it resolves to the same
  // font and cannot skew the line's metrics, and it must not advance the
  // line: spacing applies per cluster, so a zero-width glyph would still
  // widen the paragraph by one letter-spacing.
  let mark_context = text_item_context.unwrap_or(context);
  let mut mark_style = SizedFontStyle::from_style(&mark_context.style, mark_context);
  mark_style.letter_spacing = 0.0;
  mark_style.word_spacing = 0.0;

  Some(ProcessedInlineSpan::DirectionMark {
    direction: context.style.direction,
    style: Box::new(mark_style),
  })
}

/// Measures an inline-level node and sizes the box that stands in for it.
fn inline_box_span<'c>(
  render_node: &'c RenderNode,
  decorations: Option<Rc<DecorationLink<'c>>>,
  available_space: Size<AvailableSpace>,
  index: usize,
  id: u64,
) -> ProcessedInlineSpan<'c> {
  let context = &render_node.context;
  let kind = render_node.inline_box_kind();
  let vertical_align = context
    .style
    .vertical_align
    .resolve(&context.sizing, context.sizing.line_height);
  let margin = render_node.margin_px();
  let padding = render_node.padding_px();
  let border = Rect {
    top: (
      context.style.border_top_style,
      context.style.border_top_width,
    ),
    right: (
      context.style.border_right_style,
      context.style.border_right_width,
    ),
    bottom: (
      context.style.border_bottom_style,
      context.style.border_bottom_width,
    ),
    left: (
      context.style.border_left_style,
      context.style.border_left_width,
    ),
  }
  .map(|(border_style, width)| {
    if border_style.is_rendered() {
      Length::from(width).to_px(&context.sizing, 0.0)
    } else {
      0.0
    }
  });

  // An out-of-flow box only marks its static position, so the line never sizes it.
  let atomic_metrics = render_node
    .node
    .as_ref()
    .filter(|_| kind != InlineBoxKind::OutOfFlow)
    .map(|_| render_node.measure_inline_box(available_space));
  let content_size = atomic_metrics.map_or(Size::ZERO, |metrics| metrics.size);
  let raw_baseline_offset = atomic_metrics.and_then(|metrics| metrics.baseline_offset);

  let paint_width = if render_node.participates_as_inline_box() {
    content_size.width + margin.horizontal()
  } else {
    content_size.width + margin.horizontal() + padding.horizontal() + border.horizontal()
  };
  let paint_height = if render_node.participates_as_inline_box() {
    content_size.height + margin.vertical()
  } else {
    content_size.height + margin.vertical() + padding.vertical() + border.vertical()
  };
  // Parley breaks the line after every box it places itself, while Blink never breaks at an
  // out-of-flow object, so the line breaker takes these back and appends them without one.
  let inline_box = InlineBox {
    index,
    id,
    kind: match kind {
      InlineBoxKind::OutOfFlow => InlineBoxKind::CustomOutOfFlow,
      kind => kind,
    },
    width: paint_width,
    height: paint_height,
  };
  let baseline_offset = raw_baseline_offset.map(|baseline| baseline.clamp(0.0, inline_box.height));

  ProcessedInlineSpan::Box(InlineBoxItem {
    render_node,
    decorations,
    inline_box,
    paint_width,
    paint_height,
    margin,
    padding,
    border,
    baseline_offset,
    vertical_align,
  })
}

/// Shapes `spans` into a layout, through the render's shape cache when the
/// content is text and direction marks only. Inline boxes bake
/// constraint-dependent measured sizes into the layout, so they never cache.
fn shape_spans(
  context: &RenderContext,
  spans: &[ProcessedInlineSpan<'_>],
  style: &SizedFontStyle,
  shape_cacheable: bool,
) -> (InlineLayout, String) {
  let cacheable = shape_cacheable
    && spans.iter().all(|span| {
      matches!(
        span,
        ProcessedInlineSpan::Text { .. } | ProcessedInlineSpan::DirectionMark { .. }
      )
    });
  let cache_key = cacheable
    .then(|| shape_fingerprint(spans, style, context.style.lang.as_ref().map(Lang::as_str)));
  // The stored text double-checks the fingerprint against hash collisions.
  let expected_text = cacheable.then(|| {
    spans.iter().fold(String::new(), |mut joined, span| {
      match span {
        ProcessedInlineSpan::Text { text, .. } => joined.push_str(text),
        ProcessedInlineSpan::DirectionMark { direction, .. } => {
          joined.push_str(direction.bidi_mark())
        }
        ProcessedInlineSpan::Box(_) | ProcessedInlineSpan::Spacer { .. } => {}
      }
      joined
    })
  });
  context
    .inline_cache()
    .get_or_shape(cache_key.zip(expected_text.as_deref()), || {
      context.tree_builder(style.into(), chromium_line_breaks(spans), |builder| {
        push_spans_into_builder(builder, spans, &context.fonts().classes)
      })
    })
}

/// Indents `layout` and breaks it at `options.max_width`; true when `options.max_height` may have
/// dropped lines.
pub(super) fn break_into_lines(
  layout: &mut InlineLayout,
  options: RebreakOptions,
  style: &SizedFontStyle,
  spans: &[ProcessedInlineSpan<'_>],
  positioned_floats: &mut Vec<PositionedInlineBox>,
) -> bool {
  apply_text_indent(layout, style, options.max_width);
  options.rebreak(
    layout,
    LineWidths::uniform(options.max_width),
    spans,
    positioned_floats,
  )
}

/// Build, wrap, and align the inline layout for a request.
pub fn create_inline_layout<'c>(request: InlineLayoutRequest<'c>) -> BuiltInlineLayout<'c> {
  let InlineLayoutRequest {
    items,
    available_space,
    max_width,
    max_height,
    style,
    context,
    mode,
    shape_cacheable,
  } = request;
  let mut built =
    build_inline_layout_tree(&items, available_space, style, context, shape_cacheable);
  let rebreak = RebreakOptions {
    max_width,
    max_height,
    line_height_hint: inline_line_height_hint(style),
    text_wrap_mode: style.parent.resolved_text_wrap_mode(),
  };

  built.clamped = break_into_lines(
    &mut built.layout,
    rebreak,
    style,
    &built.spans,
    &mut built.positioned_floats,
  );

  if mode == InlineLayoutMode::Draw {
    let BuiltInlineLayout {
      layout,
      text,
      spans,
      positioned_floats,
      ..
    } = &mut built;

    if style.parent.text_overflow == TextOverflow::Ellipsis {
      // A line's advance is an f32 sum over glyphs, so an exactly-fitting line
      // can land a hair past max_width and must not sprout an ellipsis.
      // Overflow shows up two ways: text truncated past the last committed
      // line, or a line wider than the box because nothing in it could break.
      // Browsers ellipsize the second case too: Blink runs
      // LineTruncator::TruncateLine on any overflowing line under
      // text-overflow: ellipsis and finds the cut with
      // ShapeResult::OffsetToFit, which walks shaped glyph positions rather
      // than break opportunities (blink/renderer/core/layout/inline/
      // line_truncator.cc). The spec never conditions ellipsing on soft wrap
      // opportunities either: https://drafts.csswg.org/css-overflow-3/#text-overflow
      let is_overflowing = layout.lines().last().is_some_and(|last_line| {
        let metrics = last_line.metrics();
        last_line.text_range().end < text.len()
          || metrics.inline_min_coord + metrics.advance - metrics.trailing_whitespace
            > max_width + LAYOUT_UNIT_EPSILON
      });

      if is_overflowing {
        make_ellipsis_layout(layout, spans, rebreak, style, context, positioned_floats);
      }
    }

    let line_count = layout.lines().count();

    if style.parent.text_wrap_style == TextWrapStyle::Balance {
      make_balanced_text(
        layout,
        rebreak,
        line_count,
        style.sizing.viewport.device_pixel_ratio,
        spans,
        positioned_floats,
      );
    }

    if style.parent.text_wrap_style == TextWrapStyle::Pretty {
      make_pretty_text(layout, rebreak, spans, positioned_floats);
    }
  }

  if style.parent.text_fit.mode != TextFitMode::None
    && text_fit_is_applicable(&built.positioned_floats)
  {
    built.line_fits = text_fit_lines(&built.layout, max_width, style);
  }

  built
    .layout
    .align(style.parent.text_align.into_parley(), Default::default());
  built
}

/// Resolve the max height constraint from line clamping and content box height.
pub(crate) fn resolve_inline_max_height(
  font_style: &SizedFontStyle,
  content_box_height: f32,
) -> Option<MaxHeight> {
  font_style
    .parent
    .clamp_lines()
    .map(|lines| MaxHeight::HeightAndLines(content_box_height, lines))
    .or_else(|| {
      (font_style.parent.text_overflow == TextOverflow::Ellipsis)
        .then_some(MaxHeight::Absolute(content_box_height))
    })
}

/// `value` rounded up to Blink's `LayoutUnit`, a 64th of a pixel, as `LayoutUnit::FromFloatCeil`
/// rounds a measured inline size.
fn layout_unit_ceil(value: f32) -> f32 {
  (value * 64.0).ceil() / 64.0
}

/// Per-line setup (scale state, baseline shift, resolved metrics) for the inline painting walk.
pub(crate) struct LineSetup {
  /// How `text-fit` fits the line.
  pub(crate) fit: LineFit,
  /// Text-fit scale state for text on the line's baseline.
  pub(crate) state: LineScaleState,
  /// Baseline shift applied to glyphs on the line.
  pub(crate) baseline_shift: f32,
  /// Pre-scale horizontal origin used for text-fit alignment.
  pub(crate) line_scale_origin_x: f32,
  /// Resolved vertical metrics for the line.
  pub(crate) resolved_metrics: ResolvedLineMetrics,
}

impl LineSetup {
  /// Resolves a line's scale state, baseline, and metrics for the inline walk.
  pub(crate) fn new(
    line: &Line<'_, InlineBrush>,
    layout: ComputedLayout,
    line_vertical_metrics: &[ResolvedLineMetrics],
    fit: LineFit,
    line_index: usize,
    rtl: bool,
  ) -> Option<Self> {
    let resolved_metrics = line_vertical_metrics.get(line_index)?.clone();
    let line_scale = fit.scale;
    let (line_scale_origin_x, alignment_correction) =
      text_fit_line_alignment_correction(line, line_scale, layout.content_box_width(), rtl);
    let content = layout.content_box_offset();

    Some(Self {
      fit,
      state: LineScaleState {
        scale: line_scale,
        alignment_correction,
        layout_origin: Point {
          x: content.x + line_scale_origin_x,
          y: content.y + resolved_metrics.resolved_baseline,
        },
      },
      baseline_shift: resolved_metrics.baseline_shift,
      line_scale_origin_x,
      resolved_metrics,
    })
  }

  /// The scale state of text shifted `baseline_shift` below the line's layout baseline, scaled
  /// about its own baseline as Blink scales a text fragment about its text origin.
  pub(crate) fn run_scale(&self, baseline_shift: f32) -> LineScaleState {
    let mut state = self.state;

    state.layout_origin.y += baseline_shift - self.baseline_shift;
    state
  }

  /// Scales a line-local `x` for text-fit, mirroring the horizontal correction in
  /// [`LineScaleState::transform`].
  pub(crate) fn scale_x(&self, x: f32, static_inline_prefix: f32) -> f32 {
    let LineScaleState {
      scale,
      alignment_correction,
      ..
    } = self.state;

    if (scale - 1.0).abs() <= f32::EPSILON {
      return x;
    }

    text_fit_x_correction(scale, static_inline_prefix, alignment_correction)
      + self.line_scale_origin_x
      + (x - self.line_scale_origin_x) * scale
  }

  /// Scales a line-local rect of text shifted `baseline_shift` below the line's layout baseline
  /// for text-fit, about the text's own baseline.
  pub(crate) fn scale_rect(
    &self,
    origin: Point<f32>,
    size: Size<f32>,
    static_inline_prefix: f32,
    baseline_shift: f32,
  ) -> (Point<f32>, Size<f32>) {
    let scale = self.state.scale;

    if (scale - 1.0).abs() <= f32::EPSILON {
      return (origin, size);
    }

    let baseline = self.resolved_metrics.resolved_baseline + baseline_shift - self.baseline_shift;

    (
      Point {
        x: self.scale_x(origin.x, static_inline_prefix),
        y: baseline + (origin.y - baseline) * scale,
      },
      Size {
        width: size.width * scale,
        height: size.height * scale,
      },
    )
  }
}

/// A glyph run's advance, stretched by `stretch`, by its ascent plus descent, as a line-local
/// top-left and size.
pub(crate) fn glyph_run_rect(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  hanging: HangingWhitespace,
  stretch: &SpacingStretch,
  baseline_shift: f32,
) -> (Point<f32>, Size<f32>) {
  let metrics = glyph_run.run().metrics();

  (
    Point {
      x: glyph_run.offset() + hanging.shift,
      y: glyph_run.baseline() + baseline_shift - metrics.ascent,
    },
    Size {
      width: glyph_run.advance() + stretch.advance,
      height: metrics.ascent + metrics.descent,
    },
  )
}

/// A line under an item walk: its index, setup, and resolved state.
pub(crate) struct WalkedLine {
  pub(crate) index: usize,
  pub(crate) setup: LineSetup,
  pub(crate) state: ResolvedInlineLineState,
}

impl WalkedLine {
  /// The baseline shift of content inside the innermost span of `chain`, which `vertical-align`
  /// moves off the line's own.
  pub(crate) fn baseline_shift_in(&self, chain: Option<&Rc<DecorationLink<'_>>>) -> f32 {
    self.setup.baseline_shift
      + chain.map_or(0.0, |link| {
        self.state.offsets.of(BoxKey::Span(link.decoration.id))
      })
  }
}

impl<'c> BuiltInlineLayout<'c> {
  /// The spans around `glyph_run`, innermost first.
  pub(crate) fn run_chain(
    &self,
    glyph_run: &GlyphRun<'_, InlineBrush>,
  ) -> Option<&Rc<DecorationLink<'c>>> {
    self.span_chain(glyph_run.style().brush.source_span_id)
  }

  /// The spans around the text span `span_id`, innermost first.
  pub(crate) fn span_chain(&self, span_id: Option<u64>) -> Option<&Rc<DecorationLink<'c>>> {
    match span_id.and_then(|span_id| self.spans.get(span_id as usize)) {
      Some(ProcessedInlineSpan::Text { decorations, .. }) => decorations.as_ref(),
      _ => None,
    }
  }

  /// The baseline shift `glyph_run` sits at on `line`.
  pub(crate) fn run_baseline_shift(
    &self,
    line: &WalkedLine,
    glyph_run: &GlyphRun<'_, InlineBrush>,
  ) -> f32 {
    line.baseline_shift_in(self.run_chain(glyph_run))
  }
}

/// One item placed on a walked line, with the static advance of the boxes before it.
pub(crate) enum PlacedItem<'a> {
  Run {
    glyph_run: GlyphRun<'a, InlineBrush>,
    static_inline_prefix: f32,
    /// The line-end whitespace this run carries.
    hanging: HangingWhitespace,
    /// How far `text-fit` moves the run's glyphs so its fixed spacing stays unscaled.
    stretch: SpacingStretch,
  },
  /// An in-flow box, its `x` already scaled for text-fit.
  Box(VisualInlineBox),
  /// Where an out-of-flow box sits in the line, its `x` already scaled for text-fit.
  Placeholder(VisualInlineBox),
}

impl BuiltInlineLayout<'_> {
  /// Visits every glyph run and in-flow box line by line, resolving the line
  /// state and text-fit prefix each visitor would otherwise track itself.
  pub(crate) fn walk_items<E>(
    &self,
    layout: ComputedLayout,
    mut visit: impl FnMut(&WalkedLine, PlacedItem<'_>) -> Result<(), E>,
  ) -> Result<(), E> {
    let line_vertical_metrics = self.line_metrics();
    let line_states = resolve_inline_line_states(&self.layout, &line_vertical_metrics);

    for (index, line) in self.layout.lines().enumerate() {
      let Some(setup) = LineSetup::new(
        &line,
        layout,
        &line_vertical_metrics,
        self.line_fit(index),
        index,
        self.layout.is_rtl(),
      ) else {
        continue;
      };
      let walked = WalkedLine {
        index,
        setup,
        state: line_states[index].clone(),
      };
      let items: Vec<_> = line.items().collect();
      let hanging = distribute_trailing_whitespace(&items, &line, self.layout.is_rtl());
      let mut static_inline_prefix = 0.0_f32;
      let mut cursor = GlyphCursor::default();

      for (item_index, item) in items.into_iter().enumerate() {
        match item {
          PositionedLayoutItem::GlyphRun(glyph_run) => {
            let (stretch, spacing) = cursor.stretch(&glyph_run, walked.setup.state.scale);

            visit(
              &walked,
              PlacedItem::Run {
                glyph_run,
                static_inline_prefix,
                hanging: hanging[item_index],
                stretch,
              },
            )?;
            static_inline_prefix += spacing;
          }
          PositionedLayoutItem::InlineBox(inline_box) => {
            let kind = self.box_kind(&inline_box);

            if kind == InlineBoxKind::CustomOutOfFlow {
              continue;
            }
            let Some(resolved) =
              resolve_visual_inline_box(inline_box, Some(&walked.state), &self.spans)
            else {
              continue;
            };
            let inline_box = VisualInlineBox {
              x: walked.setup.scale_x(resolved.x, static_inline_prefix),
              ..resolved
            };

            if kind == InlineBoxKind::OutOfFlow {
              visit(&walked, PlacedItem::Placeholder(inline_box))?;
              continue;
            }
            visit(&walked, PlacedItem::Box(inline_box))?;
            static_inline_prefix += resolved.width;
          }
        }
      }
    }

    Ok(())
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
  use std::{fs::File, io::Read, path::Path, sync::Arc};

  use super::{decorations::DecorationPlacement, runs::slice_text_at_char_boundaries, *};
  use crate::{
    Fonts,
    context::RenderContext,
    geometry::{PathCommand, Point, Rect},
    layout::{node::Node, tree::RenderNode},
    resources::font::{FontOverride, FontResource, GenericFamily},
    style::{
      Affine, AppliedTextDecoration, BorderStyle, Color, ColorInput, Display, FontSize, Length,
      Sides, SizedTextDecorationThickness, SizingContext, SpacePair, Style, StyleDeclaration,
      TextDecorationLines, TextDecorationStyle, TextUnderlinePosition, WhiteSpace,
    },
    viewport::Viewport,
  };

  fn create_test_context() -> Fonts {
    let mut context = Fonts::default();
    let path =
      Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/fonts/geist/Geist[wght].woff2");
    let mut font_data = Vec::new();
    let mut file = File::open(&path)
      .unwrap_or_else(|error| panic!("failed to open test font {}: {error}", path.display()));
    file
      .read_to_end(&mut font_data)
      .unwrap_or_else(|error| panic!("failed to read test font {}: {error}", path.display()));
    context
      .register(
        FontResource::new(font_data)
          .override_info(FontOverride {
            family_name: Some("Geist".into()),
            ..Default::default()
          })
          .generic_family(GenericFamily::SANS_SERIF),
      )
      .unwrap_or_else(|error| panic!("failed to load test font {}: {error}", path.display()));
    context
  }

  fn shaped_run() -> ShapedRun {
    ShapedRun {
      glyphs: Vec::new(),
      offset: 0.0,
      baseline: 0.0,
      advance: 0.0,
      hanging: HangingWhitespace::default(),
      brush: InlineBrush::default(),
      metrics: RunMetrics {
        ascent: 40.0,
        descent: 10.0,
        line_height: 50.0,
        underline_offset: -5.0,
        underline_size: 2.0,
      },
      font_size: 100.0,
      font_index: 0,
      text_range: 0..0,
      cluster_ranges: Vec::new(),
      variations: Vec::new(),
      synthetic_bold: None,
      synthetic_skew: None,
      font_data: parley::fontique::Blob::new(Arc::new(Vec::new())),
    }
  }

  #[test]
  fn an_explicit_zero_line_height_beats_the_run_metrics() {
    let brush = InlineBrush {
      line_height: BoxLineHeight::Length(LayoutUnit::ZERO),
      ..InlineBrush::default()
    };
    let (above, below) = brush.line_box_contribution(12.0, 4.0, 0.0);

    assert_eq!((above, below), (4.0, -4.0));
  }

  #[test]
  fn a_fully_trimmed_run_paints_no_decoration() {
    let mut run = shaped_run();
    run.brush.decorations = [AppliedTextDecoration {
      line: TextDecorationLines::UNDERLINE,
      style: TextDecorationStyle::Solid,
      color: Color::black(),
      thickness: SizedTextDecorationThickness::Value(2.0),
      underline_offset: None,
      underline_position: TextUnderlinePosition::Auto,
    }]
    .into_iter()
    .collect();
    run.advance = 5.2;
    run.hanging.advance = 5.2;
    run.offset = 10.4;

    let decorations = run.decoration_lines(
      &DecorationPlacement::default(),
      Affine::IDENTITY,
      Affine::IDENTITY,
      Point::ZERO,
    );

    assert_eq!(decorations.len(), 0);
  }

  #[test]
  fn a_line_limit_marks_the_layout_clamped() {
    let fonts = create_test_context();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();
    let node = Node::text("a\nb\nc".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with_white_space(WhiteSpace::pre_wrap()),
    );
    let render_node = RenderNode::from_node(&context, node);
    let font_style = SizedFontStyle::from_style(&render_node.context.style, &render_node.context);
    let build = |max_height| {
      create_inline_layout(InlineLayoutRequest {
        items: collect_inline_items(&render_node),
        available_space: Size {
          width: AvailableSpace::Definite(1200.0),
          height: AvailableSpace::Definite(630.0),
        },
        max_width: 1200.0,
        max_height,
        style: &font_style,
        context: &render_node.context,
        mode: InlineLayoutMode::Measure,
        shape_cacheable: false,
      })
    };

    assert!(!build(None).clamped);
    assert!(!build(Some(MaxHeight::Lines(4))).clamped);
    assert!(!build(Some(MaxHeight::Lines(3))).clamped);
    assert!(build(Some(MaxHeight::Lines(2))).clamped);
    assert!(build(Some(MaxHeight::Lines(1))).clamped);

    let three_lines = build(Some(MaxHeight::Lines(3))).layout.height();
    assert!(!build(Some(MaxHeight::Absolute(three_lines))).clamped);
    assert!(build(Some(MaxHeight::Absolute(three_lines * 2.0 / 3.0))).clamped);
  }

  #[test]
  fn trailing_whitespace_share_caps_at_the_run_advance() {
    let fonts = create_test_context();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();
    let node = Node::container([
      Node::text("ab ".to_string()),
      Node::container([Node::text(" ".to_string())]).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::font_size(FontSize::Length(Length::Px(
            40.0,
          )))),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(FontSize::Length(Length::Px(
          20.0,
        ))))
        .with_white_space(WhiteSpace::pre()),
    );
    let render_node = RenderNode::from_node(&context, node);
    let font_style = SizedFontStyle::from_style(&render_node.context.style, &render_node.context);
    let built = create_inline_layout(InlineLayoutRequest {
      items: collect_inline_items(&render_node),
      available_space: Size {
        width: AvailableSpace::Definite(1200.0),
        height: AvailableSpace::Definite(630.0),
      },
      max_width: 1200.0,
      max_height: None,
      style: &font_style,
      context: &render_node.context,
      mode: InlineLayoutMode::Draw,
      shape_cacheable: false,
    });
    let layout = ComputedLayout {
      location: Point::ZERO,
      size: Size::new(1200.0, 630.0),
      border: Rect::default(),
      padding: Rect::default(),
    };
    let runs = built.resolve_runs(&render_node.context, layout).unwrap();

    let trailing: Vec<(f32, f32)> = runs
      .runs
      .iter()
      .map(|run| (run.glyph_run.advance, run.glyph_run.hanging.advance))
      .collect();

    // The 40px space run hangs entirely; earlier runs keep what layout kept.
    let last = trailing.last().unwrap();
    assert!(
      (last.1 - last.0).abs() < 0.01,
      "last run is all whitespace: {trailing:?}"
    );
    for (advance, ws) in &trailing {
      assert!(
        ws <= advance,
        "share capped by the run advance: {trailing:?}"
      );
    }
  }

  #[test]
  fn a_descendant_font_does_not_grow_the_span_background() {
    let fonts = create_test_context();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();
    let node = Node::container([
      Node::text("Mixed ".to_string()),
      Node::container([
        Node::text("small ".to_string()),
        Node::container([Node::text("BIG".to_string())]).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Inline))
            .with(StyleDeclaration::font_size(crate::style::FontSize::Length(
              crate::style::Length::Px(34.0),
            ))),
        ),
        Node::text(" small".to_string()),
      ])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 237, 213, 255]),
          ))),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(crate::style::FontSize::Length(
          crate::style::Length::Px(20.0),
        ))),
    );
    let render_node = RenderNode::from_node(&context, node);
    let font_style = SizedFontStyle::from_style(&render_node.context.style, &render_node.context);
    let built = create_inline_layout(InlineLayoutRequest {
      items: collect_inline_items(&render_node),
      available_space: Size {
        width: AvailableSpace::Definite(1200.0),
        height: AvailableSpace::Definite(630.0),
      },
      max_width: 1200.0,
      max_height: None,
      style: &font_style,
      context: &render_node.context,
      mode: InlineLayoutMode::Draw,
      shape_cacheable: false,
    });
    let layout = ComputedLayout {
      location: crate::geometry::Point::ZERO,
      size: Size::new(1200.0, 630.0),
      border: crate::geometry::Rect::default(),
      padding: crate::geometry::Rect::default(),
    };
    let runs = built.resolve_runs(&render_node.context, layout).unwrap();

    let heights: Vec<f32> = runs.background_fragments.iter().map(|f| f.height).collect();

    assert!(!heights.is_empty(), "no background fragments resolved");
    assert!(
      heights.iter().all(|h| *h < 40.0),
      "bg grew to the BIG font: {heights:?}"
    );
  }

  #[test]
  fn a_padding_only_span_paints_a_line_height_background() {
    let fonts = create_test_context();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();
    let node = Node::container([
      Node::text("before".to_string()),
      Node::container([]).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::padding_left(crate::style::Length::Px(
            12.0,
          )))
          .with(StyleDeclaration::padding_right(crate::style::Length::Px(
            12.0,
          )))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color([255, 0, 0, 255]),
          ))),
      ),
      Node::text("after".to_string()),
    ])
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
    let render_node = RenderNode::from_node(&context, node);
    let font_style = SizedFontStyle::from_style(&render_node.context.style, &render_node.context);
    let built = create_inline_layout(InlineLayoutRequest {
      items: collect_inline_items(&render_node),
      available_space: Size {
        width: AvailableSpace::Definite(1200.0),
        height: AvailableSpace::Definite(630.0),
      },
      max_width: 1200.0,
      max_height: None,
      style: &font_style,
      context: &render_node.context,
      mode: InlineLayoutMode::Draw,
      shape_cacheable: false,
    });
    let layout = ComputedLayout {
      location: crate::geometry::Point::ZERO,
      size: Size::new(1200.0, 630.0),
      border: crate::geometry::Rect::default(),
      padding: crate::geometry::Rect::default(),
    };
    let runs = built.resolve_runs(&render_node.context, layout).unwrap();
    let fragment = runs
      .background_fragments
      .first()
      .expect("padding-only span paints a fragment");

    assert!((fragment.width - 24.0).abs() < 0.5, "{}", fragment.width);
    assert!(fragment.height > 0.0);
  }

  #[test]
  fn slice_text_at_char_boundaries_trims_invalid_utf8_edges() {
    let text = "a🦀b";

    assert_eq!(slice_text_at_char_boundaries(text, 0..3), "a");
    assert_eq!(slice_text_at_char_boundaries(text, 1..5), "🦀");
    assert_eq!(slice_text_at_char_boundaries(text, 2..5), "");
    assert_eq!(slice_text_at_char_boundaries(text, 0..text.len()), text);
  }

  fn glyph_run_segments(node: Node, fonts: &Fonts) -> Vec<(Option<u64>, String, Color)> {
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();

    let render_node = RenderNode::from_node(&context, node);
    let font_style = SizedFontStyle::from_style(&render_node.context.style, &render_node.context);
    let (max_width, max_height) = create_inline_constraint(
      &render_node.context,
      Size {
        width: AvailableSpace::Definite(1200.0),
        height: AvailableSpace::Definite(630.0),
      },
      Size::NONE,
    );
    let built = create_inline_layout(InlineLayoutRequest {
      items: collect_inline_items(&render_node),
      available_space: Size {
        width: AvailableSpace::Definite(1200.0),
        height: AvailableSpace::Definite(630.0),
      },
      max_width,
      max_height,
      style: &font_style,
      context: &render_node.context,
      mode: InlineLayoutMode::Measure,
      shape_cacheable: false,
    });

    built
      .layout
      .lines()
      .flat_map(|line| line.items())
      .filter_map(|item| match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let range = glyph_run.run().text_range();
          Some((
            glyph_run.style().brush.source_span_id,
            built.text[range].to_string(),
            glyph_run.style().brush.color,
          ))
        }
        PositionedLayoutItem::InlineBox(_) => None,
      })
      .collect()
  }

  #[test]
  fn pre_wrap_keeps_style_boundary_for_same_edge_character() {
    let fonts = create_test_context();
    let orange = Color([238, 102, 51, 255]);
    let blue = Color([26, 110, 245, 255]);

    let node = Node::container([
      Node::text("now support".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::color(ColorInput::Value(orange))),
      ),
      Node::text("\n      ".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
      Node::text("text-fit".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::color(ColorInput::Value(blue))),
      ),
      Node::text(" property.".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Length::from(300.0)))
        .with_white_space(WhiteSpace::pre_wrap()),
    );

    let segments = glyph_run_segments(node, &fonts);

    assert!(
      segments
        .iter()
        .any(|(span_id, _, color)| *span_id == Some(1) && *color == orange),
      "{segments:#?}"
    );
    assert!(
      segments
        .iter()
        .any(|(span_id, _, color)| *span_id == Some(3) && *color == blue),
      "{segments:#?}"
    );
  }

  #[test]
  fn inline_block_with_text_does_not_reenter_font_borrow() {
    let fonts = create_test_context();

    let node = Node::container([
      Node::text("before ".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
      Node::container([Node::text("inside".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))])
      .with_style(Style::default().with(StyleDeclaration::display(Display::InlineBlock))),
      Node::text(" after".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    ])
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));

    let segments = glyph_run_segments(node, &fonts);

    assert!(
      segments.iter().any(|(_, text, _)| text.contains("before")),
      "{segments:#?}"
    );
  }

  fn outline_rect(
    owner: usize,
    line_index: usize,
    x: f32,
    y: f32,
    width: f32,
  ) -> InlineOutlineRect {
    InlineOutlineRect {
      owner,
      line_index,
      x,
      y,
      width,
      height: 10.0,
      radius: Sides::default(),
      outline: InlineOutline {
        width: 0.0,
        offset: 0.0,
        color: Color::black(),
        style: BorderStyle::Solid,
      },
      opacity: 1.0,
    }
  }

  #[test]
  fn outline_islands_join_only_the_lines_that_meet() {
    // The first element's lines 0 to 2 touch and line 4 stands apart; the second has one line.
    let islands = OutlineIsland::of(&[
      outline_rect(0, 0, 0.0, 0.0, 10.0),
      outline_rect(0, 1, 0.0, 10.0, 10.0),
      outline_rect(0, 2, 0.0, 20.0, 10.0),
      outline_rect(0, 4, 0.0, 60.0, 10.0),
      outline_rect(1, 4, 40.0, 60.0, 10.0),
    ]);

    assert_eq!(islands.len(), 3);
    assert_eq!(
      islands
        .iter()
        .map(|island| island.lone_rect().is_some())
        .collect::<Vec<_>>(),
      [false, false, true]
    );
  }

  #[test]
  fn equal_width_lines_round_to_one_rectangle() {
    let rects: Vec<InlineOutlineRect> = (0..50)
      .map(|line| outline_rect(0, line, 0.0, line as f32 * 10.0, 40.0))
      .collect();
    let island = OutlineIsland::of(&rects).remove(0);
    let radius = Sides([SpacePair::from_single(4.0); 4]);
    let arcs = island
      .right_angle_path(0.0, 0.0)
      .expect("an island encloses area")
      .rounded(radius, radius)
      .iter()
      .filter(|command| matches!(command, PathCommand::CubicTo(..)))
      .count();

    assert_eq!(arcs, 4);
  }

  #[test]
  fn outline_rects_that_snap_together_touch() {
    let rect = |x: f32, width: f32| outline_rect(0, 0, x, 0.0, width);

    assert!(rect(0.0, 10.0).meets(rect(10.4, 10.0)));
    assert!(!rect(0.0, 10.0).meets(rect(10.6, 10.0)));
  }

  #[test]
  fn outline_rects_meet_by_the_whole_pixels_they_paint_with() {
    let rect = |x: f32| InlineOutlineRect {
      outline: InlineOutline {
        width: 1.5,
        offset: 0.0,
        color: Color::black(),
        style: BorderStyle::Solid,
      },
      ..outline_rect(0, 0, x, 0.0, 10.0)
    };

    assert!(rect(0.0).meets(rect(12.0)));
    assert!(!rect(0.0).meets(rect(13.0)));
  }
}

use crate::{
  context::RenderContext,
  font_style::{SizedFontStyle, contains_variation_selector, presentation_segments},
  geometry::{AvailableSpace, ComputedLayout, LAYOUT_UNIT_EPSILON, Point, Rect, Size},
  layout::tree::RenderNode,
  resources::font::FontClasses,
  style::{
    Color, Direction, FontSynthesis, Lang, Length, SizedTextDecorationThickness,
    TextDecorationLines, TextDecorationSkipInk, TextFitMode, TextOverflow, TextUnderlinePosition,
    TextWrapMode, TextWrapStyle, VerticalAlign, WordBreak,
  },
  text_processing::{
    MaxHeight, RebreakOptions, apply_text_transform, apply_white_space_collapse,
    make_balanced_text, make_pretty_text,
  },
};
use parley::{
  GlyphRun, IndentOptions, InlineBox, InlineBoxKind, Line, PositionedInlineBox,
  PositionedLayoutItem, TextStyle, TreeBuilder,
};
use std::{cell::RefCell, collections::HashMap, convert::Infallible, rc::Rc};
use xxhash_rust::xxh3::Xxh3;

mod background;
mod breaking;
mod decorations;
mod floats;
mod items;
mod metrics;
mod outline;
mod runs;
mod text_fit;
mod truncation;

pub use self::{
  background::InlineBackgroundFragment,
  decorations::DecorationRect,
  items::{DecorationLink, InlineBoxItem, InlineItem, ProcessedInlineSpan, collect_inline_items},
  metrics::VisualInlineBox,
  outline::{InlineOutlineRect, outline_island_contour, outline_islands},
  runs::{
    InlineRunLayout, MeasuredInlineBox, MeasuredInlineRun, PositionedGlyph, PositionedInlineRun,
    RunMetrics, ShapedRun,
  },
};
use self::{
  breaking::distribute_trailing_whitespace,
  items::inline_box_kind,
  metrics::text_line_box_contribution,
  runs::measured_run_text,
  text_fit::{text_fit_is_applicable, text_fit_line_scales},
  truncation::make_ellipsis_layout,
};
pub(crate) use self::{
  breaking::{break_lines, create_inline_constraint},
  items::InlineContentKind,
  metrics::{
    ParentFontMetrics, ResolvedInlineLineState, ResolvedLineMetrics, get_parent_font_metrics,
    resolve_inline_line_metrics, resolve_inline_line_states, resolve_visual_inline_box,
  },
  text_fit::{LineScaleState, scale_text_fit_x, text_fit_line_alignment_correction},
};

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

/// A per-render cache of shaped text-only parley layouts, keyed by a hash of
/// the span texts and styles. Shaping is width-independent for pure text
/// (inline boxes bake in measured sizes, so they bypass the cache); line
/// breaking and alignment run on a clone per call. A layout is stored only
/// once its fingerprint repeats (`None` marks first sight), so single-use
/// text costs one map entry, not a retained layout clone.
pub(crate) type ShapeCache = Rc<RefCell<HashMap<u64, Option<(InlineLayout, String)>>>>;

/// Per-render map from a text node's measure inputs to its measured size.
/// Keyed by content hash plus text length as a cheap collision guard.
pub(crate) type MeasureCache = Rc<RefCell<HashMap<(u64, u32), Size<f32>>>>;

/// Hashes everything shaping depends on: each span's processed text and
/// style, plus the root style and language.
fn shape_fingerprint(
  spans: &[ProcessedInlineSpan<'_>],
  style: &SizedFontStyle<'_>,
  lang: Option<&str>,
) -> u64 {
  use std::hash::{Hash, Hasher};

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
  /// Per-line text-fit scale factors.
  pub line_scales: Vec<f32>,
}

impl BuiltInlineLayout<'_> {
  /// Parent font metrics from the first run.
  pub(crate) fn parent_font_metrics(&self) -> Option<ParentFontMetrics> {
    get_parent_font_metrics(&self.layout)
  }

  /// Resolved metrics for each line.
  pub(crate) fn line_metrics(&self) -> Vec<ResolvedLineMetrics> {
    resolve_inline_line_metrics(
      &self.layout,
      &self.spans,
      self.parent_font_metrics(),
      &self.line_scales,
    )
  }

  /// Resolved state for each line.
  pub(crate) fn line_states(&self) -> Vec<ResolvedInlineLineState> {
    resolve_inline_line_states(
      &self.layout,
      &self.spans,
      self.parent_font_metrics(),
      &self.line_scales,
    )
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
      let line_scale_origin_y = setup.resolved_metrics.resolved_baseline;

      match item {
        PlacedItem::Run {
          glyph_run,
          static_inline_prefix,
          ..
        } => {
          let span_id = glyph_run.style().brush.source_span_id;
          let text = measured_run_text(&self.text, &self.spans, &glyph_run, span_id);
          if text.is_empty()
            || (glyph_run.style().brush.is_direction_mark && glyph_run.advance() == 0.0)
          {
            return Ok(());
          }

          let metrics = glyph_run.run().metrics();
          let mut x = glyph_run.offset();
          let mut y = glyph_run.baseline() + setup.baseline_shift - metrics.ascent;
          let mut width = glyph_run.advance();
          let mut height = metrics.ascent + metrics.descent;
          if (setup.state.scale - 1.0).abs() > f32::EPSILON {
            x = scale_text_fit_x(
              x,
              setup.line_scale_origin_x,
              setup.state.scale,
              static_inline_prefix,
              setup.state.alignment_correction,
            );
            y = line_scale_origin_y + (y - line_scale_origin_y) * setup.state.scale;
            width *= setup.state.scale;
            height *= setup.state.scale;
          }

          let link = span_id.and_then(|span_id| match self.spans.get(span_id as usize) {
            Some(ProcessedInlineSpan::Text { link, .. }) => link.as_deref(),
            _ => None,
          });

          runs.push(MeasuredInlineRun {
            text,
            x,
            y,
            width,
            height,
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineMeasureOptions {
  pub(crate) max_width: f32,
  pub(crate) ceil_width: bool,
  pub(crate) parent_font_metrics: Option<ParentFontMetrics>,
  /// A min-content query wraps at zero and reports the widest run it could not break, so the width
  /// it wrapped against must not cap the answer.
  pub(crate) clamp_to_max_width: bool,
}

#[derive(Clone, PartialEq, Copy, Debug)]
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
  /// Text decoration color.
  pub decoration_color: Color,
  /// Decoration line thickness.
  pub decoration_thickness: SizedTextDecorationThickness,
  /// Extra offset of the underline away from the text, in pixels.
  pub underline_offset: f32,
  /// Which baseline the underline is measured from.
  pub underline_position: TextUnderlinePosition,
  /// Which decoration lines to draw.
  pub decoration_line: TextDecorationLines,
  /// Whether decorations skip over glyph ink.
  pub decoration_skip_ink: TextDecorationSkipInk,
  /// `-webkit-text-stroke` colour, which a span may set for itself.
  pub stroke_color: Color,
  /// `-webkit-text-stroke` width in pixels.
  pub stroke_width: f32,
  pub(crate) font_synthesis: FontSynthesis,
  pub(crate) line_height_scales_with_text_fit: bool,
  /// Used line height in px; `None` falls back to the run metrics.
  pub(crate) line_height_px: Option<f32>,
  /// Whether the line height is `normal`, letting fallback-font runs grow the line.
  pub(crate) line_height_is_normal: bool,
  pub(crate) vertical_align: VerticalAlign,
}

impl InlineBrush {
  /// The run's line-box contribution. Parley's run metrics can carry a
  /// neighboring span's style at run boundaries, so the brush line height wins
  /// when it is set. Under `line-height: normal` a fallback-font run grows the
  /// line to its own rounded height, like Blink's
  /// `InlineBoxState::AccumulateUsedFonts`.
  fn line_box_contribution(
    &self,
    metrics_line_height: f32,
    ascent: f32,
    descent: f32,
  ) -> (f32, f32) {
    let line_height = self.line_height_px.unwrap_or(metrics_line_height);
    let (above, below) = text_line_box_contribution(line_height, ascent, descent);

    if self.line_height_is_normal {
      (above.max(ascent.round()), below.max(descent.round()))
    } else {
      (above, below)
    }
  }
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      source_span_id: None,
      is_direction_mark: false,
      opacity: 1.0,
      color: Color::black(),
      decoration_color: Color::black(),
      decoration_thickness: SizedTextDecorationThickness::Value(0.0),
      underline_offset: 0.0,
      underline_position: TextUnderlinePosition::default(),
      decoration_line: TextDecorationLines::empty(),
      decoration_skip_ink: TextDecorationSkipInk::default(),
      stroke_color: Color::black(),
      stroke_width: 0.0,
      font_synthesis: FontSynthesis::default(),
      line_height_scales_with_text_fit: false,
      line_height_px: None,
      line_height_is_normal: false,
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

pub(super) fn apply_text_indent(layout: &mut InlineLayout, style: &SizedFontStyle, max_width: f32) {
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

pub(super) fn inline_line_height_hint(style: &SizedFontStyle) -> f32 {
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

pub(crate) fn measure_inline_layout(
  layout: &mut InlineLayout,
  spans: &[ProcessedInlineSpan<'_>],
  positioned_floats: &[PositionedInlineBox],
  line_scales: &[f32],
  options: InlineMeasureOptions,
) -> Size<f32> {
  let InlineMeasureOptions {
    max_width,
    ceil_width,
    parent_font_metrics,
    clamp_to_max_width,
  } = options;
  let max_run_width = layout
    .lines()
    .map(|line| line.metrics().inline_min_coord + line.metrics().advance)
    .fold(0.0, f32::max);
  let line_metrics = resolve_inline_line_metrics(layout, spans, parent_font_metrics, line_scales);
  let total_height = line_metrics
    .last()
    .map(|metrics| metrics.resolved_line_bottom)
    .unwrap_or(0.0);
  let float_box_width = positioned_floats
    .iter()
    .map(|inline_box| inline_box.x + inline_box.width)
    .fold(0.0, f32::max);
  let float_box_height = positioned_floats
    .iter()
    .map(|inline_box| inline_box.y + inline_box.height)
    .fold(0.0, f32::max);

  let measured_width = if ceil_width {
    max_run_width.max(float_box_width).ceil()
  } else {
    max_run_width.max(float_box_width)
  };

  Size {
    width: if clamp_to_max_width {
      measured_width.min(max_width)
    } else {
      measured_width
    },
    height: total_height.max(float_box_height).ceil(),
  }
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
  let mut previous_collapsible_space = false;
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
        previous_collapsible_space = false;
        previous_was_line_break = false;
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

  let (layout, text) = shape_spans(context, &spans, style, shape_cacheable);

  BuiltInlineLayout {
    layout,
    text,
    spans,
    positioned_floats: Vec::new(),
    line_scales: Vec::new(),
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
  decorations: Option<Rc<DecorationLink>>,
  available_space: Size<AvailableSpace>,
  index: usize,
  id: u64,
) -> ProcessedInlineSpan<'c> {
  let context = &render_node.context;
  let vertical_align = context.style.vertical_align.resolve(
    &context.sizing,
    context.sizing.font_size,
    context.style.line_height,
  );
  let margin = Rect {
    top: context.style.margin_top,
    right: context.style.margin_right,
    bottom: context.style.margin_bottom,
    left: context.style.margin_left,
  }
  .map(|length| length.to_px(&context.sizing, 0.0));
  let padding = Rect {
    top: context.style.padding_top,
    right: context.style.padding_right,
    bottom: context.style.padding_bottom,
    left: context.style.padding_left,
  }
  .map(|length| length.to_px(&context.sizing, 0.0));
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

  let atomic_metrics = render_node
    .node
    .as_ref()
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
  let inline_box = InlineBox {
    index,
    id,
    kind: inline_box_kind(render_node),
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
/// content is pure text. Inline boxes bake constraint-dependent measured sizes
/// into the layout, so only pure-text content is safe to cache across calls.
fn shape_spans(
  context: &RenderContext,
  spans: &[ProcessedInlineSpan<'_>],
  style: &SizedFontStyle,
  shape_cacheable: bool,
) -> (InlineLayout, String) {
  let cacheable = shape_cacheable
    && spans
      .iter()
      .all(|span| matches!(span, ProcessedInlineSpan::Text { .. }));
  let cache_key = cacheable
    .then(|| shape_fingerprint(spans, style, context.style.lang.as_ref().map(Lang::as_str)));
  // The stored text double-checks the fingerprint against hash collisions.
  let expected_text = cacheable.then(|| {
    spans.iter().fold(String::new(), |mut joined, span| {
      if let ProcessedInlineSpan::Text { text, .. } = span {
        joined.push_str(text);
      }
      joined
    })
  });
  let (cached, seen) = match (cache_key, &expected_text) {
    (Some(key), Some(expected)) => match context.shape_cache().borrow().get(&key) {
      Some(Some((layout, text))) if text == expected => {
        ((Some((layout.clone(), text.clone()))), true)
      }
      Some(_) => (None, true),
      None => (None, false),
    },
    _ => (None, false),
  };
  if let Some(cached) = cached {
    return cached;
  }

  let (layout, text) = context.tree_builder(style.into(), chromium_line_breaks(spans), |builder| {
    push_spans_into_builder(builder, spans, &context.fonts().classes)
  });

  if let Some(key) = cache_key {
    let stored = seen.then(|| (layout.clone(), text.clone()));

    context.shape_cache().borrow_mut().insert(key, stored);
  }
  (layout, text)
}

fn prepare_inline_layout(
  built: &mut BuiltInlineLayout<'_>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  style: &SizedFontStyle,
) -> (TextWrapMode, f32) {
  let text_wrap_mode = style.parent.resolved_text_wrap_mode();
  let line_height_hint = inline_line_height_hint(style);
  apply_text_indent(&mut built.layout, style, max_width);
  break_lines(
    &mut built.layout,
    max_width,
    max_height,
    line_height_hint,
    text_wrap_mode,
    &built.spans,
    &mut built.positioned_floats,
  );
  (text_wrap_mode, line_height_hint)
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
  let (text_wrap_mode, line_height_hint) =
    prepare_inline_layout(&mut built, max_width, max_height, style);

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
        make_ellipsis_layout(
          layout,
          spans,
          max_width,
          max_height,
          style,
          context,
          positioned_floats,
        );
      }
    }

    let line_count = layout.lines().count();

    if style.parent.text_wrap_style == TextWrapStyle::Balance {
      make_balanced_text(
        layout,
        RebreakOptions {
          max_width,
          max_height,
          line_height_hint,
          text_wrap_mode,
        },
        line_count,
        style.sizing.viewport.device_pixel_ratio,
        spans,
        positioned_floats,
      );
    }

    if style.parent.text_wrap_style == TextWrapStyle::Pretty {
      make_pretty_text(
        layout,
        RebreakOptions {
          max_width,
          max_height,
          line_height_hint,
          text_wrap_mode,
        },
        spans,
        positioned_floats,
      );
    }
  }

  if style.parent.text_fit.mode != TextFitMode::None
    && text_fit_is_applicable(&built.positioned_floats)
  {
    built.line_scales = text_fit_line_scales(&built.layout, max_width, style);
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

/// Per-line setup (scale state, baseline shift, resolved metrics) for the inline painting walk.
pub(crate) struct LineSetup {
  /// Text-fit scale state for the line.
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
    line_scales: &[f32],
    line_index: usize,
  ) -> Option<Self> {
    let resolved_metrics = *line_vertical_metrics.get(line_index)?;
    let line_scale = line_scales.get(line_index).copied().unwrap_or(1.0);
    let (line_scale_origin_x, alignment_correction) =
      text_fit_line_alignment_correction(line, line_scale, layout.content_box_size().width);
    Some(Self {
      state: LineScaleState {
        scale: line_scale,
        alignment_correction,
        layout_origin: Point {
          x: layout.border.left + layout.padding.left + line_scale_origin_x,
          y: layout.border.top + layout.padding.top + resolved_metrics.resolved_baseline,
        },
      },
      baseline_shift: resolved_metrics.baseline_shift,
      line_scale_origin_x,
      resolved_metrics,
    })
  }
}

/// A line under an item walk: its index, setup, and resolved state.
pub(crate) struct WalkedLine {
  pub(crate) index: usize,
  pub(crate) setup: LineSetup,
  pub(crate) state: ResolvedInlineLineState,
}

/// One item placed on a walked line, with the static advance of the boxes before it.
pub(crate) enum PlacedItem<'a> {
  Run {
    glyph_run: GlyphRun<'a, InlineBrush>,
    static_inline_prefix: f32,
    /// The line-end whitespace advance this run carries.
    trailing_whitespace: f32,
  },
  /// An in-flow box, its `x` already scaled for text-fit.
  Box(VisualInlineBox),
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
    let line_states = self.line_states();

    for (index, line) in self.layout.lines().enumerate() {
      let Some(setup) = LineSetup::new(
        &line,
        layout,
        &line_vertical_metrics,
        &self.line_scales,
        index,
      ) else {
        continue;
      };
      let walked = WalkedLine {
        index,
        setup,
        state: line_states[index],
      };
      let items: Vec<_> = line.items().collect();
      let trailing_whitespace = distribute_trailing_whitespace(&items, &line);
      let mut static_inline_prefix = 0.0_f32;

      for (item_index, item) in items.into_iter().enumerate() {
        match item {
          PositionedLayoutItem::GlyphRun(glyph_run) => visit(
            &walked,
            PlacedItem::Run {
              glyph_run,
              static_inline_prefix,
              trailing_whitespace: trailing_whitespace[item_index],
            },
          )?,
          PositionedLayoutItem::InlineBox(inline_box) => {
            if inline_box.kind != InlineBoxKind::InFlow {
              continue;
            }
            let Some(resolved) =
              resolve_visual_inline_box(inline_box, Some(walked.state), &self.spans)
            else {
              continue;
            };
            let inline_box = VisualInlineBox {
              x: scale_text_fit_x(
                resolved.x,
                walked.setup.line_scale_origin_x,
                walked.setup.state.scale,
                static_inline_prefix,
                walked.setup.state.alignment_correction,
              ),
              ..resolved
            };

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

  use super::{outline::x_ranges_touch, runs::slice_text_at_char_boundaries, *};
  use crate::{
    Fonts,
    context::RenderContext,
    geometry::{Point, Rect},
    layout::{node::Node, tree::RenderNode},
    resources::font::{FontOverride, FontResource, GenericFamily},
    style::Affine,
    style::{
      Color, ColorInput, Display, FontSize, Length, SizingContext, Style, StyleDeclaration,
      WhiteSpace,
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

  fn shaped_run(position: TextUnderlinePosition, underline_offset: f32) -> ShapedRun {
    ShapedRun {
      glyphs: Vec::new(),
      offset: 0.0,
      baseline: 0.0,
      advance: 0.0,
      trailing_whitespace: 0.0,
      brush: InlineBrush {
        underline_offset,
        underline_position: position,
        ..Default::default()
      },
      metrics: RunMetrics {
        ascent: 40.0,
        descent: 10.0,
        underline_offset: -5.0,
        underline_size: 2.0,
        strikethrough_offset: 20.0,
        strikethrough_size: 2.0,
      },
      font_size: 100.0,
      font_index: 0,
      text_range: 0..0,
      cluster_ranges: Vec::new(),
      variations: Vec::new(),
      synthetic_bold: None,
      synthetic_skew: None,
      // Not a font: `em_box_descent` falls back to the run metrics instead of OS/2.
      font_data: parley::fontique::Blob::new(Arc::new(Vec::new())),
    }
  }

  #[test]
  fn an_explicit_zero_line_height_beats_the_run_metrics() {
    let brush = InlineBrush {
      line_height_px: Some(0.0),
      ..InlineBrush::default()
    };
    let (above, below) = brush.line_box_contribution(20.0, 12.0, 4.0);

    assert_eq!((above, below), (4.0, -4.0));
  }

  #[test]
  fn a_fully_trimmed_run_paints_no_decoration() {
    let mut run = shaped_run(TextUnderlinePosition::Auto, 0.0);
    run.brush.decoration_line = TextDecorationLines::UNDERLINE;
    run.brush.decoration_thickness = SizedTextDecorationThickness::Value(2.0);
    run.advance = 5.2;
    run.trailing_whitespace = 5.2;
    run.offset = 10.4;

    let layout = ComputedLayout {
      location: crate::geometry::Point::ZERO,
      size: Size::new(100.0, 100.0),
      border: crate::geometry::Rect::default(),
      padding: crate::geometry::Rect::default(),
    };
    let decorations = run.decorations(&HashMap::new(), layout, 0.0, Affine::IDENTITY);

    assert_eq!(decorations.len(), 0);
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
      .map(|run| (run.glyph_run.advance, run.glyph_run.trailing_whitespace))
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
  fn underline_offset_from_baseline_follows_the_underline_position() {
    // The font's underline offset is negative above the baseline, so `auto` flips it.
    assert_eq!(
      shaped_run(TextUnderlinePosition::Auto, 0.0).underline_offset_from_baseline(),
      5.0
    );
    assert_eq!(
      shaped_run(TextUnderlinePosition::FromFont, 0.0).underline_offset_from_baseline(),
      5.0
    );
    // 100px em split in the metrics' 40:10 ratio puts the em box bottom 20px down.
    assert_eq!(
      shaped_run(TextUnderlinePosition::Under, 0.0).underline_offset_from_baseline(),
      20.0
    );
  }

  #[test]
  fn underline_offset_from_baseline_adds_the_style_offset() {
    assert_eq!(
      shaped_run(TextUnderlinePosition::Auto, 3.0).underline_offset_from_baseline(),
      8.0
    );
    assert_eq!(
      shaped_run(TextUnderlinePosition::Under, -4.0).underline_offset_from_baseline(),
      16.0
    );
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
        .with(StyleDeclaration::width(300.0.into()))
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

  #[test]
  fn outline_rects_a_layout_unit_apart_touch() {
    let rect = |x: f32, width: f32| InlineOutlineRect {
      span_id: 0,
      line_index: 0,
      x,
      y: 0.0,
      width,
      height: 10.0,
    };

    assert!(x_ranges_touch(rect(0.0, 10.0), rect(10.01, 10.0)));
    assert!(!x_ranges_touch(rect(0.0, 10.0), rect(10.1, 10.0)));
  }
}

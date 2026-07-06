use std::{borrow::Cow, collections::HashMap, ops::Range};

use parley::{
  BreakReason, GlyphRun, IndentOptions, InlineBox, InlineBoxKind, Line, LineMetrics,
  PositionedInlineBox, PositionedLayoutItem, TextStyle, TreeBuilder, YieldData,
};
use skrifa::{FontRef, MetadataProvider};
use taffy::AvailableSpace;

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, PathCommand, Point, Rect, Size},
  layout::{border::BorderPath, node::Node, tree::RenderNode},
  resources::font::{FontError, ResolvedColorLayer, ResolvedGlyph, ResolvedOutlineGlyph},
  style::{
    Affine, BoxSizing, Color, Float, FontSynthesis, Length, ResolvedVerticalAlign,
    SizedTextDecorationThickness, TextDecorationLines, TextDecorationSkipInk, TextFitMode,
    TextFitTarget, TextOverflow, TextWrapMode, TextWrapStyle, VerticalAlign, VerticalAlignKeyword,
  },
  text_processing::{
    MaxHeight, RebreakOptions, apply_text_transform, apply_white_space_collapse,
    make_balanced_text, make_pretty_text,
  },
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
}

/// A completed inline layout with its source text, spans, and per-line scales.
pub struct BuiltInlineLayout<'c> {
  /// The parley layout.
  pub layout: InlineLayout,
  /// Concatenated laid-out text.
  pub text: String,
  /// Processed spans backing the layout.
  pub spans: Vec<ProcessedInlineSpan<'c>>,
  /// Out-of-flow inline boxes positioned separately.
  pub custom_inline_boxes: Vec<PositionedInlineBox>,
  /// Per-line text-fit scale factors.
  pub line_scales: Vec<f32>,
}

impl BuiltInlineLayout<'_> {
  /// Parent font metrics from the first run.
  pub fn parent_font_metrics(&self) -> Option<ParentFontMetrics> {
    get_parent_font_metrics(&self.layout)
  }

  /// Resolved metrics for each line.
  pub fn line_metrics(&self) -> Vec<ResolvedLineMetrics> {
    resolve_inline_line_metrics(
      &self.layout,
      &self.spans,
      self.parent_font_metrics(),
      &self.line_scales,
    )
  }

  /// Resolved state for each line.
  pub fn line_states(&self) -> Vec<ResolvedInlineLineState> {
    resolve_inline_line_states(
      &self.layout,
      &self.spans,
      self.parent_font_metrics(),
      &self.line_scales,
    )
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

/// An inline box and its resolved box-model dimensions.
pub struct InlineBoxItem<'c> {
  /// The render node this box wraps.
  pub render_node: &'c RenderNode,
  pub(crate) inline_box: InlineBox,
  pub(crate) paint_width: f32,
  pub(crate) paint_height: f32,
  /// Margin around the box.
  pub margin: Rect<f32>,
  pub(crate) padding: Rect<f32>,
  pub(crate) border: Rect<f32>,
  pub(crate) baseline_offset: Option<f32>,
  pub(crate) vertical_align: ResolvedVerticalAlign,
}

impl From<&InlineBoxItem<'_>> for ComputedLayout {
  fn from(value: &InlineBoxItem<'_>) -> Self {
    ComputedLayout {
      location: Point::ZERO,
      size: Size::new(value.paint_width, value.paint_height),
      border: value.border,
      padding: value.padding,
    }
  }
}

fn inline_box_kind(render_node: &RenderNode) -> InlineBoxKind {
  if render_node.context.style.position.is_out_of_flow() {
    InlineBoxKind::OutOfFlow
  } else if render_node.context.style.float != Float::None {
    InlineBoxKind::CustomOutOfFlow
  } else {
    InlineBoxKind::InFlow
  }
}

/// An inline item after text processing, ready for layout.
pub enum ProcessedInlineSpan<'c> {
  /// A styled text span.
  Text {
    /// Identifier of the source span.
    span_id: u64,
    /// Byte range within the laid-out text.
    byte_range: Range<usize>,
    /// Processed text content.
    text: String,
    /// Resolved font style.
    style: SizedFontStyle<'c>,
  },
  /// An inline box.
  Box(InlineBoxItem<'c>),
}

/// A piece of inline content collected from the tree.
pub enum InlineItem<'c> {
  /// An inline-level render node.
  RenderNode {
    /// The node.
    render_node: &'c RenderNode,
  },
  /// A run of text.
  Text {
    /// The text content.
    text: Cow<'c, str>,
    /// Render context for the text.
    context: &'c RenderContext,
  },
}

/// Flatten a render node subtree into its inline items.
pub fn collect_inline_items<'n>(root: &'n RenderNode) -> Vec<InlineItem<'n>> {
  let mut items = Vec::new();
  collect_inline_items_impl(root, 0, &mut items);
  items
}

fn collect_inline_items_impl<'n>(
  node: &'n RenderNode,
  depth: usize,
  items: &mut Vec<InlineItem<'n>>,
) {
  if depth > 0 && node.participates_as_inline_box() {
    items.push(InlineItem::RenderNode { render_node: node });
    return;
  }

  if let Some(text) = node.anonymous_text_content.as_deref() {
    items.push(InlineItem::Text {
      text: Cow::Borrowed(text),
      context: &node.context,
    });
  }

  if let Some(inline_content) = node.node.as_ref().and_then(Node::inline_content) {
    match inline_content {
      InlineContentKind::Box => items.push(InlineItem::RenderNode { render_node: node }),
      InlineContentKind::Text(text) => items.push(InlineItem::Text {
        text,
        context: &node.context,
      }),
    }
  }

  if let Some(children) = &node.children {
    for child in children {
      collect_inline_items_impl(child, depth + 1, items);
    }
  }
}

pub(crate) enum InlineContentKind<'c> {
  Text(Cow<'c, str>),
  Box,
}

/// Parley layout specialized to [`InlineBrush`].
pub(crate) type InlineLayout = parley::Layout<InlineBrush>;

#[derive(Clone, Copy, Debug)]
/// x-height and ascent/descent of the parent font.
pub struct ParentFontMetrics {
  pub(crate) x_height: Option<f32>,
  pub(crate) text_metrics: (f32, f32),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineMeasureOptions {
  pub(crate) max_width: f32,
  pub(crate) ceil_width: bool,
  pub(crate) parent_font_metrics: Option<ParentFontMetrics>,
}

#[derive(Clone, PartialEq, Copy, Debug)]
/// Paint attributes carried per glyph run through the inline layout.
pub struct InlineBrush {
  /// Span this run originated from, if any.
  pub source_span_id: Option<u64>,
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
  /// Which decoration lines to draw.
  pub decoration_line: TextDecorationLines,
  /// Whether decorations skip over glyph ink.
  pub decoration_skip_ink: TextDecorationSkipInk,
  pub(crate) stroke_color: Color,
  pub(crate) font_synthesis: FontSynthesis,
  pub(crate) line_height_scales_with_text_fit: bool,
  pub(crate) vertical_align: VerticalAlign,
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      source_span_id: None,
      opacity: 1.0,
      color: Color::black(),
      decoration_color: Color::black(),
      decoration_thickness: SizedTextDecorationThickness::Value(0.0),
      underline_offset: 0.0,
      decoration_line: TextDecorationLines::empty(),
      decoration_skip_ink: TextDecorationSkipInk::default(),
      stroke_color: Color::black(),
      font_synthesis: FontSynthesis::default(),
      line_height_scales_with_text_fit: false,
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

fn refresh_text_span_ranges(spans: &mut [ProcessedInlineSpan<'_>]) {
  let mut byte_offset = 0;

  for span in spans {
    if let ProcessedInlineSpan::Text {
      text, byte_range, ..
    } = span
    {
      let end = byte_offset + text.len();
      *byte_range = byte_offset..end;
      byte_offset = end;
    }
  }
}

fn tail_text_span<'a, 'c>(
  spans: &'a [ProcessedInlineSpan<'c>],
) -> Option<(&'a SizedFontStyle<'c>, u64)> {
  spans.iter().rev().find_map(|span| match span {
    ProcessedInlineSpan::Text { span_id, style, .. } => Some((style, *span_id)),
    ProcessedInlineSpan::Box(_) => None,
  })
}

fn measure_ellipsis_width(
  context: &RenderContext,
  ellipsis_style: &SizedFontStyle,
  ellipsis_char: &str,
) -> f32 {
  let (mut ellipsis_layout, _) = context.tree_builder(ellipsis_style.into(), |builder| {
    builder.push_text(ellipsis_char);
  });
  ellipsis_layout.break_all_lines(None);
  ellipsis_layout
    .lines()
    .next()
    .map(|line| line.runs().map(|run| run.advance()).sum::<f32>())
    .unwrap_or(0.0)
}

/// Font metrics of the first run, used as the parent reference.
pub fn get_parent_font_metrics(layout: &InlineLayout) -> Option<ParentFontMetrics> {
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
pub struct ResolvedLineMetrics {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FloatSide {
  Left,
  Right,
}

#[derive(Clone, Copy, Debug)]
struct ActiveFloat {
  side: FloatSide,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

impl ActiveFloat {
  fn bottom(self) -> f32 {
    self.y + self.height
  }

  fn overlaps_range(self, top: f32, bottom: f32) -> bool {
    self.y < bottom && top < self.bottom()
  }
}

struct FloatLayoutState {
  max_width: f32,
  line_height_hint: f32,
  active_floats: Vec<ActiveFloat>,
}

impl FloatLayoutState {
  fn new(max_width: f32, line_height_hint: f32) -> Self {
    Self {
      max_width,
      line_height_hint,
      active_floats: Vec::new(),
    }
  }

  fn side_for_inline_box(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    inline_box_id: u64,
  ) -> Option<FloatSide> {
    let ProcessedInlineSpan::Box(item) = spans.get(inline_box_id as usize)? else {
      return None;
    };

    match item
      .render_node
      .context
      .style
      .float
      .resolve(item.render_node.context.style.direction)
    {
      taffy::Float::Left => Some(FloatSide::Left),
      taffy::Float::Right => Some(FloatSide::Right),
      taffy::Float::None => None,
    }
  }

  fn clear_for_inline_box(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    inline_box_id: u64,
  ) -> taffy::Clear {
    let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box_id as usize) else {
      return taffy::Clear::None;
    };

    item
      .render_node
      .context
      .style
      .clear
      .resolve(item.render_node.context.style.direction)
  }

  fn next_float_bottom(&self, top: f32, height: f32) -> Option<f32> {
    let bottom = top + height.max(0.0);
    self
      .active_floats
      .iter()
      .filter_map(|float| float.overlaps_range(top, bottom).then_some(float.bottom()))
      .min_by(f32::total_cmp)
  }

  fn bounds_for_range(&self, top: f32, height: f32) -> (f32, f32) {
    let bottom = top + height.max(0.0);
    let mut left = 0.0_f32;
    let mut right = self.max_width;

    for active_float in &self.active_floats {
      if !active_float.overlaps_range(top, bottom) {
        continue;
      }

      match active_float.side {
        FloatSide::Left => left = left.max(active_float.x + active_float.width),
        FloatSide::Right => right = right.min(active_float.x),
      }
    }

    (
      left.min(self.max_width),
      right.max(left).min(self.max_width),
    )
  }

  fn line_bounds(&self, line_y: f32) -> (f32, f32) {
    self.bounds_for_range(line_y, self.line_height_hint)
  }

  fn clearance_y(&self, start_y: f32, clear: taffy::Clear) -> f32 {
    self
      .active_floats
      .iter()
      .filter(|float| float.bottom() > start_y)
      .filter(|float| {
        matches!(
          (clear, float.side),
          (taffy::Clear::Left, FloatSide::Left)
            | (taffy::Clear::Right, FloatSide::Right)
            | (taffy::Clear::Both, _)
        )
      })
      .map(|float| float.bottom())
      .fold(start_y.max(0.0), f32::max)
  }

  fn find_float_y(&self, start_y: f32, width: f32, height: f32) -> f32 {
    let mut line_y = start_y.max(0.0);

    loop {
      let (left, right) = self.bounds_for_range(line_y, height);
      if width <= right - left || (left == 0.0 && right == self.max_width) {
        return line_y;
      }

      let Some(next_y) = self.next_float_bottom(line_y, height) else {
        return line_y;
      };
      line_y = next_y;
    }
  }

  fn find_line_y_for_advance(&self, start_y: f32, current_advance: f32) -> f32 {
    let mut line_y = start_y.max(0.0);

    loop {
      let (left, right) = self.line_bounds(line_y);
      if current_advance <= right - left || (left == 0.0 && right == self.max_width) {
        return line_y;
      }

      let Some(next_y) = self.next_float_bottom(line_y, self.line_height_hint) else {
        return line_y;
      };
      line_y = next_y;
    }
  }

  fn push_float(
    &mut self,
    side: FloatSide,
    clear: taffy::Clear,
    start_y: f32,
    inline_box: &InlineBox,
  ) -> PositionedInlineBox {
    let cleared_y = self.clearance_y(start_y, clear);
    let float_y = self.find_float_y(cleared_y, inline_box.width, inline_box.height);
    let (left, right) = self.bounds_for_range(float_y, inline_box.height);
    let float_x = match side {
      FloatSide::Left => left,
      FloatSide::Right => (right - inline_box.width).max(left),
    };

    self.active_floats.push(ActiveFloat {
      side,
      x: float_x,
      y: float_y,
      width: inline_box.width,
      height: inline_box.height,
    });

    PositionedInlineBox {
      x: float_x,
      y: float_y,
      width: inline_box.width,
      height: inline_box.height,
      id: inline_box.id,
      kind: inline_box.kind,
    }
  }

  fn update_breaker_line(&self, breaker: &mut parley::BreakLines<'_, InlineBrush>, line_y: f32) {
    let (line_x, line_right) = self.line_bounds(line_y);
    let state = breaker.state_mut();
    state.set_layout_max_advance(self.max_width);
    state.set_line_x(line_x);
    state.set_line_y(f64::from(line_y));
    state.set_line_max_advance((line_right - line_x).max(0.0));
  }
}

fn quantized_baseline(line_height: f32, ascent: f32, descent: f32) -> f32 {
  let rounded_ascent = ascent.round();
  let rounded_descent = descent.round();
  let leading = line_height - (rounded_ascent + rounded_descent);
  let leading_above = (leading * 0.5).floor();
  rounded_ascent + leading_above
}

fn text_line_box_contribution(line_height: f32, ascent: f32, descent: f32) -> (f32, f32) {
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
pub fn resolve_inline_line_metrics(
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
    ProcessedInlineSpan::Text { .. } => false,
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
          let (base_above, base_below) =
            text_line_box_contribution(metrics.line_height, metrics.ascent, metrics.descent);
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
pub struct ResolvedInlineLineState {
  pub(crate) adjusted_metrics: LineMetrics,
  /// Vertical shift applied to the baseline.
  pub baseline_shift: f32,
  pub(crate) parent_x_height: Option<f32>,
  pub(crate) parent_text_metrics: Option<(f32, f32)>,
}

/// Resolve per-line state used when placing inline boxes and glyphs.
pub fn resolve_inline_line_states(
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
      baseline_shift: resolved.baseline_shift,
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
  pub(crate) layout_x: f32,
  pub(crate) layout_advance: f32,
}

/// Resolve a positioned inline box into its painted geometry.
pub fn resolve_visual_inline_box(
  inline_box: PositionedInlineBox,
  line_state: Option<ResolvedInlineLineState>,
  spans: &[ProcessedInlineSpan<'_>],
) -> Option<VisualInlineBox> {
  let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box.id as usize) else {
    return None;
  };

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
    layout_x: positioned.x,
    layout_advance: positioned.width,
  })
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
        ProcessedInlineSpan::Box(_) => {
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
  spans.iter().find_map(|span| match span {
    ProcessedInlineSpan::Text {
      span_id: current_span_id,
      style,
      ..
    } if *current_span_id == span_id => Some(style),
    ProcessedInlineSpan::Text { .. } | ProcessedInlineSpan::Box(_) => None,
  })
}

fn truncated_tail_text_span_id<'c>(
  spans: &[ProcessedInlineSpan<'c>],
  span_cut_idx: Option<usize>,
) -> Option<u64> {
  span_cut_idx.and_then(|cut_idx| {
    spans[..cut_idx].iter().rev().find_map(|span| match span {
      ProcessedInlineSpan::Text { span_id, .. } => Some(*span_id),
      ProcessedInlineSpan::Box(_) => None,
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

pub(crate) fn measure_inline_layout(
  layout: &mut InlineLayout,
  spans: &[ProcessedInlineSpan<'_>],
  custom_inline_boxes: &[PositionedInlineBox],
  line_scales: &[f32],
  options: InlineMeasureOptions,
) -> Size<f32> {
  let InlineMeasureOptions {
    max_width,
    ceil_width,
    parent_font_metrics,
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
  let custom_box_width = custom_inline_boxes
    .iter()
    .map(|inline_box| inline_box.x + inline_box.width)
    .fold(0.0, f32::max);
  let custom_box_height = custom_inline_boxes
    .iter()
    .map(|inline_box| inline_box.y + inline_box.height)
    .fold(0.0, f32::max);

  let measured_width = if ceil_width {
    max_run_width.max(custom_box_width).ceil()
  } else {
    max_run_width.max(custom_box_width)
  };

  Size {
    width: measured_width.min(max_width),
    height: total_height.max(custom_box_height).ceil(),
  }
}

fn push_spans_into_builder(
  builder: &mut TreeBuilder<'_, InlineBrush>,
  spans: &[ProcessedInlineSpan<'_>],
) {
  for span in spans {
    match span {
      ProcessedInlineSpan::Text {
        span_id,
        text,
        style,
        ..
      } => {
        builder.push_style_span(text_style_with_span_id(style, Some(*span_id)));
        builder.push_text(text);
        builder.pop_style_span();
      }
      ProcessedInlineSpan::Box(item) => {
        builder.push_inline_box(item.inline_box.clone());
      }
    }
  }
}

fn build_inline_layout_tree<'c>(
  items: &[InlineItem<'c>],
  available_space: Size<AvailableSpace>,
  style: &'c SizedFontStyle,
  context: &'c RenderContext,
) -> BuiltInlineLayout<'c> {
  // Build spans first: measuring an inline box re-enters layout, so it must run
  // before `tree_builder` holds the shared font borrow.
  let mut spans: Vec<ProcessedInlineSpan<'c>> = Vec::new();
  let mut index_pos = 0;
  let mut previous_collapsible_space = false;
  let mut previous_was_line_break = false;

  for item in items {
    match item {
      InlineItem::Text { text, context } => {
        let span_style = SizedFontStyle::from_style(&context.style, context);
        let transformed = apply_text_transform(text, context.style.text_transform);
        let collapsed = apply_white_space_collapse(
          &transformed,
          style.parent.white_space_collapse,
          &mut previous_collapsible_space,
          &mut previous_was_line_break,
        );
        let span_id = spans.len() as u64;
        let start = index_pos;
        let end = start + collapsed.len();
        index_pos = end;

        spans.push(ProcessedInlineSpan::Text {
          span_id,
          byte_range: start..end,
          text: collapsed.into_owned(),
          style: span_style,
        });
      }
      InlineItem::RenderNode { render_node } => {
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
          index: index_pos,
          id: spans.len() as u64,
          kind: inline_box_kind(render_node),
          width: paint_width,
          height: paint_height,
        };
        let baseline_offset =
          raw_baseline_offset.map(|baseline| baseline.clamp(0.0, inline_box.height));

        spans.push(ProcessedInlineSpan::Box(InlineBoxItem {
          render_node,
          inline_box,
          paint_width,
          paint_height,
          margin,
          padding,
          border,
          baseline_offset,
          vertical_align,
        }));
        previous_collapsible_space = false;
        previous_was_line_break = false;
      }
    }
  }

  let (layout, text) = context.tree_builder(style.into(), |builder| {
    push_spans_into_builder(builder, &spans)
  });

  BuiltInlineLayout {
    layout,
    text,
    spans,
    custom_inline_boxes: Vec::new(),
    line_scales: Vec::new(),
  }
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
    &mut built.custom_inline_boxes,
  );
  (text_wrap_mode, line_height_hint)
}

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

fn text_span_disables_text_fit(style: &SizedFontStyle) -> bool {
  style.letter_spacing != 0.0 || style.word_spacing != 0.0
}

fn text_fit_is_applicable(
  spans: &[ProcessedInlineSpan<'_>],
  custom_inline_boxes: &[PositionedInlineBox],
) -> bool {
  custom_inline_boxes.is_empty()
    && spans.iter().all(|span| match span {
      ProcessedInlineSpan::Text { style, .. } => !text_span_disables_text_fit(style),
      ProcessedInlineSpan::Box(_) => true,
    })
}

/// Returns `(text_advance, static_advance)` for a line.
/// `text_advance` excludes trailing whitespace and inline boxes; `static_advance` is inline-box width only.
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

fn text_fit_line_scales(layout: &InlineLayout, max_width: f32, style: &SizedFontStyle) -> Vec<f32> {
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
pub fn text_fit_line_alignment_correction(
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
  } = request;
  let mut built = build_inline_layout_tree(&items, available_space, style, context);
  let (text_wrap_mode, line_height_hint) =
    prepare_inline_layout(&mut built, max_width, max_height, style);

  if mode == InlineLayoutMode::Draw {
    let BuiltInlineLayout {
      layout,
      text,
      spans,
      custom_inline_boxes,
      ..
    } = &mut built;

    if style.parent.text_overflow == TextOverflow::Ellipsis {
      let is_overflowing = layout
        .lines()
        .last()
        .is_some_and(|last_line| last_line.text_range().end < text.len());

      if is_overflowing {
        make_ellipsis_layout(
          layout,
          spans,
          max_width,
          max_height,
          style,
          context,
          custom_inline_boxes,
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
        custom_inline_boxes,
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
        custom_inline_boxes,
      );
    }
  }

  if style.parent.text_fit.mode != TextFitMode::None
    && text_fit_is_applicable(&built.spans, &built.custom_inline_boxes)
  {
    built.line_scales = text_fit_line_scales(&built.layout, max_width, style);
  }

  built
    .layout
    .align(style.parent.text_align.into(), Default::default());
  built
}

/// Resolve the max height constraint from line clamping and content box height.
pub fn resolve_inline_max_height(
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

/// Per-line text-fit scaling state: `scale` applied about `layout_origin`, plus
/// the horizontal `alignment_correction` for a scaled-down line.
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
pub(crate) fn text_fit_x_correction(
  scale: f32,
  static_inline_prefix: f32,
  alignment_correction: f32,
) -> f32 {
  static_inline_prefix * (1.0 - scale) + alignment_correction
}

impl LineScaleState {
  /// Composes the affine transform for a glyph run on this (possibly scaled)
  /// line: `base * T(x_correction) * scale-about-origin`. Shared by the raster
  /// walk and the vector producer so the positioning math has a single home.
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

/// Per-line setup (scale state, baseline shift, resolved metrics) for the inline
/// painting walk.
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
  /// Returns `None` when `line_index` is out of range for `line_vertical_metrics`.
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

/// A glyph run's text-outline rectangle on a line, in border-box space. The
/// raster backend merges these into stroked islands; the vector backend emits
/// them as outline contours. Pure geometry, no backend types.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct InlineOutlineRect {
  /// Source inline span id (identifies the styled run the rect belongs to).
  pub span_id: u64,
  /// Line index the rect sits on.
  pub(crate) line_index: usize,
  /// Left edge in border-box space.
  pub(crate) x: f32,
  /// Top edge in border-box space.
  pub(crate) y: f32,
  /// Rect width (run advance).
  pub(crate) width: f32,
  /// Rect height (resolved line height).
  pub(crate) height: f32,
}

fn scale_outline_rect(
  rect: InlineOutlineRect,
  state: LineScaleState,
  static_inline_prefix: f32,
) -> InlineOutlineRect {
  if (state.scale - 1.0).abs() <= f32::EPSILON {
    return rect;
  }
  let x_correction = text_fit_x_correction(
    state.scale,
    static_inline_prefix,
    state.alignment_correction,
  );
  InlineOutlineRect {
    x: x_correction + state.layout_origin.x + (rect.x - state.layout_origin.x) * state.scale,
    y: state.layout_origin.y + (rect.y - state.layout_origin.y) * state.scale,
    width: rect.width * state.scale,
    height: rect.height * state.scale,
    ..rect
  }
}

fn collect_glyph_run_outline_rect(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  layout: ComputedLayout,
  line_index: usize,
  line_top: f32,
  line_height: f32,
  line_scale: LineScaleState,
  static_inline_prefix: f32,
) -> Option<InlineOutlineRect> {
  let span_id = glyph_run.style().brush.source_span_id?;
  Some(scale_outline_rect(
    InlineOutlineRect {
      span_id,
      line_index,
      x: layout.border.left + layout.padding.left + glyph_run.offset(),
      y: line_top,
      width: glyph_run.advance(),
      height: line_height,
    },
    line_scale,
    static_inline_prefix,
  ))
}

const OUTLINE_COORD_TOLERANCE: f32 = 1e-3;

fn x_ranges_touch(left: InlineOutlineRect, right: InlineOutlineRect) -> bool {
  left.x <= right.x + right.width + OUTLINE_COORD_TOLERANCE
    && right.x <= left.x + left.width + OUTLINE_COORD_TOLERANCE
}

fn expand_outline_rect(rect: InlineOutlineRect, amount: f32) -> Option<InlineOutlineRect> {
  let width = rect.width + amount * 2.0;
  let height = rect.height + amount * 2.0;
  if width <= 0.0 || height <= 0.0 {
    return None;
  }
  Some(InlineOutlineRect {
    x: rect.x - amount,
    y: rect.y - amount,
    width,
    height,
    ..rect
  })
}

/// Merges adjacent per-line outline rects, then groups them into
/// vertically-continuous islands; each island becomes one stroked contour.
/// Backend-agnostic; both the raster and vector backends consume the islands.
pub fn outline_islands(mut outline_rects: Vec<InlineOutlineRect>) -> Vec<Vec<InlineOutlineRect>> {
  outline_rects.sort_by(|left, right| {
    left
      .span_id
      .cmp(&right.span_id)
      .then(left.line_index.cmp(&right.line_index))
      .then(left.x.total_cmp(&right.x))
  });

  let mut merged_rects: Vec<InlineOutlineRect> = Vec::with_capacity(outline_rects.len());
  for outline_rect in outline_rects {
    let Some(previous_rect) = merged_rects.last_mut() else {
      merged_rects.push(outline_rect);
      continue;
    };

    let same_group = previous_rect.span_id == outline_rect.span_id
      && previous_rect.line_index == outline_rect.line_index;
    let touching =
      outline_rect.x <= previous_rect.x + previous_rect.width + OUTLINE_COORD_TOLERANCE;
    let same_band = (outline_rect.y - previous_rect.y).abs() <= OUTLINE_COORD_TOLERANCE
      && (outline_rect.height - previous_rect.height).abs() <= OUTLINE_COORD_TOLERANCE;

    if same_group && same_band && touching {
      let right_edge =
        (previous_rect.x + previous_rect.width).max(outline_rect.x + outline_rect.width);
      previous_rect.x = previous_rect.x.min(outline_rect.x);
      previous_rect.y = previous_rect.y.min(outline_rect.y);
      previous_rect.width = right_edge - previous_rect.x;
      previous_rect.height = previous_rect.height.max(outline_rect.height);
    } else {
      merged_rects.push(outline_rect);
    }
  }

  let mut line_rect_counts = HashMap::new();
  for outline_rect in &merged_rects {
    *line_rect_counts
      .entry((outline_rect.span_id, outline_rect.line_index))
      .or_insert(0usize) += 1;
  }

  let mut islands: Vec<Vec<InlineOutlineRect>> = Vec::new();
  for outline_rect in merged_rects {
    let mut matched_island = None;

    for (index, island) in islands.iter().enumerate() {
      let Some(previous_rect) = island.last().copied() else {
        continue;
      };
      if previous_rect.span_id != outline_rect.span_id {
        continue;
      }
      if outline_rect.line_index != previous_rect.line_index + 1 {
        continue;
      }

      let previous_is_unique =
        line_rect_counts.get(&(previous_rect.span_id, previous_rect.line_index)) == Some(&1);
      let current_is_unique =
        line_rect_counts.get(&(outline_rect.span_id, outline_rect.line_index)) == Some(&1);
      if (previous_is_unique && current_is_unique) || x_ranges_touch(previous_rect, outline_rect) {
        matched_island = Some(index);
        break;
      }
    }

    if let Some(index) = matched_island {
      islands[index].push(outline_rect);
    } else {
      islands.push(vec![outline_rect]);
    }
  }

  islands
}

/// Builds the rectilinear contour around one island of outline rects, expanded
/// by `expansion` (outline-offset plus half the outline width). Pure path
/// geometry; callers stroke it with their own backend.
pub fn outline_island_contour(island: &[InlineOutlineRect], expansion: f32) -> Vec<PathCommand> {
  let mut path = Vec::with_capacity(island.len() * 6);
  let mut expanded_rects = island
    .iter()
    .filter_map(|r| expand_outline_rect(*r, expansion));
  let Some(first_rect) = expanded_rects.next() else {
    return path;
  };

  path.move_to((first_rect.x, first_rect.y));
  path.line_to((first_rect.x + first_rect.width, first_rect.y));

  let mut current_rect = first_rect;
  for next_rect in expanded_rects {
    path.line_to((current_rect.x + current_rect.width, next_rect.y));
    path.line_to((next_rect.x + next_rect.width, next_rect.y));
    current_rect = next_rect;
  }
  let last_rect = current_rect;

  path.line_to((
    last_rect.x + last_rect.width,
    last_rect.y + last_rect.height,
  ));
  path.line_to((last_rect.x, last_rect.y + last_rect.height));

  let mut expanded_rev = island
    .iter()
    .rev()
    .filter_map(|r| expand_outline_rect(*r, expansion));
  let Some(mut lower_rect) = expanded_rev.next() else {
    return path;
  };

  for upper_rect in expanded_rev {
    path.line_to((lower_rect.x, upper_rect.y + upper_rect.height));
    path.line_to((upper_rect.x, upper_rect.y + upper_rect.height));
    lower_rect = upper_rect;
  }

  path.close();
  path
}

/// Scales an inline box's `x` for a text-fit-scaled line, mirroring the
/// horizontal correction in [`LineScaleState::transform`].
pub fn scale_text_fit_x(
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

/// A shaped glyph positioned within its run, in run-local coordinates.
#[derive(Clone, Copy, Debug)]
pub struct PositionedGlyph {
  /// Glyph id in the run's font.
  pub id: u32,
  /// Run-local horizontal position.
  pub x: f32,
  /// Run-local vertical position.
  pub y: f32,
}

/// Vertical font metrics for a shaped run, in px.
#[derive(Clone, Copy, Debug)]
pub struct RunMetrics {
  /// Typographic ascent.
  pub ascent: f32,
  /// Typographic descent.
  pub descent: f32,
  /// Underline offset from the baseline.
  pub underline_offset: f32,
  /// Underline stroke thickness.
  pub underline_size: f32,
  /// Strikethrough offset from the baseline.
  pub strikethrough_offset: f32,
  /// Strikethrough stroke thickness.
  pub strikethrough_size: f32,
}

/// A shaped, positioned glyph run — the core-owned replacement for
/// `parley::GlyphRun`. Owns everything both backends need to paint a run; carries
/// no borrow into the parley layout.
pub struct ShapedRun {
  /// The run's glyphs, in run-local coordinates.
  pub glyphs: Vec<PositionedGlyph>,
  /// Horizontal offset of the run's start from the line origin.
  pub offset: f32,
  /// Baseline position within the line.
  pub baseline: f32,
  /// Total horizontal advance of the run.
  pub advance: f32,
  /// Paint attributes carried by the run.
  pub brush: InlineBrush,
  /// Vertical font metrics for the run.
  pub metrics: RunMetrics,
  /// Collection index for `skrifa::FontRef::from_index`, paired with [`Self::font_data`].
  pub font_index: u32,
  // Accessor, not a `pub` field: the backing `parley` blob must not leak into the public API.
  font_data: parley::fontique::Blob<u8>,
}

impl ShapedRun {
  /// Font bytes for `skrifa::FontRef::from_index`, paired with [`Self::font_index`].
  pub fn font_data(&self) -> &[u8] {
    self.font_data.as_ref()
  }
}

/// One glyph run positioned on its line, carrying everything both backends need
/// to paint it.
#[non_exhaustive]
pub struct PositionedInlineRun {
  /// The shaped glyph run (metrics, brush, positioned glyphs, font).
  pub glyph_run: ShapedRun,
  /// Glyphs resolved to outlines/bitmaps, keyed by glyph id.
  pub resolved_glyphs: HashMap<u32, ResolvedGlyph>,
  /// Text-fit scale state for the line.
  pub(crate) line_scale: LineScaleState,
  /// Cumulative in-flow inline-box width before this run on the line.
  pub(crate) static_inline_prefix: f32,
  /// Baseline shift applied to glyphs on the line.
  pub baseline_shift: f32,
}

impl PositionedInlineRun {
  /// The run's affine transform composed onto `base` (the element transform for
  /// raster, identity for vector emission).
  pub fn transform(&self, base: Affine) -> Affine {
    self.line_scale.transform(base, self.static_inline_prefix)
  }

  /// Per-glyph inline-offset origin (border/padding box top-left + baseline shift).
  pub fn glyph_offset(&self, layout: ComputedLayout) -> Point<f32> {
    let offset = layout.content_box_offset();
    Point {
      x: offset.x,
      y: offset.y + self.baseline_shift,
    }
  }

  /// Resolves a COLR outline glyph's color layers to `(color, paths)` pairs for
  /// vector emission, applying the run's font palette and `foreground` (current)
  /// color exactly as the raster backend does. Returns empty for non-color glyphs.
  pub fn resolve_color_layers<'g>(
    &self,
    outline: &'g ResolvedOutlineGlyph,
    foreground: Color,
  ) -> Vec<(Color, &'g [PathCommand])> {
    let Some(layers) = outline.color_layers() else {
      return Vec::new();
    };
    let font = FontRef::from_index(self.glyph_run.font_data(), self.glyph_run.font_index).ok();
    let palettes = font.as_ref().map(MetadataProvider::color_palettes);
    let palette = palettes.as_ref().and_then(|palettes| palettes.get(0));
    let foreground_opacity = foreground.0[3] as f32 / 255.0;

    layers
      .iter()
      .filter_map(|layer: &ResolvedColorLayer| {
        let color = if layer.palette_index == u16::MAX {
          let alpha = (foreground_opacity * layer.alpha * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
          Color([foreground.0[0], foreground.0[1], foreground.0[2], alpha])
        } else {
          let record = palette
            .as_ref()?
            .colors()
            .get(usize::from(layer.palette_index))?;
          let alpha = ((record.alpha() as f32 / 255.0) * layer.alpha * foreground_opacity * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
          Color([record.red(), record.green(), record.blue(), alpha])
        };
        Some((color, layer.paths.as_slice()))
      })
      .collect()
  }
}

/// The single inline enumeration shared by both backends: positioned glyph runs
/// in paint order, positioned inline boxes, and text-outline rects. Built once by
/// [`resolve_inline_runs`]; the raster backend rasterizes it and the vector
/// backend emits it; only the painting differs.
#[non_exhaustive]
pub struct InlineRunLayout {
  /// Glyph runs in line/visual order.
  pub runs: Vec<PositionedInlineRun>,
  /// In-flow and out-of-flow inline boxes, positioned, sorted by id.
  pub inline_boxes: Vec<VisualInlineBox>,
  /// Text-outline rects (unmerged), in collection order.
  pub outline_rects: Vec<InlineOutlineRect>,
}

/// Walks `built` once, resolving every glyph run, inline box, and outline rect
/// into backend-agnostic positioned drawables. This is the one inline-layout
/// enumeration; backends differ only in how they paint the result.
pub fn resolve_inline_runs(
  built: &BuiltInlineLayout<'_>,
  context: &RenderContext,
  layout: ComputedLayout,
) -> Result<InlineRunLayout, FontError> {
  let BuiltInlineLayout {
    layout: inline_layout,
    spans,
    custom_inline_boxes,
    line_scales,
    ..
  } = built;
  let line_vertical_metrics = built.line_metrics();
  let line_states = built.line_states();

  let need_outline = spans.iter().any(|span| match span {
    ProcessedInlineSpan::Text { style, .. } => {
      style.outline_width > 0.0 && style.outline_style.is_rendered()
    }
    ProcessedInlineSpan::Box(_) => false,
  });

  let mut runs = Vec::new();
  let mut outline_rects = Vec::new();
  let mut positioned_inline_boxes: HashMap<u64, VisualInlineBox> = HashMap::new();

  for (line_index, line) in inline_layout.lines().enumerate() {
    let Some(setup) = LineSetup::new(
      &line,
      layout,
      &line_vertical_metrics,
      line_scales,
      line_index,
    ) else {
      continue;
    };
    let mut static_inline_prefix = 0.0_f32;

    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let run = glyph_run.run();
          let font = FontRef::from_index(run.font().data.as_ref(), run.font().index)
            .map_err(|_| FontError::InvalidFontIndex)?;
          let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);
          let resolved_glyphs = context
            .fonts
            .with_context(|fonts| fonts.resolve_glyphs(&glyph_run, font, glyph_ids));

          if need_outline
            && let Some(outline_rect) = collect_glyph_run_outline_rect(
              &glyph_run,
              layout,
              line_index,
              layout.border.top + layout.padding.top + glyph_run.baseline() + setup.baseline_shift
                - setup.resolved_metrics.resolved_ascent,
              setup.resolved_metrics.resolved_line_height,
              setup.state,
              static_inline_prefix,
            )
          {
            outline_rects.push(outline_rect);
          }

          let metrics = run.metrics();
          let shaped = ShapedRun {
            glyphs: glyph_run
              .positioned_glyphs()
              .map(|g| PositionedGlyph {
                id: g.id,
                x: g.x,
                y: g.y,
              })
              .collect(),
            offset: glyph_run.offset(),
            baseline: glyph_run.baseline(),
            advance: glyph_run.advance(),
            brush: glyph_run.style().brush,
            metrics: RunMetrics {
              ascent: metrics.ascent,
              descent: metrics.descent,
              underline_offset: metrics.underline_offset,
              underline_size: metrics.underline_size,
              strikethrough_offset: metrics.strikethrough_offset,
              strikethrough_size: metrics.strikethrough_size,
            },
            font_data: run.font().data.clone(),
            font_index: run.font().index,
          };

          runs.push(PositionedInlineRun {
            glyph_run: shaped,
            resolved_glyphs,
            line_scale: setup.state,
            static_inline_prefix,
            baseline_shift: setup.baseline_shift,
          });
        }
        PositionedLayoutItem::InlineBox(inline_box) => {
          if inline_box.kind != InlineBoxKind::InFlow {
            continue;
          }
          let Some(inline_box) =
            resolve_visual_inline_box(inline_box, Some(line_states[line_index]), spans)
          else {
            continue;
          };
          let inline_box = VisualInlineBox {
            x: scale_text_fit_x(
              inline_box.layout_x,
              setup.line_scale_origin_x,
              setup.state.scale,
              static_inline_prefix,
              setup.state.alignment_correction,
            ),
            ..inline_box
          };
          positioned_inline_boxes.insert(inline_box.id, inline_box);
          static_inline_prefix += inline_box.layout_advance;
        }
      }
    }
  }

  for inline_box in custom_inline_boxes {
    let Some(inline_box) = resolve_visual_inline_box(inline_box.clone(), None, spans) else {
      continue;
    };
    positioned_inline_boxes.insert(inline_box.id, inline_box);
  }

  let mut inline_boxes: Vec<_> = positioned_inline_boxes.into_values().collect();
  inline_boxes.sort_by_key(|inline_box| inline_box.id);

  Ok(InlineRunLayout {
    runs,
    inline_boxes,
    outline_rects,
  })
}

/// A text decoration line (underline/overline/line-through) as a fillable rect.
/// `transform` maps the `(0, 0, width, height)` rect into border-box space.
pub struct DecorationRect {
  /// Rect width in pixels (run advance, snapped like the raster path).
  pub width: f32,
  /// Rect height in pixels (decoration thickness).
  pub height: f32,
  /// Decoration color, already resolved against `current-color`.
  pub color: Color,
  /// Affine transform into border-box space (`[a, b, c, d, e, f]`).
  pub transform: [f32; 6],
  /// Whether the line paints above glyphs (line-through) vs below (under/overline).
  pub over: bool,
}

/// The active decoration lines for a glyph run, in border-box space. Mirrors the
/// raster geometry in `draw_decoration` (skip-ink is a raster-only refinement and
/// omitted here). `transform` is the run's border-box transform
/// ([`PositionedInlineRun::transform`] with an identity base).
pub fn run_decorations(
  glyph_run: &ShapedRun,
  layout: ComputedLayout,
  baseline_shift: f32,
  transform: Affine,
) -> Vec<DecorationRect> {
  let mut out = Vec::new();
  let brush = &glyph_run.brush;
  let lines = brush.decoration_line;
  if lines.is_empty() {
    return out;
  }
  let metrics = &glyph_run.metrics;
  let start_x = layout.border.left + layout.padding.left + glyph_run.offset;
  let snapped_start_x = start_x.floor();
  let width = (start_x + glyph_run.advance).ceil() - snapped_start_x;
  if width <= 0.0 {
    return out;
  }
  let baseline = glyph_run.baseline + baseline_shift;
  let top = layout.border.top + layout.padding.top;
  let thickness = |from_font: f32| match brush.decoration_thickness {
    SizedTextDecorationThickness::Value(value) => value,
    SizedTextDecorationThickness::FromFont => from_font,
  };
  let mut emit = |y_offset: f32, height: f32, over: bool| {
    if height <= 0.0 {
      return;
    }
    let matrix = transform * Affine::translation(snapped_start_x, top + y_offset);
    out.push(DecorationRect {
      width,
      height,
      color: brush.decoration_color,
      transform: matrix.to_cols_array(),
      over,
    });
  };
  if lines.contains(TextDecorationLines::UNDERLINE) {
    emit(
      baseline - metrics.underline_offset + brush.underline_offset,
      thickness(metrics.underline_size),
      false,
    );
  }
  if lines.contains(TextDecorationLines::OVERLINE) {
    emit(
      baseline - metrics.ascent - metrics.underline_offset,
      thickness(metrics.underline_size),
      false,
    );
  }
  if lines.contains(TextDecorationLines::LINE_THROUGH) {
    emit(
      baseline - metrics.strikethrough_offset,
      thickness(metrics.strikethrough_size),
      true,
    );
  }
  out
}

/// Resolve the inline layout's max width and optional max height from available space and known dimensions.
pub fn create_inline_constraint(
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
  let mut width_constraint = known_width.or(available_width).unwrap_or(f32::INFINITY);

  if known_width.is_some()
    && context.style.box_sizing == BoxSizing::BorderBox
    && width_constraint.is_finite()
  {
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
  custom_inline_boxes: &mut Vec<PositionedInlineBox>,
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
        custom_inline_boxes.push(positioned_float);
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

/// Truncates text in the layout to fit within `max_width` and appends an ellipsis.
fn make_ellipsis_layout<'c>(
  layout: &mut InlineLayout,
  spans: &mut Vec<ProcessedInlineSpan<'c>>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  root_style: &'c SizedFontStyle,
  context: &RenderContext,
  custom_inline_boxes: &mut Vec<PositionedInlineBox>,
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

  let (mut final_layout, _) = context.tree_builder(root_style.into(), |builder| {
    push_spans_into_builder(builder, spans);
    builder.push_style_span(text_style_with_span_id(ellipsis_style, None));
    builder.push_text(ellipsis_char);
    builder.pop_style_span();
  });

  apply_text_indent(&mut final_layout, root_style, max_width);
  let text_wrap_mode = root_style.parent.resolved_text_wrap_mode();
  custom_inline_boxes.clear();
  break_lines(
    &mut final_layout,
    max_width,
    max_height,
    inline_line_height_hint(root_style),
    text_wrap_mode,
    spans,
    custom_inline_boxes,
  );
  *layout = final_layout;
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
  use std::{fs::File, io::Read, path::Path};

  use super::*;
  use crate::{
    Fonts,
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::{FontOverride, FontResource, GenericFamily},
    style::{Color, ColorInput, Display, SizingContext, Style, StyleDeclaration, WhiteSpace},
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
        .any(|(span_id, _, color)| *span_id == Some(0) && *color == orange),
      "{segments:#?}"
    );
    assert!(
      segments
        .iter()
        .any(|(span_id, _, color)| *span_id == Some(2) && *color == blue),
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
}

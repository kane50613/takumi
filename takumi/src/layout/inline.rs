use std::{borrow::Cow, ops::Range};

use parley::{
  BreakReason, IndentOptions, InlineBox, InlineBoxKind, Line, LineMetrics, PositionedInlineBox,
  PositionedLayoutItem, TextStyle, YieldData,
};
use taffy::{AvailableSpace, Layout, Rect, Size};

use crate::{
  GlobalContext,
  layout::{
    node::Node,
    style::{
      BoxSizing, Color, Float, FontSynthesis, Position, ResolvedVerticalAlign, SizedFontStyle,
      SizedTextDecorationThickness, TextDecorationLines, TextDecorationSkipInk, TextFitMode,
      TextFitTarget, TextOverflow, TextWrapMode, TextWrapStyle, VerticalAlign,
    },
    tree::RenderNode,
  },
  rendering::{
    MaxHeight, RebreakOptions, RenderContext, apply_text_transform, apply_white_space_collapse,
    make_balanced_text, make_pretty_text,
  },
};

pub(crate) struct InlineLayoutRequest<'c, 'g> {
  pub(crate) items: Vec<InlineItem<'c, 'g>>,
  pub(crate) available_space: Size<AvailableSpace>,
  pub(crate) max_width: f32,
  pub(crate) max_height: Option<MaxHeight>,
  pub(crate) style: &'c SizedFontStyle<'c>,
  pub(crate) global: &'g GlobalContext,
  pub(crate) mode: InlineLayoutMode,
}

pub(crate) struct BuiltInlineLayout<'c, 'g> {
  pub(crate) layout: InlineLayout,
  pub(crate) text: String,
  pub(crate) spans: Vec<ProcessedInlineSpan<'c, 'g>>,
  pub(crate) custom_inline_boxes: Vec<PositionedInlineBox>,
  pub(crate) line_scales: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InlineLayoutMode {
  Measure,
  Draw,
}

pub(crate) struct InlineBoxItem<'c, 'g> {
  pub(crate) render_node: &'c RenderNode<'g>,
  pub(crate) inline_box: InlineBox,
  pub(crate) paint_width: f32,
  pub(crate) paint_height: f32,
  pub(crate) margin: Rect<f32>,
  pub(crate) padding: Rect<f32>,
  pub(crate) border: Rect<f32>,
  pub(crate) baseline_offset: Option<f32>,
  pub(crate) vertical_align: ResolvedVerticalAlign,
}

impl From<&InlineBoxItem<'_, '_>> for Layout {
  fn from(value: &InlineBoxItem<'_, '_>) -> Self {
    Layout {
      size: Size {
        width: value.paint_width,
        height: value.paint_height,
      },
      margin: value.margin,
      padding: value.padding,
      border: value.border,
      ..Default::default()
    }
  }
}

fn inline_box_kind(render_node: &RenderNode<'_>) -> InlineBoxKind {
  if matches!(render_node.context.style.position, Position::Absolute) {
    InlineBoxKind::OutOfFlow
  } else if render_node.context.style.float != Float::None {
    InlineBoxKind::CustomOutOfFlow
  } else {
    InlineBoxKind::InFlow
  }
}

pub(crate) enum ProcessedInlineSpan<'c, 'g> {
  Text {
    span_id: u64,
    byte_range: Range<usize>,
    text: String,
    style: SizedFontStyle<'c>,
  },
  Box(InlineBoxItem<'c, 'g>),
}

pub(crate) enum InlineItem<'c, 'g> {
  RenderNode {
    render_node: &'c RenderNode<'g>,
  },
  Text {
    text: Cow<'c, str>,
    context: &'c RenderContext<'g>,
  },
}

pub(crate) fn collect_inline_items<'n, 'g>(root: &'n RenderNode<'g>) -> Vec<InlineItem<'n, 'g>> {
  let mut items = Vec::new();
  collect_inline_items_impl(root, 0, &mut items);
  items
}

fn collect_inline_items_impl<'n, 'g>(
  node: &'n RenderNode<'g>,
  depth: usize,
  items: &mut Vec<InlineItem<'n, 'g>>,
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

pub type InlineLayout = parley::Layout<InlineBrush>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParentFontMetrics {
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
pub(crate) struct InlineBrush {
  pub source_span_id: Option<u64>,
  pub opacity: f32,
  pub color: Color,
  pub decoration_color: Color,
  pub decoration_thickness: SizedTextDecorationThickness,
  pub decoration_line: TextDecorationLines,
  pub decoration_skip_ink: TextDecorationSkipInk,
  pub stroke_color: Color,
  pub font_synthesis: FontSynthesis,
  pub line_height_scales_with_text_fit: bool,
  pub vertical_align: VerticalAlign,
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      source_span_id: None,
      opacity: 1.0,
      color: Color::black(),
      decoration_color: Color::black(),
      decoration_thickness: SizedTextDecorationThickness::Value(0.0),
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

fn refresh_text_span_ranges(spans: &mut [ProcessedInlineSpan<'_, '_>]) {
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

fn tail_text_span<'a, 'c, 'g>(
  spans: &'a [ProcessedInlineSpan<'c, 'g>],
) -> Option<(&'a SizedFontStyle<'c>, u64)> {
  spans.iter().rev().find_map(|span| match span {
    ProcessedInlineSpan::Text { span_id, style, .. } => Some((style, *span_id)),
    ProcessedInlineSpan::Box(_) => None,
  })
}

fn measure_ellipsis_width(
  global: &GlobalContext,
  ellipsis_style: &SizedFontStyle,
  ellipsis_char: &str,
) -> f32 {
  let (mut ellipsis_layout, _) =
    global
      .font_context
      .tree_builder(ellipsis_style.into(), |builder| {
        builder.push_text(ellipsis_char);
      });
  ellipsis_layout.break_all_lines(None);
  ellipsis_layout
    .lines()
    .next()
    .map(|line| line.runs().map(|run| run.advance()).sum::<f32>())
    .unwrap_or(0.0)
}

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
pub(crate) struct ResolvedLineMetrics {
  pub(crate) resolved_ascent: f32,
  pub(crate) resolved_descent: f32,
  pub(crate) resolved_leading: f32,
  pub(crate) resolved_line_height: f32,
  pub(crate) resolved_baseline: f32,
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
    spans: &[ProcessedInlineSpan<'_, '_>],
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
    spans: &[ProcessedInlineSpan<'_, '_>],
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
  item: &InlineBoxItem<'_, '_>,
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

pub(crate) fn resolve_inline_line_metrics(
  inline_layout: &InlineLayout,
  spans: &[ProcessedInlineSpan<'_, '_>],
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
          has_contribution = true;
        }
      }
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
pub(crate) struct ResolvedInlineLineState {
  pub(crate) adjusted_metrics: LineMetrics,
  pub(crate) baseline_shift: f32,
  pub(crate) parent_x_height: Option<f32>,
  pub(crate) parent_text_metrics: Option<(f32, f32)>,
}

pub(crate) fn resolve_inline_line_states(
  inline_layout: &InlineLayout,
  spans: &[ProcessedInlineSpan<'_, '_>],
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
  spans: &[ProcessedInlineSpan<'_, '_>],
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
pub(crate) struct VisualInlineBox {
  pub(crate) id: u64,
  pub(crate) x: f32,
  pub(crate) y: f32,
  pub(crate) width: f32,
  pub(crate) height: f32,
  pub(crate) layout_x: f32,
  pub(crate) layout_advance: f32,
}

pub(crate) fn resolve_visual_inline_box(
  inline_box: PositionedInlineBox,
  line_state: Option<ResolvedInlineLineState>,
  spans: &[ProcessedInlineSpan<'_, '_>],
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

fn truncation_plan<'c, 'g>(
  checkpoints: &[TruncationCheckpoint],
  spans: &[ProcessedInlineSpan<'c, 'g>],
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

fn text_span_style_by_id<'a, 'c, 'g>(
  spans: &'a [ProcessedInlineSpan<'c, 'g>],
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

fn truncated_tail_text_span_id<'c, 'g>(
  spans: &[ProcessedInlineSpan<'c, 'g>],
  span_cut_idx: Option<usize>,
) -> Option<u64> {
  span_cut_idx.and_then(|cut_idx| {
    spans[..cut_idx].iter().rev().find_map(|span| match span {
      ProcessedInlineSpan::Text { span_id, .. } => Some(*span_id),
      ProcessedInlineSpan::Box(_) => None,
    })
  })
}

fn apply_truncation_plan<'c, 'g>(
  spans: &mut Vec<ProcessedInlineSpan<'c, 'g>>,
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
  spans: &[ProcessedInlineSpan<'_, '_>],
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

fn build_inline_layout_tree<'c, 'g: 'c>(
  items: &[InlineItem<'c, 'g>],
  available_space: Size<AvailableSpace>,
  style: &'c SizedFontStyle,
  global: &'g GlobalContext,
) -> BuiltInlineLayout<'c, 'g> {
  let mut spans: Vec<ProcessedInlineSpan<'c, 'g>> = Vec::new();
  let (layout, text) = global.font_context.tree_builder(style.into(), |builder| {
    let mut index_pos = 0;
    let mut previous_collapsible_space = false;
    let mut previous_was_line_break = false;

    for item in items {
      match item {
        InlineItem::Text { text, context } => {
          let span_style = context.style.to_sized_font_style(context);
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

          builder.push_style_span(text_style_with_span_id(&span_style, Some(span_id)));
          builder.push_text(&collapsed);
          builder.pop_style_span();

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
            top: context.style.border_top_width,
            right: context.style.border_right_width,
            bottom: context.style.border_bottom_width,
            left: context.style.border_left_width,
          }
          .map(|length| length.to_px(&context.sizing, 0.0));

          let atomic_metrics = render_node
            .node
            .as_ref()
            .map(|_| render_node.measure_inline_box(available_space));
          let content_size = atomic_metrics.map_or(Size::zero(), |metrics| metrics.size);
          let raw_baseline_offset = atomic_metrics.and_then(|metrics| metrics.baseline_offset);

          let paint_width = if render_node.participates_as_inline_box() {
            content_size.width + margin.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
          } else {
            content_size.width
              + margin.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
              + padding.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
              + border.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
          };
          let paint_height = if render_node.participates_as_inline_box() {
            content_size.height + margin.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
          } else {
            content_size.height
              + margin.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
              + padding.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
              + border.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
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
            inline_box: inline_box.clone(),
            paint_width,
            paint_height,
            margin,
            padding,
            border,
            baseline_offset,
            vertical_align,
          }));
          builder.push_inline_box(inline_box);
          previous_collapsible_space = false;
          previous_was_line_break = false;
        }
      }
    }
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
  built: &mut BuiltInlineLayout<'_, '_>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  style: &SizedFontStyle,
) -> (TextWrapMode, f32) {
  let text_wrap_mode = style.parent.text_wrap_mode_and_line_clamp().0;
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
  spans: &[ProcessedInlineSpan<'_, '_>],
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

pub(crate) fn create_inline_layout<'c, 'g: 'c>(
  request: InlineLayoutRequest<'c, 'g>,
) -> BuiltInlineLayout<'c, 'g> {
  let InlineLayoutRequest {
    items,
    available_space,
    max_width,
    max_height,
    style,
    global,
    mode,
  } = request;
  let mut built = build_inline_layout_tree(&items, available_space, style, global);
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
          global,
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

pub(crate) fn resolve_inline_max_height(
  font_style: &SizedFontStyle,
  content_box_height: f32,
) -> Option<MaxHeight> {
  let resolved_line_clamp = font_style.parent.text_wrap_mode_and_line_clamp().1;
  resolved_line_clamp
    .as_ref()
    .map(|clamp| MaxHeight::HeightAndLines(content_box_height, clamp.count))
    .or_else(|| {
      (font_style.parent.text_overflow == TextOverflow::Ellipsis)
        .then_some(MaxHeight::Absolute(content_box_height))
    })
}

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
  let mut width_constraint = known_width.or(available_width).unwrap_or(f32::MAX);

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
        context.style.border_left_width.to_px(sizing, 0.0)
      }
      + if !context.style.border_right_style.is_rendered() {
        0.0
      } else {
        context.style.border_right_width.to_px(sizing, 0.0)
      };
    width_constraint = (width_constraint - horizontal_insets).max(0.0);
  }

  // applies a maximum height to reduce unnecessary calculation.
  let max_height = match (
    context.sizing.viewport.size.height,
    context.style.text_wrap_mode_and_line_clamp().1,
  ) {
    (Some(height), Some(line_clamp)) => {
      Some(MaxHeight::HeightAndLines(height as f32, line_clamp.count))
    }
    (Some(height), None) => Some(MaxHeight::Absolute(height as f32)),
    (None, Some(line_clamp)) => Some(MaxHeight::Lines(line_clamp.count)),
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
  spans: &[ProcessedInlineSpan<'_, '_>],
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
fn make_ellipsis_layout<'c, 'g: 'c>(
  layout: &mut InlineLayout,
  spans: &mut Vec<ProcessedInlineSpan<'c, 'g>>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  root_style: &'c SizedFontStyle,
  global: &GlobalContext,
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
    let ellipsis_w = measure_ellipsis_width(global, ellipsis_style, ellipsis_char);

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

  let (mut final_layout, _) = global
    .font_context
    .tree_builder(root_style.into(), |builder| {
      for span in spans.iter() {
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
      builder.push_style_span(text_style_with_span_id(ellipsis_style, None));
      builder.push_text(ellipsis_char);
      builder.pop_style_span();
    });

  apply_text_indent(&mut final_layout, root_style, max_width);
  let text_wrap_mode = root_style.parent.text_wrap_mode_and_line_clamp().0;
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
  use super::*;
  use std::{fs::File, io::Read, path::Path};

  use crate::{
    GlobalContext,
    layout::{
      Viewport,
      node::Node,
      style::{Color, ColorInput, Display, Style, StyleDeclaration, WhiteSpace},
      tree::RenderNode,
    },
    rendering::RenderContext,
    resources::font::FontResource,
  };
  use parley::{GenericFamily, fontique::FontInfoOverride};

  fn create_test_context() -> GlobalContext {
    let mut context = GlobalContext::default();
    let path =
      Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/fonts/geist/Geist[wght].woff2");
    let mut font_data = Vec::new();
    let mut file = File::open(&path)
      .unwrap_or_else(|error| panic!("failed to open test font {}: {error}", path.display()));
    file
      .read_to_end(&mut font_data)
      .unwrap_or_else(|error| panic!("failed to read test font {}: {error}", path.display()));
    context
      .font_context
      .load_and_store(
        FontResource::new(font_data)
          .override_info(FontInfoOverride {
            family_name: Some("Geist"),
            ..Default::default()
          })
          .generic_family(GenericFamily::SansSerif),
      )
      .unwrap_or_else(|error| panic!("failed to load test font {}: {error}", path.display()));
    context
  }

  fn glyph_run_segments(node: Node, global: &GlobalContext) -> Vec<(Option<u64>, String, Color)> {
    let context = RenderContext::new_test(global, Viewport::new((1200, 630)));
    let render_node = RenderNode::from_node(&context, node);
    let font_style = render_node
      .context
      .style
      .to_sized_font_style(&render_node.context);
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
      global,
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
    let global = create_test_context();
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

    let segments = glyph_run_segments(node, &global);

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
}

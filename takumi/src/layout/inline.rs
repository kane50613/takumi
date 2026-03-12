use std::{borrow::Cow, ops::Range};

use parley::{InlineBox, PositionedLayoutItem, TextStyle};
use taffy::{AvailableSpace, Layout, Rect, Size};

use crate::{
  GlobalContext,
  layout::{
    node::Node,
    style::{
      Color, FontSynthesis, ResolvedVerticalAlign, SizedFontStyle, SizedTextDecorationThickness,
      TextDecorationLines, TextDecorationSkipInk, TextOverflow, TextWrapStyle, VerticalAlign,
    },
    tree::RenderNode,
  },
  rendering::{
    MaxHeight, RenderContext, apply_text_transform, apply_white_space_collapse, make_balanced_text,
    make_pretty_text,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineLayoutStage {
  Measure,
  Draw,
}

pub(crate) struct InlineBoxItem<'c, 'g, N: Node<N>> {
  pub(crate) render_node: &'c RenderNode<'g, N>,
  pub(crate) inline_box: InlineBox,
  pub(crate) margin: Rect<f32>,
  pub(crate) padding: Rect<f32>,
  pub(crate) border: Rect<f32>,
  pub(crate) vertical_align: ResolvedVerticalAlign,
}

impl<N: Node<N>> From<&InlineBoxItem<'_, '_, N>> for Layout {
  fn from(value: &InlineBoxItem<'_, '_, N>) -> Self {
    Layout {
      size: Size {
        width: value.inline_box.width,
        height: value.inline_box.height,
      },
      margin: value.margin,
      padding: value.padding,
      border: value.border,
      ..Default::default()
    }
  }
}

pub(crate) enum ProcessedInlineSpan<'c, 'g, N: Node<N>> {
  Text {
    span_id: u64,
    byte_range: Range<usize>,
    text: String,
    style: SizedFontStyle<'c>,
  },
  Box(InlineBoxItem<'c, 'g, N>),
}

pub(crate) enum InlineItem<'c, 'g, N: Node<N>> {
  RenderNode {
    render_node: &'c RenderNode<'g, N>,
  },
  Text {
    text: Cow<'c, str>,
    context: &'c RenderContext<'g>,
  },
}

pub(crate) fn collect_inline_items<'n, 'g, N: Node<N>>(
  root: &'n RenderNode<'g, N>,
) -> Vec<InlineItem<'n, 'g, N>> {
  let mut items = Vec::new();
  collect_inline_items_impl(root, 0, &mut items);
  items
}

fn collect_inline_items_impl<'n, 'g, N: Node<N>>(
  node: &'n RenderNode<'g, N>,
  depth: usize,
  items: &mut Vec<InlineItem<'n, 'g, N>>,
) {
  if depth > 0 && node.is_inline_atomic_container() {
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

pub enum InlineContentKind<'c> {
  Text(Cow<'c, str>),
  Box,
}

pub type InlineLayout = parley::Layout<InlineBrush>;

#[derive(Clone, PartialEq, Copy, Debug)]
pub(crate) struct InlineBrush {
  pub source_span_id: Option<u64>,
  pub color: Color,
  pub decoration_color: Color,
  pub decoration_thickness: SizedTextDecorationThickness,
  pub decoration_line: TextDecorationLines,
  pub decoration_skip_ink: TextDecorationSkipInk,
  pub stroke_color: Color,
  pub font_synthesis: FontSynthesis,
  pub vertical_align: VerticalAlign,
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      source_span_id: None,
      color: Color::black(),
      decoration_color: Color::black(),
      decoration_thickness: SizedTextDecorationThickness::Value(0.0),
      decoration_line: TextDecorationLines::empty(),
      decoration_skip_ink: TextDecorationSkipInk::default(),
      stroke_color: Color::black(),
      font_synthesis: FontSynthesis::default(),
      vertical_align: VerticalAlign::default(),
    }
  }
}

fn text_style_with_span_id<'s>(
  style: &'s SizedFontStyle<'s>,
  source_span_id: Option<u64>,
) -> TextStyle<'s, InlineBrush> {
  let mut text_style: TextStyle<'s, InlineBrush> = style.into();
  text_style.brush.source_span_id = source_span_id;
  text_style
}

fn refresh_text_span_ranges<N: Node<N>>(spans: &mut [ProcessedInlineSpan<'_, '_, N>]) {
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

fn tail_text_span<'a, 'c, 'g, N: Node<N>>(
  spans: &'a [ProcessedInlineSpan<'c, 'g, N>],
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

fn truncation_plan<'c, 'g, N: Node<N>>(
  checkpoints: &[TruncationCheckpoint],
  spans: &[ProcessedInlineSpan<'c, 'g, N>],
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

fn text_span_style_by_id<'a, 'c, 'g, N: Node<N>>(
  spans: &'a [ProcessedInlineSpan<'c, 'g, N>],
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

fn truncated_tail_text_span_id<'c, 'g, N: Node<N>>(
  spans: &[ProcessedInlineSpan<'c, 'g, N>],
  span_cut_idx: Option<usize>,
) -> Option<u64> {
  span_cut_idx.and_then(|cut_idx| {
    spans[..cut_idx].iter().rev().find_map(|span| match span {
      ProcessedInlineSpan::Text { span_id, .. } => Some(*span_id),
      ProcessedInlineSpan::Box(_) => None,
    })
  })
}

fn apply_truncation_plan<'c, 'g, N: Node<N>>(
  spans: &mut Vec<ProcessedInlineSpan<'c, 'g, N>>,
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

pub(crate) fn measure_inline_layout(layout: &mut InlineLayout, max_width: f32) -> Size<f32> {
  let (max_run_width, total_height) =
    layout
      .lines()
      .fold((0.0, 0.0), |(max_run_width, total_height), line| {
        let metrics = line.metrics();

        (
          metrics.advance.max(max_run_width),
          total_height + metrics.line_height,
        )
      });

  Size {
    width: max_run_width.ceil().min(max_width),
    height: total_height.ceil(),
  }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_inline_layout<'c, 'g: 'c, N: Node<N> + 'c>(
  items: impl Iterator<Item = InlineItem<'c, 'g, N>>,
  available_space: Size<AvailableSpace>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  style: &'c SizedFontStyle,
  global: &'g GlobalContext,
  stage: InlineLayoutStage,
) -> (InlineLayout, String, Vec<ProcessedInlineSpan<'c, 'g, N>>) {
  let mut spans: Vec<ProcessedInlineSpan<'c, 'g, N>> = Vec::new();

  let (mut layout, text) = global.font_context.tree_builder(style.into(), |builder| {
    let mut index_pos = 0;

    for item in items {
      match item {
        InlineItem::Text { text, context } => {
          let span_style = context.style.to_sized_font_style(context);
          let transformed = apply_text_transform(&text, context.style.text_transform);
          let collapsed =
            apply_white_space_collapse(&transformed, style.parent.white_space_collapse);
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

          let content_size = if render_node.is_inline_atomic_container() {
            render_node.measure_atomic_subtree(available_space)
          } else if let Some(node) = &render_node.node {
            node.measure(
              context,
              available_space,
              Size::NONE,
              &taffy::Style::default(),
            )
          } else {
            Size::zero()
          };

          let inline_box = InlineBox {
            index: index_pos,
            id: spans.len() as u64,
            width: if render_node.is_inline_atomic_container() {
              content_size.width + margin.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
            } else {
              content_size.width
                + margin.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
                + padding.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
                + border.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)
            },
            height: if render_node.is_inline_atomic_container() {
              content_size.height + margin.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
            } else {
              content_size.height
                + margin.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
                + padding.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
                + border.grid_axis_sum(taffy::AbsoluteAxis::Vertical)
            },
          };

          spans.push(ProcessedInlineSpan::Box(InlineBoxItem {
            render_node,
            inline_box: inline_box.clone(),
            margin,
            padding,
            border,
            vertical_align,
          }));

          builder.push_inline_box(inline_box);
        }
      }
    }
  });

  break_lines(&mut layout, max_width, max_height);

  if stage == InlineLayoutStage::Measure {
    return (layout, text, spans);
  }

  // Handle ellipsis when text overflows
  if style.parent.text_overflow == TextOverflow::Ellipsis {
    let is_overflowing = layout
      .lines()
      .last()
      .is_some_and(|last_line| last_line.text_range().end < text.len());

    if is_overflowing {
      make_ellipsis_layout(
        &mut layout,
        &mut spans,
        max_width,
        max_height,
        style,
        global,
      );
    }
  }

  let line_count = layout.lines().count();

  if style.parent.text_wrap_style == TextWrapStyle::Balance {
    make_balanced_text(
      &mut layout,
      max_width,
      max_height,
      line_count,
      style.sizing.viewport.device_pixel_ratio,
    );
  }

  if style.parent.text_wrap_style == TextWrapStyle::Pretty {
    make_pretty_text(&mut layout, max_width, max_height);
  }

  layout.align(
    Some(max_width),
    style.parent.text_align.into(),
    Default::default(),
  );

  (layout, text, spans)
}

pub(crate) fn create_inline_constraint(
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
) -> (f32, Option<MaxHeight>) {
  let width_constraint = known_dimensions
    .width
    .or(match available_space.width {
      AvailableSpace::MinContent => Some(0.0),
      AvailableSpace::MaxContent => None,
      AvailableSpace::Definite(width) => Some(width),
    })
    .unwrap_or(f32::MAX);

  // applies a maximum height to reduce unnecessary calculation.
  let max_height = match (
    context.sizing.viewport.height,
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
) {
  let Some(max_height) = max_height else {
    return layout.break_all_lines(Some(max_width));
  };

  let (limit_height, limit_lines) = match max_height {
    MaxHeight::Lines(lines) => (f32::MAX, lines),
    MaxHeight::Absolute(height) => (height, u32::MAX),
    MaxHeight::HeightAndLines(height, lines) => (height, lines),
  };

  let mut total_height = 0.0;
  let mut line_count = 0;
  let mut breaker = layout.break_lines();

  while total_height < limit_height && line_count < limit_lines {
    let Some((_, height)) = breaker.break_next(max_width) else {
      break;
    };
    total_height += height;
    line_count += 1;
  }

  if total_height > limit_height {
    breaker.revert();
  }

  breaker.finish();
}

/// Truncates text in the layout to fit within `max_width` and appends an ellipsis.
fn make_ellipsis_layout<'c, 'g: 'c, N: Node<N> + 'c>(
  layout: &mut InlineLayout,
  spans: &mut Vec<ProcessedInlineSpan<'c, 'g, N>>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  root_style: &'c SizedFontStyle,
  global: &GlobalContext,
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

  break_lines(&mut final_layout, max_width, max_height);
  *layout = final_layout;
}

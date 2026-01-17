use std::borrow::Cow;

use parley::InlineBox;
use taffy::{AvailableSpace, Size};

use crate::{
  GlobalContext,
  layout::{
    node::Node,
    style::{Color, SizedFontStyle, TextOverflow, TextWrapStyle},
    tree::NodeTree,
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

pub(crate) struct InlineNodeItem<'c, 'g, N: Node<N>> {
  pub(crate) node: &'c N,
  pub(crate) context: &'c RenderContext<'g>,
}

pub(crate) enum ProcessedInlineSpan<'c, 'g, N: Node<N>> {
  Text {
    text: String,
    style: SizedFontStyle<'c>,
  },
  Box {
    node: InlineNodeItem<'c, 'g, N>,
    inline_box: InlineBox,
  },
}

pub(crate) enum InlineItem<'c, 'g, N: Node<N>> {
  Node(InlineNodeItem<'c, 'g, N>),
  Text {
    text: Cow<'c, str>,
    context: &'c RenderContext<'g>,
  },
}

pub enum InlineContentKind<'c> {
  Text(Cow<'c, str>),
  Box,
}

pub type InlineLayout = parley::Layout<InlineBrush>;

#[derive(Clone, PartialEq, Copy, Debug)]
pub struct InlineBrush {
  pub color: Color,
  pub decoration_color: Color,
  pub stroke_color: Color,
  pub background_color: Color,
}

impl Default for InlineBrush {
  fn default() -> Self {
    Self {
      color: Color::black(),
      decoration_color: Color::black(),
      stroke_color: Color::black(),
      background_color: Color::transparent(),
    }
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
    let mut idx = 0;
    let mut index_pos = 0;

    for item in items {
      match item {
        InlineItem::Text { text, context } => {
          let span_style = context.style.to_sized_font_style(context);
          let transformed = apply_text_transform(&text, context.style.text_transform);
          let collapsed =
            apply_white_space_collapse(&transformed, style.parent.white_space_collapse());

          builder.push_style_span((&span_style).into());
          builder.push_text(&collapsed);
          builder.pop_style_span();

          index_pos += collapsed.len();

          spans.push(ProcessedInlineSpan::Text {
            text: collapsed.into_owned(),
            style: span_style,
          });
        }
        InlineItem::Node(item) => {
          let size = item.node.measure(
            item.context,
            available_space,
            Size::NONE,
            &taffy::Style::default(),
          );

          let inline_box = InlineBox {
            index: index_pos,
            id: idx,
            width: size.width,
            height: size.height,
          };

          spans.push(ProcessedInlineSpan::Box {
            node: item,
            inline_box: inline_box.clone(),
          });

          builder.push_inline_box(inline_box);
          idx += 1;
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

  let text_wrap_style = style
    .parent
    .text_wrap_style
    .unwrap_or(style.parent.text_wrap.style);
  let line_count = layout.lines().count();

  if text_wrap_style == TextWrapStyle::Balance {
    make_balanced_text(&mut layout, max_width, line_count);
  }

  if text_wrap_style == TextWrapStyle::Pretty {
    make_pretty_text(&mut layout, max_width);
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

/// Truncates text and inline boxes in the layout and appends an ellipsis character.
/// This function handles both text spans with their individual styles and inline boxes.
fn make_ellipsis_layout<'c, 'g: 'c, N: Node<N> + 'c>(
  layout: &mut InlineLayout,
  spans: &mut Vec<ProcessedInlineSpan<'c, 'g, N>>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  root_style: &'c SizedFontStyle,
  global: &GlobalContext,
) {
  loop {
    let (mut new_layout, text) = global
      .font_context
      .tree_builder(root_style.into(), |builder| {
        for span in spans.iter() {
          match span {
            ProcessedInlineSpan::Text { text, style } => {
              builder.push_style_span(style.into());
              builder.push_text(text);
              builder.pop_style_span();
            }
            ProcessedInlineSpan::Box { inline_box, .. } => {
              builder.push_inline_box(inline_box.clone());
            }
          }
        }

        builder.push_text(root_style.parent.ellipsis_char());
      });

    break_lines(&mut new_layout, max_width, max_height);

    // If there are no spans, return the new layout
    if spans.is_empty() {
      *layout = new_layout;
      return;
    }

    // Check if all content (including ellipsis) is visible
    if let Some(last_line) = new_layout.lines().last()
      && last_line.text_range().end == text.len()
    {
      *layout = new_layout;
      return;
    }

    // Try to truncate from the last span
    let Some(last_span) = spans.last_mut() else {
      *layout = new_layout;
      return;
    };

    match last_span {
      ProcessedInlineSpan::Box { .. } => {
        // Remove the last inline box if it overflows
        spans.pop();
      }
      ProcessedInlineSpan::Text { text, .. } => {
        if let Some((char_idx, _)) = text.char_indices().next_back() {
          text.truncate(char_idx);
        } else {
          // Text span is empty, remove it
          spans.pop();
        }
      }
    }
  }
}

pub(crate) struct InlineItemIterator<'n, 'g, N: Node<N>> {
  pub(crate) stack: Vec<(&'n NodeTree<'g, N>, usize)>, // (node, depth)
  pub(crate) current_node_content: Option<InlineItem<'n, 'g, N>>,
}

impl<'n, 'g, N: Node<N>> Iterator for InlineItemIterator<'n, 'g, N> {
  type Item = InlineItem<'n, 'g, N>;

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if let Some(content) = self.current_node_content.take() {
        return Some(content);
      }

      let (node, depth) = self.stack.pop()?;

      if let Some(children) = &node.children {
        for child in children.iter().rev() {
          self.stack.push((child, depth + 1));
        }
      }

      if let Some(inline_content) = node.node.as_ref().and_then(Node::inline_content) {
        match inline_content {
          InlineContentKind::Box => {
            if let Some(n) = &node.node {
              self.current_node_content = Some(InlineItem::Node(InlineNodeItem {
                node: n,
                context: &node.context,
              }));
            }
          }
          InlineContentKind::Text(text) => {
            self.current_node_content = Some(InlineItem::Text {
              text,
              context: &node.context,
            });
          }
        }
      }
    }
  }
}

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::{
  layout::{
    inline::{InlineBrush, InlineContentKind, break_lines, create_inline_constraint},
    node::Node,
    style::{InheritedStyle, SizedFontStyle, Style, TextOverflow, tw::TailwindProperties},
  },
  rendering::{
    Canvas, MaxHeight, RenderContext, apply_text_transform, apply_white_space_collapse,
    inline_drawing::draw_inline_layout, make_ellipsis_text,
  },
};

/// A node that renders text content.
///
/// Text nodes display text with configurable font properties,
/// alignment, and styling options.
#[derive(Debug, Clone, Deserialize)]
pub struct TextNode {
  /// The styling properties for this text node
  pub style: Option<Style>,
  /// The text content to be rendered
  pub text: String,
  /// The tailwind properties for this text node
  pub tw: Option<TailwindProperties>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for TextNode {
  fn get_tailwind_properties(&self) -> Option<&TailwindProperties> {
    self.tw.as_ref()
  }

  fn create_inherited_style(&mut self, parent_style: &InheritedStyle) -> InheritedStyle {
    let mut style = self.style.take().unwrap_or_default();

    if let Some(tw) = self.tw.as_ref() {
      tw.apply(&mut style);
    }

    style.inherit(parent_style)
  }

  fn inline_content(&self, context: &RenderContext) -> Option<InlineContentKind> {
    let transformed = apply_text_transform(&self.text, context.style.text_transform);
    let collapsed = apply_white_space_collapse(
      &transformed,
      context.style.white_space().white_space_collapse,
    );

    Some(InlineContentKind::Text(collapsed.into_owned()))
  }

  fn draw_content(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    let font_style = context.style.to_sized_font_style(context);
    let size = layout.content_box_size();

    if font_style.font_size == 0.0 {
      return;
    }

    let max_height = match font_style.parent.line_clamp.as_ref() {
      Some(clamp) => Some(MaxHeight::HeightAndLines(size.height, clamp.count)),
      None => Some(MaxHeight::Absolute(size.height)),
    };

    let inline_layout = create_text_only_layout(
      &self.text,
      context,
      size.width,
      max_height,
      &font_style,
      false,
    );

    draw_inline_layout(context, canvas, layout, inline_layout, &font_style);
  }

  fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    _style: &taffy::Style,
  ) -> Size<f32> {
    let (max_width, max_height) =
      create_inline_constraint(context, available_space, known_dimensions);

    let font_style = context.style.to_sized_font_style(context);

    let layout = create_text_only_layout(
      &self.text,
      context,
      max_width,
      max_height,
      &font_style,
      true,
    );

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

    taffy::Size {
      width: max_run_width.ceil().min(max_width),
      height: total_height.ceil(),
    }
  }

  fn get_style(&self) -> Option<&Style> {
    self.style.as_ref()
  }
}

fn create_text_only_layout(
  text: &str,
  context: &RenderContext,
  max_width: f32,
  max_height: Option<MaxHeight>,
  font_style: &SizedFontStyle<'_>,
  measure_only: bool,
) -> parley::Layout<InlineBrush> {
  let (mut inline_layout, text) = {
    let transformed = apply_text_transform(text, context.style.text_transform);
    let collapsed = apply_white_space_collapse(
      &transformed,
      font_style.parent.white_space().white_space_collapse,
    );

    context
      .global
      .font_context
      .tree_builder(font_style.into(), |builder| {
        builder.push_text(&collapsed);
      })
  };

  break_lines(
    &mut inline_layout,
    max_width,
    max_height,
    font_style.parent.white_space().text_wrap_mode,
  );

  if measure_only {
    return inline_layout;
  }

  let Some(last_line) = inline_layout.lines().last() else {
    return inline_layout;
  };

  let should_handle_ellipsis = font_style.parent.text_overflow == TextOverflow::Ellipsis
    && last_line.text_range().end < text.len();

  if should_handle_ellipsis {
    let truncated = make_ellipsis_text(
      &text,
      last_line.text_range(),
      font_style,
      context.global,
      max_width,
      font_style.ellipsis_char(),
    );

    return create_text_only_layout(
      &truncated, context, max_width, max_height, font_style, false,
    );
  }

  inline_layout.align(
    Some(max_width),
    context.style.text_align.into(),
    Default::default(),
  );

  inline_layout
}

use taffy::{AvailableSpace, Layout, Size};

use crate::{
  Result,
  layout::{
    inline::{
      InlineContentKind, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions,
      create_inline_constraint, create_inline_layout, get_parent_font_metrics,
      measure_inline_layout, resolve_inline_max_height,
    },
    node::TextData,
  },
  rendering::{
    Canvas, RenderContext,
    inline_drawing::{InlineLayoutDrawData, draw_inline_layout},
  },
};

pub(crate) fn text_inline_content(text: &TextData) -> Option<InlineContentKind<'_>> {
  Some(InlineContentKind::Text(text.text.as_str().into()))
}

pub(crate) fn draw_text_node_content(
  text: &TextData,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let font_style = context.style.to_sized_font_style(context);
  let size = layout.content_box_size();

  if font_style.sizing.font_size == 0.0 {
    return Ok(());
  }

  let max_height = resolve_inline_max_height(&font_style, size.height);

  let inline_text: InlineItem<'_, '_> = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
  };

  let built = create_inline_layout(InlineLayoutRequest {
    items: vec![inline_text],
    available_space: Size {
      width: AvailableSpace::Definite(size.width),
      height: AvailableSpace::Definite(size.height),
    },
    max_width: size.width,
    max_height,
    style: &font_style,
    global: context.global,
    mode: InlineLayoutMode::Draw,
  });

  draw_inline_layout(
    context,
    canvas,
    layout,
    built.layout,
    &font_style,
    InlineLayoutDrawData {
      spans: &built.spans,
      custom_inline_boxes: &built.custom_inline_boxes,
      line_scales: &built.line_scales,
    },
  )?;

  Ok(())
}

pub(crate) fn measure_text_node(
  text: &TextData,
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
) -> Size<f32> {
  let inline_content: InlineItem<'_, '_> = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
  };

  let (max_width, max_height) =
    create_inline_constraint(context, available_space, known_dimensions);
  let font_style = context.style.to_sized_font_style(context);

  let mut built = create_inline_layout(InlineLayoutRequest {
    items: vec![inline_content],
    available_space,
    max_width,
    max_height,
    style: &font_style,
    global: context.global,
    mode: InlineLayoutMode::Measure,
  });

  let parent_font_metrics = get_parent_font_metrics(&built.layout);

  measure_inline_layout(
    &mut built.layout,
    &built.spans,
    &built.custom_inline_boxes,
    &built.line_scales,
    InlineMeasureOptions {
      max_width,
      ceil_width: true,
      parent_font_metrics,
    },
  )
}

use taffy::{AvailableSpace, Size};

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  layout::{
    inline::{
      InlineContentKind, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions,
      create_inline_constraint, create_inline_layout, get_parent_font_metrics,
      measure_inline_layout,
    },
    node::TextData,
  },
};

pub fn text_inline_content(text: &TextData) -> Option<InlineContentKind<'_>> {
  Some(InlineContentKind::Text(text.text.as_str().into()))
}

pub fn measure_text_node(
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
  let font_style = SizedFontStyle::from_style(&context.style, context);

  let mut built = create_inline_layout(InlineLayoutRequest {
    items: vec![inline_content],
    available_space,
    max_width,
    max_height,
    style: &font_style,
    font_context: context.font_context,
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

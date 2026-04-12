use std::iter::once;

use taffy::{AvailableSpace, Layout, Size};

use crate::{
  Result,
  layout::{
    inline::{
      InlineContentKind, InlineItem, InlineLayoutStage, create_inline_constraint,
      create_inline_layout, measure_inline_layout,
    },
    node::TextData,
    style::TextOverflow,
  },
  rendering::{Canvas, MaxHeight, RenderContext, inline_drawing::draw_inline_layout},
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

  let resolved_line_clamp = font_style.parent.text_wrap_mode_and_line_clamp().1;
  let max_height = resolved_line_clamp
    .as_ref()
    .map(|clamp| MaxHeight::HeightAndLines(size.height, clamp.count))
    .or_else(|| {
      (font_style.parent.text_overflow == TextOverflow::Ellipsis)
        .then_some(MaxHeight::Absolute(size.height))
    });

  let inline_text: InlineItem<'_, '_> = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
  };

  let (inline_layout, _, spans) = create_inline_layout(
    once(inline_text),
    Size {
      width: AvailableSpace::Definite(size.width),
      height: AvailableSpace::Definite(size.height),
    },
    size.width,
    max_height,
    &font_style,
    context.global,
    InlineLayoutStage::Draw,
  );

  draw_inline_layout(context, canvas, layout, inline_layout, &font_style, &spans)?;

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

  let (mut layout, _, _) = create_inline_layout(
    once(inline_content),
    available_space,
    max_width,
    max_height,
    &font_style,
    context.global,
    InlineLayoutStage::Measure,
  );

  measure_inline_layout(&mut layout, max_width, true)
}

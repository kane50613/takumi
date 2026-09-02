use std::hash::Hasher;

use xxhash_rust::xxh3::Xxh3;

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, Size},
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions,
      create_inline_constraint, create_inline_layout, measure_inline_layout,
    },
    node::TextData,
  },
  style::TextOverflow,
  text_processing::MaxHeight,
};

/// Hashes the inputs that affect measured size beyond shaping.
fn measure_cache_key(
  text: &TextData,
  context: &RenderContext,
  font_style: &SizedFontStyle<'_>,
  max_width: f32,
  max_height: Option<MaxHeight>,
  clamp_to_max_width: bool,
) -> (u64, u32) {
  let style = &context.style;
  let mut hasher = Xxh3::new();

  hasher.write(text.text.as_bytes());
  font_style.hash_shaping_inputs(&mut hasher);
  hasher.write_u32(max_width.to_bits());
  hasher.write_u8(u8::from(clamp_to_max_width));
  match max_height {
    None => hasher.write_u8(0),
    Some(MaxHeight::Absolute(height)) => {
      hasher.write_u8(1);
      hasher.write_u32(height.to_bits());
    }
    Some(MaxHeight::Lines(lines)) => {
      hasher.write_u8(2);
      hasher.write_u32(lines);
    }
    Some(MaxHeight::HeightAndLines(height, lines)) => {
      hasher.write_u8(3);
      hasher.write_u32(height.to_bits());
      hasher.write_u32(lines);
    }
  }
  hasher.write_u8(style.text_transform as u8);
  hasher.write_u8(style.white_space_collapse as u8);
  hasher.write_usize(style.tab_size.spaces());
  hasher.write_u8(style.text_wrap_mode as u8);
  hasher.write_u8(style.text_wrap_style as u8);
  hasher.write_u8(style.text_align as u8);
  match &style.text_overflow {
    TextOverflow::Clip => hasher.write_u8(0),
    TextOverflow::Ellipsis => hasher.write_u8(1),
    TextOverflow::Custom(marker) => {
      hasher.write_u8(2);
      hasher.write(marker.as_bytes());
    }
  }
  style.text_indent.amount.hash_bits(&mut hasher);
  hasher.write_u8(style.text_indent.each_line as u8);
  hasher.write_u8(style.text_indent.hanging as u8);
  hasher.write_u8(style.text_fit.mode as u8);
  hasher.write_u8(style.text_fit.target as u8);
  hasher.write_u32(style.text_fit.limit.unwrap_or(f32::NAN).to_bits());

  (hasher.finish(), text.text.len() as u32)
}

pub(crate) fn measure_text_node(
  text: &TextData,
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
) -> Size<f32> {
  let (max_width, max_height) =
    create_inline_constraint(context, available_space, known_dimensions);
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let clamp_to_max_width =
    !context.intrinsic_min_content || !matches!(available_space.width, AvailableSpace::MinContent);
  let key = measure_cache_key(
    text,
    context,
    &font_style,
    max_width,
    max_height,
    clamp_to_max_width,
  );

  if let Some(size) = context.measure_cache().borrow().get(&key) {
    return *size;
  }
  let inline_content: InlineItem<'_> = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
    link: None,
    decorations: None,
  };
  let mut built = create_inline_layout(InlineLayoutRequest {
    items: vec![inline_content],
    available_space,
    max_width,
    max_height,
    style: &font_style,
    context,
    mode: InlineLayoutMode::Measure,
    shape_cacheable: true,
  });
  let parent_font_metrics = built.parent_font_metrics();
  let size = measure_inline_layout(
    &mut built.layout,
    &built.spans,
    &built.positioned_floats,
    &built.line_scales,
    InlineMeasureOptions {
      max_width,
      ceil_width: true,
      parent_font_metrics,
      clamp_to_max_width,
    },
  );

  context.measure_cache().borrow_mut().insert(key, size);
  size
}

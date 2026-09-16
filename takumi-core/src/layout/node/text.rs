use std::hash::Hasher;

use xxhash_rust::xxh3::Xxh3;

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, Size},
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineMeasureOptions, InlineMeasurement,
      create_inline_constraint, create_inline_layout, measure_inline_layout,
    },
    node::TextData,
  },
  style::TextOverflow,
  text_processing::MaxHeight,
};

/// Hashes the style inputs that affect measured size beyond shaping.
fn style_measure_digest(context: &RenderContext) -> u64 {
  let style = &context.style;
  let mut hasher = Xxh3::new();

  SizedFontStyle::from_style(style, context).hash_shaping_inputs(&mut hasher);
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
  hasher.finish()
}

fn measure_cache_key(
  text: &TextData,
  context: &RenderContext,
  max_width: f32,
  max_height: Option<MaxHeight>,
  min_content_query: bool,
) -> (u64, u32) {
  let mut hasher = Xxh3::new();

  hasher.write_u64(context.text_measure_digest(|| style_measure_digest(context)));
  hasher.write(text.text.as_bytes());
  hasher.write_u32(max_width.to_bits());
  hasher.write_u8(u8::from(min_content_query));
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

  (hasher.finish(), text.text.len() as u32)
}

impl TextData {
  /// The size this text lays out at, given the space its container offers.
  pub(crate) fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> Size<f32> {
    self
      .measurement(context, available_space, known_dimensions)
      .size
  }

  /// The size and line baselines of this text, memoized per render on its inputs.
  pub(crate) fn measurement(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> InlineMeasurement {
    let (max_width, max_height) =
      create_inline_constraint(context, available_space, known_dimensions);
    let min_content_query = known_dimensions.width.is_none()
      && matches!(available_space.width, AvailableSpace::MinContent);
    let key = measure_cache_key(self, context, max_width, max_height, min_content_query);

    context.inline_cache().get_or_measure(key, || {
      let font_style = SizedFontStyle::from_style(&context.style, context);
      let inline_content: InlineItem<'_> = InlineItem::Text {
        text: self.text.as_str().into(),
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
      InlineMeasurement {
        clamped: built.clamped,
        ..measure_inline_layout(
          &mut built.layout,
          &built.spans,
          &built.positioned_floats,
          &built.line_scales,
          InlineMeasureOptions {
            max_width,
            ceil_width: true,
            parent_font_metrics,
            min_content_query,
          },
        )
      }
    })
  }
}

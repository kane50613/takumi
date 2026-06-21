use data_url::DataUrl;
use taffy::{AvailableSpace, CompactLength, MaybeResolve, Size};

use crate::{
  context::RenderContext,
  layout::{
    node::{ImageData, ImageSourceInput, Node, NodeStyleLayers},
    style::{Length, Style, StyleDeclaration},
  },
  resources::image::{ImageResourceError, ImageResult, ImageSource, is_svg_like},
};

pub(crate) fn image_resource_url(image: &ImageData) -> Option<&str> {
  match &image.src {
    ImageSourceInput::Url(src) if src.starts_with("https://") || src.starts_with("http://") => {
      Some(src.as_ref())
    }
    _ => None,
  }
}

pub(crate) fn take_image_style_layers(
  node: &mut Node,
  width: Option<f32>,
  height: Option<f32>,
) -> NodeStyleLayers {
  let mut preset = node.metadata.preset.take();
  if width.is_some() || height.is_some() {
    let preset_style = preset.get_or_insert_with(Style::default);
    if let Some(width) = width {
      preset_style.push(StyleDeclaration::width(Length::Px(width)), false);
    }
    if let Some(height) = height {
      preset_style.push(StyleDeclaration::height(Length::Px(height)), false);
    }
  }

  NodeStyleLayers {
    preset,
    author_tw: node.metadata.tw.take(),
    inline: node.metadata.style.take(),
    dir: node.metadata.dir.take(),
  }
}

pub(crate) fn measure_image_node(
  image: &ImageData,
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
  style: &taffy::Style,
) -> Size<f32> {
  let Ok(image_source) = image.src.resolve(context) else {
    return Size::zero();
  };

  let intrinsic_size = match &image_source {
    #[cfg(feature = "svg")]
    ImageSource::Svg(svg) => Size {
      width: svg.tree.size().width(),
      height: svg.tree.size().height(),
    },
    ImageSource::Gif(gif) => {
      let frame = gif.frame_at_time(context.time_ms);
      Size {
        width: frame.width() as f32,
        height: frame.height() as f32,
      }
    }
    ImageSource::Bitmap(bitmap) => Size {
      width: bitmap.width() as f32,
      height: bitmap.height() as f32,
    },
  };

  let intrinsic_aspect_ratio =
    (intrinsic_size.height != 0.0).then_some(intrinsic_size.width / intrinsic_size.height);
  let preferred_size = match (image.width, image.height) {
    (Some(width), Some(height)) => Size { width, height },
    (Some(width), None) => Size {
      width,
      height: intrinsic_aspect_ratio
        .map(|ratio| width / ratio)
        .unwrap_or(intrinsic_size.height),
    },
    (None, Some(height)) => Size {
      width: intrinsic_aspect_ratio
        .map(|ratio| height * ratio)
        .unwrap_or(intrinsic_size.width),
      height,
    },
    (None, None) => intrinsic_size,
  }
  .map(|value| value * context.sizing.viewport.device_pixel_ratio);

  let style_known_dimensions = Size {
    width: resolve_style_size_axis(style.size.width, available_space.width, context),
    height: resolve_style_size_axis(style.size.height, available_space.height, context),
  };

  if let Size {
    width: Some(width),
    height: Some(height),
  } = style_known_dimensions
  {
    return Size { width, height };
  }

  let known_dimensions = Size {
    width: style_known_dimensions.width.or(known_dimensions.width),
    height: style_known_dimensions.height.or(known_dimensions.height),
  };

  let known_dimensions = if style.size.width.is_auto()
    && style.size.height.is_auto()
    && known_dimensions.width.is_none()
    && known_dimensions.height.is_none()
    && matches!(
      available_space.height,
      AvailableSpace::MinContent | AvailableSpace::MaxContent
    ) {
    Size {
      width: available_space.width.into_option(),
      height: None,
    }
  } else {
    known_dimensions
  };

  let aspect_ratio = style.aspect_ratio.or_else(|| {
    (preferred_size.height != 0.0).then_some(preferred_size.width / preferred_size.height)
  });
  let known_dimensions = known_dimensions.maybe_apply_aspect_ratio(aspect_ratio);

  if let Size {
    width: Some(width),
    height: Some(height),
  } = known_dimensions
  {
    return Size { width, height };
  }

  preferred_size
}

fn resolve_style_size_axis(
  size: taffy::Dimension,
  available: AvailableSpace,
  context: &RenderContext,
) -> Option<f32> {
  match size.tag() {
    CompactLength::AUTO_TAG => None,
    CompactLength::LENGTH_TAG => Some(size.value()),
    CompactLength::PERCENT_TAG => available.into_option(),
    _ => size.maybe_resolve(available.into_option(), |val, basis| {
      context.sizing.calc_arena.resolve_calc_value(val, basis)
    }),
  }
}

const DATA_URI_PREFIX: &str = "data:";

fn parse_data_uri_image(src: &str) -> ImageResult {
  let url = DataUrl::process(src).map_err(|_| ImageResourceError::InvalidDataUriFormat)?;
  let (data, _) = url
    .decode_to_vec()
    .map_err(|_| ImageResourceError::InvalidDataUriFormat)?;

  ImageSource::from_bytes(&data)
}

pub fn resolve_image(src: &str, context: &RenderContext) -> ImageResult {
  if src.starts_with(DATA_URI_PREFIX) {
    return parse_data_uri_image(src);
  }

  if is_svg_like(src) {
    #[cfg(feature = "svg")]
    return ImageSource::from_bytes(src.as_bytes());
    #[cfg(not(feature = "svg"))]
    return Err(ImageResourceError::SvgParseNotSupported);
  }

  if let Some(img) = context.images.get(src) {
    return Ok(img.clone());
  }

  Err(ImageResourceError::Unknown)
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use image::RgbaImage;
  use serde_json::from_value;
  use taffy::{AvailableSpace, Dimension, Size, Style};
  use takumi_css::SizingContext;

  use super::{image_resource_url, measure_image_node};
  use crate::{
    Fonts,
    context::RenderContext,
    layout::{
      Viewport,
      node::{ImageData, ImageSourceInput},
    },
    resources::image::ImageSource,
  };

  #[test]
  fn deserialize_image_src_from_string() -> std::result::Result<(), serde_json::Error> {
    let image: ImageData = from_value(serde_json::json!({
      "src": "https://example.com/image.png"
    }))?;

    assert_matches!(image.src, ImageSourceInput::Url(_));
    let src = match image.src {
      ImageSourceInput::Url(src) => src,
      _ => return Ok(()),
    };

    assert_eq!(src.as_ref(), "https://example.com/image.png");
    assert_eq!(
      image_resource_url(&ImageData {
        src: ImageSourceInput::Url(src),
        width: None,
        height: None
      }),
      Some("https://example.com/image.png")
    );

    Ok(())
  }

  #[test]
  fn deserialize_image_src_from_buffer_source() -> std::result::Result<(), serde_json::Error> {
    let image: ImageData = from_value(serde_json::json!({
      "src": [137, 80, 78, 71]
    }))?;

    assert_matches!(image.src, ImageSourceInput::Buffer(_));
    let data = match image.src {
      ImageSourceInput::Buffer(data) => data,
      _ => return Ok(()),
    };

    assert_eq!(&data[..], [137, 80, 78, 71]);
    assert_eq!(
      image_resource_url(&ImageData {
        src: ImageSourceInput::Buffer(data),
        width: None,
        height: None
      }),
      None
    );

    Ok(())
  }

  #[test]
  fn deserialize_image_src_from_bytes_value() -> std::result::Result<(), serde::de::value::Error> {
    use serde::{
      Deserialize,
      de::{Deserializer, Visitor, value::Error},
      forward_to_deserialize_any,
    };

    // Mirror how napi / wasm surface a `Uint8Array`/`ArrayBuffer`: a bytes value
    // via `deserialize_any`, not a JSON-style number array.
    struct BytesValue<'a>(&'a [u8]);

    impl<'de> Deserializer<'de> for BytesValue<'_> {
      type Error = Error;

      fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_bytes(self.0)
      }

      forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
      }
    }

    // PNG signature: invalid UTF-8, so it can't be captured as a URL string.
    let src = ImageSourceInput::deserialize(BytesValue(&[0x89, 0x50, 0x4e, 0x47]))?;

    assert_matches!(src, ImageSourceInput::Buffer(_));
    let data = match src {
      ImageSourceInput::Buffer(data) => data,
      _ => return Ok(()),
    };
    assert_eq!(&data[..], [0x89, 0x50, 0x4e, 0x47]);
    Ok(())
  }

  #[test]
  fn from_pixmap_creates_loaded_image_source_input() {
    let bitmap = RgbaImage::new(2, 2);
    let image = ImageData::from(bitmap);

    assert_matches!(image.src, ImageSourceInput::Loaded(ImageSource::Bitmap(_)));
  }

  #[test]
  fn fixed_style_size_uses_declared_lengths_instead_of_available_space() {
    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();
    let image = ImageData::from(ImageSource::from(RgbaImage::new(10, 10)));
    let style = Style {
      size: Size {
        width: Dimension::length(42.0),
        height: Dimension::length(28.0),
      },
      ..Style::default()
    };

    let measured = measure_image_node(
      &image,
      &context,
      Size {
        width: AvailableSpace::Definite(480.0),
        height: AvailableSpace::Definite(320.0),
      },
      Size::NONE,
      &style,
    );

    assert_eq!(
      measured,
      Size {
        width: 42.0,
        height: 28.0,
      }
    );
  }
}

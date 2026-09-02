use std::sync::Weak;

use taffy::{CompactLength, MaybeResolve};

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, Size},
  layout::node::{ImageData, ImageSourceInput, Node, NodeStyleLayers},
  resources::image::{ImageError, ImageResult, ImageSource, decode_data_uri, is_svg_like},
  style::{Length, Style, StyleDeclaration},
};

pub(crate) fn image_url(image: &ImageData) -> Option<&str> {
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
    lang: node.metadata.lang.take(),
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
    return Size::ZERO;
  };

  let intrinsic_sizing = image_source.intrinsic_sizing();
  const DEFAULT_WIDTH: f32 = 300.0;
  const DEFAULT_HEIGHT: f32 = 150.0;

  let intrinsic_size = match (intrinsic_sizing.width, intrinsic_sizing.height) {
    (Some(width), Some(height)) => Size { width, height },
    (Some(width), None) => {
      let height = match intrinsic_sizing.ratio {
        Some(ratio) if ratio > 0.0 => width / ratio,
        _ => DEFAULT_HEIGHT,
      };
      Size { width, height }
    }
    (None, Some(height)) => {
      let width = match intrinsic_sizing.ratio {
        Some(ratio) if ratio > 0.0 => height * ratio,
        _ => DEFAULT_WIDTH,
      };
      Size { width, height }
    }
    (None, None) => match intrinsic_sizing.ratio {
      Some(ratio) if ratio > 0.0 => {
        let solution_width = DEFAULT_HEIGHT * ratio;
        if solution_width <= DEFAULT_WIDTH {
          Size {
            width: solution_width,
            height: DEFAULT_HEIGHT,
          }
        } else {
          Size {
            width: DEFAULT_WIDTH,
            height: DEFAULT_WIDTH / ratio,
          }
        }
      }
      _ => Size {
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
      },
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
  .map(|value| context.sizing.to_device(value));

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
  let known_dimensions = known_dimensions.fill_missing_axis_from_aspect_ratio(aspect_ratio);

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
      context.sizing.resolve_calc(val, basis)
    }),
  }
}

const DATA_URI_PREFIX: &str = "data:";

fn parse_data_uri_image(src: &str) -> ImageResult {
  let decoded = decode_data_uri(src).map_err(|_| ImageError::InvalidDataUriFormat)?;

  ImageSource::from_bytes_lazy(&decoded.bytes, 0, Weak::new())
}

/// Resolve an image source string (data URI, SVG, or registered URL) to its bytes.
pub fn resolve_image(src: &str, context: &RenderContext) -> ImageResult {
  if src.starts_with(DATA_URI_PREFIX) {
    return parse_data_uri_image(src);
  }

  if is_svg_like(src) {
    #[cfg(feature = "svg")]
    return ImageSource::from_bytes(src.as_bytes());
    #[cfg(not(feature = "svg"))]
    return Err(ImageError::SvgParseNotSupported);
  }

  if let Some(img) = context.images().get(src) {
    return Ok(img.clone());
  }

  Err(ImageError::Unknown)
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use image::RgbaImage;
  use serde_json::from_value;
  use taffy::{Dimension, Size as TaffySize, Style};

  #[cfg(feature = "svg")]
  use super::parse_data_uri_image;
  use super::{image_url, measure_image_node};
  use crate::{
    Fonts,
    context::RenderContext,
    geometry::{AvailableSpace, Size},
    layout::node::{ImageData, ImageSourceInput},
    resources::{image::ImageSource, image_buffer::ImageBuffer},
    style::SizingContext,
    viewport::Viewport,
  };

  #[cfg(feature = "svg")]
  #[test]
  fn parse_data_uri_svg_with_unescaped_hash() {
    let source = parse_data_uri_image(
      "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10' fill='#f00'/></svg>",
    )
    .unwrap();

    assert_matches!(source, ImageSource::Svg(_));
  }

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
      image_url(&ImageData {
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
      image_url(&ImageData {
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
    let buffer = ImageBuffer::from_rgba_bytes(bitmap.into_raw(), 2, 2).unwrap();
    let image = ImageData::from(buffer);

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
    let buffer = ImageBuffer::from_rgba_bytes(RgbaImage::new(10, 10).into_raw(), 10, 10).unwrap();
    let image = ImageData::from(ImageSource::from(buffer));
    let style = Style {
      size: TaffySize {
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

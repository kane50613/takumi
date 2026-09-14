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

/// What a replaced element's source states about its own size, per css-images-3 5.
struct NaturalSize {
  /// The size the element takes when CSS leaves both axes `auto`, with the default object
  /// size standing in for an axis the source leaves open.
  size: Size<f32>,
  /// The ratio the source states, either outright or through both of its own dimensions.
  /// A default object dimension standing in for a missing axis states none.
  ratio: Option<f32>,
  /// Set when the source states no size of its own and carries only an aspect ratio. css
  /// 2.1 10.3.2 leaves that case to the containing block rather than to the source.
  from_ratio_only: bool,
}

impl ImageData {
  fn natural(&self, context: &RenderContext) -> Option<NaturalSize> {
    let image_source = self.src.resolve(context).ok()?;
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

    // css-images-3 4: a default object dimension standing in for an axis the source leaves
    // open states no ratio, so only the source's own ratio or its own two dimensions give
    // one. Without a ratio the other axis keeps the default object dimension.
    let source_ratio = intrinsic_sizing
      .ratio
      .filter(|ratio| *ratio > 0.0)
      .or_else(|| match (intrinsic_sizing.width, intrinsic_sizing.height) {
        (Some(width), Some(height)) if height != 0.0 => Some(width / height),
        _ => None,
      });
    let preferred_size = match (self.width, self.height) {
      (Some(width), Some(height)) => Size { width, height },
      (Some(width), None) => Size {
        width,
        height: source_ratio
          .map(|ratio| width / ratio)
          .unwrap_or(intrinsic_size.height),
      },
      (None, Some(height)) => Size {
        width: source_ratio
          .map(|ratio| height * ratio)
          .unwrap_or(intrinsic_size.width),
        height,
      },
      (None, None) => intrinsic_size,
    }
    .map(|value| context.sizing.to_device(value));

    Some(NaturalSize {
      size: preferred_size,
      ratio: source_ratio,
      // Blink's `ComputeNormalizedNaturalSize`: the default object size stands in whenever
      // the source carries no ratio, so only a ratio-carrying source is left without one.
      from_ratio_only: intrinsic_sizing.width.is_none()
        && intrinsic_sizing.height.is_none()
        && self.width.is_none()
        && self.height.is_none()
        && intrinsic_sizing.ratio.is_some_and(|ratio| ratio > 0.0),
    })
  }

  /// The size this element lays out at, given the space its container offers.
  pub(crate) fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &taffy::Style,
  ) -> Size<f32> {
    let Some(natural) = self.natural(context) else {
      return Size::ZERO;
    };

    // taffy reads a measure in content dimensions, so a style that states a border box
    // carries the insets inside the size it gives and they come off here.
    let insets = match style.box_sizing {
      taffy::BoxSizing::ContentBox => Size {
        width: 0.0,
        height: 0.0,
      },
      taffy::BoxSizing::BorderBox => {
        let basis = available_space.width.into_option();
        let resolve = |value| resolve_inset(value, basis, context);

        Size {
          width: resolve(style.padding.left)
            + resolve(style.padding.right)
            + resolve(style.border.left)
            + resolve(style.border.right),
          height: resolve(style.padding.top)
            + resolve(style.padding.bottom)
            + resolve(style.border.top)
            + resolve(style.border.bottom),
        }
      }
    };
    let style_known_dimensions = Size {
      width: resolve_style_size_axis(style.size.width, available_space.width, context)
        .map(|width| (width - insets.width).max(0.0)),
      height: resolve_style_size_axis(style.size.height, available_space.height, context)
        .map(|height| (height - insets.height).max(0.0)),
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

    // css 2.1 10.3.2 gives a replaced element with no `width` or `height` its natural size,
    // whatever box type it generated. A source carrying only an aspect ratio has none, and
    // Blink fills the offered width there instead.
    let known_dimensions = if natural.from_ratio_only
      && style.size.width.is_auto()
      && style.size.height.is_auto()
      && known_dimensions.width.is_none()
      && known_dimensions.height.is_none()
    {
      Size {
        width: available_space.width.into_option(),
        height: None,
      }
    } else {
      known_dimensions
    };

    let aspect_ratio = style.aspect_ratio.or(natural.ratio);
    let known_dimensions = known_dimensions.fill_missing_axis_from_aspect_ratio(aspect_ratio);

    // Without a ratio the axes are independent: an axis CSS states keeps its value and the
    // other one keeps the default object dimension.
    Size {
      width: known_dimensions.width.unwrap_or(natural.size.width),
      height: known_dimensions.height.unwrap_or(natural.size.height),
    }
  }
}

/// A padding or border length in device pixels, with a percentage resolved against the
/// containing block and an unresolvable one counted as zero.
fn resolve_inset(
  value: taffy::LengthPercentage,
  basis: Option<f32>,
  context: &RenderContext,
) -> f32 {
  value
    .maybe_resolve(basis, |val, basis| context.sizing.resolve_calc(val, basis))
    .unwrap_or_default()
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

  use super::image_url;
  #[cfg(feature = "svg")]
  use super::parse_data_uri_image;
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

    let measured = image.measure(
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

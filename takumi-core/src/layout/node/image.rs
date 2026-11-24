use std::sync::Arc;

use crate::{
  context::RenderContext,
  layout::{
    length_utils::NaturalSize,
    node::{ImageData, ImageSourceInput, Node, NodeKind, html_dimension::parse_dimension_value},
  },
  layout_unit::LayoutUnit,
  resources::image::{ImageCacheMode, ImageError, ImageResult, decode_data_uri, is_svg_like},
  style::{FromCssStr, Length, Style, StyleDeclaration},
};

/// Whether `src` names an image the render looks up in its `images`.
pub(crate) fn is_image_reference(src: &str) -> bool {
  !src.is_empty() && !src.starts_with(DATA_URI_PREFIX) && !is_svg_like(src)
}

impl ImageData {
  /// The source URL when the render looks it up in its `images`.
  pub(crate) fn url(&self) -> Option<&str> {
    match &self.src {
      ImageSourceInput::Url(src) if is_image_reference(src) => Some(src.as_ref()),
      _ => None,
    }
  }

  /// The natural size and ratio its source states, per css-images-3 5, in device pixels. A source
  /// that fails to load has a natural size of zero.
  pub(crate) fn natural_size(&self, context: &RenderContext) -> NaturalSize {
    let Ok(image_source) = self.src.resolve(context) else {
      return NaturalSize {
        inline_size: Some(LayoutUnit::ZERO),
        block_size: Some(LayoutUnit::ZERO),
        ..NaturalSize::default()
      };
    };
    let intrinsic_sizing = image_source.intrinsic_sizing();
    let to_device = |size: f32| LayoutUnit::from_float(context.sizing.to_device(size));

    NaturalSize {
      inline_size: intrinsic_sizing.width.map(to_device),
      block_size: intrinsic_sizing.height.map(to_device),
      aspect_ratio: intrinsic_sizing
        .ratio()
        .filter(|ratio| *ratio > 0.0)
        .or_else(|| match (intrinsic_sizing.width, intrinsic_sizing.height) {
          (Some(width), Some(height)) if height != 0.0 => Some(width / height),
          _ => None,
        }),
      ..NaturalSize::default()
    }
  }
}

impl Node {
  /// Pushes the image's `width` and `height` into its preset, its pixels before its attributes.
  pub(super) fn push_image_size_preset(&self, preset: &mut Option<Arc<Style>>) {
    let NodeKind::Image(image) = &self.kind else {
      return;
    };

    for (name, pixels, declaration) in [
      (
        "width",
        image.width,
        StyleDeclaration::width as fn(Length) -> StyleDeclaration,
      ),
      ("height", image.height, StyleDeclaration::height),
    ] {
      let length = pixels.map(Length::Px).or_else(|| {
        self
          .tag_name()
          .zip(self.attribute(name))
          .and_then(|(tag, value)| size_attribute(tag, value))
      });

      if let Some(length) = length {
        Arc::make_mut(preset.get_or_insert_default()).push(declaration(length), false);
      }
    }
  }
}

/// The `width` or `height` attribute of an `<img>` or `<svg>` as Blink maps it to style: an
/// `<img>` takes HTML's rules for parsing dimension values, a number of pixels or a percentage,
/// and an outermost `<svg>` a CSS length, a number counting as pixels. Blink's
/// `HTMLImageElement::CollectStyleForPresentationAttribute` and
/// `SVGSVGElement::CollectStyleForPresentationAttribute`.
pub(crate) fn size_attribute(tag: &str, value: &str) -> Option<Length> {
  match tag {
    // Blink maps no negative width or height to style.
    "svg" => {
      let value = value.trim();

      Length::from_css_str(value)
        .ok()
        .or_else(|| {
          value
            .parse::<f32>()
            .ok()
            .filter(|number| number.is_finite())
            .map(Length::Px)
        })
        .filter(|length| !length.is_negative())
    }
    "img" => parse_dimension_value(value),
    _ => None,
  }
}

const DATA_URI_PREFIX: &str = "data:";

/// Resolve an image source string (data URI, SVG, or registered URL) to its bytes.
pub fn resolve_image(src: &str, context: &RenderContext) -> ImageResult {
  if src.starts_with(DATA_URI_PREFIX) {
    let decoded = decode_data_uri(src).map_err(|_| ImageError::InvalidDataUriFormat)?;

    return context
      .cache()
      .decode_image(&decoded.bytes, ImageCacheMode::Auto);
  }

  if is_svg_like(src) {
    #[cfg(feature = "svg-sizing")]
    return context
      .cache()
      .decode_image(src.as_bytes(), ImageCacheMode::Auto);
    #[cfg(not(feature = "svg-sizing"))]
    return Err(ImageError::SvgParseNotSupported);
  }

  if let Some(img) = context.images().get(src) {
    return Ok(img.clone());
  }

  log::debug!(target: "takumi::image", "no image is loaded for {src:?}");

  Err(ImageError::Unknown)
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use super::size_attribute;
  use crate::style::Length;

  #[test]
  fn size_attributes_map_as_blink_maps_them() {
    assert_eq!(
      size_attribute("img", "50.%"),
      Some(Length::Percentage(50.0))
    );
    assert_eq!(
      size_attribute("img", "50.5%"),
      Some(Length::Percentage(50.5))
    );
    assert_eq!(size_attribute("img", "24px"), Some(Length::Px(24.0)));
    assert_eq!(size_attribute("svg", "2em"), Some(Length::Em(2.0)));
    assert_eq!(size_attribute("svg", "100"), Some(Length::Px(100.0)));
    assert_eq!(size_attribute("svg", "-2em"), None);
    assert_eq!(size_attribute("svg", "-20"), None);
    assert_eq!(size_attribute("svg", "-0%"), Some(Length::Percentage(-0.0)));
    assert_eq!(size_attribute("svg", "NaN"), None);
    assert_eq!(size_attribute("svg", "inf"), None);
  }

  use serde_json::from_value;

  #[cfg(any(feature = "svg", feature = "png"))]
  use super::resolve_image;
  #[cfg(any(feature = "svg", feature = "png"))]
  use crate::{
    Fonts, context::RenderContext, resources::cache::Cache, style::SizingContext,
    viewport::Viewport,
  };
  use crate::{
    layout::node::{ImageData, ImageSourceInput},
    resources::{image::ImageSource, image_buffer::ImageBuffer},
  };

  /// A context with an empty cache.
  #[cfg(any(feature = "svg", feature = "png"))]
  fn context() -> RenderContext {
    RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .cache(Cache::default())
      .build()
  }

  #[cfg(feature = "svg")]
  #[test]
  fn parse_data_uri_svg_with_unescaped_hash() {
    let source = resolve_image(
      "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10' fill='#f00'/></svg>",
      &context(),
    )
    .unwrap();

    assert_matches!(source, ImageSource::Svg(_));
  }

  #[cfg(feature = "png")]
  #[test]
  fn a_data_uri_decodes_once_per_cache() {
    use std::sync::Arc;

    use crate::resources::image::to_data_url;

    let png = ImageBuffer::from_rgba_bytes(vec![255; 4 * 4 * 4], 4, 4)
      .unwrap()
      .encode_png()
      .unwrap();

    let uri = to_data_url("image/png", &png);
    let context = context();
    let first = resolve_image(&uri, &context).unwrap();
    let second = resolve_image(&uri, &context).unwrap();

    assert_matches!(
      (first, second),
      (ImageSource::Encoded(first), ImageSource::Encoded(second)) if Arc::ptr_eq(&first, &second)
    );
  }

  #[cfg(feature = "svg")]
  #[test]
  fn inline_svg_parses_once_per_cache() {
    use std::sync::Arc;

    let context = context();
    let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10'/></svg>";
    let first = resolve_image(svg, &context).unwrap();
    let second = resolve_image(svg, &context).unwrap();

    assert_matches!(
      (first, second),
      (ImageSource::Svg(first), ImageSource::Svg(second)) if Arc::ptr_eq(&first, &second)
    );
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
      ImageData {
        src: ImageSourceInput::Url(src),
        width: None,
        height: None
      }
      .url(),
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
      ImageData {
        src: ImageSourceInput::Buffer(data),
        width: None,
        height: None
      }
      .url(),
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
    let buffer = ImageBuffer::from_rgba_bytes(vec![0; 2 * 2 * 4], 2, 2).unwrap();
    let image = ImageData::from(buffer);

    assert_matches!(image.src, ImageSourceInput::Loaded(ImageSource::Bitmap(_)));
  }
}

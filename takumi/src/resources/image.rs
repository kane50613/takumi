//! Image resource management for the takumi rendering system.
//!
//! This module provides types and utilities for managing image resources,
//! including loading states, error handling, and image processing operations.

use std::{borrow::Cow, sync::Arc};

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::HashMap};

#[cfg(not(target_arch = "wasm32"))]
use dashmap::DashMap;
use image::RgbaImage;

use super::image_decoder;
use crate::{
  layout::style::{Color, ImageScalingAlgorithm},
  rendering::{fast_resize, unpremultiply_alpha},
};
use thiserror::Error;

/// Represents the state of an image resource.
pub type ImageResult = Result<Arc<ImageSource>, ImageResourceError>;

#[derive(Debug, Clone)]
/// Represents the source of an image.
#[non_exhaustive]
pub enum ImageSource {
  /// An svg image source
  #[cfg(feature = "svg")]
  Svg {
    /// Original SVG source used for reparsing with style overrides.
    source: Arc<str>,
    /// Parsed SVG tree used for size and initial metadata.
    tree: Box<resvg::usvg::Tree>,
  },
  /// A bitmap image source
  Bitmap(RgbaImage),
}

/// Represents a persistent image store.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct PersistentImageStore {
  #[cfg(target_arch = "wasm32")]
  map: RefCell<HashMap<String, Arc<ImageSource>>>,
  #[cfg(not(target_arch = "wasm32"))]
  map: DashMap<String, Arc<ImageSource>>,
}

impl PersistentImageStore {
  /// Returns the stored image for the provided source, if present.
  pub fn get(&self, src: &str) -> Option<Arc<ImageSource>> {
    #[cfg(target_arch = "wasm32")]
    {
      self.map.borrow().get(src).cloned()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
      self.map.get(src).map(|image| image.clone())
    }
  }

  /// Stores or replaces a persistent image for the provided source.
  pub fn insert(&self, src: String, image: Arc<ImageSource>) {
    #[cfg(target_arch = "wasm32")]
    {
      self.map.borrow_mut().insert(src, image);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
      self.map.insert(src, image);
    }
  }

  /// Removes all stored persistent images.
  pub fn clear(&self) {
    #[cfg(target_arch = "wasm32")]
    {
      self.map.borrow_mut().clear();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
      self.map.clear();
    }
  }
}

impl From<RgbaImage> for ImageSource {
  fn from(bitmap: RgbaImage) -> Self {
    ImageSource::Bitmap(bitmap)
  }
}

impl ImageSource {
  /// Load an image source from raw bytes.
  ///
  /// - When the `svg` feature is enabled and the bytes look like SVG XML, they
  ///   are parsed as an SVG using `resvg::usvg`.
  /// - Otherwise, the bytes are decoded as a raster image.
  pub fn from_bytes(bytes: &[u8]) -> ImageResult {
    #[cfg(feature = "svg")]
    {
      use std::str::from_utf8;

      if let Ok(text) = from_utf8(bytes)
        && is_svg_like(text)
      {
        return parse_svg_str(text);
      }
    }

    let img = image_decoder::decode_image(bytes).map_err(ImageResourceError::DecodeError)?;
    Ok(Arc::new(img.into()))
  }

  /// Get the size of the image source.
  pub fn size(&self) -> (f32, f32) {
    match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg { tree, .. } => (tree.size().width(), tree.size().height()),
      ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
    }
  }

  /// Render the image source to an RGBA image with the specified dimensions.
  pub fn render_to_rgba_image<'i>(
    &'i self,
    width: u32,
    height: u32,
    image_rendering: ImageScalingAlgorithm,
    current_color: Color,
  ) -> Result<Cow<'i, RgbaImage>, ImageResourceError> {
    #[cfg(not(feature = "svg"))]
    let _ = current_color;

    match self {
      ImageSource::Bitmap(bitmap) => {
        if bitmap.width() == width && bitmap.height() == height {
          return Ok(Cow::Borrowed(bitmap));
        }

        Ok(Cow::Owned(fast_resize(
          bitmap,
          width,
          height,
          image_rendering,
        )?))
      }
      #[cfg(feature = "svg")]
      ImageSource::Svg { source, tree } => {
        use resvg::{
          tiny_skia::Pixmap,
          usvg::{Options, Transform, Tree},
        };

        let options = Options {
          style_sheet: Some(format!("svg {{ color: {current_color}; }}")),
          image_rendering: image_rendering.into(),
          ..Default::default()
        };
        let reparsed_tree =
          Tree::from_str(source, &options).map_err(ImageResourceError::SvgParseError)?;

        let mut pixmap = Pixmap::new(width, height).ok_or(ImageResourceError::InvalidPixmapSize)?;

        let original_size = tree.size();
        let sx = width as f32 / original_size.width();
        let sy = height as f32 / original_size.height();

        resvg::render(
          &reparsed_tree,
          Transform::from_scale(sx, sy),
          &mut pixmap.as_mut(),
        );

        let mut image = RgbaImage::from_raw(width, height, pixmap.take())
          .ok_or(ImageResourceError::MismatchedBufferSize)?;

        for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(image.as_mut()) {
          unpremultiply_alpha(pixel);
        }

        Ok(Cow::Owned(image))
      }
    }
  }
}

/// Check if the string looks like an SVG image.
pub(crate) fn is_svg_like(src: &str) -> bool {
  src.contains("<svg") && src.contains("xmlns")
}

#[cfg(feature = "svg")]
fn strip_unsupported_svg_text_nodes(src: &str) -> String {
  use std::ops::Range;

  use roxmltree::{Document, Node};

  fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| (range.start, range.end));

    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
      if let Some(last) = merged.last_mut()
        && range.start <= last.end
      {
        last.end = last.end.max(range.end);
      } else {
        merged.push(range);
      }
    }

    merged
  }

  let Ok(document) = Document::parse(src) else {
    return src.to_owned();
  };

  let ranges = document
    .descendants()
    .filter(Node::is_element)
    .filter_map(|node| {
      let name = node.tag_name().name();
      if name == "text" || name == "tspan" {
        Some(node.range())
      } else {
        None
      }
    })
    .collect::<Vec<_>>();

  if ranges.is_empty() {
    return src.to_owned();
  }

  let merged_ranges = merge_ranges(ranges);
  let mut stripped = String::with_capacity(src.len());
  let mut cursor = 0;

  for range in merged_ranges {
    if range.start > cursor {
      stripped.push_str(&src[cursor..range.start]);
    }
    cursor = cursor.max(range.end);
  }

  if cursor < src.len() {
    stripped.push_str(&src[cursor..]);
  }

  stripped
}

#[cfg(feature = "svg")]
/// Parse SVG from &str.
pub fn parse_svg_str(src: &str) -> ImageResult {
  use resvg::usvg::Tree;

  let sanitized_svg = strip_unsupported_svg_text_nodes(src);
  let tree = Tree::from_str(&sanitized_svg, &Default::default())
    .map_err(ImageResourceError::SvgParseError)?;

  Ok(Arc::new(ImageSource::Svg {
    source: Arc::from(sanitized_svg),
    tree: Box::new(tree),
  }))
}

/// Represents the state of an image in the rendering system.
///
/// This enum tracks whether an image has been successfully loaded and decoded,
/// or if there was an error during the process.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ImageResourceError {
  /// An error occurred while decoding the image data
  #[error("An error occurred while decoding the image data: {0}")]
  DecodeError(#[from] image::ImageError),
  /// The image data URI is in an invalid format
  #[error("The image data URI is in an invalid format")]
  InvalidDataUriFormat,
  /// The image data URI is malformed and cannot be parsed
  #[error("The image data URI is malformed and cannot be parsed")]
  MalformedDataUri,
  #[cfg(feature = "svg")]
  /// An error occurred while parsing an SVG image
  #[error("An error occurred while parsing an SVG image: {0}")]
  SvgParseError(#[from] resvg::usvg::Error),
  /// SVG parsing is not supported in this build
  #[cfg(not(feature = "svg"))]
  #[error("SVG parsing is not supported in this build")]
  SvgParseNotSupported,
  /// The image source is unknown
  #[error("The image source is unknown")]
  Unknown,
  /// The pixmap size is invalid
  #[error("The pixmap size is invalid")]
  InvalidPixmapSize,
  /// The buffer size does not match the target image size
  #[error("The buffer size does not match the target image size")]
  MismatchedBufferSize,
  /// An error occurred while resizing the image
  #[error("An error occurred while resizing the image: {0}")]
  ResizeError(#[from] fast_image_resize::ResizeError),
}

#[cfg(test)]
mod tests {
  use image::Rgba;

  use super::*;

  fn rgba_at(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_changes_output() -> Result<(), ImageResourceError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = parse_svg_str(svg)?;

    let red = image
      .render_to_rgba_image(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0xFF0000))?
      .into_owned();
    let blue = image
      .render_to_rgba_image(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0x0000FF))?
      .into_owned();

    assert_ne!(rgba_at(&red, 2, 2), rgba_at(&blue, 2, 2));
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_applies_alpha() -> Result<(), ImageResourceError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = parse_svg_str(svg)?;
    let color = Color([255, 0, 0, 128]);

    let rendered = image
      .render_to_rgba_image(4, 4, ImageScalingAlgorithm::Auto, color)?
      .into_owned();
    let alpha = rgba_at(&rendered, 2, 2)[3];

    assert!((alpha as i16 - 128).abs() <= 1);
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_fixed_fill_is_not_affected_by_current_color() -> Result<(), ImageResourceError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ff0000"/></svg>"##;
    let image = parse_svg_str(svg)?;

    let first = image
      .render_to_rgba_image(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0x00FF00))?
      .into_owned();
    let second = image
      .render_to_rgba_image(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0x0000FF))?
      .into_owned();

    assert_eq!(first.as_raw(), second.as_raw());
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn parse_svg_str_strips_text_and_tspan_nodes() -> Result<(), ImageResourceError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect x="0" y="0" width="20" height="20" fill="#ff0000"/><text x="2" y="10">hello <tspan>world</tspan></text><g><tspan>orphan</tspan></g></svg>"##;
    let image = parse_svg_str(svg)?;
    let ImageSource::Svg { source, .. } = image.as_ref() else {
      unreachable!()
    };

    assert!(source.contains("<rect"));
    assert!(!source.contains("<text"));
    assert!(!source.contains("<tspan"));
    Ok(())
  }

  #[test]
  fn bitmap_is_not_affected_by_current_color() -> Result<(), ImageResourceError> {
    let mut bitmap = RgbaImage::new(2, 2);
    bitmap.put_pixel(0, 0, Rgba([12, 34, 56, 200]));
    bitmap.put_pixel(1, 0, Rgba([78, 90, 12, 255]));
    let image = ImageSource::Bitmap(bitmap);

    let first = image
      .render_to_rgba_image(2, 2, ImageScalingAlgorithm::Auto, Color::from_rgb(0xFF0000))?
      .into_owned();
    let second = image
      .render_to_rgba_image(2, 2, ImageScalingAlgorithm::Auto, Color::from_rgb(0x0000FF))?
      .into_owned();

    assert_eq!(first.as_raw(), second.as_raw());
    Ok(())
  }

  #[test]
  fn bitmap_resize_smoke_for_scaling_algorithm() -> Result<(), ImageResourceError> {
    let mut bitmap = RgbaImage::new(2, 2);
    bitmap.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    bitmap.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
    bitmap.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
    bitmap.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
    let image = ImageSource::Bitmap(bitmap);

    let resized = image
      .render_to_rgba_image(4, 4, ImageScalingAlgorithm::Pixelated, Color::black())?
      .into_owned();

    assert_eq!(resized.width(), 4);
    assert_eq!(resized.height(), 4);
    Ok(())
  }
}

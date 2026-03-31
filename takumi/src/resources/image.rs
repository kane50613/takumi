//! Image resource management for the takumi rendering system.
//!
//! This module provides types and utilities for managing image resources,
//! including loading states, error handling, and image processing operations.

use std::{str::FromStr, sync::Arc};

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::HashMap};

#[cfg(not(target_arch = "wasm32"))]
use dashmap::DashMap;
use image::RgbaImage;
use tiny_skia::Pixmap;

use crate::{
  layout::style::{Color, ImageScalingAlgorithm},
  rendering::Sizing,
  resources::image_decoder::decode_image,
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
  Svg(SvgSource),
  /// A bitmap image source
  Bitmap(RgbaImage),
}

/// Represents the resolved SVG source.
#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgSource {
  /// Original SVG source used for reparsing with style overrides.
  source: Arc<str>,
  /// Parsed SVG tree used for size and initial metadata.
  pub(crate) tree: Box<resvg::usvg::Tree>,
}

impl From<SvgSource> for ImageSource {
  fn from(svg: SvgSource) -> Self {
    ImageSource::Svg(svg)
  }
}

/// Image data prepared for layout rendering.
#[derive(Debug, Clone)]
pub(crate) enum RenderedImage<'a> {
  /// A fully rasterized image, used for SVGs.
  Rasterized(Pixmap),
  /// A borrowed bitmap that should be sampled directly.
  Borrowed {
    /// The original bitmap source.
    source: &'a RgbaImage,
    /// The logical width that will be rendered on the canvas.
    width: u32,
    /// The logical height that will be rendered on the canvas.
    height: u32,
    /// The sampling algorithm to use.
    algorithm: ImageScalingAlgorithm,
  },
}

/// Represents a persistent image store.
#[derive(Debug, Default)]
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

#[cfg(feature = "svg")]
impl FromStr for SvgSource {
  type Err = ImageResourceError;

  fn from_str(src: &str) -> Result<Self, Self::Err> {
    use resvg::usvg::Tree;

    let sanitized_svg = strip_unsupported_svg_text_nodes(src);
    let tree = Tree::from_str(&sanitized_svg, &Default::default())
      .map_err(ImageResourceError::SvgParseError)?;

    Ok(SvgSource {
      source: Arc::from(sanitized_svg),
      tree: Box::new(tree),
    })
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
        return Ok(Arc::new(ImageSource::Svg(text.parse()?)));
      }
    }

    let img = decode_image(bytes).map_err(ImageResourceError::DecodeError)?;
    Ok(Arc::new(img.into()))
  }

  /// Prepare image data for layout rendering.
  ///
  /// Bitmap images are kept borrowed so the renderer can sample them directly.
  /// SVG images are rasterized to a bitmap first.
  pub(crate) fn render_for_layout<'i>(
    &'i self,
    width: u32,
    height: u32,
    image_rendering: ImageScalingAlgorithm,
    current_color: Color,
  ) -> Result<RenderedImage<'i>, ImageResourceError> {
    #[cfg(not(feature = "svg"))]
    let _ = current_color;

    match self {
      ImageSource::Bitmap(bitmap) => Ok(RenderedImage::Borrowed {
        source: bitmap,
        width,
        height,
        algorithm: image_rendering,
      }),
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => {
        use resvg::usvg::{Options, Transform, Tree};

        let options = Options {
          style_sheet: Some(format!("svg {{ color: {current_color}; }}")),
          image_rendering: image_rendering.into(),
          ..Default::default()
        };
        let reparsed_tree =
          Tree::from_str(&svg.source, &options).map_err(ImageResourceError::SvgParseError)?;

        let mut pixmap = Pixmap::new(width, height).ok_or(ImageResourceError::InvalidPixmapSize)?;

        let original_size = svg.tree.size();
        let sx = width as f32 / original_size.width();
        let sy = height as f32 / original_size.height();

        resvg::render(
          &reparsed_tree,
          Transform::from_scale(sx, sy),
          &mut pixmap.as_mut(),
        );

        Ok(RenderedImage::Rasterized(pixmap))
      }
    }
  }

  /// Get the image size in device pixels for the current sizing context.
  pub(crate) fn size(&self, sizing: &Sizing) -> (f32, f32) {
    let (width, height) = match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => (svg.tree.size().width(), svg.tree.size().height()),
      ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
    };

    let dpr = sizing.viewport.device_pixel_ratio;
    (width * dpr, height * dpr)
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
}

#[cfg(test)]
mod tests {
  use image::Rgba;
  use tiny_skia::PremultipliedColorU8;

  use super::*;

  fn premul_at(image: &RenderedImage<'_>, x: u32, y: u32) -> PremultipliedColorU8 {
    match image {
      RenderedImage::Rasterized(pixmap) => pixmap
        .pixel(x, y)
        .unwrap_or(PremultipliedColorU8::TRANSPARENT),
      RenderedImage::Borrowed { source, .. } => {
        let pixel = source.get_pixel(x, y).0;
        let alpha = pixel[3] as u32;
        PremultipliedColorU8::from_rgba(
          crate::rendering::fast_div_255(pixel[0] as u32 * alpha),
          crate::rendering::fast_div_255(pixel[1] as u32 * alpha),
          crate::rendering::fast_div_255(pixel[2] as u32 * alpha),
          pixel[3],
        )
        .unwrap_or_else(|| unreachable!())
      }
    }
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_changes_output() -> Result<(), ImageResourceError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let red =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0xFF0000))?;
    let blue =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0x0000FF))?;

    assert_ne!(premul_at(&red, 2, 2), premul_at(&blue, 2, 2));
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_applies_alpha() -> Result<(), ImageResourceError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;
    let color = Color([255, 0, 0, 128]);

    let rendered = image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, color)?;
    let alpha = premul_at(&rendered, 2, 2).alpha();

    assert!((alpha as i16 - 128).abs() <= 1);
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_fixed_fill_is_not_affected_by_current_color() -> Result<(), ImageResourceError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ff0000"/></svg>"##;
    let image: ImageSource = SvgSource::from_str(svg)?.into();

    let first =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0x00FF00))?;
    let second =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, Color::from_rgb(0x0000FF))?;

    let RenderedImage::Rasterized(first) = first else {
      unreachable!()
    };
    let RenderedImage::Rasterized(second) = second else {
      unreachable!()
    };
    assert_eq!(first.data(), second.data());
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn parse_svg_str_strips_text_and_tspan_nodes() -> Result<(), ImageResourceError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect x="0" y="0" width="20" height="20" fill="#ff0000"/><text x="2" y="10">hello <tspan>world</tspan></text><g><tspan>orphan</tspan></g></svg>"##;
    let image: ImageSource = SvgSource::from_str(svg)?.into();
    let ImageSource::Svg(svg) = image else {
      unreachable!()
    };

    assert!(svg.source.contains("<rect"));
    assert!(!svg.source.contains("<text"));
    assert!(!svg.source.contains("<tspan"));
    Ok(())
  }

  #[test]
  fn bitmap_is_not_affected_by_current_color() -> Result<(), ImageResourceError> {
    let mut bitmap = RgbaImage::new(2, 2);
    bitmap.put_pixel(0, 0, Rgba([12, 34, 56, 200]));
    bitmap.put_pixel(1, 0, Rgba([78, 90, 12, 255]));
    let image = ImageSource::Bitmap(bitmap);

    let first =
      image.render_for_layout(2, 2, ImageScalingAlgorithm::Auto, Color::from_rgb(0xFF0000))?;
    let second =
      image.render_for_layout(2, 2, ImageScalingAlgorithm::Auto, Color::from_rgb(0x0000FF))?;

    let RenderedImage::Borrowed { source: first, .. } = first else {
      unreachable!()
    };
    let RenderedImage::Borrowed { source: second, .. } = second else {
      unreachable!()
    };
    assert_eq!(first.as_raw(), second.as_raw());
    Ok(())
  }

  #[test]
  fn bitmap_render_for_layout_keeps_borrowed_sampling_parameters() -> Result<(), ImageResourceError>
  {
    let mut bitmap = RgbaImage::new(2, 2);
    bitmap.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    bitmap.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
    bitmap.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
    bitmap.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
    let image = ImageSource::Bitmap(bitmap);

    let rendered =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Pixelated, Color::black())?;
    let RenderedImage::Borrowed {
      width,
      height,
      algorithm: algo,
      ..
    } = rendered
    else {
      unreachable!()
    };

    assert_eq!(width, 4);
    assert_eq!(height, 4);
    assert!(matches!(algo, ImageScalingAlgorithm::Pixelated));
    Ok(())
  }
}

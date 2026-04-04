//! Image resource management for the takumi rendering system.
//!
//! This module provides types and utilities for managing image resources,
//! including loading states, error handling, and image processing operations.

use std::borrow::Cow;
use std::{str::FromStr, sync::Arc};

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::HashMap};

#[cfg(not(target_arch = "wasm32"))]
use dashmap::DashMap;
use image::RgbaImage;
use tiny_skia::Pixmap;

use crate::{
  layout::style::{Color, ImageScalingAlgorithm},
  rendering::{Sizing, premultiplied_pixmap_from_rgba},
  resources::image_decoder::{DecodedGif, DecodedImage, decode_image},
};
use thiserror::Error;

/// Represents the state of an image resource.
pub type ImageResult = Result<ImageSource, ImageResourceError>;

#[derive(Debug, Clone)]
/// Represents the source of an image.
#[non_exhaustive]
pub enum ImageSource {
  /// An svg image source
  #[cfg(feature = "svg")]
  Svg(SvgSource),
  /// A bitmap image source
  Bitmap(Arc<Pixmap>),
  /// An animated gif source.
  Gif(GifSource),
}

/// Represents the resolved SVG source.
#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgSource {
  /// Original SVG source used for reparsing with style overrides.
  source: Arc<str>,
  /// Whether the SVG depends on currentColor and must be reparsed per render.
  uses_current_color: bool,
  /// Parsed SVG tree used for size and initial metadata.
  pub(crate) tree: Arc<resvg::usvg::Tree>,
  raster_cache: Arc<SvgRasterCache>,
}

/// A decoded GIF image with frame timing metadata.
#[derive(Debug, Clone)]
pub struct GifSource {
  frames: Vec<GifFrame>,
  total_duration_ms: u64,
}

/// A single GIF frame.
#[derive(Debug, Clone)]
struct GifFrame {
  pixmap: Arc<Pixmap>,
  duration_ms: u32,
}

impl GifSource {
  fn from_decoded(decoded: DecodedGif) -> Result<Self, ImageResourceError> {
    if decoded.frames.is_empty() {
      return Err(ImageResourceError::InvalidGif);
    }

    let mut frames = Vec::with_capacity(decoded.frames.len());
    let mut total_duration_ms = 0_u64;
    for frame in decoded.frames {
      total_duration_ms = total_duration_ms.saturating_add(frame.duration_ms as u64);
      frames.push(GifFrame {
        pixmap: frame.pixmap,
        duration_ms: frame.duration_ms,
      });
    }

    Ok(Self {
      frames,
      total_duration_ms,
    })
  }

  pub(crate) fn frame_at_time(&self, time_ms: u64) -> &Pixmap {
    if self.total_duration_ms == 0 {
      return &self.frames[0].pixmap;
    }

    let target_time = time_ms % self.total_duration_ms;
    let mut elapsed_ms = 0_u64;

    for frame in &self.frames {
      elapsed_ms = elapsed_ms.saturating_add(frame.duration_ms as u64);
      if target_time < elapsed_ms {
        return &frame.pixmap;
      }
    }

    &self.frames[0].pixmap
  }

  pub(crate) fn frame_at_time_arc(&self, time_ms: u64) -> Arc<Pixmap> {
    if self.total_duration_ms == 0 {
      return self.frames[0].pixmap.clone();
    }

    let target_time = time_ms % self.total_duration_ms;
    let mut elapsed_ms = 0_u64;

    for frame in &self.frames {
      elapsed_ms = elapsed_ms.saturating_add(frame.duration_ms as u64);
      if target_time < elapsed_ms {
        return frame.pixmap.clone();
      }
    }

    self.frames[0].pixmap.clone()
  }
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
  Rasterized(Arc<Pixmap>),
  /// A borrowed bitmap that should be sampled directly.
  Borrowed {
    /// The original bitmap source.
    source: &'a Pixmap,
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
  map: RefCell<HashMap<String, ImageSource>>,
  #[cfg(not(target_arch = "wasm32"))]
  map: DashMap<String, ImageSource>,
}

impl PersistentImageStore {
  /// Returns the stored image for the provided source, if present.
  pub fn get(&self, src: &str) -> Option<ImageSource> {
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
  pub fn insert(&self, src: String, image: ImageSource) {
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
    let pixmap =
      premultiplied_pixmap_from_rgba(Cow::Owned(bitmap)).unwrap_or_else(|| unreachable!());
    ImageSource::Bitmap(Arc::new(pixmap))
  }
}

impl From<Pixmap> for ImageSource {
  fn from(pixmap: Pixmap) -> Self {
    ImageSource::Bitmap(Arc::new(pixmap))
  }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SvgRasterCacheKey {
  width: u32,
  height: u32,
  image_rendering: u8,
  current_color: u32,
}

#[cfg(feature = "svg")]
impl SvgRasterCacheKey {
  fn new(
    width: u32,
    height: u32,
    image_rendering: ImageScalingAlgorithm,
    current_color: Color,
  ) -> Self {
    Self {
      width,
      height,
      image_rendering: match image_rendering {
        ImageScalingAlgorithm::Auto => 0,
        ImageScalingAlgorithm::Smooth => 1,
        ImageScalingAlgorithm::Pixelated => 2,
      },
      current_color: u32::from_be_bytes(current_color.0),
    }
  }
}

#[cfg(feature = "svg")]
#[derive(Debug, Default)]
struct SvgRasterCache {
  #[cfg(target_arch = "wasm32")]
  map: RefCell<HashMap<SvgRasterCacheKey, Arc<Pixmap>>>,
  #[cfg(not(target_arch = "wasm32"))]
  map: DashMap<SvgRasterCacheKey, Arc<Pixmap>>,
}

#[cfg(feature = "svg")]
impl SvgRasterCache {
  fn get(&self, key: SvgRasterCacheKey) -> Option<Arc<Pixmap>> {
    #[cfg(target_arch = "wasm32")]
    {
      self.map.borrow().get(&key).cloned()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
      self.map.get(&key).map(|pixmap| pixmap.clone())
    }
  }

  fn insert(&self, key: SvgRasterCacheKey, pixmap: Arc<Pixmap>) {
    #[cfg(target_arch = "wasm32")]
    {
      self.map.borrow_mut().insert(key, pixmap);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
      self.map.insert(key, pixmap);
    }
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
      uses_current_color: sanitized_svg.contains("currentColor"),
      source: Arc::from(sanitized_svg),
      tree: Arc::new(tree),
      raster_cache: Arc::default(),
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
        return Ok(ImageSource::Svg(text.parse()?));
      }
    }

    let image = decode_image(bytes).map_err(ImageResourceError::DecodeError)?;
    let source = match image {
      DecodedImage::Pixmap(pixmap) => ImageSource::Bitmap(Arc::new(pixmap)),
      DecodedImage::Gif(gif) => ImageSource::Gif(GifSource::from_decoded(gif)?),
    };

    Ok(source)
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
    time_ms: u64,
    current_color: Color,
  ) -> Result<RenderedImage<'i>, ImageResourceError> {
    #[cfg(not(feature = "svg"))]
    let _ = current_color;

    match self {
      ImageSource::Bitmap(bitmap) => Ok(RenderedImage::Borrowed {
        source: bitmap.as_ref(),
        width,
        height,
        algorithm: image_rendering,
      }),
      ImageSource::Gif(gif) => Ok(RenderedImage::Borrowed {
        source: gif.frame_at_time(time_ms),
        width,
        height,
        algorithm: image_rendering,
      }),
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => {
        use resvg::usvg::{Options, Transform, Tree};

        let cache_key = SvgRasterCacheKey::new(
          width,
          height,
          image_rendering,
          if svg.uses_current_color {
            current_color
          } else {
            Color::transparent()
          },
        );
        if let Some(pixmap) = svg.raster_cache.get(cache_key) {
          return Ok(RenderedImage::Rasterized(pixmap));
        }

        let tree = if svg.uses_current_color {
          let options = Options {
            style_sheet: Some(format!("svg {{ color: {current_color}; }}")),
            image_rendering: image_rendering.into(),
            ..Default::default()
          };
          Some(Tree::from_str(&svg.source, &options).map_err(ImageResourceError::SvgParseError)?)
        } else {
          None
        };
        let tree = tree.as_ref().unwrap_or(&svg.tree);

        let mut pixmap = Pixmap::new(width, height).ok_or(ImageResourceError::InvalidPixmapSize)?;

        let original_size = tree.size();
        let sx = width as f32 / original_size.width();
        let sy = height as f32 / original_size.height();

        resvg::render(tree, Transform::from_scale(sx, sy), &mut pixmap.as_mut());

        let pixmap = Arc::new(pixmap);
        svg.raster_cache.insert(cache_key, pixmap.clone());

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
      ImageSource::Gif(gif) => {
        let frame = &gif.frames[0].pixmap;
        (frame.width() as f32, frame.height() as f32)
      }
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
  /// GIF decoding produced no frames.
  #[error("The GIF image does not contain any decodable frames")]
  InvalidGif,
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
      RenderedImage::Borrowed { source, .. } => source
        .pixel(x, y)
        .unwrap_or(PremultipliedColorU8::TRANSPARENT),
    }
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_changes_output() -> Result<(), ImageResourceError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let red = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0xFF0000),
    )?;
    let blue = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0x0000FF),
    )?;

    assert_ne!(premul_at(&red, 2, 2), premul_at(&blue, 2, 2));
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_applies_alpha() -> Result<(), ImageResourceError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;
    let color = Color([255, 0, 0, 128]);

    let rendered = image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0, color)?;
    let alpha = premul_at(&rendered, 2, 2).alpha();

    assert!((alpha as i16 - 128).abs() <= 1);
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_fixed_fill_is_not_affected_by_current_color() -> Result<(), ImageResourceError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ff0000"/></svg>"##;
    let image: ImageSource = SvgSource::from_str(svg)?.into();

    let first = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0x00FF00),
    )?;
    let second = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0x0000FF),
    )?;

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
  fn svg_fixed_fill_reuses_rasterized_pixmap() -> Result<(), ImageResourceError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ff0000"/></svg>"##;
    let image: ImageSource = SvgSource::from_str(svg)?.into();

    let first = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0x00FF00),
    )?;
    let second = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0x0000FF),
    )?;

    let RenderedImage::Rasterized(first) = first else {
      unreachable!()
    };
    let RenderedImage::Rasterized(second) = second else {
      unreachable!()
    };

    assert!(Arc::ptr_eq(&first, &second));
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
    let image = ImageSource::from(bitmap);

    let first = image.render_for_layout(
      2,
      2,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0xFF0000),
    )?;
    let second = image.render_for_layout(
      2,
      2,
      ImageScalingAlgorithm::Auto,
      0,
      Color::from_rgb(0x0000FF),
    )?;

    let RenderedImage::Borrowed { source: first, .. } = first else {
      unreachable!()
    };
    let RenderedImage::Borrowed { source: second, .. } = second else {
      unreachable!()
    };
    assert_eq!(first.data(), second.data());
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
    let image = ImageSource::from(bitmap);

    let rendered =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Pixelated, 0, Color::black())?;
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

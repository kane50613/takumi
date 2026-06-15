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
#[cfg(feature = "svg")]
use tiny_skia::Pixmap;

use crate::{
  layout::style::{Color, ImageScalingAlgorithm, IntrinsicSizing, SizingContext},
  resources::image_buffer::ImageBuffer,
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
  Bitmap(Arc<ImageBuffer>),
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
  /// Intrinsic dimensions (non-percentage `width`/`height`) and `viewBox`
  /// aspect ratio, for CSS `background-size`/`mask-size` resolution.
  intrinsic: SvgIntrinsic,
  raster_cache: Arc<SvgRasterCache>,
}

/// Intrinsic width/height (in SVG user units) and aspect ratio of an SVG root.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, Default)]
struct SvgIntrinsic {
  width: Option<f32>,
  height: Option<f32>,
  ratio: Option<f32>,
}

/// A decoded GIF image with frame timing metadata.
#[derive(Debug, Clone)]
pub struct GifSource {
  frames: Arc<[GifFrame]>,
  total_duration_ms: u64,
}

/// A single GIF frame.
#[derive(Debug, Clone)]
struct GifFrame {
  buffer: Arc<ImageBuffer>,
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
        buffer: frame.buffer,
        duration_ms: frame.duration_ms,
      });
    }

    Ok(Self {
      frames: frames.into(),
      total_duration_ms,
    })
  }

  pub(crate) fn frame_at_time(&self, time_ms: u64) -> &ImageBuffer {
    if self.total_duration_ms == 0 {
      return &self.frames[0].buffer;
    }

    let target_time = time_ms % self.total_duration_ms;
    let mut elapsed_ms = 0_u64;

    for frame in self.frames.iter() {
      elapsed_ms = elapsed_ms.saturating_add(frame.duration_ms as u64);
      if target_time < elapsed_ms {
        return &frame.buffer;
      }
    }

    &self.frames[0].buffer
  }

  pub(crate) fn frame_at_time_arc(&self, time_ms: u64) -> Arc<ImageBuffer> {
    if self.total_duration_ms == 0 {
      return self.frames[0].buffer.clone();
    }

    let target_time = time_ms % self.total_duration_ms;
    let mut elapsed_ms = 0_u64;

    for frame in self.frames.iter() {
      elapsed_ms = elapsed_ms.saturating_add(frame.duration_ms as u64);
      if target_time < elapsed_ms {
        return frame.buffer.clone();
      }
    }

    self.frames[0].buffer.clone()
  }
}

#[cfg(feature = "svg")]
impl From<SvgSource> for ImageSource {
  fn from(svg: SvgSource) -> Self {
    ImageSource::Svg(svg)
  }
}

/// Image data prepared for layout rendering.
#[derive(Debug, Clone)]
pub(crate) enum RenderedImage<'a> {
  /// A fully rasterized image, used for SVGs.
  Rasterized(Arc<ImageBuffer>),
  /// A borrowed bitmap that should be sampled directly.
  Borrowed {
    /// The original bitmap source.
    source: &'a ImageBuffer,
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
    let buffer = ImageBuffer::from_rgba(Cow::Owned(bitmap)).unwrap_or_else(|| {
      let mut edge = 1_u32;
      loop {
        if let Some(buffer) = ImageBuffer::new(edge, edge) {
          break buffer;
        }
        edge = edge.saturating_add(1);
      }
    });
    ImageSource::Bitmap(Arc::new(buffer))
  }
}

impl From<ImageBuffer> for ImageSource {
  fn from(buffer: ImageBuffer) -> Self {
    ImageSource::Bitmap(Arc::new(buffer))
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
  map: RefCell<HashMap<SvgRasterCacheKey, Arc<ImageBuffer>>>,
  #[cfg(not(target_arch = "wasm32"))]
  map: DashMap<SvgRasterCacheKey, Arc<ImageBuffer>>,
}

#[cfg(feature = "svg")]
impl SvgRasterCache {
  fn get(&self, key: SvgRasterCacheKey) -> Option<Arc<ImageBuffer>> {
    #[cfg(target_arch = "wasm32")]
    {
      self.map.borrow().get(&key).cloned()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
      self.map.get(&key).map(|pixmap| pixmap.clone())
    }
  }

  fn insert(&self, key: SvgRasterCacheKey, pixmap: Arc<ImageBuffer>) {
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
fn usvg_image_rendering(algorithm: ImageScalingAlgorithm) -> resvg::usvg::ImageRendering {
  match algorithm {
    ImageScalingAlgorithm::Auto => resvg::usvg::ImageRendering::default(),
    ImageScalingAlgorithm::Smooth => resvg::usvg::ImageRendering::Smooth,
    ImageScalingAlgorithm::Pixelated => resvg::usvg::ImageRendering::Pixelated,
  }
}

#[cfg(feature = "svg")]
impl FromStr for SvgSource {
  type Err = ImageResourceError;

  fn from_str(src: &str) -> Result<Self, Self::Err> {
    use resvg::usvg::{Error, Options, Tree};
    use roxmltree::{Document, ParsingOptions};

    // One parse, shared with usvg via `from_xmltree` (what `from_str` does
    // internally). No text stripping: usvg drops `<text>`/`<tspan>` with its
    // `text` feature off.
    let options = ParsingOptions {
      allow_dtd: true,
      ..Default::default()
    };
    let document = Document::parse_with_options(src, options).map_err(Error::ParsingFailed)?;

    let tree = Tree::from_xmltree(&document, &Options::default())?;
    let intrinsic = svg_intrinsic_sizing(document.root_element(), tree.size());

    Ok(SvgSource {
      uses_current_color: src.contains("currentColor"),
      source: Arc::from(src),
      tree: Arc::new(tree),
      intrinsic,
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
      DecodedImage::Buffer(buffer) => ImageSource::Bitmap(Arc::new(buffer)),
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
            image_rendering: usvg_image_rendering(image_rendering),
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

        let buffer = ImageBuffer::from_premultiplied_rgba(pixmap.data().to_vec(), width, height)
          .ok_or(ImageResourceError::InvalidPixmapSize)?;
        let buffer = Arc::new(buffer);
        svg.raster_cache.insert(cache_key, buffer.clone());

        Ok(RenderedImage::Rasterized(buffer))
      }
    }
  }

  /// Get the image size in device pixels for the current sizing context.
  pub(crate) fn size(&self, sizing: &SizingContext) -> (f32, f32) {
    let (width, height) = match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => (svg.tree.size().width(), svg.tree.size().height()),
      ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
      ImageSource::Gif(gif) => {
        let frame = &gif.frames[0].buffer;
        (frame.width() as f32, frame.height() as f32)
      }
    };

    let dpr = sizing.viewport.device_pixel_ratio;
    (width * dpr, height * dpr)
  }

  /// Intrinsic sizing for `background-size`/`mask-size` (§5.3). Bitmaps and GIFs
  /// have both dimensions; an SVG may have only a `viewBox` ratio.
  pub(crate) fn intrinsic_sizing(&self, sizing: &SizingContext) -> IntrinsicSizing {
    let dpr = sizing.viewport.device_pixel_ratio;
    match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => IntrinsicSizing {
        width: svg.intrinsic.width.map(|width| width * dpr),
        height: svg.intrinsic.height.map(|height| height * dpr),
        ratio: svg.intrinsic.ratio,
      },
      ImageSource::Bitmap(bitmap) => {
        IntrinsicSizing::from_dimensions(bitmap.width() as f32 * dpr, bitmap.height() as f32 * dpr)
      }
      ImageSource::Gif(gif) => {
        let frame = &gif.frames[0].buffer;
        IntrinsicSizing::from_dimensions(frame.width() as f32 * dpr, frame.height() as f32 * dpr)
      }
    }
  }
}

/// Check if the string looks like an SVG image.
pub(crate) fn is_svg_like(src: &str) -> bool {
  src.contains("<svg") && src.contains("xmlns")
}

/// SVG root intrinsic sizing per <https://www.w3.org/TR/SVG/coords.html#IntrinsicSizing>:
/// a non-percentage `width`/`height` is an intrinsic dimension, the `viewBox`
/// gives the ratio. Absolute px come from `resolved_size` (usvg's parsed size)
/// to avoid reimplementing SVG length units.
#[cfg(feature = "svg")]
fn svg_intrinsic_sizing(root: roxmltree::Node, resolved_size: resvg::usvg::Size) -> SvgIntrinsic {
  let is_absolute = |name| {
    root
      .attribute(name)
      .map(str::trim)
      .is_some_and(|value| !value.is_empty() && !value.ends_with('%'))
  };

  let width = is_absolute("width").then(|| resolved_size.width());
  let height = is_absolute("height").then(|| resolved_size.height());

  let ratio = match (width, height) {
    (Some(width), Some(height)) if width != 0.0 && height != 0.0 => Some(width / height),
    _ => root.attribute("viewBox").and_then(parse_viewbox_ratio),
  };

  SvgIntrinsic {
    width,
    height,
    ratio,
  }
}

/// Parse the aspect ratio (`width / height`) from a `viewBox` (`min-x min-y
/// width height`).
#[cfg(feature = "svg")]
fn parse_viewbox_ratio(view_box: &str) -> Option<f32> {
  let mut numbers = view_box
    .split([' ', ',', '\t', '\n', '\r'])
    .filter(|part| !part.is_empty());
  let width: f32 = numbers.nth(2)?.parse().ok()?;
  let height: f32 = numbers.next()?.parse().ok()?;
  (width > 0.0 && height > 0.0).then_some(width / height)
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
  use std::{assert_matches, borrow::Cow};

  use image::Rgba;

  use super::*;
  use crate::resources::image_decoder::DecodedGifFrame;

  /// `width`/`height` attributes give intrinsic dimensions; a `viewBox` alone
  /// gives only an aspect ratio (per the SVG/CSS intrinsic sizing rules).
  #[cfg(feature = "svg")]
  #[test]
  fn svg_intrinsic_distinguishes_viewbox_from_dimensions() {
    fn intrinsic(svg: String) -> SvgIntrinsic {
      let Ok(source) = svg.parse::<SvgSource>() else {
        unreachable!("valid svg");
      };
      source.intrinsic
    }
    let ns = r#"xmlns="http://www.w3.org/2000/svg""#;

    // viewBox only: aspect ratio, no intrinsic dimensions.
    let only = intrinsic(format!(r#"<svg {ns} viewBox="0 0 128 128"/>"#));
    assert_eq!(
      (only.width, only.height, only.ratio),
      (None, None, Some(1.0))
    );

    // Absolute width/height: intrinsic dimensions.
    let sized = intrinsic(format!(r#"<svg {ns} width="102" height="38"/>"#));
    let ratio = Some(102.0 / 38.0);
    assert_eq!(
      (sized.width, sized.height, sized.ratio),
      (Some(102.0), Some(38.0), ratio)
    );

    // Percentage width/height are not intrinsic; the ratio comes from the viewBox.
    let percentage = intrinsic(format!(
      r#"<svg {ns} width="100%" height="100%" viewBox="0 0 16 8"/>"#
    ));
    assert_eq!(
      (percentage.width, percentage.height, percentage.ratio),
      (None, None, Some(2.0))
    );
  }

  fn premul_at(image: &RenderedImage<'_>, x: u32, y: u32) -> [u8; 4] {
    match image {
      RenderedImage::Rasterized(buffer) => buffer.pixel(x, y),
      RenderedImage::Borrowed { source, .. } => source.pixel(x, y),
    }
  }

  fn frame_buffer(seed: u8) -> Arc<ImageBuffer> {
    let bitmap = RgbaImage::from_pixel(1, 1, Rgba([seed, 0, 0, 255]));
    let buffer = ImageBuffer::from_rgba(Cow::Owned(bitmap)).unwrap_or_else(|| {
      let mut edge = 1_u32;
      loop {
        if let Some(buffer) = ImageBuffer::new(edge, edge) {
          break buffer;
        }
        edge = edge.saturating_add(1);
      }
    });
    Arc::new(buffer)
  }

  fn gif_with_durations(durations: &[u32]) -> GifSource {
    let frames = durations
      .iter()
      .enumerate()
      .map(|(index, duration_ms)| DecodedGifFrame {
        buffer: frame_buffer(index as u8 + 1),
        duration_ms: *duration_ms,
      })
      .collect();
    match GifSource::from_decoded(DecodedGif { frames }) {
      Ok(gif) => gif,
      Err(_) => GifSource {
        frames: [GifFrame {
          buffer: frame_buffer(0),
          duration_ms: 0,
        }]
        .into(),
        total_duration_ms: 0,
      },
    }
  }

  fn expected_frame_index(gif: &GifSource, time_ms: u64) -> usize {
    if gif.total_duration_ms == 0 {
      return 0;
    }

    let target_time = time_ms % gif.total_duration_ms;
    let mut elapsed_ms = 0_u64;
    for (index, frame) in gif.frames.iter().enumerate() {
      elapsed_ms = elapsed_ms.saturating_add(frame.duration_ms as u64);
      if target_time < elapsed_ms {
        return index;
      }
    }

    0
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
    let alpha = premul_at(&rendered, 2, 2)[3];

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
      return Ok(());
    };
    let RenderedImage::Rasterized(second) = second else {
      return Ok(());
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
      return Ok(());
    };
    let RenderedImage::Rasterized(second) = second else {
      return Ok(());
    };

    assert!(Arc::ptr_eq(&first, &second));
    Ok(())
  }

  /// usvg drops `<text>`/`<tspan>` (text feature off), so we no longer strip
  /// them: the SVG renders identically with and without the text nodes.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_text_nodes_are_ignored_not_stripped() -> Result<(), ImageResourceError> {
    fn rendered_data(svg: &str) -> Result<Vec<u8>, ImageResourceError> {
      let image: ImageSource = SvgSource::from_str(svg)?.into();
      let rendered =
        image.render_for_layout(8, 8, ImageScalingAlgorithm::Auto, 0, Color::black())?;
      let RenderedImage::Rasterized(pixmap) = rendered else {
        unreachable!("svg renders to a rasterized pixmap");
      };
      Ok(pixmap.data().to_vec())
    }

    let with_text = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#ff0000"/><text x="1" y="5">hi <tspan>there</tspan></text></svg>"##;
    let without_text = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#ff0000"/></svg>"##;

    assert_eq!(rendered_data(with_text)?, rendered_data(without_text)?);
    Ok(())
  }

  /// `<text>` inside `clipPath` and `foreignObject` must not break parsing.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_with_unsupported_nodes_still_parses() -> Result<(), ImageResourceError> {
    let clip_path_text = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><clipPath id="c"><text>x</text></clipPath><rect width="8" height="8" fill="#ff0000" clip-path="url(#c)"/></svg>"##;
    let foreign_object = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><foreignObject width="8" height="8"><div xmlns="http://www.w3.org/1999/xhtml">x</div></foreignObject><rect width="8" height="8" fill="#ff0000"/></svg>"##;

    SvgSource::from_str(clip_path_text)?;
    SvgSource::from_str(foreign_object)?;
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
      return Ok(());
    };
    let RenderedImage::Borrowed { source: second, .. } = second else {
      return Ok(());
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
      return Ok(());
    };

    assert_eq!(width, 4);
    assert_eq!(height, 4);
    assert_matches!(algo, ImageScalingAlgorithm::Pixelated);
    Ok(())
  }

  #[test]
  fn gif_source_from_decoded_rejects_empty_frames() {
    let result = GifSource::from_decoded(DecodedGif { frames: Vec::new() });
    assert_matches!(result, Err(ImageResourceError::InvalidGif));
  }

  #[test]
  fn gif_source_from_decoded_preserves_frames_and_total_duration() {
    let gif = gif_with_durations(&[10, 25, 5]);

    assert_eq!(gif.frames.len(), 3);
    assert_eq!(gif.total_duration_ms, 40);
    assert_eq!(gif.frames[0].duration_ms, 10);
    assert_eq!(gif.frames[1].duration_ms, 25);
    assert_eq!(gif.frames[2].duration_ms, 5);
  }

  #[test]
  fn gif_source_frame_selection_matches_expected_indices() {
    let gif = gif_with_durations(&[10, 20, 30]);
    let samples = [0_u64, 9, 10, 29, 30, 59, 60, 75];

    for time_ms in samples {
      let expected_index = expected_frame_index(&gif, time_ms);
      let expected_frame = &gif.frames[expected_index];

      let frame = gif.frame_at_time(time_ms);
      assert_eq!(frame.data(), expected_frame.buffer.data());

      let frame_arc = gif.frame_at_time_arc(time_ms);
      assert!(Arc::ptr_eq(&frame_arc, &expected_frame.buffer));
    }
  }

  #[test]
  fn gif_source_zero_total_duration_always_returns_first_frame() {
    let gif = gif_with_durations(&[0, 0, 0]);

    assert_eq!(gif.total_duration_ms, 0);
    for time_ms in [0_u64, 1, 10, 1_000] {
      let frame = gif.frame_at_time(time_ms);
      assert_eq!(frame.data(), gif.frames[0].buffer.data());

      let frame_arc = gif.frame_at_time_arc(time_ms);
      assert!(Arc::ptr_eq(&frame_arc, &gif.frames[0].buffer));
    }
  }

  #[test]
  fn gif_source_clone_shares_frame_storage() {
    let gif = gif_with_durations(&[5, 15, 25]);
    let cloned = gif.clone();

    assert!(Arc::ptr_eq(&gif.frames, &cloned.frames));
  }
}

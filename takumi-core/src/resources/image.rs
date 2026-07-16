//! Image resource management for the takumi rendering system.
//!
//! This module provides types and utilities for managing image resources,
//! including loading states, error handling, and image processing operations.

#[cfg(feature = "svg")]
use std::str::{FromStr, from_utf8};
use std::sync::{Arc, OnceLock, Weak};

#[cfg(feature = "svg")]
use crate::resvg::usvg::{Options, Transform, Tree};
use quick_cache::{
  Weighter,
  sync::{Cache, GuardResult},
};

#[cfg(feature = "svg")]
use roxmltree::{Document, ParsingOptions};
use serde::Deserialize;
use thiserror::Error;
#[cfg(feature = "svg")]
use tiny_skia::Pixmap;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
  resources::{
    image_buffer::ImageBuffer,
    image_decoder::{
      DecodedGifFrame, bitmap_dimensions, decode_bitmap_scaled, decode_gif_frames, decode_image,
      gif_dimensions, is_gif,
    },
  },
  style::{ImageScalingAlgorithm, IntrinsicSizing, SizingContext},
};

/// Represents the state of an image resource.
pub(crate) type ImageResult = Result<ImageSource, ImageError>;

#[derive(Debug, Clone)]
/// Represents the source of an image.
#[non_exhaustive]
pub enum ImageSource {
  /// An svg image source
  #[cfg(feature = "svg")]
  Svg(Arc<SvgSource>),
  /// A bitmap image source
  Bitmap(Arc<ImageBuffer>),
  /// An animated gif source.
  Gif(GifSource),
  /// An encoded bitmap decoded lazily at the size it is drawn at.
  Encoded(Arc<EncodedBitmap>),
}

/// Represents the resolved SVG source.
#[cfg(feature = "svg")]
#[derive(Debug)]
pub struct SvgSource {
  /// Original SVG source, for embedding directly in a vector backend.
  source: Box<str>,
  /// Parsed SVG tree used for size and initial metadata.
  pub(crate) tree: crate::resvg::usvg::Tree,
  /// Intrinsic dimensions (non-percentage `width`/`height`) and `viewBox`
  /// aspect ratio, for CSS `background-size`/`mask-size` resolution.
  intrinsic: SvgIntrinsic,
  raster_cache: SvgRasterCache,
}

#[cfg(feature = "svg")]
impl SvgSource {
  /// The SVG canvas dimensions in pixels, from the root `width`/`height` or
  /// `viewBox`.
  pub fn dimensions(&self) -> (f32, f32) {
    let size = self.tree.size();
    (size.width(), size.height())
  }

  /// The original SVG markup, for embedding directly in a vector backend.
  pub fn source(&self) -> &str {
    &self.source
  }
}

/// Intrinsic width/height (in SVG user units) and aspect ratio of an SVG root.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, Default)]
struct SvgIntrinsic {
  width: Option<f32>,
  height: Option<f32>,
  ratio: Option<f32>,
}

/// A lazily decoded animated GIF: only the first frame is decoded up front,
/// the rest of the stream the first time a render samples past it.
#[derive(Debug, Clone)]
pub struct GifSource {
  inner: Arc<GifInner>,
}

#[derive(Debug)]
struct GifInner {
  bytes: Box<[u8]>,
  width: u32,
  height: u32,
  first: DecodedGifFrame,
  rest: OnceLock<RestFrames>,
}

/// Every frame after the first, decoded in one pass.
#[derive(Debug)]
struct RestFrames {
  frames: Box<[DecodedGifFrame]>,
  /// Duration of the whole animation, first frame included.
  total_duration_ms: u64,
  /// Size the frames were decoded at; `None` means canvas size.
  target: Option<(u32, u32)>,
}

impl GifSource {
  fn from_bytes(bytes: &[u8]) -> Result<Self, ImageError> {
    let (width, height) = gif_dimensions(bytes).map_err(ImageError::decode)?;

    let mut first = None;
    decode_gif_frames(bytes, 0, Some(1), None, |frame| first = Some(frame))
      .map_err(ImageError::decode)?;
    let Some(first) = first else {
      return Err(ImageError::InvalidGif);
    };

    Ok(Self {
      inner: Arc::new(GifInner {
        bytes: bytes.into(),
        width,
        height,
        first,
        rest: OnceLock::new(),
      }),
    })
  }

  /// The GIF logical screen dimensions in pixels.
  pub fn dimensions(&self) -> (u32, u32) {
    (self.inner.width, self.inner.height)
  }

  /// The first caller decides the memoized frame size and filter; renders of
  /// the same GIF use one draw box in practice, and a single slot is what keeps
  /// the timeline's memory bounded.
  fn rest(&self, target: (u32, u32, ImageScalingAlgorithm)) -> &RestFrames {
    self.inner.rest.get_or_init(|| {
      let mut frames = Vec::new();
      let mut total_duration_ms = self.inner.first.duration_ms as u64;
      let _ = decode_gif_frames(&self.inner.bytes, 1, None, Some(target), |frame| {
        total_duration_ms += frame.duration_ms as u64;
        frames.push(frame);
      });
      RestFrames {
        target: frames
          .first()
          .map(|frame: &DecodedGifFrame| (frame.buffer.width(), frame.buffer.height())),
        frames: frames.into(),
        total_duration_ms,
      }
    })
  }

  /// Index into `rest.frames` for the given playback time, or `None` when the
  /// time falls on the first frame.
  fn rest_index_at(&self, rest: &RestFrames, time_ms: u64) -> Option<usize> {
    if rest.total_duration_ms == 0 {
      return None;
    }

    let mut elapsed_ms = self.inner.first.duration_ms as u64;
    let target_time = time_ms % rest.total_duration_ms;
    if target_time < elapsed_ms {
      return None;
    }

    for (index, frame) in rest.frames.iter().enumerate() {
      elapsed_ms += frame.duration_ms as u64;
      if target_time < elapsed_ms {
        return Some(index);
      }
    }

    None
  }

  /// Shared frame shown at the given playback time, looping over total duration.
  #[cfg(test)]
  fn frame_at_time(&self, time_ms: u64) -> Arc<ImageBuffer> {
    self.frame_at_time_covering(
      time_ms,
      self.inner.width,
      self.inner.height,
      ImageScalingAlgorithm::Auto,
    )
  }

  /// Shared frame shown at the given playback time, looping over total
  /// duration. Frames past the first decode scaled to cover a `width` x
  /// `height` draw box (never upscaled), so the memoized timeline holds
  /// draw-sized frames; a request larger than the memoized size decodes that
  /// frame fresh.
  pub fn frame_at_time_covering(
    &self,
    time_ms: u64,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Arc<ImageBuffer> {
    let first = &self.inner.first;
    if time_ms < first.duration_ms as u64 {
      return first.buffer.clone();
    }

    let requested = cover_target((self.inner.width, self.inner.height), (width, height));
    let rest = self.rest((requested.0, requested.1, algorithm));
    let Some(index) = self.rest_index_at(rest, time_ms) else {
      return first.buffer.clone();
    };

    let memoized = rest.target.unwrap_or((self.inner.width, self.inner.height));
    if requested.0 > memoized.0 || requested.1 > memoized.1 {
      let mut frame = None;
      let target = (requested.0, requested.1, algorithm);
      let _ = decode_gif_frames(
        &self.inner.bytes,
        index + 1,
        Some(1),
        Some(target),
        |decoded| frame = Some(decoded.buffer),
      );
      if let Some(frame) = frame {
        return frame;
      }
    }

    rest.frames[index].buffer.clone()
  }

  fn decoded_bytes(&self) -> usize {
    let rest = self.inner.rest.get().map_or(0, |rest| {
      rest
        .frames
        .iter()
        .map(|frame| frame.buffer.data().len())
        .sum()
    });
    self.inner.first.buffer.data().len() + rest + self.inner.bytes.len()
  }

  #[cfg(test)]
  fn decoded_frame_count(&self) -> usize {
    1 + self.inner.rest.get().map_or(0, |rest| rest.frames.len())
  }
}

#[cfg(feature = "svg")]
impl From<SvgSource> for ImageSource {
  fn from(svg: SvgSource) -> Self {
    ImageSource::Svg(Arc::new(svg))
  }
}

/// An encoded bitmap (PNG/JPEG/WebP) that decodes lazily at draw time, scaled
/// down to the box it is drawn into. Decoded results are stored in the owning
/// [`ImageCache`] keyed by content and target size, so a source drawn at a
/// stable size decodes once while the retained bytes track the draw size, not
/// the source size.
#[derive(Debug)]
pub struct EncodedBitmap {
  bytes: Box<[u8]>,
  width: u32,
  height: u32,
  hash: u64,
  cache: Weak<SharedImageCache>,
}

impl EncodedBitmap {
  /// The bitmap dimensions in pixels, from the format header.
  pub fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  /// The original encoded bytes.
  pub fn bytes(&self) -> &[u8] {
    &self.bytes
  }

  /// Decoded buffer covering a `width` x `height` draw box, downscaled with
  /// `algorithm`'s filter but never upscaled. Returns the buffer and its scale
  /// relative to the source dimensions.
  fn decode_at(
    &self,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Result<(Arc<ImageBuffer>, (f32, f32)), ImageError> {
    let (target_width, target_height) = cover_target((self.width, self.height), (width, height));

    let buffer = self.decode_scaled(target_width, target_height, algorithm)?;
    let scale = (
      target_width as f32 / self.width as f32,
      target_height as f32 / self.height as f32,
    );

    Ok((buffer, scale))
  }

  fn decode_scaled(
    &self,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    let Some(cache) = self.cache.upgrade() else {
      return self.decode_uncached(width, height, algorithm);
    };

    let key = ImageCacheKey::sized(self.hash, width, height, algorithm);

    match cache.get_value_or_guard(&key, None) {
      GuardResult::Value(CacheEntry::Sized(buffer)) => Ok(buffer),
      GuardResult::Value(CacheEntry::Source(_)) => self.decode_uncached(width, height, algorithm),
      GuardResult::Guard(guard) => {
        let buffer = self.decode_uncached(width, height, algorithm)?;
        let _ = guard.insert(CacheEntry::Sized(buffer.clone()));
        Ok(buffer)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => self.decode_uncached(width, height, algorithm),
    }
  }

  fn decode_uncached(
    &self,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    decode_bitmap_scaled(&self.bytes, width, height, algorithm)
      .map(Arc::new)
      .map_err(ImageError::decode)
  }
}

/// Image data prepared for layout rendering.
#[derive(Debug, Clone)]
pub enum RenderedImage {
  /// A fully rasterized image, used for SVGs.
  Rasterized(Arc<ImageBuffer>),
  /// A shared bitmap that should be sampled directly.
  Sampled {
    /// The original bitmap source.
    source: Arc<ImageBuffer>,
    /// The logical width that will be rendered on the canvas.
    width: u32,
    /// The logical height that will be rendered on the canvas.
    height: u32,
    /// The sampling algorithm to use.
    algorithm: ImageScalingAlgorithm,
    /// The buffer size relative to the source's intrinsic dimensions;
    /// `(1.0, 1.0)` unless the buffer was decoded pre-scaled.
    source_scale: (f32, f32),
  },
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
}

#[cfg(feature = "svg")]
impl SvgRasterCacheKey {
  fn new(width: u32, height: u32, image_rendering: ImageScalingAlgorithm) -> Self {
    Self {
      width,
      height,
      image_rendering: match image_rendering {
        ImageScalingAlgorithm::Smooth => 1,
        ImageScalingAlgorithm::Pixelated => 2,
        _ => 0,
      },
    }
  }
}

/// An SVG is rasterized at only a handful of distinct sizes; this caps that.
#[cfg(feature = "svg")]
const SVG_RASTER_CACHE_CAPACITY: usize = 32;

/// Per-`SvgSource` cache of rasterized bitmaps keyed by target size. Count-bounded and lock-free
/// (the same `quick_cache` as `ImageCache`); byte-budget it if a huge SVG cached at many sizes ever
/// bloats memory.
#[cfg(feature = "svg")]
type SvgRasterCache = Cache<SvgRasterCacheKey, Arc<ImageBuffer>>;

#[cfg(feature = "svg")]
impl FromStr for SvgSource {
  type Err = ImageError;

  fn from_str(src: &str) -> Result<Self, Self::Err> {
    // One parse, shared with usvg via `from_xmltree` (what `from_str` does
    // internally). No text stripping: usvg drops `<text>`/`<tspan>` with its
    // `text` feature off.
    let options = ParsingOptions {
      allow_dtd: true,
      ..Default::default()
    };
    let document = Document::parse_with_options(src, options).map_err(ImageError::svg_parse)?;

    let tree = Tree::from_xmltree(&document, &Options::default()).map_err(ImageError::svg_parse)?;
    let intrinsic = svg_intrinsic_sizing(document.root_element(), tree.size());

    Ok(SvgSource {
      source: Box::from(src),
      tree,
      intrinsic,
      raster_cache: SvgRasterCache::new(SVG_RASTER_CACHE_CAPACITY),
    })
  }
}

impl ImageSource {
  /// Approximate decoded size in bytes, used for cache budgeting.
  pub(crate) fn estimated_bytes(&self) -> usize {
    match self {
      Self::Bitmap(buffer) => buffer.data().len(),
      Self::Gif(gif) => gif.decoded_bytes(),
      Self::Encoded(encoded) => encoded.bytes.len(),
      #[cfg(feature = "svg")]
      Self::Svg(svg) => svg.source.len(),
    }
  }

  /// Whether this decoded source is safe to retain in the byte-budgeted cache.
  ///
  /// SVGs and GIFs are excluded: their lazily-populated state (rasterized
  /// pixmaps, memoized frames) grows after insertion and isn't counted by
  /// [`estimated_bytes`](Self::estimated_bytes), so caching them across renders
  /// would let memory exceed the configured budget. Encoded bitmaps stay
  /// cacheable: their decoded-at-size buffers live in the same budgeted cache
  /// as their own weighted entries.
  pub(crate) fn is_cacheable(&self) -> bool {
    match self {
      Self::Bitmap(_) | Self::Encoded(_) => true,
      Self::Gif(_) => false,
      #[cfg(feature = "svg")]
      Self::Svg(_) => false,
    }
  }

  /// Load an image source from raw bytes.
  ///
  /// - When the `svg` feature is enabled and the bytes look like SVG XML, they
  ///   are parsed as an SVG using `resvg::usvg`.
  /// - Otherwise, the bytes are decoded as a raster image.
  pub fn from_bytes(bytes: &[u8]) -> ImageResult {
    #[cfg(feature = "svg")]
    {
      if let Ok(text) = from_utf8(bytes)
        && is_svg_like(text)
      {
        return Ok(ImageSource::Svg(Arc::new(text.parse()?)));
      }
    }

    if is_gif(bytes) {
      return Ok(ImageSource::Gif(GifSource::from_bytes(bytes)?));
    }

    let buffer = decode_image(bytes).map_err(ImageError::decode)?;
    Ok(ImageSource::Bitmap(Arc::new(buffer)))
  }

  /// [`from_bytes`](Self::from_bytes), but bitmaps stay encoded and decode at
  /// draw size. Sized decodes go into `cache` while it is alive; a dead handle
  /// (inline node bytes, data URIs) decodes per render.
  pub(crate) fn from_bytes_lazy(
    bytes: &[u8],
    hash: u64,
    cache: Weak<SharedImageCache>,
  ) -> ImageResult {
    #[cfg(feature = "svg")]
    {
      if let Ok(text) = from_utf8(bytes)
        && is_svg_like(text)
      {
        return Ok(ImageSource::Svg(Arc::new(text.parse()?)));
      }
    }

    if is_gif(bytes) {
      return Ok(ImageSource::Gif(GifSource::from_bytes(bytes)?));
    }

    match bitmap_dimensions(bytes) {
      Some(Ok((width, height))) => Ok(ImageSource::Encoded(Arc::new(EncodedBitmap {
        bytes: bytes.into(),
        width,
        height,
        hash,
        cache,
      }))),
      Some(Err(error)) => Err(ImageError::decode(error)),
      None => Self::from_bytes(bytes),
    }
  }

  /// Prepare image data for layout rendering.
  ///
  /// Bitmap images share their buffer so the renderer can sample them
  /// directly. SVG images are rasterized to a bitmap first.
  pub fn render_for_layout(
    &self,
    width: u32,
    height: u32,
    image_rendering: ImageScalingAlgorithm,
    time_ms: u64,
  ) -> Result<RenderedImage, ImageError> {
    match self {
      ImageSource::Bitmap(bitmap) => Ok(RenderedImage::Sampled {
        source: bitmap.clone(),
        width,
        height,
        algorithm: image_rendering,
        source_scale: (1.0, 1.0),
      }),
      ImageSource::Gif(gif) => {
        let source = gif.frame_at_time_covering(time_ms, width, height, image_rendering);
        let (native_width, native_height) = gif.dimensions();
        Ok(RenderedImage::Sampled {
          source_scale: (
            source.width() as f32 / native_width as f32,
            source.height() as f32 / native_height as f32,
          ),
          source,
          width,
          height,
          algorithm: image_rendering,
        })
      }
      ImageSource::Encoded(encoded) => {
        let (source, source_scale) = encoded.decode_at(width, height, image_rendering)?;
        Ok(RenderedImage::Sampled {
          source,
          width,
          height,
          algorithm: image_rendering,
          source_scale,
        })
      }
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => {
        let cache_key = SvgRasterCacheKey::new(width, height, image_rendering);
        if let Some(pixmap) = svg.raster_cache.get(&cache_key) {
          return Ok(RenderedImage::Rasterized(pixmap));
        }

        let tree = &svg.tree;
        let mut pixmap = Pixmap::new(width, height).ok_or(ImageError::InvalidPixmapSize)?;

        let original_size = tree.size();
        let sx = width as f32 / original_size.width();
        let sy = height as f32 / original_size.height();

        crate::resvg::render(tree, Transform::from_scale(sx, sy), &mut pixmap.as_mut());

        let buffer = ImageBuffer::from_premultiplied_rgba(pixmap.data().to_vec(), width, height)
          .ok_or(ImageError::InvalidPixmapSize)?;
        let buffer = Arc::new(buffer);
        svg.raster_cache.insert(cache_key, buffer.clone());

        Ok(RenderedImage::Rasterized(buffer))
      }
    }
  }

  /// Get the image size in device pixels for the current sizing context.
  pub fn size(&self, sizing: &SizingContext) -> (f32, f32) {
    let (width, height) = match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => svg.dimensions(),
      ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
      ImageSource::Gif(gif) => {
        let (width, height) = gif.dimensions();
        (width as f32, height as f32)
      }
      ImageSource::Encoded(encoded) => {
        let (width, height) = encoded.dimensions();
        (width as f32, height as f32)
      }
    };

    (sizing.to_device(width), sizing.to_device(height))
  }

  /// Intrinsic sizing for `background-size`/`mask-size` (§5.3). Bitmaps and GIFs
  /// have both dimensions; an SVG may have only a `viewBox` ratio.
  pub fn intrinsic_sizing(&self) -> IntrinsicSizing {
    match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => IntrinsicSizing {
        width: svg.intrinsic.width,
        height: svg.intrinsic.height,
        ratio: svg.intrinsic.ratio,
      },
      ImageSource::Bitmap(bitmap) => {
        IntrinsicSizing::from_dimensions(bitmap.width() as f32, bitmap.height() as f32)
      }
      ImageSource::Gif(gif) => {
        let (width, height) = gif.dimensions();
        IntrinsicSizing::from_dimensions(width as f32, height as f32)
      }
      ImageSource::Encoded(encoded) => {
        let (width, height) = encoded.dimensions();
        IntrinsicSizing::from_dimensions(width as f32, height as f32)
      }
    }
  }
}

/// Cover-fit target for a draw box: uniform scale, never upscaled.
fn cover_target((native_w, native_h): (u32, u32), (box_w, box_h): (u32, u32)) -> (u32, u32) {
  let scale = (box_w as f32 / native_w as f32)
    .max(box_h as f32 / native_h as f32)
    .min(1.0);
  (
    ((native_w as f32 * scale).round() as u32).clamp(1, native_w),
    ((native_h as f32 * scale).round() as u32).clamp(1, native_h),
  )
}

/// Check if the string looks like an SVG image.
pub(crate) fn is_svg_like(src: &str) -> bool {
  src.contains("<svg")
}

/// A decoded `data:` URI body with its `type/subtype` MIME string.
pub(crate) struct DecodedDataUri {
  pub mime: String,
  pub bytes: Vec<u8>,
}

pub(crate) enum DataUriError {
  /// The URI could not be processed.
  Malformed,
  /// The body could not be decoded.
  Undecodable,
}

/// Decodes a `data:` URI. A raw `#` in the body (hex colors, `url(#id)` in
/// inline SVG) is a URL fragment delimiter and would truncate it, so it is
/// escaped first.
pub(crate) fn decode_data_uri(src: &str) -> Result<DecodedDataUri, DataUriError> {
  let escaped = src.split_once(',').and_then(|(header, body)| {
    body
      .contains('#')
      .then(|| format!("{header},{}", body.replace('#', "%23")))
  });
  let url = data_url::DataUrl::process(escaped.as_deref().unwrap_or(src))
    .map_err(|_| DataUriError::Malformed)?;

  let mime = url.mime_type();
  let mime = format!("{}/{}", mime.type_, mime.subtype);
  let (bytes, _) = url.decode_to_vec().map_err(|_| DataUriError::Undecodable)?;

  Ok(DecodedDataUri { mime, bytes })
}

/// Applies SVG `<filter>` markup (carrying `id="{filter_id}"`) to a
/// premultiplied-RGBA layer in place through the resvg filter pipeline.
///
/// The markup is resolved against the layer bounds and applied straight to
/// the layer pixels; no render tree is built and nothing is re-encoded.
#[cfg(feature = "svg")]
pub fn apply_svg_filter(
  layer: &mut [u8],
  width: u32,
  height: u32,
  markup: &str,
  filter_id: &str,
) -> Result<(), ImageError> {
  let filters = crate::resvg::usvg::filters_from_markup(
    markup,
    filter_id,
    width as f32,
    height as f32,
    &Options::default(),
  )
  .map_err(ImageError::svg_parse)?;

  let Some(filters) = filters else {
    // An invalid filter reference hides the element.
    layer.fill(0);
    return Ok(());
  };

  crate::resvg::apply_filters_to_layer(&filters, layer, width, height)
    .ok_or(ImageError::InvalidPixmapSize)
}

/// Encodes bytes as a base64 `data:` URI.
pub fn to_data_url(mime: &str, bytes: &[u8]) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out =
    String::with_capacity("data:;base64,".len() + mime.len() + bytes.len().div_ceil(3) * 4);

  out.push_str("data:");
  out.push_str(mime);
  out.push_str(";base64,");
  for chunk in bytes.chunks(3) {
    let b = [
      chunk[0],
      *chunk.get(1).unwrap_or(&0),
      *chunk.get(2).unwrap_or(&0),
    ];
    let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
    let mut encoded = [
      ALPHABET[(n >> 18) as usize & 63],
      ALPHABET[(n >> 12) as usize & 63],
      ALPHABET[(n >> 6) as usize & 63],
      ALPHABET[n as usize & 63],
    ];
    if chunk.len() < 3 {
      encoded[3] = b'=';
    }
    if chunk.len() < 2 {
      encoded[2] = b'=';
    }
    out.push_str(std::str::from_utf8(&encoded).unwrap_or_default());
  }
  out
}

/// SVG root intrinsic sizing per <https://www.w3.org/TR/SVG/coords.html#IntrinsicSizing>:
/// a non-percentage `width`/`height` is an intrinsic dimension, the `viewBox`
/// gives the ratio. Absolute px come from `resolved_size` (usvg's parsed size)
/// to avoid reimplementing SVG length units.
#[cfg(feature = "svg")]
fn svg_intrinsic_sizing(
  root: roxmltree::Node,
  resolved_size: crate::resvg::usvg::Size,
) -> SvgIntrinsic {
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
pub enum ImageError {
  /// An error occurred while decoding the image data
  #[error("An error occurred while decoding the image data: {0}")]
  DecodeError(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  /// The image data URI is in an invalid format
  #[error("The image data URI is in an invalid format")]
  InvalidDataUriFormat,
  /// The image data URI is malformed and cannot be parsed
  #[error("The image data URI is malformed and cannot be parsed")]
  MalformedDataUri,
  #[cfg(feature = "svg")]
  /// An error occurred while parsing an SVG image
  #[error("An error occurred while parsing an SVG image: {0}")]
  SvgParseError(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
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

impl ImageError {
  /// Wraps a decoder error opaquely so takumi's public API stays independent of
  /// the `image` crate's version.
  pub(crate) fn decode(err: impl std::error::Error + Send + Sync + 'static) -> Self {
    Self::DecodeError(Box::new(err))
  }

  /// Wraps an SVG parse error opaquely so takumi's public API stays independent
  /// of the `resvg`/`usvg` version.
  #[cfg(feature = "svg")]
  pub(crate) fn svg_parse(err: impl std::error::Error + Send + Sync + 'static) -> Self {
    Self::SvgParseError(Box::new(err))
  }
}

/// Decoded-image budget before entries start getting evicted.
const DEFAULT_MAX_BYTES: u64 = 64 << 20; // 64 MiB

/// Cache policy for a decoded image, applied per [`ImageCache::get_or_decode`] call.
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ImageCacheMode {
  /// Cache the decoded image for reuse (evictable).
  #[default]
  Auto,
  /// Skip the decoded-image cache.
  None,
}

/// Cache key: the content hash alone addresses a source entry; a target size
/// and filter address a decoded-at-size entry.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ImageCacheKey {
  hash: u64,
  width: u32,
  height: u32,
  algorithm: u8,
}

impl ImageCacheKey {
  fn source(hash: u64) -> Self {
    Self {
      hash,
      width: 0,
      height: 0,
      algorithm: 0,
    }
  }

  fn sized(hash: u64, width: u32, height: u32, algorithm: ImageScalingAlgorithm) -> Self {
    Self {
      hash,
      width,
      height,
      algorithm: match algorithm {
        ImageScalingAlgorithm::Smooth => 1,
        ImageScalingAlgorithm::Pixelated => 2,
        _ => 0,
      },
    }
  }
}

#[derive(Clone)]
pub(crate) enum CacheEntry {
  Source(ImageSource),
  Sized(Arc<ImageBuffer>),
}

#[derive(Clone)]
pub(crate) struct ImageWeighter;

impl Weighter<ImageCacheKey, CacheEntry> for ImageWeighter {
  fn weight(&self, _key: &ImageCacheKey, entry: &CacheEntry) -> u64 {
    let bytes = match entry {
      CacheEntry::Source(source) => source.estimated_bytes(),
      CacheEntry::Sized(buffer) => buffer.data().len(),
    };
    (bytes as u64).max(1)
  }
}

pub(crate) type SharedImageCache = Cache<ImageCacheKey, CacheEntry, ImageWeighter>;

/// Content-addressed store of decoded images with a byte budget, used by the renderer to avoid re-decoding.
pub struct ImageCache {
  cache: Arc<SharedImageCache>,
}

impl Default for ImageCache {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_MAX_BYTES)
  }
}

impl ImageCache {
  fn with_capacity(max_bytes: u64) -> Self {
    // ~64 KiB average decoded image ⇒ a reasonable item-count hint for the budget.
    let estimated_items = (max_bytes / (64 << 10)).max(1) as usize;

    Self {
      cache: Arc::new(Cache::with_weighter(
        estimated_items,
        max_bytes,
        ImageWeighter,
      )),
    }
  }

  /// Returns the decoded image for `bytes`, decoding on a miss and caching it unless `mode`
  /// is [`ImageCacheMode::None`].
  ///
  /// When caching, concurrent misses for the same bytes are single-flighted: one thread
  /// decodes while the others wait, so each unique image is decoded once.
  pub fn get_or_decode(&self, bytes: &[u8], mode: ImageCacheMode) -> ImageResult {
    let hash = xxh3_64(bytes);
    let key = ImageCacheKey::source(hash);

    if matches!(mode, ImageCacheMode::None) {
      return match self.cache.get(&key) {
        Some(CacheEntry::Source(source)) => Ok(source),
        _ => ImageSource::from_bytes(bytes),
      };
    }

    match self.cache.get_value_or_guard(&key, None) {
      GuardResult::Value(CacheEntry::Source(source)) => Ok(source),
      GuardResult::Value(CacheEntry::Sized(_)) => ImageSource::from_bytes(bytes),
      GuardResult::Guard(guard) => {
        let source = ImageSource::from_bytes_lazy(bytes, hash, Arc::downgrade(&self.cache))?;
        if source.is_cacheable() {
          let _ = guard.insert(CacheEntry::Source(source.clone()));
        }
        Ok(source)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => ImageSource::from_bytes(bytes),
    }
  }
}

#[cfg(test)]
mod image_cache_tests {
  use quick_cache::sync::Cache;

  use super::{
    CacheEntry, ImageCache, ImageCacheKey, ImageCacheMode, ImageWeighter, RenderedImage,
  };
  use crate::{
    resources::{image::ImageSource, image_buffer::ImageBuffer},
    style::ImageScalingAlgorithm,
  };

  /// PNG bytes that decode to a tiny bitmap (cacheable).
  fn png_bytes() -> Vec<u8> {
    ImageBuffer::new(2, 2).unwrap().encode_png().unwrap()
  }

  #[test]
  fn decodes_and_reuses_on_hit() {
    let cache = ImageCache::default();
    let bytes = png_bytes();

    let first = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();
    let second = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    match (&first, &second) {
      (ImageSource::Encoded(a), ImageSource::Encoded(b)) => {
        assert!(std::sync::Arc::ptr_eq(a, b))
      }
      _ => panic!("expected encoded bitmaps"),
    }
  }

  fn sized_png_bytes(width: u32, height: u32) -> Vec<u8> {
    ImageBuffer::new(width, height)
      .unwrap()
      .encode_png()
      .unwrap()
  }

  fn rendered_buffer(
    source: &ImageSource,
    width: u32,
    height: u32,
  ) -> (std::sync::Arc<ImageBuffer>, (f32, f32)) {
    match source
      .render_for_layout(width, height, ImageScalingAlgorithm::Auto, 0)
      .unwrap()
    {
      RenderedImage::Sampled {
        source,
        source_scale,
        ..
      } => (source, source_scale),
      _ => panic!("expected sampled"),
    }
  }

  #[test]
  fn encoded_decodes_at_draw_size_and_reuses() {
    let cache = ImageCache::default();
    let bytes = sized_png_bytes(64, 64);
    let source = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    let (first, scale) = rendered_buffer(&source, 16, 16);
    let (second, _) = rendered_buffer(&source, 16, 16);

    assert_eq!((first.width(), first.height()), (16, 16));
    assert_eq!(scale, (0.25, 0.25));
    assert!(std::sync::Arc::ptr_eq(&first, &second));
  }

  #[test]
  fn encoded_covers_the_larger_axis() {
    let cache = ImageCache::default();
    let bytes = sized_png_bytes(64, 64);
    let source = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    let (buffer, scale) = rendered_buffer(&source, 32, 16);

    assert_eq!((buffer.width(), buffer.height()), (32, 32));
    assert_eq!(scale, (0.5, 0.5));
  }

  #[test]
  fn encoded_never_upscales() {
    let cache = ImageCache::default();
    let bytes = sized_png_bytes(8, 8);
    let source = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    let (buffer, scale) = rendered_buffer(&source, 32, 32);

    assert_eq!((buffer.width(), buffer.height()), (8, 8));
    assert_eq!(scale, (1.0, 1.0));
  }

  #[test]
  fn store_false_does_not_populate_cache() {
    let cache = ImageCache::default();
    let bytes = png_bytes();

    let a = cache.get_or_decode(&bytes, ImageCacheMode::None).unwrap();
    let b = cache.get_or_decode(&bytes, ImageCacheMode::None).unwrap();

    match (&a, &b) {
      (ImageSource::Bitmap(x), ImageSource::Bitmap(y)) => assert!(!std::sync::Arc::ptr_eq(x, y)),
      _ => panic!("expected bitmaps"),
    }
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_is_not_cached() {
    let cache = ImageCache::default();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#;

    let a = cache.get_or_decode(svg, ImageCacheMode::Auto).unwrap();
    let b = cache.get_or_decode(svg, ImageCacheMode::Auto).unwrap();

    assert!(matches!(a, ImageSource::Svg(_)));
    match (&a, &b) {
      (ImageSource::Svg(x), ImageSource::Svg(y)) => {
        assert!(!std::sync::Arc::ptr_eq(x, y))
      }
      _ => panic!("expected svgs"),
    }
  }

  /// Builds a bitmap source of approximately `bytes` decoded size (premultiplied RGBA, 1px tall).
  fn image(bytes: u32) -> ImageSource {
    let width = (bytes / 4).max(1);
    ImageSource::from(ImageBuffer::new(width, 1).unwrap())
  }

  #[test]
  fn eviction_stays_within_byte_budget() {
    let max_bytes = 4096u64;
    let cache = Cache::with_weighter(8, max_bytes, ImageWeighter);

    for key in 0..64u64 {
      cache.insert(ImageCacheKey::source(key), CacheEntry::Source(image(1024)));
    }

    assert!(cache.weight() <= max_bytes);
  }
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use image::{Rgba, RgbaImage};

  use super::*;

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

  fn premul_at(image: &RenderedImage, x: u32, y: u32) -> [u8; 4] {
    match image {
      RenderedImage::Rasterized(buffer) => buffer.pixel(x, y),
      RenderedImage::Sampled { source, .. } => source.pixel(x, y),
    }
  }

  const GIF_COLORS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];

  /// Encodes one 4x4 solid frame per `(color index, delay ms)` pair. Delays
  /// must be multiples of 10 (GIF stores centiseconds).
  fn encoded_gif(frames: &[(usize, u32)]) -> Vec<u8> {
    use image::{Delay, Frame, codecs::gif::GifEncoder};

    let mut bytes = Vec::new();
    let mut encoder = GifEncoder::new(&mut bytes);

    encoder
      .encode_frames(frames.iter().map(|&(color, delay_ms)| {
        Frame::from_parts(
          RgbaImage::from_pixel(4, 4, Rgba(GIF_COLORS[color])),
          0,
          0,
          Delay::from_numer_denom_ms(delay_ms, 1),
        )
      }))
      .unwrap();
    drop(encoder);

    bytes
  }

  fn gif_source(frames: &[(usize, u32)]) -> GifSource {
    let Ok(ImageSource::Gif(gif)) = ImageSource::from_bytes(&encoded_gif(frames)) else {
      unreachable!("valid gif");
    };
    gif
  }

  fn expected_frame_index(durations: &[u32], time_ms: u64) -> usize {
    let total: u64 = durations.iter().map(|d| *d as u64).sum();
    if total == 0 {
      return 0;
    }

    let target_time = time_ms % total;
    let mut elapsed_ms = 0_u64;
    for (index, duration_ms) in durations.iter().enumerate() {
      elapsed_ms += *duration_ms as u64;
      if target_time < elapsed_ms {
        return index;
      }
    }

    0
  }

  #[test]
  fn to_data_url_matches_rfc4648_vectors() {
    assert_eq!(to_data_url("x", b""), "data:x;base64,");
    assert_eq!(to_data_url("x", b"f"), "data:x;base64,Zg==");
    assert_eq!(to_data_url("x", b"fo"), "data:x;base64,Zm8=");
    assert_eq!(to_data_url("x", b"foo"), "data:x;base64,Zm9v");
    assert_eq!(to_data_url("x", b"foob"), "data:x;base64,Zm9vYg==");
    assert_eq!(to_data_url("x", b"fooba"), "data:x;base64,Zm9vYmE=");
    assert_eq!(to_data_url("x", b"foobar"), "data:x;base64,Zm9vYmFy");
  }

  // usvg accepts a namespace-less root, so markup without `xmlns` is still
  // detected and rendered as SVG.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_without_xmlns_renders() -> Result<(), ImageError> {
    let svg = r##"<svg width="4" height="4"><rect width="4" height="4" fill="#ff0000"/></svg>"##;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    assert!(matches!(image, ImageSource::Svg(_)));
    let rendered = image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0)?;
    assert_eq!(premul_at(&rendered, 2, 2), [255, 0, 0, 255]);
    Ok(())
  }

  // `currentColor` in an SVG loaded as an image is not resolved against the host
  // color (matching browsers/satori); it renders as the SVG's own default, black.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_is_not_host_resolved() -> Result<(), ImageError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let rendered = image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0)?;

    assert_eq!(premul_at(&rendered, 2, 2), [0, 0, 0, 255]);
    Ok(())
  }

  #[cfg(feature = "svg")]
  #[test]
  fn svg_reuses_rasterized_pixmap() -> Result<(), ImageError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ff0000"/></svg>"##;
    let image: ImageSource = SvgSource::from_str(svg)?.into();

    let first = image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0)?;
    let second = image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0)?;

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
  fn svg_text_nodes_are_ignored_not_stripped() -> Result<(), ImageError> {
    fn rendered_data(svg: &str) -> Result<Vec<u8>, ImageError> {
      let image: ImageSource = SvgSource::from_str(svg)?.into();
      let rendered = image.render_for_layout(8, 8, ImageScalingAlgorithm::Auto, 0)?;
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
  fn svg_with_unsupported_nodes_still_parses() -> Result<(), ImageError> {
    let clip_path_text = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><clipPath id="c"><text>x</text></clipPath><rect width="8" height="8" fill="#ff0000" clip-path="url(#c)"/></svg>"##;
    let foreign_object = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><foreignObject width="8" height="8"><div xmlns="http://www.w3.org/1999/xhtml">x</div></foreignObject><rect width="8" height="8" fill="#ff0000"/></svg>"##;

    SvgSource::from_str(clip_path_text)?;
    SvgSource::from_str(foreign_object)?;
    Ok(())
  }

  #[test]
  fn bitmap_renders_sampled() -> Result<(), ImageError> {
    let mut bitmap = RgbaImage::new(2, 2);
    bitmap.put_pixel(0, 0, Rgba([12, 34, 56, 200]));
    bitmap.put_pixel(1, 0, Rgba([78, 90, 12, 255]));
    let buffer = ImageBuffer::from_rgba_bytes(bitmap.into_raw(), 2, 2).unwrap();
    let image = ImageSource::from(buffer);

    let rendered = image.render_for_layout(2, 2, ImageScalingAlgorithm::Auto, 0)?;

    assert!(matches!(rendered, RenderedImage::Sampled { .. }));
    Ok(())
  }

  #[test]
  fn bitmap_render_for_layout_keeps_sampling_parameters() -> Result<(), ImageError> {
    let mut bitmap = RgbaImage::new(2, 2);
    bitmap.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    bitmap.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
    bitmap.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
    bitmap.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
    let buffer = ImageBuffer::from_rgba_bytes(bitmap.into_raw(), 2, 2).unwrap();
    let image = ImageSource::from(buffer);

    let rendered = image.render_for_layout(4, 4, ImageScalingAlgorithm::Pixelated, 0)?;
    let RenderedImage::Sampled {
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
  fn gif_source_rejects_undecodable_stream() {
    let result = ImageSource::from_bytes(b"GIF89a\x01\x02\x03");
    assert_matches!(result, Err(_));
  }

  #[test]
  fn gif_static_render_decodes_only_first_frame() {
    let gif = gif_source(&[(0, 10), (1, 10), (2, 10)]);
    assert_eq!(gif.decoded_frame_count(), 1);

    let frame = gif.frame_at_time(0);
    assert_eq!(gif.decoded_frame_count(), 1);
    assert_eq!(frame.pixel(2, 2), GIF_COLORS[0]);
  }

  #[test]
  fn gif_source_frame_selection_matches_expected_indices() {
    let durations = [10, 20, 30];
    let gif = gif_source(&[(0, 10), (1, 20), (2, 30)]);
    let samples = [0_u64, 9, 10, 29, 30, 59, 60, 75];

    for time_ms in samples {
      let expected_color = GIF_COLORS[expected_frame_index(&durations, time_ms)];
      assert_eq!(gif.frame_at_time(time_ms).pixel(2, 2), expected_color);
    }
    assert_eq!(gif.decoded_frame_count(), 3);
  }

  #[test]
  fn gif_source_zero_delay_clamps_to_one_ms() {
    let gif = gif_source(&[(0, 0), (1, 0)]);

    assert_eq!(gif.frame_at_time(0).pixel(2, 2), GIF_COLORS[0]);
    assert_eq!(gif.frame_at_time(1).pixel(2, 2), GIF_COLORS[1]);
    assert_eq!(gif.frame_at_time(2).pixel(2, 2), GIF_COLORS[0]);
  }

  #[test]
  fn gif_source_clone_shares_decoded_state() {
    let gif = gif_source(&[(0, 10), (1, 10), (2, 10)]);
    let cloned = gif.clone();

    drop(cloned.frame_at_time(25));
    assert_eq!(gif.decoded_frame_count(), 3);
  }

  #[test]
  fn gif_frames_memoize_at_draw_size() {
    let gif = gif_source(&[(0, 10), (1, 10), (2, 10)]);

    let scaled = gif.frame_at_time_covering(15, 2, 2, ImageScalingAlgorithm::Auto);
    assert_eq!((scaled.width(), scaled.height()), (2, 2));

    let again = gif.frame_at_time_covering(15, 2, 2, ImageScalingAlgorithm::Auto);
    assert!(Arc::ptr_eq(&scaled, &again));

    let smaller = gif.frame_at_time_covering(15, 1, 1, ImageScalingAlgorithm::Auto);
    assert!(Arc::ptr_eq(&scaled, &smaller));

    let larger = gif.frame_at_time_covering(15, 4, 4, ImageScalingAlgorithm::Auto);
    assert_eq!((larger.width(), larger.height()), (4, 4));
    assert!(!Arc::ptr_eq(&scaled, &larger));
  }

  #[test]
  fn gif_first_frame_stays_native() {
    let gif = gif_source(&[(0, 10), (1, 10)]);

    let first = gif.frame_at_time_covering(0, 2, 2, ImageScalingAlgorithm::Auto);
    assert_eq!((first.width(), first.height()), (4, 4));
    assert_eq!(gif.decoded_frame_count(), 1);
  }

  #[test]
  fn gif_scaled_frame_matches_resized_native() {
    use crate::resources::image_resampler::resample_premultiplied;

    let source = gif_source(&[(0, 10), (1, 10)]);
    let (width, height) = source.dimensions();
    let native = source.frame_at_time_covering(15, width, height, ImageScalingAlgorithm::Auto);
    let scaled =
      gif_source(&[(0, 10), (1, 10)]).frame_at_time_covering(15, 2, 2, ImageScalingAlgorithm::Auto);

    let expected = resample_premultiplied(
      native.data(),
      (native.width(), native.height()),
      (2, 2),
      ImageScalingAlgorithm::Auto,
    )
    .unwrap();
    assert_eq!(scaled.data(), expected.data());
  }

  #[test]
  fn gif_dimensions_come_from_header_without_full_decode() {
    let gif = gif_source(&[(0, 10), (1, 10)]);
    assert_eq!(gif.dimensions(), (4, 4));
    assert_eq!(gif.decoded_frame_count(), 1);
  }
}

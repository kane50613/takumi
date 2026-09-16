//! Image resource management for the takumi rendering system.
//!
//! This module provides types and utilities for managing image resources,
//! including loading states, error handling, and image processing operations.

#[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
use super::animated::AnimatedFormat;
#[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
pub use super::animated::AnimatedSource;
#[cfg(feature = "svg")]
use std::borrow::Cow;
#[cfg(feature = "svg-sizing")]
use std::str::{FromStr, from_utf8};
use std::sync::{Arc, Weak};

use quick_cache::{
  DefaultHashBuilder, OptionsBuilder, Weighter,
  sync::{Cache, DefaultLifecycle, GuardResult},
};
#[cfg(feature = "svg")]
use roxmltree::{Document, ParsingOptions};
use serde::Deserialize;
use thiserror::Error;
#[cfg(feature = "svg")]
use tiny_skia::Pixmap;
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

#[cfg(not(all(feature = "png", feature = "jpeg", feature = "webp", feature = "gif")))]
use crate::resources::image_decoder::decoder_compiled_out;
#[cfg(feature = "svg-sizing")]
use crate::resources::svg_size::SvgSize;
#[cfg(feature = "svg")]
use crate::resvg::{
  apply_filters_to_layer, render as render_svg_tree,
  usvg::{Options, Transform, Tree, filters_from_markup},
};
#[cfg(feature = "svg")]
pub use crate::svg_vector::{
  SvgFill, SvgGradient, SvgGradientStop, SvgLineCap, SvgLineJoin, SvgOp, SvgPaint, SvgSpreadMethod,
  SvgStrokeStyle,
};
use crate::{
  resources::{
    font::FontsSnapshot,
    image_buffer::ImageBuffer,
    image_decoder::{bitmap_dimensions, decode_bitmap_scaled, decode_image},
  },
  style::{Color, ImageScalingAlgorithm, IntrinsicSizing, SizingContext, StyleSheet},
};

#[cfg(feature = "svg")]
const MAX_RASTER_PIXELS: u64 = 64 << 20;

#[cfg(feature = "svg")]
fn within_raster_pixel_budget(width: u32, height: u32) -> bool {
  u64::from(width) * u64::from(height) <= MAX_RASTER_PIXELS
}

/// Represents the state of an image resource.
pub(crate) type ImageResult = Result<ImageSource, ImageError>;

#[derive(Debug, Clone)]
/// Represents the source of an image.
#[non_exhaustive]
pub enum ImageSource {
  /// An svg image source
  #[cfg(feature = "svg-sizing")]
  Svg(Arc<SvgSource>),
  /// A bitmap image source
  Bitmap(Arc<ImageBuffer>),
  /// An animated image source.
  #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
  Animated(AnimatedSource),
  /// An encoded bitmap decoded lazily at the size it is drawn at.
  Encoded(Arc<EncodedBitmap>),
}

/// The resolved SVG source. Without the `svg` feature it holds the markup and its
/// root-element size only, so it lays out but cannot be drawn.
#[cfg(feature = "svg-sizing")]
#[derive(Debug)]
pub struct SvgSource {
  /// Original SVG source, for embedding directly in a vector backend.
  source: Box<str>,
  /// Canvas size and CSS intrinsic sizing read from the root element.
  sizing: SvgSize,
  /// Parsed SVG tree used for size and initial metadata.
  #[cfg(feature = "svg")]
  pub(crate) tree: crate::resvg::usvg::Tree,
  /// Whether rendering depends on the host `color`: the markup references
  /// `currentColor` and the root element sets no `color` of its own.
  #[cfg(feature = "svg")]
  uses_current_color: bool,
  /// Whether the markup contains `<text`, so rendering re-parses with fonts.
  #[cfg(feature = "svg")]
  has_text: bool,
  /// Text-capable re-parse of `source`, keyed by the font registry revision
  /// it was converted with; a registration re-converts on the next render.
  #[cfg(feature = "svg")]
  text_tree: std::sync::Mutex<Option<(u64, Arc<crate::resvg::usvg::Tree>)>>,
  #[cfg(feature = "svg")]
  hash: u64,
  #[cfg(feature = "svg")]
  cache: Weak<SharedResourceCache>,
}

#[cfg(feature = "svg-sizing")]
impl SvgSource {
  /// The SVG canvas dimensions in pixels, from the root `width`/`height` or
  /// `viewBox`.
  #[cfg(feature = "svg")]
  pub fn dimensions(&self) -> (f32, f32) {
    let size = self.tree.size();
    (size.width(), size.height())
  }

  /// The SVG canvas dimensions in pixels, from the root `width`/`height` or
  /// `viewBox`.
  #[cfg(not(feature = "svg"))]
  pub fn dimensions(&self) -> (f32, f32) {
    (self.sizing.width, self.sizing.height)
  }

  /// The original SVG markup, for embedding directly in a vector backend.
  pub fn source(&self) -> &str {
    &self.source
  }
}

#[cfg(all(feature = "svg-sizing", not(feature = "svg")))]
impl SvgSource {
  fn parse(src: &str, _hash: u64, _cache: Weak<SharedResourceCache>) -> Result<Self, ImageError> {
    Ok(SvgSource {
      source: Box::from(src),
      sizing: SvgSize::parse(src).map_err(ImageError::svg_parse)?,
    })
  }
}

#[cfg(feature = "svg")]
impl SvgSource {
  /// Markup for embedding in a vector backend, with the host `color` injected
  /// as a root presentation attribute when `currentColor` depends on it.
  pub fn source_with_current_color(&self, current_color: Color) -> Cow<'_, str> {
    if !self.uses_current_color {
      return Cow::Borrowed(&self.source);
    }

    let Some(tag_start) = self.source.find("<svg") else {
      return Cow::Borrowed(&self.source);
    };

    let insert_at = tag_start + "<svg".len();

    if !matches!(
      self.source[insert_at..].chars().next(),
      Some(c) if c == '>' || c == '/' || c.is_whitespace()
    ) {
      return Cow::Borrowed(&self.source);
    }

    let [red, green, blue, alpha] = current_color.0;
    let mut markup = String::with_capacity(self.source.len() + 32);

    markup.push_str(&self.source[..insert_at]);
    markup.push_str(&format!(
      " color=\"#{red:02x}{green:02x}{blue:02x}{alpha:02x}\""
    ));
    markup.push_str(&self.source[insert_at..]);
    Cow::Owned(markup)
  }

  /// Flattens the SVG into backend-agnostic vector drawing ops in SVG canvas
  /// coordinates. `raster_scale` is the device-pixels-per-user-unit factor
  /// used when a subtree (filters, embedded bitmaps) has to fall back to
  /// rasterization.
  pub fn vector_ops(
    &self,
    raster_scale: f32,
    current_color: Color,
    fonts: Option<&FontsSnapshot>,
  ) -> Vec<SvgOp> {
    match self.tree_with_current_color(current_color, fonts) {
      Some(tree) => crate::svg_vector::flatten(&tree, raster_scale),
      None => {
        let text_tree = self.text_tree(fonts);

        crate::svg_vector::flatten(text_tree.as_deref().unwrap_or(&self.tree), raster_scale)
      }
    }
  }

  /// Re-parses the markup with `configure` applied to the parse options.
  fn reparse(
    &self,
    fonts: Option<&FontsSnapshot>,
    configure: impl FnOnce(&mut Options),
  ) -> Option<crate::resvg::usvg::Tree> {
    let parsing = ParsingOptions {
      allow_dtd: true,
      ..Default::default()
    };
    let document = Document::parse_with_options(&self.source, parsing).ok()?;
    let mut options = svg_parse_options();

    if let Some(fonts) = fonts.filter(|_| self.has_text) {
      options.fontdb = fonts.svg_fontdb();
    }

    configure(&mut options);
    Tree::from_xmltree(&document, &options).ok()
  }

  /// Re-parses the markup with `current_color` as the `currentColor` fallback.
  /// `None` when rendering does not depend on the host color.
  fn tree_with_current_color(
    &self,
    current_color: Color,
    fonts: Option<&FontsSnapshot>,
  ) -> Option<crate::resvg::usvg::Tree> {
    if !self.uses_current_color {
      return None;
    }

    let [red, green, blue, alpha] = current_color.0;

    self.reparse(fonts, |options| {
      options.current_color = Some(svgtypes::Color::new_rgba(red, green, blue, alpha));
    })
  }
}

#[cfg(feature = "svg-sizing")]
impl From<SvgSource> for ImageSource {
  fn from(svg: SvgSource) -> Self {
    ImageSource::Svg(Arc::new(svg))
  }
}

/// An encoded bitmap (PNG/JPEG/WebP) that decodes lazily at draw time, scaled
/// down to the box it is drawn into. Decoded results are stored in the owning
/// [`ResourceCache`] keyed by content and target size, so a source drawn at a
/// stable size decodes once while the retained bytes track the draw size, not
/// the source size.
#[derive(Debug)]
pub struct EncodedBitmap {
  bytes: Box<[u8]>,
  width: u32,
  height: u32,
  hash: u64,
  cache: Weak<SharedResourceCache>,
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

    let key = ResourceCacheKey::sized(self.hash, width, height, algorithm);

    match cache.get_value_or_guard(&key, None) {
      GuardResult::Value(CacheEntry::Sized(buffer)) => Ok(buffer),
      GuardResult::Value(_) => self.decode_uncached(width, height, algorithm),
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

/// Parse options for untrusted SVG markup: the string href resolver is
/// disabled so `<image>`/`<feImage href>` cannot read local files. `data:`
/// URIs still resolve through the default data resolver.
#[cfg(feature = "svg")]
fn svg_parse_options() -> Options<'static> {
  let mut options = Options::default();
  options.image_href_resolver.resolve_string = Box::new(|_, _| None);
  options
}

#[cfg(feature = "svg")]
impl SvgSource {
  /// Parses SVG markup; rasterized pixmaps go into `cache` while it is alive,
  /// keyed by content hash and target size. A dead handle rasterizes per call.
  fn parse(src: &str, hash: u64, cache: Weak<SharedResourceCache>) -> Result<Self, ImageError> {
    // One parse, shared with usvg via `from_xmltree` (what `from_str` does
    // internally). No text stripping: usvg drops `<text>`/`<tspan>` with its
    // `text` feature off.
    let options = ParsingOptions {
      allow_dtd: true,
      ..Default::default()
    };
    let document = Document::parse_with_options(src, options).map_err(ImageError::svg_parse)?;

    let options = svg_parse_options();
    let tree = Tree::from_xmltree(&document, &options).map_err(ImageError::svg_parse)?;
    let sizing = SvgSize::from_root(document.root_element()).map_err(ImageError::svg_parse)?;
    // Set during parsing whenever a `currentColor` finds no `color` attribute
    // on its ancestors, so it also catches entity-encoded values a source-text
    // scan would miss.
    let uses_current_color = options
      .current_color_used
      .load(std::sync::atomic::Ordering::Relaxed);

    Ok(SvgSource {
      has_text: document
        .descendants()
        .any(|node| node.tag_name().name() == "text"),
      source: Box::from(src),
      sizing,
      tree,
      uses_current_color,
      text_tree: std::sync::Mutex::new(None),
      hash,
      cache,
    })
  }

  /// The text-capable re-parse for the snapshot's font revision when the
  /// markup holds `<text>`, or `None` to use the parse-time tree.
  fn text_tree(&self, fonts: Option<&FontsSnapshot>) -> Option<Arc<crate::resvg::usvg::Tree>> {
    let fonts = fonts.filter(|_| self.has_text)?;
    let revision = fonts.revision();
    let mut cached = self.text_tree.lock().ok()?;

    if let Some((cached_revision, tree)) = cached.as_ref()
      && *cached_revision == revision
    {
      return Some(tree.clone());
    }

    let tree = Arc::new(self.reparse(Some(fonts), |_| {})?);

    *cached = Some((revision, tree.clone()));
    Some(tree)
  }

  fn rasterize(
    &self,
    width: u32,
    height: u32,
    current_color: Color,
    fonts: Option<&FontsSnapshot>,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    if !within_raster_pixel_budget(width, height) {
      return Err(ImageError::InvalidPixmapSize);
    }

    let mut pixmap = Pixmap::new(width, height).ok_or(ImageError::InvalidPixmapSize)?;

    let original_size = self.tree.size();
    let sx = width as f32 / original_size.width();
    let sy = height as f32 / original_size.height();

    let recolored = self.tree_with_current_color(current_color, fonts);
    let text_tree = recolored.is_none().then(|| self.text_tree(fonts)).flatten();

    render_svg_tree(
      recolored
        .as_ref()
        .or(text_tree.as_deref())
        .unwrap_or(&self.tree),
      Transform::from_scale(sx, sy),
      &mut pixmap.as_mut(),
    );

    ImageBuffer::from_premultiplied_rgba(pixmap.data().to_vec(), width, height)
      .map(Arc::new)
      .ok_or(ImageError::InvalidPixmapSize)
  }

  fn rasterize_cached(
    &self,
    width: u32,
    height: u32,
    image_rendering: ImageScalingAlgorithm,
    current_color: Color,
    fonts: Option<&FontsSnapshot>,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    let Some(cache) = self.cache.upgrade() else {
      return self.rasterize(width, height, current_color, fonts);
    };

    let mut hash = if self.uses_current_color {
      self.hash ^ xxh3_64(&current_color.0)
    } else {
      self.hash
    };

    // Text rasterization depends on which fonts are registered.
    if self.has_text
      && let Some(fonts) = fonts
    {
      hash ^= xxh3_64(&fonts.revision().to_le_bytes());
    }

    let key = ResourceCacheKey::sized(hash, width, height, image_rendering);

    match cache.get_value_or_guard(&key, None) {
      GuardResult::Value(CacheEntry::Sized(buffer)) => Ok(buffer),
      GuardResult::Value(_) => self.rasterize(width, height, current_color, fonts),
      GuardResult::Guard(guard) => {
        let buffer = self.rasterize(width, height, current_color, fonts)?;
        let _ = guard.insert(CacheEntry::Sized(buffer.clone()));
        Ok(buffer)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => self.rasterize(width, height, current_color, fonts),
    }
  }
}

#[cfg(feature = "svg-sizing")]
impl FromStr for SvgSource {
  type Err = ImageError;

  fn from_str(src: &str) -> Result<Self, Self::Err> {
    Self::parse(src, xxh3_64(src.as_bytes()), Weak::new())
  }
}

impl ImageSource {
  /// Approximate retained size in bytes, used for cache budgeting.
  pub(crate) fn estimated_bytes(&self) -> usize {
    match self {
      Self::Bitmap(buffer) => buffer.data().len(),
      #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
      Self::Animated(animated) => animated.decoded_bytes(),
      Self::Encoded(encoded) => encoded.bytes.len(),
      // Markup plus a parsed-tree estimate; rasterized pixmaps are weighted
      // separately as their own sized entries.
      #[cfg(feature = "svg-sizing")]
      Self::Svg(svg) => svg.source.len() * 3,
    }
  }

  /// Load an image source from raw bytes.
  ///
  /// - When the `svg` feature is enabled and the bytes look like SVG XML, they
  ///   are parsed as an SVG using `resvg::usvg`.
  /// - Otherwise, the bytes are decoded as a raster image.
  pub fn from_bytes(bytes: &[u8]) -> ImageResult {
    #[cfg(feature = "svg-sizing")]
    {
      if let Ok(text) = from_utf8(bytes)
        && is_svg_like(text)
      {
        return Ok(ImageSource::Svg(Arc::new(text.parse()?)));
      }
    }

    #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
    if let Some(format) = AnimatedFormat::detect(bytes) {
      return Ok(ImageSource::Animated(AnimatedSource::from_bytes(
        format, bytes,
      )?));
    }

    match decode_image(bytes) {
      Ok(buffer) => Ok(ImageSource::Bitmap(Arc::new(buffer))),
      #[cfg(all(feature = "png", feature = "jpeg", feature = "webp", feature = "gif"))]
      Err(error) => Err(ImageError::decode(error)),
      #[cfg(not(all(feature = "png", feature = "jpeg", feature = "webp", feature = "gif")))]
      Err(error) => match bitmap_dimensions(bytes).filter(|_| decoder_compiled_out(bytes)) {
        Some(Ok(dimensions)) => Ok(Self::encoded(bytes, dimensions, 0, Weak::new())),
        _ => Err(ImageError::decode(error)),
      },
    }
  }

  /// [`from_bytes`](Self::from_bytes), but bitmaps stay encoded and decode at
  /// draw size, and SVG rasters go into `cache`. Sized entries go into `cache`
  /// while it is alive; a dead handle (inline node bytes, data URIs) decodes
  /// per render.
  pub(crate) fn from_bytes_lazy(
    bytes: &[u8],
    hash: u64,
    cache: Weak<SharedResourceCache>,
  ) -> ImageResult {
    #[cfg(feature = "svg-sizing")]
    {
      if let Ok(text) = from_utf8(bytes)
        && is_svg_like(text)
      {
        return Ok(ImageSource::Svg(Arc::new(SvgSource::parse(
          text, hash, cache,
        )?)));
      }
    }

    #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
    if let Some(format) = AnimatedFormat::detect(bytes) {
      return Ok(ImageSource::Animated(AnimatedSource::from_bytes(
        format, bytes,
      )?));
    }

    match bitmap_dimensions(bytes) {
      Some(Ok((width, height))) => Ok(Self::encoded(bytes, (width, height), hash, cache)),
      Some(Err(error)) => Err(ImageError::decode(error)),
      None => Self::from_bytes(bytes),
    }
  }

  /// A bitmap kept in the bytes it arrived in.
  fn encoded(
    bytes: &[u8],
    (width, height): (u32, u32),
    hash: u64,
    cache: Weak<SharedResourceCache>,
  ) -> Self {
    ImageSource::Encoded(Arc::new(EncodedBitmap {
      bytes: bytes.into(),
      width,
      height,
      hash,
      cache,
    }))
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
    #[cfg_attr(
      not(any(feature = "png", feature = "gif", feature = "webp")),
      allow(unused_variables)
    )]
    time_ms: u64,
    #[cfg_attr(not(feature = "svg"), allow(unused_variables))] current_color: Color,
    #[cfg_attr(not(feature = "svg"), allow(unused_variables))] fonts: Option<&FontsSnapshot>,
  ) -> Result<RenderedImage, ImageError> {
    match self {
      ImageSource::Bitmap(bitmap) => Ok(RenderedImage::Sampled {
        source: bitmap.clone(),
        width,
        height,
        algorithm: image_rendering,
        source_scale: (1.0, 1.0),
      }),
      #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
      ImageSource::Animated(animated) => {
        let source = animated.frame_at_time_covering(time_ms, width, height, image_rendering);
        let (native_width, native_height) = animated.dimensions();
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
      ImageSource::Svg(svg) => Ok(RenderedImage::Rasterized(svg.rasterize_cached(
        width,
        height,
        image_rendering,
        current_color,
        fonts,
      )?)),
      #[cfg(all(feature = "svg-sizing", not(feature = "svg")))]
      ImageSource::Svg(_) => Err(ImageError::SvgParseNotSupported),
    }
  }

  /// Get the image size in device pixels for the current sizing context.
  pub fn size(&self, sizing: &SizingContext) -> (f32, f32) {
    let (width, height) = match self {
      #[cfg(feature = "svg-sizing")]
      ImageSource::Svg(svg) => svg.dimensions(),
      ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
      #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
      ImageSource::Animated(animated) => {
        let (width, height) = animated.dimensions();
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
      #[cfg(feature = "svg-sizing")]
      ImageSource::Svg(svg) => svg.sizing.intrinsic,
      ImageSource::Bitmap(bitmap) => {
        IntrinsicSizing::from_dimensions(bitmap.width() as f32, bitmap.height() as f32)
      }
      #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
      ImageSource::Animated(animated) => {
        let (width, height) = animated.dimensions();
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
pub(crate) fn cover_target(
  (native_w, native_h): (u32, u32),
  (box_w, box_h): (u32, u32),
) -> (u32, u32) {
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
  #[cfg_attr(not(feature = "svg"), allow(dead_code))]
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
  let filters = filters_from_markup(
    markup,
    filter_id,
    width as f32,
    height as f32,
    &svg_parse_options(),
  )
  .map_err(ImageError::svg_parse)?;

  let Some(filters) = filters else {
    // An invalid filter reference hides the element.
    layer.fill(0);
    return Ok(());
  };

  apply_filters_to_layer(&filters, layer, width, height).ok_or(ImageError::InvalidPixmapSize)
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
  #[cfg(feature = "svg-sizing")]
  /// An error occurred while parsing an SVG image
  #[error("An error occurred while parsing an SVG image: {0}")]
  SvgParseError(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  /// SVG rendering is not supported in this build
  #[cfg(not(feature = "svg"))]
  #[error("SVG rendering is not supported in this build")]
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
  /// An animated image decoded to no frames at all.
  #[error("The animated image does not contain any decodable frames")]
  InvalidAnimation,
}

impl ImageError {
  /// Wraps a decoder error opaquely so takumi's public API stays independent of
  /// the `image` crate's version.
  pub(crate) fn decode(err: impl std::error::Error + Send + Sync + 'static) -> Self {
    Self::DecodeError(Box::new(err))
  }

  /// Wraps an SVG parse error opaquely so takumi's public API stays independent
  /// of the `resvg`/`usvg` version.
  #[cfg(feature = "svg-sizing")]
  pub(crate) fn svg_parse(err: impl std::error::Error + Send + Sync + 'static) -> Self {
    Self::SvgParseError(Box::new(err))
  }
}

/// Resource budget before entries start getting evicted. Deliberately
/// conservative: a single-template server's working set fits comfortably, and
/// heavier workloads raise it through [`ResourceCache::new`].
const DEFAULT_MAX_BYTES: u64 = 64 << 20;

/// Bytes each cache shard holds. An entry over 97% of its shard is never
/// admitted, so a shard has to be large enough for one decoded photo.
/// `quick_cache` rounds the shard count up to a power of two, so the count is
/// rounded down first to keep this floor.
const SHARD_BYTES: u64 = 64 << 20;

/// Cache policy for a decoded image, applied per [`ResourceCache::get_or_decode`] call.
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
/// and filter address a decoded-at-size entry; a stylesheet hash addresses a
/// parsed sheet. `kind` keeps the hash domains apart.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ResourceCacheKey {
  hash: u64,
  width: u32,
  height: u32,
  algorithm: u8,
  kind: u8,
}

const KIND_SOURCE: u8 = 0;
const KIND_SIZED: u8 = 1;
const KIND_STYLESHEET: u8 = 2;

impl ResourceCacheKey {
  fn source(hash: u64) -> Self {
    Self {
      hash,
      width: 0,
      height: 0,
      algorithm: 0,
      kind: KIND_SOURCE,
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
      kind: KIND_SIZED,
    }
  }

  fn stylesheet(hash: u64) -> Self {
    Self {
      hash,
      width: 0,
      height: 0,
      algorithm: 0,
      kind: KIND_STYLESHEET,
    }
  }
}

#[derive(Clone)]
pub(crate) enum CacheEntry {
  Source(ImageSource),
  Sized(Arc<ImageBuffer>),
  Stylesheet { sheet: Arc<StyleSheet>, weight: u32 },
}

#[derive(Clone)]
pub(crate) struct ResourceWeighter;

impl Weighter<ResourceCacheKey, CacheEntry> for ResourceWeighter {
  fn weight(&self, _key: &ResourceCacheKey, entry: &CacheEntry) -> u64 {
    let bytes = match entry {
      CacheEntry::Source(source) => source.estimated_bytes(),
      CacheEntry::Sized(buffer) => buffer.data().len(),
      CacheEntry::Stylesheet { weight, .. } => *weight as usize,
    };
    (bytes as u64).max(1)
  }
}

pub(crate) type SharedResourceCache = Cache<ResourceCacheKey, CacheEntry, ResourceWeighter>;

/// Content-addressed store of decoded render resources — images, SVG rasters,
/// parsed stylesheets — sharing one byte budget, used by the renderer to avoid
/// re-decoding and re-parsing.
pub struct ResourceCache {
  cache: Arc<SharedResourceCache>,
}

impl Default for ResourceCache {
  fn default() -> Self {
    Self::new(DEFAULT_MAX_BYTES)
  }
}

impl ResourceCache {
  /// Creates a cache holding at most `max_bytes` across every entry kind.
  /// `0` disables retention: lookups miss and nothing is kept.
  pub fn new(max_bytes: u64) -> Self {
    // ~64 KiB average decoded image ⇒ a reasonable item-count hint for the budget.
    let estimated_items = (max_bytes / (64 << 10)).max(1) as usize;
    let parallelism = std::thread::available_parallelism().map_or(1, |n| n.get() as u64);
    let shards = (max_bytes / SHARD_BYTES).clamp(1, parallelism);
    let shards = if shards.is_power_of_two() {
      shards
    } else {
      shards.next_power_of_two() / 2
    } as usize;
    let options = OptionsBuilder::new()
      .estimated_items_capacity(estimated_items)
      .weight_capacity(max_bytes)
      .shards(shards)
      .build();
    let cache = match options {
      Ok(options) => Cache::with_options(
        options,
        ResourceWeighter,
        DefaultHashBuilder::default(),
        DefaultLifecycle::default(),
      ),
      Err(_) => Cache::with_weighter(estimated_items, max_bytes, ResourceWeighter),
    };

    Self {
      cache: Arc::new(cache),
    }
  }

  /// Returns the decoded image for `bytes`, decoding on a miss and caching it unless `mode`
  /// is [`ImageCacheMode::None`].
  ///
  /// When caching, concurrent misses for the same bytes are single-flighted: one thread
  /// decodes while the others wait, so each unique image is decoded once.
  pub fn get_or_decode(&self, bytes: &[u8], mode: ImageCacheMode) -> ImageResult {
    let hash = xxh3_64(bytes);
    let key = ResourceCacheKey::source(hash);

    if matches!(mode, ImageCacheMode::None) {
      return match self.cache.get(&key) {
        Some(CacheEntry::Source(source)) => Ok(source),
        _ => ImageSource::from_bytes(bytes),
      };
    }

    match self.cache.get_value_or_guard(&key, None) {
      GuardResult::Value(CacheEntry::Source(source)) => Ok(source),
      GuardResult::Value(_) => ImageSource::from_bytes(bytes),
      GuardResult::Guard(guard) => {
        let source = ImageSource::from_bytes_lazy(bytes, hash, Arc::downgrade(&self.cache))?;
        let _ = guard.insert(CacheEntry::Source(source.clone()));
        Ok(source)
      }
      // `None` timeout never times out.
      GuardResult::Timeout => ImageSource::from_bytes(bytes),
    }
  }

  /// Returns the parsed sheet for `sources`, parsing on a miss. Keyed by the
  /// source text, so a server re-sending the same CSS parses it once.
  pub fn get_or_parse_stylesheet(&self, sources: Vec<String>) -> Arc<StyleSheet> {
    let mut hasher = Xxh3::new();
    for source in &sources {
      hasher.update(source.as_bytes());
      hasher.update(&(source.len() as u64).to_le_bytes());
    }
    let key = ResourceCacheKey::stylesheet(hasher.digest());

    // Parsed rules retain roughly this much beyond the source text.
    let weight = sources
      .iter()
      .map(String::len)
      .sum::<usize>()
      .saturating_mul(3)
      .min(u32::MAX as usize) as u32;

    match self.cache.get_value_or_guard(&key, None) {
      GuardResult::Value(CacheEntry::Stylesheet { sheet, .. }) => sheet,
      GuardResult::Value(_) => Arc::new(StyleSheet::parse_owned_list_loosy(sources)),
      GuardResult::Guard(guard) => {
        let sheet = Arc::new(StyleSheet::parse_owned_list_loosy(sources));
        let _ = guard.insert(CacheEntry::Stylesheet {
          sheet: sheet.clone(),
          weight,
        });
        sheet
      }
      // `None` timeout never times out.
      GuardResult::Timeout => Arc::new(StyleSheet::parse_owned_list_loosy(sources)),
    }
  }
}

#[cfg(all(test, feature = "png"))]
mod resource_cache_tests {
  use std::sync::Arc;

  use quick_cache::sync::Cache;

  use super::{
    CacheEntry, ImageCacheMode, RenderedImage, ResourceCache, ResourceCacheKey, ResourceWeighter,
  };
  use crate::{
    resources::{image::ImageSource, image_buffer::ImageBuffer},
    style::{Color, ImageScalingAlgorithm},
  };

  /// PNG bytes that decode to a tiny bitmap (cacheable).
  fn png_bytes() -> Vec<u8> {
    ImageBuffer::new(2, 2).unwrap().encode_png().unwrap()
  }

  /// A PNG whose header parses but whose pixels do not fails at load. Only a
  /// format this build has no decoder for is allowed to stay encoded.
  #[test]
  fn a_corrupt_png_fails_to_load() {
    let mut bytes = png_bytes();
    let tail = bytes.len() - 16;

    bytes[tail..].fill(0);

    assert!(ImageSource::from_bytes(&bytes).is_err());
  }

  #[test]
  fn decodes_and_reuses_on_hit() {
    let cache = ResourceCache::default();
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
      .render_for_layout(
        width,
        height,
        ImageScalingAlgorithm::Auto,
        0,
        Color::black(),
        None,
      )
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
    let cache = ResourceCache::default();
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
    let cache = ResourceCache::default();
    let bytes = sized_png_bytes(64, 64);
    let source = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    let (buffer, scale) = rendered_buffer(&source, 32, 16);

    assert_eq!((buffer.width(), buffer.height()), (32, 32));
    assert_eq!(scale, (0.5, 0.5));
  }

  #[test]
  fn encoded_never_upscales() {
    let cache = ResourceCache::default();
    let bytes = sized_png_bytes(8, 8);
    let source = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    let (buffer, scale) = rendered_buffer(&source, 32, 32);

    assert_eq!((buffer.width(), buffer.height()), (8, 8));
    assert_eq!(scale, (1.0, 1.0));
  }

  #[test]
  fn store_false_does_not_populate_cache() {
    let cache = ResourceCache::default();
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
  fn svg_source_and_raster_are_cached() {
    let cache = ResourceCache::default();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#;

    let a = cache.get_or_decode(svg, ImageCacheMode::Auto).unwrap();
    let b = cache.get_or_decode(svg, ImageCacheMode::Auto).unwrap();

    match (&a, &b) {
      (ImageSource::Svg(x), ImageSource::Svg(y)) => {
        assert!(std::sync::Arc::ptr_eq(x, y))
      }
      _ => panic!("expected svgs"),
    }

    let (first, second) = (rendered_raster(&a, 4, 4), rendered_raster(&b, 4, 4));
    assert!(std::sync::Arc::ptr_eq(&first, &second));
  }

  #[cfg(feature = "svg")]
  fn rendered_raster(source: &ImageSource, width: u32, height: u32) -> std::sync::Arc<ImageBuffer> {
    match source
      .render_for_layout(
        width,
        height,
        ImageScalingAlgorithm::Auto,
        0,
        Color::black(),
        None,
      )
      .unwrap()
    {
      RenderedImage::Rasterized(buffer) => buffer,
      _ => panic!("expected rasterized"),
    }
  }

  #[test]
  fn stylesheet_is_parsed_once() {
    let cache = ResourceCache::default();
    let sources = vec![".a { color: red; }".to_string()];

    let first = cache.get_or_parse_stylesheet(sources.clone());
    let second = cache.get_or_parse_stylesheet(sources);

    assert!(std::sync::Arc::ptr_eq(&first, &second));
  }

  #[test]
  fn a_sized_entry_near_the_budget_is_retained() {
    let cache = ResourceCache::new(16 << 20);
    let key = ResourceCacheKey::sized(1, 2400, 1601, ImageScalingAlgorithm::Auto);
    let buffer = Arc::new(ImageBuffer::new(2400, 1601).unwrap());

    cache.cache.insert(key, CacheEntry::Sized(buffer));

    assert!(cache.cache.get(&key).is_some());
  }

  #[test]
  fn an_uneven_budget_keeps_a_whole_shard_per_entry() {
    let cache = ResourceCache::new(192 << 20);
    let key = ResourceCacheKey::sized(1, 4000, 3900, ImageScalingAlgorithm::Auto);
    let buffer = Arc::new(ImageBuffer::new(4000, 3900).unwrap());

    cache.cache.insert(key, CacheEntry::Sized(buffer));

    assert!(cache.cache.get(&key).is_some());
  }

  #[test]
  fn zero_budget_disables_retention() {
    let cache = ResourceCache::new(0);
    let bytes = png_bytes();

    let a = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();
    let b = cache.get_or_decode(&bytes, ImageCacheMode::Auto).unwrap();

    match (&a, &b) {
      (ImageSource::Encoded(x), ImageSource::Encoded(y)) => {
        assert!(!std::sync::Arc::ptr_eq(x, y))
      }
      _ => panic!("expected encoded bitmaps"),
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
    let cache = Cache::with_weighter(8, max_bytes, ResourceWeighter);

    for key in 0..64u64 {
      cache.insert(
        ResourceCacheKey::source(key),
        CacheEntry::Source(image(1024)),
      );
    }

    assert!(cache.weight() <= max_bytes);
  }
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use image::{Rgba, RgbaImage};

  #[cfg(feature = "png")]
  use crate::resources::animated::AnimatedFormat;
  #[cfg(any(feature = "png", feature = "webp"))]
  use crate::resources::image_decoder::DecodeTarget;

  use super::*;

  /// `SvgSize` reads the root element only; usvg's size and the intrinsic
  /// rules the backends used before must agree with it on every SVG in the repo.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_size_matches_usvg_across_the_corpus() {
    use std::{
      fs,
      path::{Path, PathBuf},
    };

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
      for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
          walk(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "svg") {
          out.push(path);
        }
      }
    }

    fn previous_intrinsic(root: roxmltree::Node, size: (f32, f32)) -> IntrinsicSizing {
      let is_absolute = |name| {
        root
          .attribute(name)
          .map(str::trim)
          .is_some_and(|value| !value.is_empty() && !value.ends_with('%'))
      };
      let width = is_absolute("width").then_some(size.0);
      let height = is_absolute("height").then_some(size.1);
      let ratio = match (width, height) {
        (Some(width), Some(height)) if width != 0.0 && height != 0.0 => Some(width / height),
        _ => root.attribute("viewBox").and_then(|view_box| {
          let mut numbers = view_box
            .split([' ', ',', '\t', '\n', '\r'])
            .filter(|part| !part.is_empty());
          let width: f32 = numbers.nth(2)?.parse().ok()?;
          let height: f32 = numbers.next()?.parse().ok()?;
          (width > 0.0 && height > 0.0).then_some(width / height)
        }),
      };
      IntrinsicSizing {
        width,
        height,
        ratio,
      }
    }

    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    walk(&repo.join("assets"), &mut files);
    walk(&repo.join("takumi/tests"), &mut files);
    assert!(files.len() > 100, "corpus too small: {}", files.len());

    let mut compared = 0;
    for path in files {
      let Ok(markup) = fs::read_to_string(&path) else {
        continue;
      };
      let Ok(source) = markup.parse::<SvgSource>() else {
        continue;
      };
      let size =
        SvgSize::parse(&markup).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
      let document = Document::parse_with_options(
        &markup,
        ParsingOptions {
          allow_dtd: true,
          ..Default::default()
        },
      )
      .unwrap();
      let root = document.root_element();
      let usvg_size = source.dimensions();

      let percent = |name| {
        root
          .attribute(name)
          .is_some_and(|value| value.trim_end().ends_with('%'))
      };
      if root.attribute("viewBox").is_some() || !(percent("width") || percent("height")) {
        let close = |a: f32, b: f32| (a - b).abs() <= 1e-3 * b.abs().max(1.0);
        assert!(
          close(size.width, usvg_size.0) && close(size.height, usvg_size.1),
          "{}: {:?} vs usvg {:?}",
          path.display(),
          (size.width, size.height),
          usvg_size
        );
      }
      assert_eq!(
        size.intrinsic,
        previous_intrinsic(root, usvg_size),
        "{}",
        path.display()
      );
      compared += 1;
    }
    assert!(compared > 100, "compared only {compared}");
  }

  /// `width`/`height` attributes give intrinsic dimensions; a `viewBox` alone
  /// gives only an aspect ratio (per the SVG/CSS intrinsic sizing rules).
  #[cfg(feature = "svg")]
  #[test]
  fn svg_intrinsic_distinguishes_viewbox_from_dimensions() {
    fn intrinsic(svg: String) -> IntrinsicSizing {
      let Ok(source) = svg.parse::<SvgSource>() else {
        unreachable!("valid svg");
      };
      source.sizing.intrinsic
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

  #[cfg(feature = "svg")]
  fn premul_at(image: &RenderedImage, x: u32, y: u32) -> [u8; 4] {
    match image {
      RenderedImage::Rasterized(buffer) => buffer.pixel(x, y),
      RenderedImage::Sampled { source, .. } => source.pixel(x, y),
    }
  }

  #[cfg(feature = "png")]
  const HALF_TRANSPARENT_BLUE: [u8; 4] = [0, 0, 255, 128];
  #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
  const FRAME_COLORS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];

  /// Encodes one 4x4 solid frame per `(color index, delay ms)` pair. Delays
  /// must be multiples of 10 (GIF stores centiseconds).
  #[cfg(feature = "gif")]
  fn encoded_gif(frames: &[(usize, u32)]) -> Vec<u8> {
    use image::{Delay, Frame, codecs::gif::GifEncoder};

    let mut bytes = Vec::new();
    let mut encoder = GifEncoder::new(&mut bytes);

    encoder
      .encode_frames(frames.iter().map(|&(color, delay_ms)| {
        Frame::from_parts(
          RgbaImage::from_pixel(4, 4, Rgba(FRAME_COLORS[color])),
          0,
          0,
          Delay::from_numer_denom_ms(delay_ms, 1),
        )
      }))
      .unwrap();
    drop(encoder);

    bytes
  }

  #[cfg(feature = "png")]
  /// One 4x4 solid frame per `(color index, delay ms)` pair, as an APNG.
  fn encoded_apng(frames: &[(usize, u32)]) -> Vec<u8> {
    use png::{BitDepth, ColorType, Encoder};

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 4, 4);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_animated(frames.len() as u32, 0).unwrap();

    let mut writer = encoder.write_header().unwrap();
    for &(color, delay_ms) in frames {
      writer
        .set_frame_delay(delay_ms.try_into().unwrap(), 1000)
        .unwrap();
      writer
        .write_image_data(&FRAME_COLORS[color].repeat(16))
        .unwrap();
    }
    writer.finish().unwrap();

    bytes
  }

  #[cfg(feature = "png")]
  fn apng_source(frames: &[(usize, u32)]) -> AnimatedSource {
    let Ok(ImageSource::Animated(animated)) = ImageSource::from_bytes(&encoded_apng(frames)) else {
      unreachable!("valid apng");
    };
    animated
  }

  #[cfg(feature = "png")]
  #[test]
  fn apng_walks_its_frame_timeline() {
    let apng = apng_source(&[(0, 100), (1, 100), (2, 100)]);

    assert_eq!(apng.dimensions(), (4, 4));
    assert_eq!(apng.frame_at_time(0).pixel(2, 2), FRAME_COLORS[0]);
    assert_eq!(apng.frame_at_time(150).pixel(2, 2), FRAME_COLORS[1]);
    assert_eq!(apng.frame_at_time(250).pixel(2, 2), FRAME_COLORS[2]);
    assert_eq!(apng.frame_at_time(350).pixel(2, 2), FRAME_COLORS[0]);
  }

  #[cfg(feature = "png")]
  #[test]
  fn apng_frames_cover_the_draw_box() {
    let apng = apng_source(&[(0, 100), (1, 100)]);
    let smaller = apng.frame_at_time_covering(150, 2, 2, ImageScalingAlgorithm::Auto);

    assert_eq!((smaller.width(), smaller.height()), (2, 2));
    assert_eq!(smaller.pixel(1, 1), FRAME_COLORS[1]);
  }

  #[cfg(feature = "png")]
  #[test]
  fn apng_composites_a_blended_subframe() {
    use png::{BitDepth, BlendOp, ColorType, DisposeOp, Encoder};

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 4, 4);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_animated(2, 0).unwrap();

    let mut writer = encoder.write_header().unwrap();
    writer.set_frame_delay(100, 1000).unwrap();
    writer.set_dispose_op(DisposeOp::None).unwrap();
    writer
      .write_image_data(&FRAME_COLORS[0].repeat(16))
      .unwrap();

    writer.set_frame_delay(100, 1000).unwrap();
    writer.set_frame_dimension(2, 2).unwrap();
    writer.set_frame_position(0, 0).unwrap();
    writer.set_blend_op(BlendOp::Over).unwrap();
    writer
      .write_image_data(&HALF_TRANSPARENT_BLUE.repeat(4))
      .unwrap();
    writer.finish().unwrap();

    let Ok(ImageSource::Animated(apng)) = ImageSource::from_bytes(&bytes) else {
      unreachable!("valid apng");
    };

    assert_eq!(apng.frame_at_time(150).pixel(0, 0), [127, 0, 128, 255]);
    assert_eq!(apng.frame_at_time(150).pixel(3, 3), FRAME_COLORS[0]);
  }

  #[cfg(feature = "png")]
  #[test]
  fn apng_default_image_stays_off_the_timeline() {
    use png::{BitDepth, ColorType, Encoder};

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 4, 4);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_animated(2, 0).unwrap();
    encoder.set_sep_def_img(true).unwrap();

    let mut writer = encoder.write_header().unwrap();
    writer
      .write_image_data(&FRAME_COLORS[2].repeat(16))
      .unwrap();

    writer.set_frame_delay(100, 1000).unwrap();
    writer
      .write_image_data(&FRAME_COLORS[0].repeat(16))
      .unwrap();
    writer.set_frame_delay(100, 1000).unwrap();
    writer
      .write_image_data(&FRAME_COLORS[1].repeat(16))
      .unwrap();
    writer.finish().unwrap();

    let Ok(ImageSource::Animated(apng)) = ImageSource::from_bytes(&bytes) else {
      unreachable!("valid apng");
    };

    assert_eq!(apng.frame_at_time(0).pixel(2, 2), FRAME_COLORS[0]);
    assert_eq!(apng.frame_at_time(150).pixel(2, 2), FRAME_COLORS[1]);
    assert_eq!(apng.frame_at_time(250).pixel(2, 2), FRAME_COLORS[0]);
  }

  #[cfg(feature = "png")]
  #[test]
  fn seeking_a_frame_matches_replaying_to_it() {
    let sources: Vec<(&str, Vec<u8>)> = vec![
      #[cfg(feature = "gif")]
      ("gif", encoded_gif(&[(0, 100), (1, 100), (2, 100)])),
      ("apng", encoded_apng(&[(0, 100), (1, 100), (2, 100)])),
      #[cfg(feature = "webp")]
      (
        "webp",
        encoded_animated_webp(&[(0, 100), (1, 100), (2, 100)]),
      ),
    ];

    for (format, bytes) in sources {
      let Ok(ImageSource::Animated(_)) = ImageSource::from_bytes(&bytes) else {
        unreachable!("valid {format}");
      };
      let animated_format = AnimatedFormat::detect(&bytes).unwrap();

      for index in 1..3 {
        let seeked = animated_format
          .decode_frame_alone(
            &bytes,
            index,
            DecodeTarget {
              width: 4,
              height: 4,
              algorithm: ImageScalingAlgorithm::Auto,
            },
          )
          .unwrap_or_else(|| panic!("{format} frame {index} should seek"));

        let mut replayed = None;
        animated_format
          .decode_frames(&bytes, index, Some(1), None, |frame| replayed = Some(frame))
          .expect("replay decodes");
        let replayed = replayed.expect("replayed frame");

        assert_eq!(
          seeked.data(),
          replayed.data(),
          "{format} frame {index} differs between seek and replay"
        );
      }
    }
  }

  #[cfg(feature = "webp")]
  #[test]
  fn a_webp_frame_lying_about_its_size_declines_to_seek() {
    let mut bytes = encoded_animated_webp(&[(0, 100), (1, 100)]);

    // 2x2 bitstream under an ANMF header that still claims the 4x4 canvas.
    let small = {
      use image_webp::{ColorType, WebPEncoder};

      let mut encoded = Vec::new();
      WebPEncoder::new(&mut encoded)
        .encode(&FRAME_COLORS[1].repeat(4), 2, 2, ColorType::Rgba8)
        .unwrap();
      encoded[12..].to_vec()
    };

    let last_anmf = bytes
      .windows(4)
      .rposition(|window| window == b"ANMF")
      .expect("an ANMF chunk");
    let payload_start = last_anmf + 8;
    let header: Vec<u8> = bytes[payload_start..payload_start + 16].to_vec();

    bytes.truncate(payload_start);
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&small);
    let size = (16 + small.len()) as u32;
    bytes[last_anmf + 4..last_anmf + 8].copy_from_slice(&size.to_le_bytes());
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());

    assert!(
      AnimatedFormat::WebP
        .decode_frame_alone(
          &bytes,
          1,
          DecodeTarget {
            width: 4,
            height: 4,
            algorithm: ImageScalingAlgorithm::Auto
          }
        )
        .is_none()
    );
  }

  #[cfg(feature = "png")]
  #[test]
  fn a_dependent_frame_declines_to_seek() {
    use png::{BitDepth, BlendOp, ColorType, Encoder};

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 4, 4);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_animated(2, 0).unwrap();

    let mut writer = encoder.write_header().unwrap();
    writer.set_frame_delay(100, 1000).unwrap();
    writer
      .write_image_data(&FRAME_COLORS[0].repeat(16))
      .unwrap();
    writer.set_frame_delay(100, 1000).unwrap();
    writer.set_frame_dimension(2, 2).unwrap();
    writer.set_frame_position(0, 0).unwrap();
    writer.set_blend_op(BlendOp::Over).unwrap();
    writer.write_image_data(&FRAME_COLORS[1].repeat(4)).unwrap();
    writer.finish().unwrap();

    let Ok(ImageSource::Animated(apng)) = ImageSource::from_bytes(&bytes) else {
      unreachable!("valid apng");
    };

    assert!(!apng.stands_alone(1));
  }

  /// The shape most transparent GIFs take: a full-canvas first frame, then
  /// subframes that each clear back to the background. A frame after such a
  /// clear starts from a blank canvas, so it needs nothing before it.
  #[cfg(feature = "gif")]
  #[test]
  fn a_frame_after_a_background_dispose_stands_alone() {
    use image::{Delay, Frame, RgbaImage, codecs::gif::GifEncoder, codecs::gif::Repeat};

    let mut bytes = Vec::new();
    let mut encoder = GifEncoder::new(&mut bytes);
    encoder.set_repeat(Repeat::Infinite).unwrap();
    encoder
      .encode_frames((0..3).map(|index| {
        Frame::from_parts(
          RgbaImage::from_pixel(4, 4, Rgba(FRAME_COLORS[index])),
          0,
          0,
          Delay::from_numer_denom_ms(100, 1),
        )
      }))
      .unwrap();
    drop(encoder);

    let Ok(ImageSource::Animated(gif)) = ImageSource::from_bytes(&bytes) else {
      unreachable!("valid gif");
    };

    assert!(gif.stands_alone(2));
    assert_eq!(gif.frame_at_time(250).pixel(2, 2), FRAME_COLORS[2]);
  }

  #[cfg(feature = "png")]
  #[test]
  fn still_png_stays_a_bitmap() {
    use png::{BitDepth, ColorType, Encoder};

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 4, 4);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
      .write_image_data(&FRAME_COLORS[0].repeat(16))
      .unwrap();
    writer.finish().unwrap();

    assert_matches!(ImageSource::from_bytes(&bytes), Ok(ImageSource::Bitmap(_)));
  }

  /// One 4x4 solid frame per `(color index, delay ms)` pair, as an animated WebP.
  #[cfg(feature = "webp")]
  fn encoded_animated_webp(frames: &[(usize, u32)]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
      let mut bytes = id.to_vec();
      bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
      bytes.extend_from_slice(payload);
      if payload.len() % 2 == 1 {
        bytes.push(0);
      }
      bytes
    }

    fn still_frame(color: [u8; 4]) -> Vec<u8> {
      use image_webp::{ColorType, WebPEncoder};

      let pixels: Vec<u8> = color.iter().copied().cycle().take(4 * 4 * 4).collect();
      let mut bytes = Vec::new();
      WebPEncoder::new(&mut bytes)
        .encode(&pixels, 4, 4, ColorType::Rgba8)
        .unwrap();

      // Keep only the bitstream chunk; the ANMF wraps it in its own container.
      bytes[12..].to_vec()
    }

    let mut body = chunk(b"VP8X", &[0b0000_0010, 0, 0, 0, 3, 0, 0, 3, 0, 0]);
    body.extend_from_slice(&chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]));

    for &(color, duration_ms) in frames {
      let mut payload = vec![0, 0, 0, 0, 0, 0, 3, 0, 0, 3, 0, 0];
      // Bit 1 set: replace the canvas rect instead of alpha-blending onto it.
      payload.extend_from_slice(&duration_ms.to_le_bytes()[..3]);
      payload.push(0b0000_0010);
      payload.extend_from_slice(&still_frame(FRAME_COLORS[color]));
      body.extend_from_slice(&chunk(b"ANMF", &payload));
    }

    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WEBP");
    bytes.extend_from_slice(&body);

    bytes
  }

  #[cfg(feature = "webp")]
  fn animated_webp_source(frames: &[(usize, u32)]) -> AnimatedSource {
    let Ok(ImageSource::Animated(animated)) =
      ImageSource::from_bytes(&encoded_animated_webp(frames))
    else {
      unreachable!("valid animated webp");
    };
    animated
  }

  #[cfg(feature = "gif")]
  fn gif_source(frames: &[(usize, u32)]) -> AnimatedSource {
    let Ok(ImageSource::Animated(animated)) = ImageSource::from_bytes(&encoded_gif(frames)) else {
      unreachable!("valid gif");
    };
    animated
  }

  #[cfg(feature = "webp")]
  #[test]
  fn animated_webp_walks_its_frame_timeline() {
    let webp = animated_webp_source(&[(0, 100), (1, 100), (2, 100)]);

    assert_eq!(webp.dimensions(), (4, 4));
    assert_eq!(webp.frame_at_time(0).pixel(2, 2), FRAME_COLORS[0]);
    assert_eq!(webp.frame_at_time(150).pixel(2, 2), FRAME_COLORS[1]);
    assert_eq!(webp.frame_at_time(250).pixel(2, 2), FRAME_COLORS[2]);
    assert_eq!(webp.frame_at_time(350).pixel(2, 2), FRAME_COLORS[0]);
  }

  #[cfg(feature = "webp")]
  #[test]
  fn animated_webp_frames_cover_the_draw_box() {
    let webp = animated_webp_source(&[(0, 100), (1, 100)]);
    let smaller = webp.frame_at_time_covering(150, 2, 2, ImageScalingAlgorithm::Auto);

    assert_eq!((smaller.width(), smaller.height()), (2, 2));
    assert_eq!(smaller.pixel(1, 1), FRAME_COLORS[1]);
  }

  #[cfg(feature = "webp")]
  #[test]
  fn animated_webp_decodes_as_a_still_first_frame() {
    let bytes = encoded_animated_webp(&[(0, 100), (1, 100)]);
    let buffer = crate::resources::image_decoder::decode_image(&bytes).unwrap();

    assert_eq!((buffer.width(), buffer.height()), (4, 4));
    assert_eq!(buffer.pixel(2, 2), FRAME_COLORS[0]);
  }

  #[cfg(feature = "webp")]
  #[test]
  fn still_webp_stays_a_bitmap() {
    use image_webp::{ColorType, WebPEncoder};

    let mut bytes = Vec::new();
    WebPEncoder::new(&mut bytes)
      .encode(&[255, 0, 0, 255].repeat(16), 4, 4, ColorType::Rgba8)
      .unwrap();

    assert_matches!(ImageSource::from_bytes(&bytes), Ok(ImageSource::Bitmap(_)));
  }

  #[cfg(feature = "gif")]
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
    let rendered =
      image.render_for_layout(4, 4, ImageScalingAlgorithm::Auto, 0, Color::black(), None)?;
    assert_eq!(premul_at(&rendered, 2, 2), [255, 0, 0, 255]);
    Ok(())
  }

  // `currentColor` resolves against the host color, like an inline SVG
  // inheriting `color` from its parent element.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_current_color_resolves_to_host_color() -> Result<(), ImageError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let rendered = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color([255, 0, 0, 255]),
      None,
    )?;

    assert_eq!(premul_at(&rendered, 2, 2), [255, 0, 0, 255]);
    Ok(())
  }

  // The dependency on the host color is detected on decoded attribute values,
  // so an entity-encoded `currentColor` inherits too.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_entity_encoded_current_color_resolves_to_host_color() -> Result<(), ImageError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="current&#67;olor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let rendered = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color([255, 0, 0, 255]),
      None,
    )?;

    assert_eq!(premul_at(&rendered, 2, 2), [255, 0, 0, 255]);
    Ok(())
  }

  // A `color` attribute on the root wins over the host color, so the host
  // never overrides what the SVG defines itself.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_own_color_attribute_beats_host_color() -> Result<(), ImageError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" color="#00ff00"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"##;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let rendered = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Auto,
      0,
      Color([255, 0, 0, 255]),
      None,
    )?;

    assert_eq!(premul_at(&rendered, 2, 2), [0, 255, 0, 255]);
    Ok(())
  }

  // The markup a vector backend embeds carries the host color as a root
  // presentation attribute, so a standalone viewer resolves it identically.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_source_with_current_color_injects_root_attribute() -> Result<(), ImageError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="currentColor"/></svg>"#;
    let source: SvgSource = svg.parse()?;

    let injected = source.source_with_current_color(Color([255, 0, 0, 255]));
    assert!(injected.starts_with(r##"<svg color="#ff0000ff""##));

    let plain = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#ff0000"/></svg>"##;
    let source: SvgSource = plain.parse()?;

    assert!(matches!(
      source.source_with_current_color(Color([255, 0, 0, 255])),
      Cow::Borrowed(_)
    ));
    Ok(())
  }

  /// usvg drops `<text>`/`<tspan>` (text feature off), so we no longer strip
  /// them: the SVG renders identically with and without the text nodes.
  #[cfg(feature = "svg")]
  #[test]
  fn svg_text_nodes_are_ignored_not_stripped() -> Result<(), ImageError> {
    fn rendered_data(svg: &str) -> Result<Vec<u8>, ImageError> {
      let image: ImageSource = SvgSource::from_str(svg)?.into();
      let rendered =
        image.render_for_layout(8, 8, ImageScalingAlgorithm::Auto, 0, Color::black(), None)?;
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

    let rendered =
      image.render_for_layout(2, 2, ImageScalingAlgorithm::Auto, 0, Color::black(), None)?;

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

    let rendered = image.render_for_layout(
      4,
      4,
      ImageScalingAlgorithm::Pixelated,
      0,
      Color::black(),
      None,
    )?;
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

  #[cfg(feature = "svg")]
  #[test]
  fn svg_rasterization_rejects_oversized_target() {
    let source: ImageSource = "<svg width=\"1\" height=\"1\"/>"
      .parse::<SvgSource>()
      .unwrap()
      .into();

    let result = source.render_for_layout(
      8193,
      8192,
      ImageScalingAlgorithm::Auto,
      0,
      Color::black(),
      None,
    );

    assert_matches!(result, Err(ImageError::InvalidPixmapSize));
  }

  #[cfg(feature = "gif")]
  #[test]
  fn gif_source_rejects_undecodable_stream() {
    let result = ImageSource::from_bytes(b"GIF89a\x01\x02\x03");
    assert_matches!(result, Err(_));
  }

  #[cfg(feature = "gif")]
  #[test]
  fn gif_source_frame_selection_matches_expected_indices() {
    let durations = [10, 20, 30];
    let gif = gif_source(&[(0, 10), (1, 20), (2, 30)]);
    let samples = [0_u64, 9, 10, 29, 30, 59, 60, 75];

    for time_ms in samples {
      let expected_color = FRAME_COLORS[expected_frame_index(&durations, time_ms)];
      assert_eq!(gif.frame_at_time(time_ms).pixel(2, 2), expected_color);
    }
  }

  #[cfg(feature = "gif")]
  #[test]
  fn gif_source_zero_delay_clamps_to_one_ms() {
    let gif = gif_source(&[(0, 0), (1, 0)]);

    assert_eq!(gif.frame_at_time(0).pixel(2, 2), FRAME_COLORS[0]);
    assert_eq!(gif.frame_at_time(1).pixel(2, 2), FRAME_COLORS[1]);
    assert_eq!(gif.frame_at_time(2).pixel(2, 2), FRAME_COLORS[0]);
  }

  #[cfg(feature = "gif")]
  #[test]
  fn gif_later_frame_decodes_at_draw_size() {
    let gif = gif_source(&[(0, 10), (1, 10), (2, 10)]);

    let scaled = gif.frame_at_time_covering(15, 2, 2, ImageScalingAlgorithm::Auto);
    assert_eq!((scaled.width(), scaled.height()), (2, 2));

    let smaller = gif.frame_at_time_covering(15, 1, 1, ImageScalingAlgorithm::Auto);
    assert_eq!((smaller.width(), smaller.height()), (1, 1));

    let larger = gif.frame_at_time_covering(15, 4, 4, ImageScalingAlgorithm::Auto);
    assert_eq!((larger.width(), larger.height()), (4, 4));
  }

  #[cfg(feature = "gif")]
  #[test]
  fn first_frame_covers_the_draw_box() {
    let gif = gif_source(&[(0, 10), (1, 10)]);

    let first = gif.frame_at_time_covering(0, 2, 2, ImageScalingAlgorithm::Auto);
    assert_eq!((first.width(), first.height()), (2, 2));
  }

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
  #[test]
  fn gif_dimensions_come_from_header() {
    let gif = gif_source(&[(0, 10), (1, 10)]);
    assert_eq!(gif.dimensions(), (4, 4));
  }

  /// An `<image>` whose href is a local file path must not be read from disk;
  /// the referenced file's pixels must never appear in the rasterized output.
  #[cfg(all(feature = "svg", feature = "png"))]
  #[test]
  fn svg_image_href_local_path_is_not_read() {
    let opaque_red = ImageBuffer::from_rgba_bytes([255, 0, 0, 255].repeat(4 * 4), 4, 4)
      .unwrap()
      .encode_png()
      .unwrap();
    let path = std::env::temp_dir().join(format!("takumi_lfi_{}.png", std::process::id()));
    std::fs::write(&path, &opaque_red).unwrap();

    let ns = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#;
    let svg = format!(
      r#"<svg {ns} width="8" height="8"><image xlink:href="{}" x="0" y="0" width="8" height="8"/></svg>"#,
      path.display()
    );
    let source = svg.parse::<SvgSource>().unwrap();
    let rasterized = source.rasterize(8, 8, Color::black(), None).unwrap();

    std::fs::remove_file(&path).ok();

    assert!(
      rasterized.data().iter().all(|&byte| byte == 0),
      "local file was read and composited into the SVG output"
    );
  }
}

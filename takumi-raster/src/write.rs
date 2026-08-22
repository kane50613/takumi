#[cfg(feature = "rayon")]
use std::collections::VecDeque;
use std::{
  borrow::{Borrow, Cow},
  io::Write,
};

use gif::{Encoder as GifEncoder, Frame as GifFrame, Repeat};
use image::{
  ExtendedColorType, ImageEncoder, RgbaImage,
  codecs::{ico::IcoEncoder, jpeg::JpegEncoder},
};
use png::{ColorType, DeflateCompression, Filter};
use typed_builder::TypedBuilder;

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub use crate::webp::write_animated_webp;
#[cfg(not(target_arch = "wasm32"))]
use crate::webp::write_webp_lossy;
use crate::{
  Result,
  error::Error,
  render::{FrameSpan, SequentialScene, frame_spans, prepare_scenes, render_frame},
  webp::{encode_animated_webp, has_any_alpha_pixel, strip_alpha_channel, write_webp_lossless},
};

/// Lossy-encoding quality, clamped to the `0..=100` range (higher means better
/// quality and larger output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality(u8);

impl Quality {
  /// Creates a quality value, clamping out-of-range input into `0..=100`.
  pub const fn new(value: u8) -> Self {
    Self(if value > 100 { 100 } else { value })
  }

  /// The raw quality value in `0..=100`.
  pub const fn get(self) -> u8 {
    self.0
  }
}

impl Default for Quality {
  /// `75`, a balanced default for lossy formats.
  fn default() -> Self {
    Self(75)
  }
}

/// Output format for rendered images. Format-specific encoding parameters live on
/// the variant that supports them, so e.g. a quality value cannot be supplied for
/// the lossless [`Png`](OutputFormat::Png) format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
  /// PNG: lossless, widely supported, and the fastest format to encode.
  Png,

  /// JPEG: lossy, does not support transparency.
  Jpeg {
    /// Encoding quality.
    quality: Quality,
  },

  /// Lossy WebP (VP8). Native-only — the wasm `image-webp` backend cannot encode
  /// lossy WebP; use [`WebPLossless`](OutputFormat::WebPLossless) there.
  #[cfg(not(target_arch = "wasm32"))]
  WebP {
    /// Encoding quality.
    quality: Quality,
  },

  /// Lossless WebP (VP8L). Available on every target.
  WebPLossless,

  /// ICO: favicons and application icons.
  Ico,
}

impl OutputFormat {
  /// Returns the MIME type for the image output format.
  pub fn content_type(&self) -> &'static str {
    match self {
      #[cfg(not(target_arch = "wasm32"))]
      OutputFormat::WebP { .. } => "image/webp",
      OutputFormat::WebPLossless => "image/webp",
      OutputFormat::Png => "image/png",
      OutputFormat::Jpeg { .. } => "image/jpeg",
      OutputFormat::Ico => "image/x-icon",
    }
  }
}

/// A rendered RGBA raster image: the output of [`render`](crate::render).
///
/// Wraps the pixel buffer so the public API does not commit to a specific
/// `image` crate version. Encode it with [`write_image`], or reach the raw bytes
/// via [`as_raw`](Bitmap::as_raw) / [`into_raw`](Bitmap::into_raw).
#[derive(Debug, Clone)]
pub struct Bitmap(RgbaImage);

impl Bitmap {
  /// Width in pixels.
  pub fn width(&self) -> u32 {
    self.0.width()
  }

  /// Height in pixels.
  pub fn height(&self) -> u32 {
    self.0.height()
  }

  /// Borrows the raw RGBA bytes, row-major, 4 bytes per pixel.
  pub fn as_raw(&self) -> &[u8] {
    self.0.as_raw()
  }

  /// Consumes the bitmap into its raw RGBA byte buffer.
  pub fn into_raw(self) -> Vec<u8> {
    self.0.into_raw()
  }

  /// Builds a bitmap from raw RGBA bytes, row-major, 4 bytes per pixel. Returns
  /// `None` when `data.len()` is not `width * height * 4`.
  pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
    RgbaImage::from_raw(width, height, data).map(Self)
  }

  pub(crate) fn from_rgba(image: RgbaImage) -> Self {
    Self(image)
  }

  pub(crate) fn as_rgba(&self) -> &RgbaImage {
    &self.0
  }

  #[cfg(test)]
  pub(crate) fn into_rgba(self) -> RgbaImage {
    self.0
  }
}

/// Represents a single frame of an animated image.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AnimationFrame {
  /// The image data for the frame.
  pub image: Bitmap,
  /// The duration of the frame in milliseconds.
  /// Maximum value is 0xffffff (24-bit), overflow will be clamped.
  pub duration_ms: u32,
}

impl AnimationFrame {
  /// Creates a new animation frame.
  pub fn new(image: Bitmap, duration_ms: u32) -> Self {
    Self { image, duration_ms }
  }
}

/// Encoding options for animated WebP output.
#[derive(Debug, Clone, Copy, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct AnimatedWebpOptions {
  /// Whether frames should be alpha-blended with previous content.
  #[builder(default = true)]
  pub blend: bool,
  /// Whether frame disposal clears to background before the next frame.
  #[builder(default = false)]
  pub dispose: bool,
  /// Number of times to loop; `None` means infinite loop.
  pub loop_count: Option<u16>,
  /// Encode losslessly. When `true`, `quality` is ignored.
  pub lossless: bool,
  /// Quality in range `0..=100` for lossy encoding; ignored when `lossless`.
  #[builder(default = 75)]
  pub quality: u8,
  /// Encoding speed in range `0..=6`; `0` is fastest (lowest compression), `6` is
  /// slowest (best compression). `None` uses the default speed of `1`.
  ///
  /// Only effective on native targets (libwebp). Ignored on WASM.
  pub speed: Option<u8>,
}

impl Default for AnimatedWebpOptions {
  fn default() -> Self {
    Self {
      blend: true,
      dispose: false,
      loop_count: None,
      lossless: true,
      quality: 75,
      speed: None,
    }
  }
}

/// Encoding options for animated PNG output.
#[derive(Debug, Clone, Copy, Default, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct AnimatedPngOptions {
  /// Number of times to loop; `None` means infinite loop.
  pub loop_count: Option<u16>,
}

/// Encoding options for animated GIF output.
#[derive(Debug, Clone, Copy, Default, TypedBuilder)]
#[builder(field_defaults(default))]
#[non_exhaustive]
pub struct AnimatedGifOptions {
  /// Number of times to loop; `None` means infinite loop.
  pub loop_count: Option<u16>,
}

fn duration_ms_to_gif_delay(duration_ms: u32) -> u16 {
  if duration_ms == 0 {
    0
  } else {
    duration_ms.div_ceil(10).min(u16::MAX as u32) as u16
  }
}

fn configure_png_encoder<T: Write>(encoder: &mut png::Encoder<'_, T>) {
  encoder.set_deflate_compression(DeflateCompression::Level(7));
  encoder.set_filter(Filter::NoFilter);
}

/// Writes a single rendered image to `destination` using `format`.
pub fn write_image<T: Write>(
  image: &Bitmap,
  destination: &mut T,
  format: OutputFormat,
) -> Result<()> {
  let rgba = image.as_rgba();

  match format {
    OutputFormat::Jpeg { quality } => {
      let width = image.width();
      let height = image.height();
      let rgb = strip_alpha_channel(Cow::Borrowed(rgba));

      let encoder = JpegEncoder::new_with_quality(destination, quality.get());
      encoder
        .write_image(&rgb, width, height, ExtendedColorType::Rgb8)
        .map_err(Error::encode)?;
    }
    OutputFormat::Png => {
      let mut encoder = png::Encoder::new(destination, image.width(), image.height());
      configure_png_encoder(&mut encoder);

      let has_alpha = has_any_alpha_pixel(rgba);

      let image_data = if has_alpha {
        Cow::Borrowed(image.as_raw())
      } else {
        Cow::Owned(strip_alpha_channel(Cow::Borrowed(rgba)))
      };

      encoder.set_color(if has_alpha {
        ColorType::Rgba
      } else {
        ColorType::Rgb
      });

      let mut writer = encoder.write_header().map_err(Error::encode)?;
      writer
        .write_image_data(&image_data)
        .map_err(Error::encode)?;
      writer.finish().map_err(Error::encode)?;
    }
    #[cfg(not(target_arch = "wasm32"))]
    OutputFormat::WebP { quality } => {
      write_webp_lossy(Cow::Borrowed(rgba), destination, quality)?;
    }
    OutputFormat::WebPLossless => {
      write_webp_lossless(Cow::Borrowed(rgba), destination)?;
    }
    OutputFormat::Ico => {
      let width = image.width();
      let height = image.height();
      let encoder = IcoEncoder::new(destination);
      encoder
        .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
        .map_err(Error::encode)?;
    }
  }

  Ok(())
}

/// Encode a sequence of RGBA frames into an animated GIF and write to `destination`.
pub fn write_animated_gif<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedGifOptions,
) -> Result<()> {
  ensure_uniform_frame_dimensions(&frames)?;
  encode_animated_gif(
    frames.into_owned().into_iter().map(Ok),
    destination,
    options,
  )
}

/// Rejects mismatched frame dimensions up front, so a slice-based encoder writes
/// nothing before failing. The streaming encoders check each frame as it arrives.
fn ensure_uniform_frame_dimensions(frames: &[AnimationFrame]) -> Result<()> {
  let Some(first) = frames.first() else {
    return Ok(());
  };
  let (width, height) = (first.image.width(), first.image.height());
  for frame in &frames[1..] {
    if frame.image.width() != width || frame.image.height() != height {
      return Err(Error::MixedAnimationFrameDimensions);
    }
  }
  Ok(())
}

/// Streams frames into an animated GIF, encoding each as it arrives so only one
/// raw frame is held at a time.
pub(crate) fn encode_animated_gif<W, I>(
  mut frames: I,
  destination: &mut W,
  options: AnimatedGifOptions,
) -> Result<()>
where
  W: Write,
  I: Iterator<Item = Result<AnimationFrame>>,
{
  let Some(first) = frames.next().transpose()? else {
    return Err(Error::EmptyAnimationFrames);
  };

  let width = first.image.width();
  let height = first.image.height();
  if width > u16::MAX as u32 || height > u16::MAX as u32 {
    return Err(Error::GifFrameDimensionsTooLarge {
      width,
      height,
      max: u16::MAX,
    });
  }

  let width = width as u16;
  let height = height as u16;
  let mut encoder = GifEncoder::new(destination, width, height, &[]).map_err(Error::encode)?;
  encoder
    .set_repeat(options.loop_count.map_or(Repeat::Infinite, Repeat::Finite))
    .map_err(Error::encode)?;

  let mut write_gif_frame = |frame: AnimationFrame| -> Result<()> {
    if frame.image.width() != u32::from(width) || frame.image.height() != u32::from(height) {
      return Err(Error::MixedAnimationFrameDimensions);
    }
    let mut pixels = frame.image.into_raw();
    let mut gif_frame = GifFrame::from_rgba_speed(width, height, &mut pixels, 28);
    gif_frame.delay = duration_ms_to_gif_delay(frame.duration_ms);
    encoder.write_frame(&gif_frame).map_err(Error::encode)
  };

  write_gif_frame(first)?;
  for frame in frames {
    write_gif_frame(frame?)?;
  }

  Ok(())
}

/// Encode a sequence of RGBA frames into an animated PNG and write to `destination`.
pub fn write_animated_png<W: Write>(
  frames: &[AnimationFrame],
  destination: &mut W,
  options: AnimatedPngOptions,
) -> Result<()> {
  if frames.is_empty() {
    return Err(Error::EmptyAnimationFrames);
  }
  ensure_uniform_frame_dimensions(frames)?;

  let frame_count = frames.len() as u32;

  encode_animated_png(frames.iter().map(Ok), frame_count, destination, options)
}

/// Streams frames into an animated PNG. `frame_count` is passed in because APNG
/// writes it into the header before the first frame.
pub(crate) fn encode_animated_png<W, F, I>(
  mut frames: I,
  frame_count: u32,
  destination: &mut W,
  options: AnimatedPngOptions,
) -> Result<()>
where
  W: Write,
  F: Borrow<AnimationFrame>,
  I: Iterator<Item = Result<F>>,
{
  let Some(first) = frames.next().transpose()? else {
    return Err(Error::EmptyAnimationFrames);
  };
  let first = first.borrow();
  let width = first.image.width();
  let height = first.image.height();

  let mut encoder = png::Encoder::new(destination, width, height);
  configure_png_encoder(&mut encoder);
  encoder.set_color(ColorType::Rgba);
  encoder
    .set_animated(frame_count, options.loop_count.unwrap_or(0) as u32)
    .map_err(Error::encode)?;
  encoder
    .set_frame_delay(first.duration_ms.min(u16::MAX as u32) as u16, 1000)
    .map_err(Error::encode)?;

  let mut writer = encoder.write_header().map_err(Error::encode)?;
  writer
    .write_image_data(first.image.as_raw())
    .map_err(Error::encode)?;

  for frame in frames {
    let frame = frame?;
    let frame = frame.borrow();
    if frame.image.width() != width || frame.image.height() != height {
      return Err(Error::MixedAnimationFrameDimensions);
    }
    writer
      .set_frame_delay(frame.duration_ms.min(u16::MAX as u32) as u16, 1000)
      .map_err(Error::encode)?;
    writer
      .write_image_data(frame.image.as_raw())
      .map_err(Error::encode)?;
  }

  writer.finish().map_err(Error::encode)?;

  Ok(())
}

/// Output format and per-format options for [`write_animation`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum AnimationFormat {
  /// Animated WebP.
  WebP(AnimatedWebpOptions),
  /// Animated PNG.
  Apng(AnimatedPngOptions),
  /// Animated GIF.
  Gif(AnimatedGifOptions),
}

impl AnimationFormat {
  /// Shortest per-frame duration, in milliseconds, the format encodes without
  /// decoders clamping it. Browsers bump any frame of 10ms or less to 100ms; GIF
  /// stores delays in centiseconds, so its shortest honored step is 20ms.
  const fn min_frame_duration_ms(self) -> u32 {
    match self {
      AnimationFormat::Gif(_) => 20,
      AnimationFormat::WebP(_) | AnimationFormat::Apng(_) => 11,
    }
  }

  /// Highest frame rate this format encodes faithfully. Above it, per-frame
  /// durations fall to or below [`min_frame_duration_ms`](Self::min_frame_duration_ms)
  /// and decoders clamp them to 100ms, stalling playback.
  pub const fn max_fps(self) -> u32 {
    1000 / self.min_frame_duration_ms()
  }
}

/// Renders a timeline at `fps` and streams each frame straight into `format`,
/// holding one raw frame at a time instead of the whole animation.
///
/// Every scene must render to the same frame size. `write_animation` encodes each
/// frame as it renders it, so it cannot reject mismatched sizes up front: a
/// timeline whose scenes use different viewports may write partial GIF or APNG
/// output before failing with
/// [`MixedAnimationFrameDimensions`](Error::MixedAnimationFrameDimensions).
///
/// `fps` must not exceed [`AnimationFormat::max_fps`]. Above that ceiling frames
/// fall to a duration decoders clamp to 100ms, so `write_animation` rejects it
/// with [`AnimationFrameRateTooHigh`](Error::AnimationFrameRateTooHigh) before
/// rendering anything.
///
/// [`render_animation`](crate::render_animation) plus a `write_animated_*` call is
/// the eager alternative. It holds every frame at once but rejects mismatched
/// dimensions before writing.
pub fn write_animation<W: Write>(
  scenes: &[SequentialScene<'_>],
  fps: u32,
  format: AnimationFormat,
  destination: &mut W,
) -> Result<()> {
  let max_fps = format.max_fps();
  if fps > max_fps {
    return Err(Error::AnimationFrameRateTooHigh { fps, max_fps });
  }

  let spans = frame_spans(scenes, fps);
  if spans.is_empty() {
    return Err(Error::EmptyAnimationFrames);
  }

  #[cfg(feature = "rayon")]
  {
    encode_frames(
      ChunkedFrames::new(scenes, &spans),
      &spans,
      format,
      destination,
    )
  }
  #[cfg(not(feature = "rayon"))]
  {
    let prepared = prepare_scenes(scenes);
    encode_frames(
      spans.iter().map(|&span| render_frame(&prepared, span)),
      &spans,
      format,
      destination,
    )
  }
}

fn encode_frames<W: Write>(
  frames: impl Iterator<Item = Result<AnimationFrame>>,
  spans: &[FrameSpan],
  format: AnimationFormat,
  destination: &mut W,
) -> Result<()> {
  match format {
    AnimationFormat::WebP(options) => encode_animated_webp(frames, destination, options),
    AnimationFormat::Gif(options) => encode_animated_gif(frames, destination, options),
    AnimationFormat::Apng(options) => {
      encode_animated_png(frames, spans.len() as u32, destination, options)
    }
  }
}

/// Renders frames one chunk at a time: `next()` renders a chunk of `rayon`
/// threads' worth of frames in parallel once the encoder has consumed the
/// previous chunk, so at most one chunk of raw frames is in memory.
#[cfg(feature = "rayon")]
struct ChunkedFrames<'a, 'g> {
  scenes: &'a [SequentialScene<'g>],
  spans: &'a [FrameSpan],
  next: usize,
  ready: VecDeque<Result<AnimationFrame>>,
}

#[cfg(feature = "rayon")]
impl<'a, 'g> ChunkedFrames<'a, 'g> {
  fn new(scenes: &'a [SequentialScene<'g>], spans: &'a [FrameSpan]) -> Self {
    Self {
      scenes,
      spans,
      next: 0,
      ready: VecDeque::new(),
    }
  }
}

#[cfg(feature = "rayon")]
impl Iterator for ChunkedFrames<'_, '_> {
  type Item = Result<AnimationFrame>;

  fn next(&mut self) -> Option<Self::Item> {
    use rayon::prelude::*;

    if self.ready.is_empty() && self.next < self.spans.len() {
      let chunk_len = rayon::current_num_threads().max(1);
      let chunk = &self.spans[self.next..(self.next + chunk_len).min(self.spans.len())];
      self.next += chunk.len();

      // `PreparedScene` is not `Send`: each worker prepares and reuses its own copy.
      self.ready = chunk
        .par_iter()
        .map_init(
          || prepare_scenes(self.scenes),
          |prepared, &span| render_frame(prepared, span),
        )
        .collect();
    }

    self.ready.pop_front()
  }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
  use std::{assert_matches, borrow::Cow, io::Cursor, mem::MaybeUninit, slice::from_raw_parts};

  use gif::{ColorOutput, DecodeOptions};
  use image::RgbaImage;
  use libwebp_sys::{WEBP_CSP_MODE::MODE_RGBA, *};
  use takumi_core::Error;

  use super::{
    AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, Bitmap,
    OutputFormat, write_animated_gif, write_animated_png, write_animated_webp, write_image,
  };
  use crate::{DitheringAlgorithm, apply_dithering};

  fn mk_frame(image: RgbaImage, duration_ms: u32) -> AnimationFrame {
    AnimationFrame {
      image: Bitmap::from_rgba(image),
      duration_ms,
    }
  }

  #[test]
  fn write_animated_png_writes_per_frame_delays() {
    let frames = vec![
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
        120,
      ),
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([0, 255, 0, 255])),
        30,
      ),
    ];

    let mut bytes = Vec::new();
    let encode_result =
      write_animated_png(&frames, &mut bytes, AnimatedPngOptions { loop_count: None });
    assert!(encode_result.is_ok(), "failed to encode animated png");

    let decode_result = png::Decoder::new(Cursor::new(&bytes)).read_info();
    assert!(decode_result.is_ok(), "failed to decode animated png");
    let mut reader = match decode_result {
      Ok(reader) => reader,
      Err(_) => return,
    };

    let Some(first) = reader.info().frame_control else {
      panic!("missing frame control for the first png frame");
    };
    assert_eq!((first.delay_num, first.delay_den), (120, 1000));

    let second = reader.next_frame_info();
    assert!(second.is_ok(), "missing frame control for the second frame");
    let Ok(second) = second else {
      return;
    };
    assert_eq!((second.delay_num, second.delay_den), (30, 1000));
  }

  #[test]
  fn write_animated_gif_writes_valid_animation_and_delays() {
    let frame_a = mk_frame(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 0 && y == 0 {
          image::Rgba([255, 0, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      45,
    );
    let frame_b = mk_frame(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 1 && y == 1 {
          image::Rgba([0, 255, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      10,
    );

    let mut bytes = Vec::new();
    let encode_result = write_animated_gif(
      Cow::Owned(vec![frame_a, frame_b]),
      &mut bytes,
      AnimatedGifOptions {
        loop_count: Some(7),
      },
    );
    assert!(encode_result.is_ok(), "failed to encode animated gif");

    let mut decoder_options = DecodeOptions::new();
    decoder_options.set_color_output(ColorOutput::RGBA);
    let decode_result = decoder_options.read_info(Cursor::new(&bytes));
    assert!(decode_result.is_ok(), "failed to decode animated gif");

    let mut decoder = match decode_result {
      Ok(decoder) => decoder,
      Err(_) => return,
    };
    let frame_one = decoder.read_next_frame();
    assert!(frame_one.is_ok(), "missing first decoded gif frame");
    let frame_one = match frame_one {
      Ok(frame_one) => frame_one,
      Err(_) => return,
    };
    assert!(frame_one.is_some(), "missing first decoded gif frame");
    let Some(frame_one) = frame_one else {
      return;
    };
    assert_eq!(frame_one.delay, 5);

    let frame_two = decoder.read_next_frame();
    assert!(frame_two.is_ok(), "missing second decoded gif frame");
    let frame_two = match frame_two {
      Ok(frame_two) => frame_two,
      Err(_) => return,
    };
    assert!(frame_two.is_some(), "missing second decoded gif frame");
    let Some(frame_two) = frame_two else {
      return;
    };
    assert_eq!(frame_two.delay, 1);

    let frame_three = decoder.read_next_frame();
    assert!(frame_three.is_ok(), "unexpected decoder error");
    assert!(
      frame_three.unwrap_or(None).is_none(),
      "only two frames should be encoded"
    );

    assert!(
      bytes
        .windows(b"NETSCAPE2.0".len())
        .any(|chunk| chunk == b"NETSCAPE2.0"),
      "encoded gif should contain application extension for loop count"
    );
    assert!(
      bytes
        .windows(5)
        .any(|chunk| chunk == [0x03, 0x01, 0x07, 0x00, 0x00]),
      "encoded gif should store loop count = 7"
    );
  }

  #[test]
  fn write_animated_gif_rejects_mismatched_frame_dimensions() {
    let frame_a = mk_frame(
      RgbaImage::from_fn(2, 2, |_, _| image::Rgba([255, 0, 0, 255])),
      10,
    );
    let frame_b = mk_frame(
      RgbaImage::from_fn(3, 2, |_, _| image::Rgba([0, 255, 0, 255])),
      10,
    );

    let mut bytes = Vec::new();
    let encode_result = write_animated_gif(
      Cow::Owned(vec![frame_a, frame_b]),
      &mut bytes,
      AnimatedGifOptions::default(),
    );
    assert!(encode_result.is_err(), "mismatched frames should error");
    assert!(
      bytes.is_empty(),
      "encoder should not write bytes before validating frame dimensions"
    );
  }

  #[test]
  fn write_animated_gif_rejects_empty_frames() {
    let mut bytes = Vec::new();
    let result = write_animated_gif(
      Cow::Owned(Vec::new()),
      &mut bytes,
      AnimatedGifOptions::default(),
    );

    assert_matches!(result, Err(Error::EmptyAnimationFrames));
  }

  #[test]
  fn write_animated_png_rejects_empty_frames() {
    let mut bytes = Vec::new();
    let result = write_animated_png(&[], &mut bytes, AnimatedPngOptions::default());

    assert_matches!(result, Err(Error::EmptyAnimationFrames));
  }

  #[test]
  fn write_animated_png_rejects_mismatched_frame_dimensions() {
    let frames = vec![
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
        100,
      ),
      mk_frame(
        RgbaImage::from_pixel(3, 2, image::Rgba([0, 255, 0, 255])),
        100,
      ),
    ];

    let mut bytes = Vec::new();
    let result = write_animated_png(&frames, &mut bytes, AnimatedPngOptions::default());

    assert_matches!(result, Err(Error::MixedAnimationFrameDimensions));
  }

  #[test]
  fn write_image_does_not_apply_dithering() {
    let mut image = RgbaImage::new(8, 8);

    for (index, pixel) in image.as_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
      let value = (index * 3) as u8;
      *pixel = [value, value, value, 255];
    }

    let mut dithered_image = image.clone();
    apply_dithering(&mut dithered_image, DitheringAlgorithm::OrderedBayer);

    let mut encoded_none = Vec::new();
    let mut encoded_dithered = Vec::new();

    let encode_none = write_image(
      &Bitmap::from_rgba(image.clone()),
      &mut encoded_none,
      OutputFormat::Png,
    );
    assert!(encode_none.is_ok(), "failed to encode non-dithered image");

    let encode_dithered = write_image(
      &Bitmap::from_rgba(dithered_image),
      &mut encoded_dithered,
      OutputFormat::Png,
    );
    assert!(encode_dithered.is_ok(), "failed to encode image");

    assert_ne!(encoded_none, encoded_dithered);
  }

  #[test]
  fn write_image_ico_produces_ico_header() {
    let image = RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
    let mut encoded = Vec::new();
    let result = write_image(&Bitmap::from_rgba(image), &mut encoded, OutputFormat::Ico);
    assert!(result.is_ok(), "failed to encode ico image");
    assert!(
      encoded.starts_with(&[0, 0, 1, 0]),
      "encoded bytes should begin with ICO header"
    );
  }

  #[test]
  fn write_image_ico_rejects_dimensions_over_256() {
    let image = RgbaImage::from_pixel(257, 16, image::Rgba([255, 0, 0, 255]));
    let mut encoded = Vec::new();
    let result = write_image(&Bitmap::from_rgba(image), &mut encoded, OutputFormat::Ico);

    let err = result.err();
    assert!(err.is_some(), "expected oversized ico image to fail");
    let Some(err) = err else {
      return;
    };
    assert!(
      err
        .to_string()
        .contains("the image width must be `1..=256`, instead width 257 was provided")
    );
  }

  #[test]
  fn write_animated_webp_respects_blend_dispose_and_loop_count() {
    let frame_a = mk_frame(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 0 && y == 0 {
          image::Rgba([255, 0, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      120,
    );
    let frame_b = mk_frame(
      RgbaImage::from_fn(2, 2, |x, y| {
        if x == 1 && y == 1 {
          image::Rgba([0, 255, 0, 255])
        } else {
          image::Rgba([0, 0, 0, 0])
        }
      }),
      240,
    );

    let mut bytes = Vec::new();
    let encode_result = write_animated_webp(
      Cow::Owned(vec![frame_a, frame_b]),
      &mut bytes,
      AnimatedWebpOptions {
        blend: true,
        dispose: true,
        loop_count: Some(7),
        lossless: true,
        quality: 75,
        speed: None,
      },
    );
    assert!(encode_result.is_ok(), "failed to encode animated webp");

    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "demux should parse encoded animation");

    let loop_count = unsafe { WebPDemuxGetI(demux, WebPFormatFeature::WEBP_FF_LOOP_COUNT) };
    assert_eq!(loop_count, 7);

    let mut iter = MaybeUninit::<WebPIterator>::zeroed();
    let has_frame = unsafe { WebPDemuxGetFrame(demux, 1, iter.as_mut_ptr()) };
    assert_eq!(has_frame, 1, "first frame should be available");

    let mut iter = unsafe { iter.assume_init() };
    assert_eq!(
      iter.dispose_method,
      WebPMuxAnimDispose::WEBP_MUX_DISPOSE_BACKGROUND
    );
    assert_eq!(iter.blend_method, WebPMuxAnimBlend::WEBP_MUX_BLEND);

    unsafe {
      WebPDemuxReleaseIterator(&mut iter);
      WebPDemuxDelete(demux);
    }
  }

  #[test]
  fn write_animated_webp_lossy_produces_valid_animation() {
    // With allow_mixed=1 libwebp may choose VP8L even at quality<100 when it
    // produces a smaller file (trivial 2×2 solid-colour images always compress
    // better losslessly). We verify the output is a parseable animated WebP.
    let frame = mk_frame(
      RgbaImage::from_fn(2, 2, |_, _| image::Rgba([20, 80, 220, 255])),
      100,
    );

    let mut bytes = Vec::new();
    let encode_result = write_animated_webp(
      Cow::Owned(vec![frame]),
      &mut bytes,
      AnimatedWebpOptions {
        lossless: false,
        quality: 70,
        ..Default::default()
      },
    );
    assert!(
      encode_result.is_ok(),
      "failed to encode lossy animated webp"
    );

    assert!(
      bytes
        .windows(4)
        .any(|chunk| chunk == b"VP8 " || chunk == b"VP8L"),
      "animation should contain a VP8 or VP8L bitstream chunk"
    );

    // Verify it parses as a valid animated WebP
    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "lossy animation should be parseable");
    unsafe { WebPDemuxDelete(demux) };
  }

  #[test]
  fn write_animated_webp_merges_consecutive_identical_frames() {
    let image_a = RgbaImage::from_fn(2, 2, |_, _| image::Rgba([120, 30, 10, 255]));
    let image_b = RgbaImage::from_fn(2, 2, |_, _| image::Rgba([5, 200, 20, 255]));
    let frame_a = mk_frame(image_a.clone(), 50);
    let frame_b = mk_frame(image_a, 70);
    let frame_c = mk_frame(image_b, 30);

    let mut bytes = Vec::new();
    let encode_result = write_animated_webp(
      Cow::Owned(vec![frame_a, frame_b, frame_c]),
      &mut bytes,
      AnimatedWebpOptions::default(),
    );
    assert!(
      encode_result.is_ok(),
      "failed to encode animated webp with repeated frames"
    );

    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "demux should parse encoded animation");

    let frame_count = unsafe { WebPDemuxGetI(demux, WebPFormatFeature::WEBP_FF_FRAME_COUNT) };
    assert_eq!(
      frame_count, 2,
      "identical consecutive frames should be merged"
    );

    let mut iter = MaybeUninit::<WebPIterator>::zeroed();
    let has_frame = unsafe { WebPDemuxGetFrame(demux, 1, iter.as_mut_ptr()) };
    assert_eq!(has_frame, 1, "first frame should be available");
    let mut iter = unsafe { iter.assume_init() };
    assert_eq!(
      iter.duration, 120,
      "merged frame should keep total duration"
    );

    unsafe {
      WebPDemuxReleaseIterator(&mut iter);
      WebPDemuxDelete(demux);
    }
  }

  #[test]
  fn write_animated_webp_rejects_zero_sized_frames() {
    let invalid = mk_frame(RgbaImage::new(0, 1), 10);

    let mut bytes = Vec::new();
    let result = write_animated_webp(
      Cow::Owned(vec![invalid]),
      &mut bytes,
      AnimatedWebpOptions::default(),
    );
    let err = result.err();
    assert!(err.is_some(), "zero-sized frame should be rejected");
    let Some(err) = err else {
      return;
    };
    assert!(
      err
        .to_string()
        .contains("WebP animation frame dimensions must be in 1..=16777216"),
      "unexpected error message: {err}"
    );
  }

  #[test]
  fn write_animated_webp_preserves_parallel_frame_order() {
    let frames = vec![
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
        10,
      ),
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([0, 255, 0, 255])),
        20,
      ),
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255])),
        30,
      ),
      mk_frame(
        RgbaImage::from_pixel(2, 2, image::Rgba([255, 255, 0, 255])),
        40,
      ),
    ];

    let mut bytes = Vec::new();
    let encode_result = write_animated_webp(
      Cow::Owned(frames),
      &mut bytes,
      AnimatedWebpOptions::default(),
    );
    assert!(
      encode_result.is_ok(),
      "failed to encode animated webp in parallel"
    );

    let webp_data = WebPData {
      bytes: bytes.as_ptr(),
      size: bytes.len(),
    };
    let mut state = WebPDemuxState::WEBP_DEMUX_PARSING_HEADER;
    let demux =
      unsafe { WebPDemuxInternal(&webp_data, 1, &mut state, WEBP_DEMUX_ABI_VERSION as i32) };
    assert!(!demux.is_null(), "demux should parse encoded animation");

    let mut decoder_config = unsafe { MaybeUninit::<WebPDecoderConfig>::zeroed().assume_init() };
    let init_ok = unsafe { WebPInitDecoderConfig(&raw mut decoder_config) };
    assert!(init_ok, "decoder config should initialize");
    decoder_config.output.colorspace = MODE_RGBA;

    let expected_dominant_channels = [
      [true, false, false],
      [false, true, false],
      [false, false, true],
      [true, true, false],
    ];
    let expected_durations = [10, 20, 30, 40];

    let mut iter = MaybeUninit::<WebPIterator>::zeroed();
    let has_frame = unsafe { WebPDemuxGetFrame(demux, 1, iter.as_mut_ptr()) };
    assert_eq!(has_frame, 1, "first frame should be available");
    let mut iter = unsafe { iter.assume_init() };

    for (expected_dominant_channels, expected_duration) in
      expected_dominant_channels.iter().zip(expected_durations)
    {
      let decode_status = unsafe {
        WebPDecode(
          iter.fragment.bytes,
          iter.fragment.size,
          &raw mut decoder_config,
        )
      };
      assert_eq!(
        decode_status,
        VP8StatusCode::VP8_STATUS_OK,
        "frame payload should decode"
      );

      let rgba = unsafe {
        from_raw_parts(
          decoder_config.output.u.RGBA.rgba,
          decoder_config.output.u.RGBA.size,
        )
      };
      let channel_flags = [rgba[0] >= 250, rgba[1] >= 250, rgba[2] >= 250];
      assert_eq!(channel_flags, *expected_dominant_channels);
      assert!(rgba[3] >= 250, "decoded frame should remain opaque");
      assert_eq!(iter.duration, expected_duration);

      unsafe { WebPFreeDecBuffer(&raw mut decoder_config.output) };
      if expected_duration != expected_durations[expected_durations.len() - 1] {
        let has_next = unsafe { WebPDemuxNextFrame(&mut iter) };
        assert_eq!(has_next, 1, "next frame should be available");
      }
    }

    unsafe {
      WebPDemuxReleaseIterator(&mut iter);
      WebPDemuxDelete(demux);
    }
  }

  #[test]
  fn animated_webp_builder_blend_matches_default() {
    let built = AnimatedWebpOptions::builder().build();

    assert!(built.blend);
    assert_eq!(built.blend, AnimatedWebpOptions::default().blend);
    assert_eq!(built.dispose, AnimatedWebpOptions::default().dispose);
  }
}

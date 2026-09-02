//! Bitmap decoding behind one format sniff: still images, animation
//! timelines, and scaled decodes that never hold a full-size frame.

use std::io::{Cursor, Error as IoError, ErrorKind};

use image::{
  DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageResult, Limits, RgbaImage,
  codecs::png::PngDecoder,
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};

use crate::{
  resources::{image_buffer::ImageBuffer, image_resampler::resample_premultiplied},
  style::ImageScalingAlgorithm,
};

mod gif;
mod jpeg;
mod png;
mod webp;

#[cfg(feature = "webp")]
pub(crate) use self::webp::{
  animated_webp_dimensions, decode_webp_frame_alone, decode_webp_frames, is_animated_webp,
  webp_frame_infos,
};
pub(crate) use self::{
  gif::{decode_gif_frame_alone, decode_gif_frames, gif_dimensions, gif_frame_infos, is_gif},
  png::{
    apng_dimensions, apng_frame_infos, decode_apng_frame_alone, decode_apng_frames, decode_png,
    is_apng,
  },
};
use self::{
  jpeg::{JPEG_SIGNATURE, decode_jpeg, jpeg_dimensions},
  png::{PNG_SIGNATURE, decode_png_scaled},
  webp::{decode_webp, decode_webp_scaled, webp_dimensions},
};

/// Maximum decoded image edge length; also the width/height limit fed to the
/// `image` crate decoders.
pub(super) const MAX_IMAGE_DIMENSION: u32 = 8192;

/// Decoded images above this pixel count are rejected (RGBA cost = 4x).
/// 8192 x 8192 — far above any sane OG-image asset, far below OOM territory.
const MAX_IMAGE_PIXELS: u64 = MAX_IMAGE_DIMENSION as u64 * MAX_IMAGE_DIMENSION as u64;

/// What a container says about one frame, read without decoding pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameInfo {
  /// Frame rectangle within the canvas, as `(x, y, width, height)`.
  pub(crate) rect: (u32, u32, u32, u32),
  pub(crate) duration_ms: u32,
  /// Composites onto what is under it rather than replacing it.
  pub(crate) blends: bool,
  pub(crate) dispose: Dispose,
}

/// What the canvas holds once a frame has been shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dispose {
  /// Leave the frame in place.
  Keep,
  /// Clear the frame rectangle.
  Background,
  /// Restore what was there before the frame.
  Previous,
}

impl FrameInfo {
  /// A single frame standing in for a stream whose metadata could not be read.
  pub(crate) fn still() -> Self {
    Self {
      rect: (0, 0, u32::MAX, u32::MAX),
      duration_ms: 1,
      blends: false,
      dispose: Dispose::Keep,
    }
  }

  fn covers(&self, canvas: (u32, u32)) -> bool {
    let (x, y, width, height) = self.rect;
    x == 0 && y == 0 && width == canvas.0 && height == canvas.1
  }
}

/// The frame that must be drawn before `index` can be, or `None` when `index`
/// stands on its own.
///
/// Follows `ImageDecoder::FindRequiredPreviousFrame` in Blink.
pub(crate) fn required_previous_frame(
  frames: &[FrameInfo],
  index: usize,
  canvas: (u32, u32),
) -> Option<usize> {
  let frame = frames.get(index)?;
  if index == 0 || (!frame.blends && frame.covers(canvas)) {
    return None;
  }

  // A frame restoring what came before it leaves the canvas as it found it, so
  // it is not the starting state for anything after it.
  let mut previous = index - 1;
  while frames[previous].dispose == Dispose::Previous {
    previous = previous.checked_sub(1)?;
  }

  match frames[previous].dispose {
    Dispose::Keep => Some(previous),
    Dispose::Background
      if frames[previous].covers(canvas)
        || required_previous_frame(frames, previous, canvas).is_none() =>
    {
      None
    }
    Dispose::Background => Some(previous),
    Dispose::Previous => None,
  }
}

/// Total pixels across all frames of an animation (frames are canvas-sized).
pub(super) const MAX_ANIMATION_TOTAL_PIXELS: u64 = 4 * MAX_IMAGE_PIXELS;

/// Frames past this point are dropped from a timeline. The pixel budget alone
/// leaves a tiny canvas free to carry an unbounded frame list.
pub(crate) const MAX_ANIMATION_FRAMES: usize = 1024;

/// Rejects decoded images whose pixel count exceeds [`MAX_IMAGE_PIXELS`].
#[cfg(feature = "webp")]
pub(super) fn check_pixel_budget(width: u32, height: u32) -> ImageResult<()> {
  if width as u64 * height as u64 > MAX_IMAGE_PIXELS {
    return Err(pixel_budget_error(width, height));
  }

  Ok(())
}

pub(super) fn pixel_budget_error(width: u32, height: u32) -> ImageError {
  ImageError::Decoding(DecodingError::new(
    ImageFormatHint::Unknown,
    IoError::new(
      ErrorKind::InvalidData,
      format!("image dimensions {width}x{height} exceed the decode budget"),
    ),
  ))
}

pub(crate) fn decode_image(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  match detect_image_format(bytes) {
    Some(DetectedImageFormat::Png) => decode_png(bytes),
    Some(DetectedImageFormat::Jpeg) => decode_jpeg(bytes),
    Some(DetectedImageFormat::WebP) => decode_webp(bytes),
    Some(DetectedImageFormat::Gif) | None => Err(ImageError::Unsupported(
      UnsupportedError::from_format_and_kind(
        ImageFormatHint::Unknown,
        UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
      ),
    )),
  }
}

pub(crate) fn detect_image_format(bytes: &[u8]) -> Option<DetectedImageFormat> {
  if bytes.starts_with(&PNG_SIGNATURE) {
    return Some(DetectedImageFormat::Png);
  }

  if bytes.starts_with(&JPEG_SIGNATURE) {
    return Some(DetectedImageFormat::Jpeg);
  }

  if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
    return Some(DetectedImageFormat::Gif);
  }

  if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
    return Some(DetectedImageFormat::WebP);
  }

  None
}

#[derive(Clone, Copy)]
pub(crate) enum DetectedImageFormat {
  Png,
  Jpeg,
  Gif,
  WebP,
}

pub(super) fn decode_limits() -> Limits {
  let mut limits = Limits::default();
  limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
  limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
  limits
}

pub(super) fn decode_with_image_crate(
  mut decoder: impl ImageDecoder,
  format: ImageFormat,
) -> ImageResult<ImageBuffer> {
  decoder.set_limits(decode_limits())?;
  rgba_to_buffer(DynamicImage::from_decoder(decoder)?.into_rgba8(), format)
}

/// The error a decode entry point returns for a format whose feature is off.
#[cfg(not(all(feature = "jpeg", feature = "webp", feature = "gif")))]
pub(super) fn format_compiled_out_error() -> ImageError {
  ImageError::Unsupported(UnsupportedError::from_format_and_kind(
    ImageFormatHint::Unknown,
    UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
  ))
}

/// Whether these bytes are a format this build has no decoder for.
#[cfg(not(all(feature = "jpeg", feature = "webp")))]
pub(crate) fn decoder_compiled_out(bytes: &[u8]) -> bool {
  match detect_image_format(bytes) {
    #[cfg(not(feature = "jpeg"))]
    Some(DetectedImageFormat::Jpeg) => true,
    #[cfg(not(feature = "webp"))]
    Some(DetectedImageFormat::WebP) => true,
    _ => false,
  }
}

/// Dimensions from the format header, for a format whose decoder is compiled
/// out.
#[cfg(not(all(feature = "jpeg", feature = "webp")))]
pub(super) fn header_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let size = imagesize::blob_size(bytes).map_err(|error| {
    ImageError::Decoding(DecodingError::new(
      ImageFormatHint::Unknown,
      IoError::new(ErrorKind::InvalidData, error.to_string()),
    ))
  })?;
  let (width, height) = (size.width as u32, size.height as u32);

  if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
    return Err(pixel_budget_error(width, height));
  }

  Ok((width, height))
}

/// Bitmap dimensions from the format header; decodes no pixels. `None` for
/// unsupported or GIF bytes (GIFs carry their own lazy source).
pub(crate) fn bitmap_dimensions(bytes: &[u8]) -> Option<ImageResult<(u32, u32)>> {
  let dimensions = match detect_image_format(bytes)? {
    DetectedImageFormat::Png => PngDecoder::new(Cursor::new(bytes)).map(|d| d.dimensions()),
    DetectedImageFormat::Jpeg => jpeg_dimensions(bytes),
    DetectedImageFormat::WebP => return Some(webp_dimensions(bytes)),
    DetectedImageFormat::Gif => return None,
  };
  Some(dimensions)
}

/// Decodes bitmap bytes scaled to cover `width` x `height`, never upscaling.
/// Non-interlaced PNGs stream row-by-row through the resampler without a
/// full-size buffer; everything else decodes fully and resizes.
pub(crate) fn decode_bitmap_scaled(
  bytes: &[u8],
  width: u32,
  height: u32,
  algorithm: ImageScalingAlgorithm,
) -> ImageResult<ImageBuffer> {
  if let Some(streamed) = decode_png_scaled(bytes, width, height, algorithm) {
    return streamed;
  }

  if let Some(scaled) = decode_webp_scaled(bytes, width, height) {
    return scaled;
  }

  let decoded = decode_image(bytes)?;
  if width >= decoded.width() && height >= decoded.height() {
    return Ok(decoded);
  }

  resample_premultiplied(
    decoded.data(),
    (decoded.width(), decoded.height()),
    (width, height),
    algorithm,
  )
  .ok_or_else(invalid_buffer_error)
}

/// Whether a frame rectangle spans the entire canvas.
pub(super) fn covers_canvas(rect: (u32, u32, u32, u32), canvas: (u32, u32)) -> bool {
  let (x, y, width, height) = rect;
  x == 0 && y == 0 && width == canvas.0 && height == canvas.1
}

/// Resamples a full-canvas buffer down to `target`, or hands it back untouched.
pub(super) fn fit_to_target(
  buffer: ImageBuffer,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
) -> ImageResult<ImageBuffer> {
  let Some((width, height, algorithm)) = target else {
    return Ok(buffer);
  };
  if width >= buffer.width() && height >= buffer.height() {
    return Ok(buffer);
  }

  let source = (buffer.width(), buffer.height());
  resample_premultiplied(buffer.data(), source, (width, height), algorithm)
    .ok_or_else(invalid_buffer_error)
}

pub(super) fn rgba_to_buffer(image: RgbaImage, format: ImageFormat) -> ImageResult<ImageBuffer> {
  let (width, height) = (image.width(), image.height());
  ImageBuffer::from_rgba_bytes(image.into_raw(), width, height).ok_or_else(|| {
    ImageError::Decoding(DecodingError::new(
      format.into(),
      IoError::new(
        ErrorKind::InvalidData,
        "decoded RGBA buffer dimensions are not representable as a buffer",
      ),
    ))
  })
}

pub(super) fn invalid_buffer_error() -> ImageError {
  webp_decode_error(IoError::new(
    ErrorKind::InvalidData,
    "decoded image buffer size did not match dimensions",
  ))
}

pub(super) fn webp_decode_error(
  error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> ImageError {
  ImageError::Decoding(DecodingError::new(ImageFormat::WebP.into(), error))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(feature = "webp")]
  #[test]
  fn check_pixel_budget_accepts_budget_edge() {
    assert!(check_pixel_budget(MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION).is_ok());
    assert!(check_pixel_budget(1, 1).is_ok());
  }

  #[cfg(feature = "webp")]
  #[test]
  fn check_pixel_budget_rejects_oversized() {
    assert!(check_pixel_budget(MAX_IMAGE_DIMENSION + 1, MAX_IMAGE_DIMENSION + 1).is_err());
    assert!(check_pixel_budget(100_000, 100_000).is_err());
  }

  #[test]
  fn decode_png_accepts_small_valid_image() {
    let bytes = include_bytes!("../../../../assets/images/yeecord.png");
    decode_image(bytes).expect("small PNG decodes within budget");
  }
}

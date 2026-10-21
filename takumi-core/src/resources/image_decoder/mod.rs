//! Bitmap decoding behind one format sniff: still images, animation timelines, and scaled decodes
//! that never hold a full-size frame.

use std::{
  error::Error,
  fmt::{self, Display, Formatter},
};

use thiserror::Error;

use crate::{
  resources::{image_buffer::ImageBuffer, image_resampler::resample_premultiplied},
  style::ImageScalingAlgorithm,
};

#[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
mod frames;
#[cfg(feature = "gif")]
mod gif;
mod jpeg;
#[cfg(feature = "png")]
mod png;
#[cfg(not(feature = "png"))]
mod png {
  //! Header-only PNG sizing when the decoder is compiled out.

  use super::{DecodeError, DecodeResult, DecodeTarget, header_dimensions};
  use crate::resources::image_buffer::ImageBuffer;

  pub(crate) fn decode_png(_bytes: &[u8]) -> DecodeResult<ImageBuffer> {
    Err(DecodeError::Unsupported)
  }

  pub(super) fn png_dimensions(bytes: &[u8]) -> DecodeResult<(u32, u32)> {
    header_dimensions(bytes)
  }

  pub(super) fn decode_png_scaled(
    _bytes: &[u8],
    _target: DecodeTarget,
  ) -> Option<DecodeResult<ImageBuffer>> {
    None
  }
}
mod webp;

#[cfg(any(feature = "png", feature = "webp"))]
pub(crate) use self::frames::covers_canvas;
#[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
pub(crate) use self::frames::{
  Dispose, FrameInfo, MAX_ANIMATION_FRAMES, MAX_ANIMATION_TOTAL_PIXELS, needs_previous_frame,
};
#[cfg(feature = "gif")]
pub(crate) use self::gif::{
  decode_gif_frame_alone, decode_gif_frames, gif_dimensions, gif_frame_infos, is_gif,
};
pub(crate) use self::png::decode_png;
#[cfg(feature = "png")]
pub(crate) use self::png::{
  apng_dimensions, apng_frame_infos, decode_apng_frame_alone, decode_apng_frames, is_apng,
};
pub(crate) use self::webp::webp_dimensions;
#[cfg(feature = "webp")]
pub(crate) use self::webp::{
  decode_webp_frame_alone, decode_webp_frames, is_animated_webp, webp_frame_infos,
};
use self::{
  jpeg::{JPEG_SIGNATURE, decode_jpeg, jpeg_dimensions},
  png::{decode_png_scaled, png_dimensions},
  webp::{decode_webp, decode_webp_scaled},
};

pub(super) const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Why decoding image bytes failed.
#[derive(Debug, Error)]
pub(crate) enum DecodeError {
  /// The format's decoder rejected the bytes.
  #[error("{format} decoding failed: {source}")]
  Format {
    format: &'static str,
    #[source]
    source: Box<dyn Error + Send + Sync>,
  },
  /// The image is larger than the decode budget.
  #[error("image dimensions {0}x{1} exceed the decode budget")]
  OverBudget(u32, u32),
  /// This build has no decoder for the format.
  #[error("the image format is not supported")]
  Unsupported,
  /// A decoded buffer's size does not match its dimensions.
  #[error("decoded image buffer size did not match dimensions")]
  InvalidBuffer,
}

impl DecodeError {
  /// `format`'s decoder rejected the bytes with `source`.
  pub(super) fn format(
    format: DetectedImageFormat,
    source: impl Into<Box<dyn Error + Send + Sync>>,
  ) -> Self {
    Self::Format {
      format: format.name(),
      source: source.into(),
    }
  }
}

pub(crate) type DecodeResult<T> = Result<T, DecodeError>;

/// Maximum decoded image edge length.
pub(super) const MAX_IMAGE_DIMENSION: u32 = 8192;

/// Decoded images above this pixel count are rejected (RGBA cost = 4x). 8192 x 8192 — far above any
/// sane OG-image asset, far below OOM territory.
#[cfg(feature = "webp")]
const MAX_IMAGE_PIXELS: u64 = MAX_IMAGE_DIMENSION as u64 * MAX_IMAGE_DIMENSION as u64;

/// Rejects decoded images whose pixel count exceeds [`MAX_IMAGE_PIXELS`].
#[cfg(feature = "webp")]
pub(super) fn check_pixel_budget(width: u32, height: u32) -> DecodeResult<()> {
  if width as u64 * height as u64 > MAX_IMAGE_PIXELS {
    return Err(DecodeError::OverBudget(width, height));
  }

  Ok(())
}

/// Rejects an image with an edge over [`MAX_IMAGE_DIMENSION`].
pub(super) fn check_dimensions(width: u32, height: u32) -> DecodeResult<()> {
  if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
    return Err(DecodeError::OverBudget(width, height));
  }

  Ok(())
}

/// The size a decode resamples down to, and how.
#[derive(Clone, Copy)]
pub(crate) struct DecodeTarget {
  pub(crate) width: u32,
  pub(crate) height: u32,
  pub(crate) algorithm: ImageScalingAlgorithm,
}

impl DecodeTarget {
  /// The target covering a `draw_box` for a `native`-sized image: uniform scale, never upscaled.
  pub(crate) fn covering(
    (native_width, native_height): (u32, u32),
    (box_width, box_height): (u32, u32),
    algorithm: ImageScalingAlgorithm,
  ) -> Self {
    let scale = (box_width as f32 / native_width as f32)
      .max(box_height as f32 / native_height as f32)
      .min(1.0);

    Self {
      width: ((native_width as f32 * scale).round() as u32).clamp(1, native_width),
      height: ((native_height as f32 * scale).round() as u32).clamp(1, native_height),
      algorithm,
    }
  }

  /// Whether a `width` by `height` canvas is larger than the target on either axis.
  pub(super) fn shrinks(self, width: u32, height: u32) -> bool {
    self.width < width || self.height < height
  }

  /// Resamples a premultiplied `source`-sized canvas to the target.
  pub(super) fn resample(self, data: &[u8], source: (u32, u32)) -> Option<ImageBuffer> {
    resample_premultiplied(data, source, (self.width, self.height), self.algorithm)
  }
}

/// Resamples a full-canvas buffer down to `target`, or hands it back untouched.
pub(crate) fn fit_to_target(
  buffer: ImageBuffer,
  target: Option<DecodeTarget>,
) -> DecodeResult<ImageBuffer> {
  let Some(target) = target.filter(|target| target.shrinks(buffer.width(), buffer.height())) else {
    return Ok(buffer);
  };

  target
    .resample(buffer.data(), (buffer.width(), buffer.height()))
    .ok_or(DecodeError::InvalidBuffer)
}

pub(crate) fn decode_image(bytes: &[u8]) -> DecodeResult<ImageBuffer> {
  match detect_image_format(bytes) {
    Some(DetectedImageFormat::Png) => decode_png(bytes),
    Some(DetectedImageFormat::Jpeg) => decode_jpeg(bytes),
    Some(DetectedImageFormat::WebP) => decode_webp(bytes),
    Some(DetectedImageFormat::Gif) | None => Err(DecodeError::Unsupported),
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum DetectedImageFormat {
  Png,
  Jpeg,
  Gif,
  WebP,
}

impl DetectedImageFormat {
  fn name(self) -> &'static str {
    match self {
      Self::Png => "PNG",
      Self::Jpeg => "JPEG",
      Self::Gif => "GIF",
      Self::WebP => "WebP",
    }
  }
}

impl Display for DetectedImageFormat {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_str(self.name())
  }
}

/// Whether these bytes are a format this build has no decoder for.
#[cfg(not(all(feature = "png", feature = "jpeg", feature = "webp", feature = "gif")))]
pub(crate) fn decoder_compiled_out(bytes: &[u8]) -> bool {
  match detect_image_format(bytes) {
    #[cfg(not(feature = "gif"))]
    Some(DetectedImageFormat::Gif) => true,
    #[cfg(not(feature = "png"))]
    Some(DetectedImageFormat::Png) => true,
    #[cfg(not(feature = "jpeg"))]
    Some(DetectedImageFormat::Jpeg) => true,
    #[cfg(not(feature = "webp"))]
    Some(DetectedImageFormat::WebP) => true,
    _ => false,
  }
}

/// Dimensions from the format header, for a format whose decoder is compiled out.
#[cfg(not(all(feature = "png", feature = "jpeg", feature = "webp", feature = "gif")))]
pub(super) fn header_dimensions(bytes: &[u8]) -> DecodeResult<(u32, u32)> {
  let format = detect_image_format(bytes).ok_or(DecodeError::Unsupported)?;
  let size = imagesize::blob_size(bytes).map_err(|error| DecodeError::format(format, error))?;
  let (width, height) = (size.width as u32, size.height as u32);

  check_dimensions(width, height)?;
  Ok((width, height))
}

/// Bitmap dimensions from the format header; decodes no pixels.
pub(crate) fn bitmap_dimensions(bytes: &[u8]) -> Option<DecodeResult<(u32, u32)>> {
  match detect_image_format(bytes)? {
    DetectedImageFormat::Png => Some(png_dimensions(bytes)),
    DetectedImageFormat::Jpeg => Some(jpeg_dimensions(bytes)),
    DetectedImageFormat::WebP => Some(webp_dimensions(bytes)),
    DetectedImageFormat::Gif => None,
  }
}

/// Decodes bitmap bytes down to `target`, never upscaling.
pub(crate) fn decode_bitmap_scaled(
  bytes: &[u8],
  target: DecodeTarget,
) -> DecodeResult<ImageBuffer> {
  if let Some(streamed) = decode_png_scaled(bytes, target) {
    return streamed;
  }

  if let Some(scaled) = decode_webp_scaled(bytes, target) {
    return scaled;
  }

  fit_to_target(decode_image(bytes)?, Some(target))
}

/// Straight-alpha RGBA `data` of a `width` by `height` image, premultiplied.
#[cfg(any(feature = "png", feature = "webp", feature = "jpeg"))]
pub(super) fn rgba_buffer(data: Vec<u8>, (width, height): (u32, u32)) -> DecodeResult<ImageBuffer> {
  ImageBuffer::from_rgba_bytes(data, width, height).ok_or(DecodeError::InvalidBuffer)
}

#[cfg(all(test, any(feature = "png", feature = "webp")))]
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

  #[cfg(feature = "png")]
  #[test]
  fn decode_png_accepts_small_valid_image() {
    let bytes = include_bytes!("../../../../assets/images/yeecord.png");
    decode_image(bytes).expect("small PNG decodes within budget");
  }
}

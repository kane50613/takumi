//! Bitmap decoding behind one format sniff: still images, animation timelines, and scaled decodes
//! that never hold a full-size frame.

use std::io::{Error as IoError, ErrorKind};

#[cfg(any(feature = "png", feature = "gif", feature = "webp", feature = "jpeg"))]
use image::RgbaImage;
#[cfg(any(feature = "jpeg", feature = "png"))]
use image::{DynamicImage, ImageDecoder, Limits};
use image::{
  ImageError, ImageFormat, ImageResult,
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};

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

  use image::ImageResult;

  use super::{DecodeTarget, header_dimensions, unsupported_format_error};
  use crate::resources::image_buffer::ImageBuffer;

  pub(crate) fn decode_png(_bytes: &[u8]) -> ImageResult<ImageBuffer> {
    Err(unsupported_format_error())
  }

  pub(super) fn png_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
    header_dimensions(bytes)
  }

  pub(super) fn decode_png_scaled(
    _bytes: &[u8],
    _target: DecodeTarget,
  ) -> Option<ImageResult<ImageBuffer>> {
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
#[cfg(feature = "webp")]
pub(crate) use self::webp::{
  animated_webp_dimensions, decode_webp_frame_alone, decode_webp_frames, is_animated_webp,
  webp_frame_infos,
};
use self::{
  jpeg::{JPEG_SIGNATURE, decode_jpeg, jpeg_dimensions},
  png::{decode_png_scaled, png_dimensions},
  webp::{decode_webp, decode_webp_scaled, webp_dimensions},
};

pub(super) const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Maximum decoded image edge length; also the width/height limit fed to the `image` crate
/// decoders.
pub(super) const MAX_IMAGE_DIMENSION: u32 = 8192;

/// Decoded images above this pixel count are rejected (RGBA cost = 4x). 8192 x 8192 — far above any
/// sane OG-image asset, far below OOM territory.
#[cfg(feature = "webp")]
const MAX_IMAGE_PIXELS: u64 = MAX_IMAGE_DIMENSION as u64 * MAX_IMAGE_DIMENSION as u64;

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
) -> ImageResult<ImageBuffer> {
  let Some(target) = target.filter(|target| target.shrinks(buffer.width(), buffer.height())) else {
    return Ok(buffer);
  };

  target
    .resample(buffer.data(), (buffer.width(), buffer.height()))
    .ok_or_else(invalid_buffer_error)
}

pub(crate) fn decode_image(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  match detect_image_format(bytes) {
    Some(DetectedImageFormat::Png) => decode_png(bytes),
    Some(DetectedImageFormat::Jpeg) => decode_jpeg(bytes),
    Some(DetectedImageFormat::WebP) => decode_webp(bytes),
    Some(DetectedImageFormat::Gif) | None => Err(unsupported_format_error()),
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

#[cfg(any(feature = "jpeg", feature = "png"))]
pub(super) fn decode_limits() -> Limits {
  let mut limits = Limits::default();
  limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
  limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
  limits
}

#[cfg(any(feature = "jpeg", feature = "png"))]
pub(super) fn decode_with_image_crate(
  mut decoder: impl ImageDecoder,
  format: ImageFormat,
) -> ImageResult<ImageBuffer> {
  decoder.set_limits(decode_limits())?;
  rgba_to_buffer(DynamicImage::from_decoder(decoder)?.into_rgba8(), format)
}

/// The error a decode entry point returns for a format it has no decoder for.
pub(super) fn unsupported_format_error() -> ImageError {
  ImageError::Unsupported(UnsupportedError::from_format_and_kind(
    ImageFormatHint::Unknown,
    UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
  ))
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

/// Bitmap dimensions from the format header; decodes no pixels.
pub(crate) fn bitmap_dimensions(bytes: &[u8]) -> Option<ImageResult<(u32, u32)>> {
  match detect_image_format(bytes)? {
    DetectedImageFormat::Png => Some(png_dimensions(bytes)),
    DetectedImageFormat::Jpeg => Some(jpeg_dimensions(bytes)),
    DetectedImageFormat::WebP => Some(webp_dimensions(bytes)),
    DetectedImageFormat::Gif => None,
  }
}

/// Decodes bitmap bytes down to `target`, never upscaling.
pub(crate) fn decode_bitmap_scaled(bytes: &[u8], target: DecodeTarget) -> ImageResult<ImageBuffer> {
  if let Some(streamed) = decode_png_scaled(bytes, target) {
    return streamed;
  }

  if let Some(scaled) = decode_webp_scaled(bytes, target) {
    return scaled;
  }

  fit_to_target(decode_image(bytes)?, Some(target))
}

#[cfg(any(feature = "png", feature = "gif", feature = "webp", feature = "jpeg"))]
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

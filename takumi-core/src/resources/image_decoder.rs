use std::{
  io::{Cursor, Error as IoError, ErrorKind},
  sync::Arc,
};

use image::{
  AnimationDecoder, DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageResult, Limits,
  RgbaImage,
  codecs::{gif::GifDecoder, jpeg::JpegDecoder, png::PngDecoder},
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};
#[cfg(target_arch = "wasm32")]
use image_webp::WebPDecoder;
#[cfg(not(target_arch = "wasm32"))]
use libwebp_sys::{WebPDecodeRGBA, WebPFree, WebPGetInfo};

use crate::resources::image_buffer::ImageBuffer;

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// Maximum decoded image edge length; also the width/height limit fed to the
/// `image` crate decoders.
const MAX_IMAGE_DIMENSION: u32 = 8192;
/// Decoded images above this pixel count are rejected (RGBA cost = 4x).
/// 8192 x 8192 — far above any sane OG-image asset, far below OOM territory.
const MAX_IMAGE_PIXELS: u64 = MAX_IMAGE_DIMENSION as u64 * MAX_IMAGE_DIMENSION as u64;
/// Total pixels across all GIF frames (frame budget for animations).
const MAX_GIF_TOTAL_PIXELS: u64 = 4 * MAX_IMAGE_PIXELS;

/// Rejects decoded images whose pixel count exceeds [`MAX_IMAGE_PIXELS`].
fn check_pixel_budget(width: u32, height: u32) -> ImageResult<()> {
  if width as u64 * height as u64 > MAX_IMAGE_PIXELS {
    return Err(pixel_budget_error(width, height));
  }

  Ok(())
}

fn pixel_budget_error(width: u32, height: u32) -> ImageError {
  ImageError::Decoding(DecodingError::new(
    ImageFormatHint::Unknown,
    IoError::new(
      ErrorKind::InvalidData,
      format!("image dimensions {width}x{height} exceed the decode budget"),
    ),
  ))
}

#[derive(Debug)]
pub(crate) struct DecodedGifFrame {
  pub(crate) buffer: Arc<ImageBuffer>,
  pub(crate) duration_ms: u32,
}

pub(crate) fn is_gif(bytes: &[u8]) -> bool {
  matches!(detect_image_format(bytes), Some(DetectedImageFormat::Gif))
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

fn detect_image_format(bytes: &[u8]) -> Option<DetectedImageFormat> {
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
enum DetectedImageFormat {
  Png,
  Jpeg,
  Gif,
  WebP,
}

fn decode_limits() -> Limits {
  let mut limits = Limits::default();
  limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
  limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
  limits
}

fn decode_with_image_crate(
  mut decoder: impl ImageDecoder,
  format: ImageFormat,
) -> ImageResult<ImageBuffer> {
  decoder.set_limits(decode_limits())?;
  rgba_to_buffer(DynamicImage::from_decoder(decoder)?.to_rgba8(), format)
}

pub(crate) fn decode_png(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(PngDecoder::new(Cursor::new(bytes))?, ImageFormat::Png)
}

fn decode_jpeg(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(JpegDecoder::new(Cursor::new(bytes))?, ImageFormat::Jpeg)
}

/// GIF logical screen dimensions from the header; decodes no frame.
pub(crate) fn gif_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let mut decoder = GifDecoder::new(Cursor::new(bytes))?;
  decoder.set_limits(decode_limits())?;
  Ok(decoder.dimensions())
}

/// Decodes GIF frames in stream order, passing each frame past the first
/// `skip` to `push`, up to `limit` pushed frames. Returns whether the stream
/// ended. A mid-stream decode error or a blown [`MAX_GIF_TOTAL_PIXELS`] budget
/// truncates the timeline (reported as ended); only a stream with no decodable
/// first frame errors.
pub(crate) fn decode_gif_frames(
  bytes: &[u8],
  skip: usize,
  limit: Option<usize>,
  mut push: impl FnMut(DecodedGifFrame),
) -> ImageResult<bool> {
  let mut decoder = GifDecoder::new(Cursor::new(bytes))?;
  decoder.set_limits(decode_limits())?;

  let mut total_pixels: u64 = 0;
  let mut pushed = 0_usize;

  for (index, frame) in decoder.into_frames().enumerate() {
    if limit.is_some_and(|limit| pushed >= limit) {
      return Ok(false);
    }

    let frame = match frame {
      Ok(frame) => frame,
      Err(error) if index == 0 => return Err(error),
      Err(_) => return Ok(true),
    };
    let (width, height) = frame.buffer().dimensions();

    total_pixels += width as u64 * height as u64;
    if total_pixels > MAX_GIF_TOTAL_PIXELS {
      return Ok(true);
    }

    let (numerator, denominator) = frame.delay().numer_denom_ms();
    let frame_delay_ms = numerator.checked_div(denominator).unwrap_or(numerator);
    let duration_ms = frame_delay_ms.max(1);
    let buffer = Arc::new(rgba_to_buffer(frame.into_buffer(), ImageFormat::Gif)?);

    if index >= skip {
      push(DecodedGifFrame {
        buffer,
        duration_ms,
      });
      pushed += 1;
    }
  }

  Ok(true)
}

#[cfg(target_arch = "wasm32")]
fn decode_webp(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  let (width, height) = decoder.dimensions();
  check_pixel_budget(width, height)?;
  let has_alpha = decoder.has_alpha();
  let channel_count = if has_alpha { 4 } else { 3 };
  let mut image_data = vec![0; width as usize * height as usize * channel_count];
  decoder
    .read_image(&mut image_data)
    .map_err(webp_decode_error)?;

  if has_alpha {
    return RgbaImage::from_raw(width, height, image_data)
      .ok_or_else(invalid_buffer_error)
      .and_then(|image| rgba_to_buffer(image, ImageFormat::WebP));
  }

  let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
  for rgb in image_data.chunks_exact(3) {
    rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], u8::MAX]);
  }

  RgbaImage::from_raw(width, height, rgba)
    .ok_or_else(invalid_buffer_error)
    .and_then(|image| rgba_to_buffer(image, ImageFormat::WebP))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_webp(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  use crate::error::WebPError;

  let mut width = 0;
  let mut height = 0;
  let header_ok = unsafe {
    // SAFETY: `bytes.as_ptr()` is valid for `bytes.len()` bytes for the duration of the call.
    WebPGetInfo(bytes.as_ptr(), bytes.len(), &mut width, &mut height)
  };

  if header_ok == 0 || width <= 0 || height <= 0 {
    return Err(webp_decode_error(WebPError::InvalidEncodedData));
  }

  check_pixel_budget(width as u32, height as u32)?;

  let decoded_ptr = unsafe {
    // SAFETY: `bytes.as_ptr()` is valid for `bytes.len()` bytes for the duration of the call,
    // and libwebp returns either a null pointer or an owned RGBA buffer freed with `WebPFree`.
    WebPDecodeRGBA(bytes.as_ptr(), bytes.len(), &mut width, &mut height)
  };

  if decoded_ptr.is_null() {
    return Err(webp_decode_error(WebPError::EncodeFailed));
  }

  if width <= 0 || height <= 0 {
    unsafe {
      WebPFree(decoded_ptr.cast());
    }
    return Err(webp_decode_error(WebPError::InvalidEncodedData));
  }

  let pixel_count = (width as usize)
    .checked_mul(height as usize)
    .and_then(|pixels| pixels.checked_mul(4))
    .ok_or_else(invalid_buffer_error)?;
  let buffer_len = pixel_count;
  let image_data = unsafe {
    // SAFETY: `decoded_ptr` points to a `buffer_len`-byte RGBA allocation returned by libwebp.
    let slice = std::slice::from_raw_parts(decoded_ptr, buffer_len);
    let owned = slice.to_vec();
    WebPFree(decoded_ptr.cast());
    owned
  };

  RgbaImage::from_raw(width as u32, height as u32, image_data)
    .ok_or_else(invalid_buffer_error)
    .and_then(|image| rgba_to_buffer(image, ImageFormat::WebP))
}

fn rgba_to_buffer(image: RgbaImage, format: ImageFormat) -> ImageResult<ImageBuffer> {
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

fn invalid_buffer_error() -> ImageError {
  webp_decode_error(IoError::new(
    ErrorKind::InvalidData,
    "decoded image buffer size did not match dimensions",
  ))
}

fn webp_decode_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ImageError {
  ImageError::Decoding(DecodingError::new(ImageFormat::WebP.into(), error))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn check_pixel_budget_accepts_budget_edge() {
    assert!(check_pixel_budget(MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION).is_ok());
    assert!(check_pixel_budget(1, 1).is_ok());
  }

  #[test]
  fn check_pixel_budget_rejects_oversized() {
    assert!(check_pixel_budget(MAX_IMAGE_DIMENSION + 1, MAX_IMAGE_DIMENSION + 1).is_err());
    assert!(check_pixel_budget(100_000, 100_000).is_err());
  }

  #[test]
  fn decode_png_accepts_small_valid_image() {
    let bytes = include_bytes!("../../../assets/images/yeecord.png");
    decode_image(bytes).expect("small PNG decodes within budget");
  }
}

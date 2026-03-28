use std::io::{Cursor, Error as IoError, ErrorKind};

use image::{
  DynamicImage, ImageError, ImageFormat, ImageResult, RgbaImage,
  codecs::{gif::GifDecoder, jpeg::JpegDecoder, png::PngDecoder},
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};

#[cfg(not(target_arch = "wasm32"))]
use libwebp_sys::{WebPDecodeRGBA, WebPFree};

#[cfg(target_arch = "wasm32")]
use image_webp::WebPDecoder;

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

pub(crate) fn decode_image(bytes: &[u8]) -> ImageResult<RgbaImage> {
  match detect_image_format(bytes) {
    Some(DetectedImageFormat::Png) => decode_png(bytes),
    Some(DetectedImageFormat::Jpeg) => decode_jpeg(bytes),
    Some(DetectedImageFormat::Gif) => decode_gif(bytes),
    Some(DetectedImageFormat::WebP) => decode_webp(bytes),
    None => Err(ImageError::Unsupported(
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

fn decode_with_image_crate(decoder: impl image::ImageDecoder) -> ImageResult<RgbaImage> {
  Ok(DynamicImage::from_decoder(decoder)?.to_rgba8())
}

pub(crate) fn decode_png(bytes: &[u8]) -> ImageResult<RgbaImage> {
  decode_with_image_crate(PngDecoder::new(Cursor::new(bytes))?)
}

fn decode_jpeg(bytes: &[u8]) -> ImageResult<RgbaImage> {
  decode_with_image_crate(JpegDecoder::new(Cursor::new(bytes))?)
}

fn decode_gif(bytes: &[u8]) -> ImageResult<RgbaImage> {
  decode_with_image_crate(GifDecoder::new(Cursor::new(bytes))?)
}

#[cfg(target_arch = "wasm32")]
fn decode_webp(bytes: &[u8]) -> ImageResult<RgbaImage> {
  let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  let (width, height) = decoder.dimensions();
  let has_alpha = decoder.has_alpha();
  let channel_count = if has_alpha { 4 } else { 3 };
  let mut image_data = vec![0; width as usize * height as usize * channel_count];
  decoder
    .read_image(&mut image_data)
    .map_err(webp_decode_error)?;

  if has_alpha {
    return RgbaImage::from_raw(width, height, image_data).ok_or_else(invalid_buffer_error);
  }

  let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
  for rgb in image_data.chunks_exact(3) {
    rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], u8::MAX]);
  }

  RgbaImage::from_raw(width, height, rgba).ok_or_else(invalid_buffer_error)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_webp(bytes: &[u8]) -> ImageResult<RgbaImage> {
  let mut width = 0;
  let mut height = 0;
  let decoded_ptr = unsafe {
    // SAFETY: `bytes.as_ptr()` is valid for `bytes.len()` bytes for the duration of the call,
    // and libwebp returns either a null pointer or an owned RGBA buffer freed with `WebPFree`.
    WebPDecodeRGBA(bytes.as_ptr(), bytes.len(), &mut width, &mut height)
  };

  if decoded_ptr.is_null() {
    return Err(webp_decode_error(IoError::new(
      ErrorKind::InvalidData,
      "libwebp failed to decode image",
    )));
  }

  let pixel_count = width
    .checked_mul(height)
    .and_then(|pixels| pixels.checked_mul(4))
    .ok_or_else(invalid_buffer_error)?;
  let buffer_len = usize::try_from(pixel_count).map_err(|_| invalid_buffer_error())?;
  let image_data = unsafe {
    // SAFETY: `decoded_ptr` points to a `buffer_len`-byte RGBA allocation returned by libwebp.
    let slice = std::slice::from_raw_parts(decoded_ptr, buffer_len);
    let owned = slice.to_vec();
    WebPFree(decoded_ptr.cast());
    owned
  };

  RgbaImage::from_raw(width as u32, height as u32, image_data).ok_or_else(invalid_buffer_error)
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

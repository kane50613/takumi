use std::{
  borrow::Cow,
  io::{Cursor, Error as IoError, ErrorKind},
  sync::Arc,
};

use image::{
  AnimationDecoder, DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageResult, RgbaImage,
  codecs::{gif::GifDecoder, jpeg::JpegDecoder, png::PngDecoder},
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};
#[cfg(target_arch = "wasm32")]
use image_webp::WebPDecoder;
#[cfg(not(target_arch = "wasm32"))]
use libwebp_sys::{WebPDecodeRGBA, WebPFree};

use crate::resources::image_buffer::ImageBuffer;

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

pub(crate) struct DecodedGifFrame {
  pub(crate) buffer: Arc<ImageBuffer>,
  pub(crate) duration_ms: u32,
}

pub(crate) struct DecodedGif {
  pub(crate) frames: Vec<DecodedGifFrame>,
}

pub(crate) enum DecodedImage {
  Buffer(ImageBuffer),
  Gif(DecodedGif),
}

pub(crate) fn decode_image(bytes: &[u8]) -> ImageResult<DecodedImage> {
  match detect_image_format(bytes) {
    Some(DetectedImageFormat::Png) => decode_png(bytes).map(DecodedImage::Buffer),
    Some(DetectedImageFormat::Jpeg) => decode_jpeg(bytes).map(DecodedImage::Buffer),
    Some(DetectedImageFormat::Gif) => decode_gif(bytes).map(DecodedImage::Gif),
    Some(DetectedImageFormat::WebP) => decode_webp(bytes).map(DecodedImage::Buffer),
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

fn decode_with_image_crate(
  decoder: impl ImageDecoder,
  format: ImageFormat,
) -> ImageResult<ImageBuffer> {
  rgba_to_buffer(DynamicImage::from_decoder(decoder)?.to_rgba8(), format)
}

pub(crate) fn decode_png(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(PngDecoder::new(Cursor::new(bytes))?, ImageFormat::Png)
}

fn decode_jpeg(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(JpegDecoder::new(Cursor::new(bytes))?, ImageFormat::Jpeg)
}

fn decode_gif(bytes: &[u8]) -> ImageResult<DecodedGif> {
  let decoder = GifDecoder::new(Cursor::new(bytes))?;
  let frames = decoder.into_frames().collect_frames()?;
  let mut decoded_frames = Vec::with_capacity(frames.len());

  for frame in frames {
    let (numerator, denominator) = frame.delay().numer_denom_ms();
    let frame_delay_ms = numerator.checked_div(denominator).unwrap_or(numerator);
    let duration_ms = frame_delay_ms.max(1);
    let buffer = Arc::new(rgba_to_buffer(frame.into_buffer(), ImageFormat::Gif)?);
    decoded_frames.push(DecodedGifFrame {
      buffer,
      duration_ms,
    });
  }

  Ok(DecodedGif {
    frames: decoded_frames,
  })
}

#[cfg(target_arch = "wasm32")]
fn decode_webp(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  let (width, height) = decoder.dimensions();
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
  ImageBuffer::from_rgba(Cow::Owned(image)).ok_or_else(|| {
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

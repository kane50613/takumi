use std::borrow::Cow;
use std::io::{Cursor, Error as IoError, ErrorKind};
use std::sync::Arc;

use image::{
  AnimationDecoder, DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageResult, RgbaImage,
  codecs::{gif::GifDecoder, jpeg::JpegDecoder, png::PngDecoder},
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};
use tiny_skia::Pixmap;

#[cfg(not(target_arch = "wasm32"))]
use libwebp_sys::{WebPDecodeRGBA, WebPFree};

#[cfg(target_arch = "wasm32")]
use image_webp::WebPDecoder;

use crate::error::WebPError;
use crate::rendering::premultiplied_pixmap_from_rgba;

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

pub(crate) struct DecodedGifFrame {
  pub(crate) pixmap: Arc<Pixmap>,
  pub(crate) duration_ms: u32,
}

pub(crate) struct DecodedGif {
  pub(crate) frames: Vec<DecodedGifFrame>,
}

pub(crate) enum DecodedImage {
  Pixmap(Pixmap),
  Gif(DecodedGif),
}

pub(crate) fn decode_image(bytes: &[u8]) -> ImageResult<DecodedImage> {
  match detect_image_format(bytes) {
    Some(DetectedImageFormat::Png) => decode_png(bytes).map(DecodedImage::Pixmap),
    Some(DetectedImageFormat::Jpeg) => decode_jpeg(bytes).map(DecodedImage::Pixmap),
    Some(DetectedImageFormat::Gif) => decode_gif(bytes).map(DecodedImage::Gif),
    Some(DetectedImageFormat::WebP) => decode_webp(bytes).map(DecodedImage::Pixmap),
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

fn decode_with_image_crate(decoder: impl ImageDecoder, format: ImageFormat) -> ImageResult<Pixmap> {
  rgba_to_pixmap(DynamicImage::from_decoder(decoder)?.to_rgba8(), format)
}

pub(crate) fn decode_png(bytes: &[u8]) -> ImageResult<Pixmap> {
  decode_with_image_crate(PngDecoder::new(Cursor::new(bytes))?, ImageFormat::Png)
}

fn decode_jpeg(bytes: &[u8]) -> ImageResult<Pixmap> {
  decode_with_image_crate(JpegDecoder::new(Cursor::new(bytes))?, ImageFormat::Jpeg)
}

fn decode_gif(bytes: &[u8]) -> ImageResult<DecodedGif> {
  let decoder = GifDecoder::new(Cursor::new(bytes))?;
  let frames = decoder.into_frames().collect_frames()?;
  let mut decoded_frames = Vec::with_capacity(frames.len());

  for frame in frames {
    let (numerator, denominator) = frame.delay().numer_denom_ms();
    let frame_delay_ms = if denominator == 0 {
      numerator
    } else {
      numerator / denominator
    };
    let duration_ms = frame_delay_ms.max(1);
    let pixmap = Arc::new(rgba_to_pixmap(frame.into_buffer(), ImageFormat::Gif)?);
    decoded_frames.push(DecodedGifFrame {
      pixmap,
      duration_ms,
    });
  }

  Ok(DecodedGif {
    frames: decoded_frames,
  })
}

#[cfg(target_arch = "wasm32")]
fn decode_webp(bytes: &[u8]) -> ImageResult<Pixmap> {
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
      .and_then(|image| rgba_to_pixmap(image, ImageFormat::WebP));
  }

  let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
  for rgb in image_data.chunks_exact(3) {
    rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], u8::MAX]);
  }

  RgbaImage::from_raw(width, height, rgba)
    .ok_or_else(invalid_buffer_error)
    .and_then(|image| rgba_to_pixmap(image, ImageFormat::WebP))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_webp(bytes: &[u8]) -> ImageResult<Pixmap> {
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
    .and_then(|image| rgba_to_pixmap(image, ImageFormat::WebP))
}

fn rgba_to_pixmap(image: RgbaImage, format: ImageFormat) -> ImageResult<Pixmap> {
  premultiplied_pixmap_from_rgba(Cow::Owned(image)).ok_or_else(|| {
    ImageError::Decoding(DecodingError::new(
      format.into(),
      IoError::new(
        ErrorKind::InvalidData,
        "decoded RGBA buffer dimensions are not representable as a pixmap",
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

//! JPEG stills through `zune-jpeg`, or header-only sizing when the decoder is compiled out.

#[cfg(feature = "jpeg")]
use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
#[cfg(feature = "jpeg")]
use zune_jpeg::JpegDecoder;

use super::DecodeResult;
#[cfg(feature = "jpeg")]
use super::{DecodeError, DetectedImageFormat, MAX_IMAGE_DIMENSION, check_dimensions, rgba_buffer};
#[cfg(not(feature = "jpeg"))]
use super::{DecodeError, header_dimensions};
use crate::resources::image_buffer::ImageBuffer;

pub(super) const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// A decoder that writes RGBA straight from any JPEG color space, within the dimension limit.
#[cfg(feature = "jpeg")]
fn decoder(bytes: &[u8]) -> JpegDecoder<ZCursor<&[u8]>> {
  let options = DecoderOptions::default()
    .set_strict_mode(false)
    .set_max_width(MAX_IMAGE_DIMENSION as usize)
    .set_max_height(MAX_IMAGE_DIMENSION as usize)
    .jpeg_set_out_colorspace(ColorSpace::RGBA);

  JpegDecoder::new_with_options(ZCursor::new(bytes), options)
}

#[cfg(feature = "jpeg")]
fn jpeg_error(error: zune_jpeg::errors::DecodeErrors) -> DecodeError {
  DecodeError::format(DetectedImageFormat::Jpeg, error.to_string())
}

#[cfg(feature = "jpeg")]
pub(super) fn decode_jpeg(bytes: &[u8]) -> DecodeResult<ImageBuffer> {
  let mut decoder = decoder(bytes);
  let pixels = decoder.decode().map_err(jpeg_error)?;
  let (width, height) = decoder.dimensions().ok_or(DecodeError::InvalidBuffer)?;

  rgba_buffer(pixels, (width as u32, height as u32))
}

#[cfg(not(feature = "jpeg"))]
pub(super) fn decode_jpeg(_bytes: &[u8]) -> DecodeResult<ImageBuffer> {
  Err(DecodeError::Unsupported)
}

#[cfg(feature = "jpeg")]
pub(super) fn jpeg_dimensions(bytes: &[u8]) -> DecodeResult<(u32, u32)> {
  let mut decoder = decoder(bytes);

  decoder.decode_headers().map_err(jpeg_error)?;

  let (width, height) = decoder.dimensions().ok_or(DecodeError::InvalidBuffer)?;
  let (width, height) = (width as u32, height as u32);

  check_dimensions(width, height)?;
  Ok((width, height))
}

#[cfg(not(feature = "jpeg"))]
pub(super) fn jpeg_dimensions(bytes: &[u8]) -> DecodeResult<(u32, u32)> {
  header_dimensions(bytes)
}

//! JPEG stills through the `image` crate, or header-only sizing when the decoder is compiled out.

#[cfg(feature = "jpeg")]
use std::io::Cursor;

use image::ImageResult;
#[cfg(feature = "jpeg")]
use image::{ImageDecoder, ImageFormat, codecs::jpeg::JpegDecoder};

#[cfg(feature = "jpeg")]
use super::decode_with_image_crate;
#[cfg(not(feature = "jpeg"))]
use super::{format_compiled_out_error, header_dimensions};
use crate::resources::image_buffer::ImageBuffer;

pub(super) const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

#[cfg(feature = "jpeg")]
pub(super) fn decode_jpeg(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(JpegDecoder::new(Cursor::new(bytes))?, ImageFormat::Jpeg)
}

#[cfg(not(feature = "jpeg"))]
pub(super) fn decode_jpeg(_bytes: &[u8]) -> ImageResult<ImageBuffer> {
  Err(format_compiled_out_error())
}

#[cfg(feature = "jpeg")]
pub(super) fn jpeg_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  JpegDecoder::new(Cursor::new(bytes)).map(|d| d.dimensions())
}

#[cfg(not(feature = "jpeg"))]
pub(super) fn jpeg_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  header_dimensions(bytes)
}

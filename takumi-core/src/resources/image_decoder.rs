use std::{
  io::{Cursor, Error as IoError, ErrorKind},
  mem::take,
  sync::Arc,
};

use gif::{ColorOutput, DecodeOptions, Decoder as GifDecoder, DisposalMethod};
use image::{
  DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageResult, Limits, RgbaImage,
  codecs::{jpeg::JpegDecoder, png::PngDecoder},
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

fn gif_decode_error(error: gif::DecodingError) -> ImageError {
  ImageError::Decoding(DecodingError::new(ImageFormat::Gif.into(), error))
}

/// Reads the GIF header and validates the logical screen dimensions.
fn gif_decoder(bytes: &[u8]) -> ImageResult<GifDecoder<Cursor<&[u8]>>> {
  let mut options = DecodeOptions::new();
  options.set_color_output(ColorOutput::RGBA);

  let decoder = options
    .read_info(Cursor::new(bytes))
    .map_err(gif_decode_error)?;
  let (width, height) = (decoder.width() as u32, decoder.height() as u32);
  if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
    return Err(pixel_budget_error(width, height));
  }

  Ok(decoder)
}

/// GIF logical screen dimensions from the header; decodes no frame.
pub(crate) fn gif_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let decoder = gif_decoder(bytes)?;
  Ok((decoder.width() as u32, decoder.height() as u32))
}

/// Overwrites the canvas rect with the frame's non-transparent pixels
/// (straight-alpha RGBA; GIF alpha is 0 or 255).
fn blit_frame(canvas: &mut [u8], canvas_width: u32, rect: GifFrameRect, pixels: &[u8]) {
  for row in 0..rect.rows(canvas_width, canvas.len()) {
    let src_row = (row * rect.width) as usize * 4;
    let dst_row = ((rect.top + row) * canvas_width + rect.left) as usize * 4;
    for col in 0..rect.cols(canvas_width) as usize {
      let src = src_row + col * 4;
      if pixels[src + 3] != 0 {
        let dst = dst_row + col * 4;
        canvas[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
      }
    }
  }
}

/// Clears the canvas rect to transparent (`Background` disposal).
fn clear_rect(canvas: &mut [u8], canvas_width: u32, rect: GifFrameRect) {
  for row in 0..rect.rows(canvas_width, canvas.len()) {
    let dst_row = ((rect.top + row) * canvas_width + rect.left) as usize * 4;
    let cols = rect.cols(canvas_width) as usize;
    canvas[dst_row..dst_row + cols * 4].fill(0);
  }
}

/// A GIF frame's placement rect, clamped to the canvas at use sites.
#[derive(Clone, Copy)]
struct GifFrameRect {
  left: u32,
  top: u32,
  width: u32,
  height: u32,
}

impl GifFrameRect {
  fn cols(self, canvas_width: u32) -> u32 {
    self.width.min(canvas_width.saturating_sub(self.left))
  }

  fn rows(self, canvas_width: u32, canvas_len: usize) -> u32 {
    let canvas_height = (canvas_len / 4) as u32 / canvas_width.max(1);
    self.height.min(canvas_height.saturating_sub(self.top))
  }
}

/// Decodes GIF frames in stream order, passing each frame past the first
/// `skip` to `push`, up to `limit` pushed frames. Returns whether the stream
/// ended. A mid-stream decode error or a blown [`MAX_GIF_TOTAL_PIXELS`] budget
/// truncates the timeline (reported as ended); only a stream with no decodable
/// first frame errors.
///
/// Compositing matches the `image` crate (and browsers): frames blend over a
/// transparent canvas, `Keep` persists the composited result, `Background`
/// clears the frame rect, `Previous` restores the pre-frame canvas.
pub(crate) fn decode_gif_frames(
  bytes: &[u8],
  skip: usize,
  limit: Option<usize>,
  mut push: impl FnMut(DecodedGifFrame),
) -> ImageResult<bool> {
  let mut decoder = gif_decoder(bytes)?;
  let (width, height) = (decoder.width() as u32, decoder.height() as u32);

  // GIF alpha is 0 or 255, so the canvas is valid premultiplied RGBA as-is:
  // blits copy opaque pixels (premultiply is identity) and cleared pixels are
  // all-zero. Emitted frames skip the premultiply pass entirely.
  let mut canvas = vec![0_u8; width as usize * height as usize * 4];
  let mut scratch = Vec::new();
  let mut total_pixels: u64 = 0;
  let mut pushed = 0_usize;
  let mut index = 0_usize;

  loop {
    if limit.is_some_and(|limit| pushed >= limit) {
      return Ok(false);
    }

    let frame = match decoder.next_frame_info() {
      Ok(Some(frame)) => frame,
      Ok(None) => break,
      Err(error) if index == 0 => return Err(gif_decode_error(error)),
      Err(_) => return Ok(true),
    };
    let current = index;
    index += 1;

    total_pixels += width as u64 * height as u64;
    if total_pixels > MAX_GIF_TOTAL_PIXELS {
      return Ok(true);
    }

    let rect = GifFrameRect {
      left: frame.left as u32,
      top: frame.top as u32,
      width: frame.width as u32,
      height: frame.height as u32,
    };
    let dispose = frame.dispose;
    let duration_ms = (frame.delay as u32 * 10).max(1);

    scratch.resize(decoder.buffer_size(), 0);
    if let Err(error) = decoder.read_into_buffer(&mut scratch) {
      if current == 0 {
        return Err(gif_decode_error(error));
      }
      return Ok(true);
    }

    // The emitted frame is the canvas after compositing, before disposal.
    // Skipped frames only update the canvas: no clone, no premultiply.
    let keep = current >= skip;
    let last_needed = keep && limit.is_some_and(|limit| pushed + 1 >= limit);
    let composited = if last_needed {
      // The canvas is never read again: emit it without cloning.
      blit_frame(&mut canvas, width, rect, &scratch);
      Some(take(&mut canvas))
    } else {
      match dispose {
        DisposalMethod::Any | DisposalMethod::Keep => {
          blit_frame(&mut canvas, width, rect, &scratch);
          keep.then(|| canvas.clone())
        }
        DisposalMethod::Background | DisposalMethod::Previous => {
          let composited = keep.then(|| {
            let mut out = canvas.clone();
            blit_frame(&mut out, width, rect, &scratch);
            out
          });
          if matches!(dispose, DisposalMethod::Background) {
            clear_rect(&mut canvas, width, rect);
          }
          composited
        }
      }
    };

    if let Some(composited) = composited {
      let buffer = ImageBuffer::from_premultiplied_rgba(composited, width, height)
        .ok_or_else(invalid_buffer_error)?;
      push(DecodedGifFrame {
        buffer: Arc::new(buffer),
        duration_ms,
      });
      pushed += 1;
      if limit.is_some_and(|limit| pushed >= limit) {
        return Ok(false);
      }
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

  /// A solid-color RGBA frame patch; `alpha: 0` pixels punch through to the
  /// canvas below.
  fn rgba_patch(width: u16, height: u16, rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat(width as usize * height as usize)
  }

  fn test_frame(
    width: u16,
    height: u16,
    (left, top): (u16, u16),
    mut pixels: Vec<u8>,
    dispose: DisposalMethod,
    delay: u16,
  ) -> gif::Frame<'static> {
    let mut frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 10);
    frame.left = left;
    frame.top = top;
    frame.dispose = dispose;
    frame.delay = delay;
    frame
  }

  fn encode_test_gif(width: u16, height: u16, frames: &[gif::Frame<'static>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = gif::Encoder::new(&mut bytes, width, height, &[]).unwrap();
    for frame in frames {
      encoder.write_frame(frame).unwrap();
    }
    drop(encoder);
    bytes
  }

  /// Reference decode through the `image` crate's compositing.
  fn reference_frames(bytes: &[u8]) -> Vec<(Vec<u8>, u32)> {
    use image::AnimationDecoder;

    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).unwrap();
    decoder
      .into_frames()
      .map(|frame| {
        let frame = frame.unwrap();
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let duration_ms = numerator
          .checked_div(denominator)
          .unwrap_or(numerator)
          .max(1);
        let buffer = rgba_to_buffer(frame.into_buffer(), ImageFormat::Gif).unwrap();
        (buffer.data().to_vec(), duration_ms)
      })
      .collect()
  }

  fn our_frames(bytes: &[u8], skip: usize) -> Vec<(Vec<u8>, u32)> {
    let mut frames = Vec::new();
    let ended = decode_gif_frames(bytes, skip, None, |frame| {
      frames.push((frame.buffer.data().to_vec(), frame.duration_ms));
    })
    .unwrap();
    assert!(ended);
    frames
  }

  fn assert_matches_reference(bytes: &[u8]) {
    let reference = reference_frames(bytes);
    assert_eq!(our_frames(bytes, 0), reference);
    assert_eq!(our_frames(bytes, 1), reference[1..]);
  }

  #[test]
  fn gif_compositing_matches_image_crate_for_keep_disposal() {
    let bytes = encode_test_gif(
      4,
      4,
      &[
        test_frame(
          4,
          4,
          (0, 0),
          rgba_patch(4, 4, [255, 0, 0, 255]),
          DisposalMethod::Keep,
          1,
        ),
        test_frame(
          2,
          2,
          (1, 1),
          rgba_patch(2, 2, [0, 255, 0, 255]),
          DisposalMethod::Keep,
          2,
        ),
        test_frame(
          2,
          2,
          (2, 2),
          rgba_patch(2, 2, [0, 0, 255, 255]),
          DisposalMethod::Keep,
          3,
        ),
      ],
    );
    assert_matches_reference(&bytes);
  }

  #[test]
  fn gif_compositing_matches_image_crate_for_transparent_patches() {
    let mut patch = rgba_patch(2, 2, [0, 255, 0, 255]);
    patch[4..8].fill(0);

    let bytes = encode_test_gif(
      4,
      4,
      &[
        test_frame(
          4,
          4,
          (0, 0),
          rgba_patch(4, 4, [255, 0, 0, 255]),
          DisposalMethod::Keep,
          1,
        ),
        test_frame(2, 2, (1, 1), patch, DisposalMethod::Keep, 1),
      ],
    );
    assert_matches_reference(&bytes);
  }

  #[test]
  fn gif_compositing_matches_image_crate_for_background_disposal() {
    let bytes = encode_test_gif(
      4,
      4,
      &[
        test_frame(
          4,
          4,
          (0, 0),
          rgba_patch(4, 4, [255, 0, 0, 255]),
          DisposalMethod::Background,
          1,
        ),
        test_frame(
          2,
          2,
          (1, 1),
          rgba_patch(2, 2, [0, 255, 0, 255]),
          DisposalMethod::Background,
          1,
        ),
        test_frame(
          2,
          2,
          (2, 2),
          rgba_patch(2, 2, [0, 0, 255, 255]),
          DisposalMethod::Keep,
          1,
        ),
      ],
    );
    assert_matches_reference(&bytes);
  }

  #[test]
  fn gif_compositing_matches_image_crate_for_previous_disposal() {
    let bytes = encode_test_gif(
      4,
      4,
      &[
        test_frame(
          4,
          4,
          (0, 0),
          rgba_patch(4, 4, [255, 0, 0, 255]),
          DisposalMethod::Keep,
          1,
        ),
        test_frame(
          2,
          2,
          (1, 1),
          rgba_patch(2, 2, [0, 255, 0, 255]),
          DisposalMethod::Previous,
          1,
        ),
        test_frame(
          2,
          2,
          (2, 2),
          rgba_patch(2, 2, [0, 0, 255, 255]),
          DisposalMethod::Keep,
          1,
        ),
      ],
    );
    assert_matches_reference(&bytes);
  }

  #[test]
  fn gif_zero_delay_clamps_to_one_ms_like_image_crate() {
    let bytes = encode_test_gif(
      2,
      2,
      &[
        test_frame(
          2,
          2,
          (0, 0),
          rgba_patch(2, 2, [255, 0, 0, 255]),
          DisposalMethod::Keep,
          0,
        ),
        test_frame(
          2,
          2,
          (0, 0),
          rgba_patch(2, 2, [0, 255, 0, 255]),
          DisposalMethod::Keep,
          0,
        ),
      ],
    );
    assert_matches_reference(&bytes);
    assert!(our_frames(&bytes, 0).iter().all(|(_, ms)| *ms == 1));
  }

  #[test]
  fn gif_limit_stops_before_decoding_later_frames() {
    let bytes = encode_test_gif(
      2,
      2,
      &[
        test_frame(
          2,
          2,
          (0, 0),
          rgba_patch(2, 2, [255, 0, 0, 255]),
          DisposalMethod::Keep,
          1,
        ),
        test_frame(
          2,
          2,
          (0, 0),
          rgba_patch(2, 2, [0, 255, 0, 255]),
          DisposalMethod::Keep,
          1,
        ),
      ],
    );

    let mut frames = Vec::new();
    let ended = decode_gif_frames(&bytes, 0, Some(1), |frame| frames.push(frame)).unwrap();
    assert!(!ended);
    assert_eq!(frames.len(), 1);
  }
}

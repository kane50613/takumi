//! WebP stills and animations: libwebp on native targets, `image-webp` on
//! wasm, and header-only sizing when the decoder is compiled out.

#[cfg(feature = "webp")]
use std::{io::Cursor, sync::Arc};

use image::ImageResult;
#[cfg(feature = "webp")]
use image::{ImageFormat, RgbaImage};
#[cfg(feature = "webp")]
use image_webp::{DecodingError as WebPDecodingError, WebPDecoder};
#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
use libwebp_sys::{
  VP8StatusCode, WEBP_CSP_MODE, WebPDecode, WebPDecoderConfig, WebPGetInfo, WebPRGBABuffer,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
use super::{DetectedImageFormat, detect_image_format};
#[cfg(feature = "webp")]
use super::{
  Dispose, FrameInfo, MAX_ANIMATION_FRAMES, MAX_ANIMATION_TOTAL_PIXELS, check_pixel_budget,
  covers_canvas, fit_to_target, invalid_buffer_error, rgba_to_buffer, webp_decode_error,
};
#[cfg(not(feature = "webp"))]
use super::{format_compiled_out_error, header_dimensions};
use crate::resources::image_buffer::ImageBuffer;
#[cfg(feature = "webp")]
use crate::style::ImageScalingAlgorithm;

#[cfg(all(target_arch = "wasm32", feature = "webp"))]
pub(super) fn webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  Ok(decoder.dimensions())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
pub(super) fn webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
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

  Ok((width as u32, height as u32))
}

#[cfg(all(target_arch = "wasm32", feature = "webp"))]
pub(super) fn decode_webp(bytes: &[u8]) -> ImageResult<ImageBuffer> {
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
  for rgb in image_data.as_chunks::<3>().0 {
    rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], u8::MAX]);
  }

  RgbaImage::from_raw(width, height, rgba)
    .ok_or_else(invalid_buffer_error)
    .and_then(|image| rgba_to_buffer(image, ImageFormat::WebP))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
pub(super) fn decode_webp(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  if is_animated_webp(bytes) {
    return decode_animated_webp_first_frame(bytes);
  }

  let (width, height) = webp_dimensions(bytes)?;

  check_pixel_budget(width, height)?;
  decode_webp_into(bytes, width, height, false)
}

/// Decodes a WebP directly at the target size via libwebp's internal rescaler,
/// so no full-size frame is ever allocated. `None` when the input isn't WebP,
/// needs no downscale, or has a target libwebp can't take; the caller decodes
/// fully. libwebp's rescaler is not CatmullRom/Lanczos, so pixels differ
/// slightly from full-decode + resample.
#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
pub(super) fn decode_webp_scaled(
  bytes: &[u8],
  width: u32,
  height: u32,
) -> Option<ImageResult<ImageBuffer>> {
  if !matches!(detect_image_format(bytes), Some(DetectedImageFormat::WebP))
    || is_animated_webp(bytes)
  {
    return None;
  }

  let (native_width, native_height) = webp_dimensions(bytes).ok()?;
  if width >= native_width && height >= native_height {
    return None;
  }

  i32::try_from(width).ok()?;
  i32::try_from(height).ok()?;

  if let Err(error) =
    check_pixel_budget(native_width, native_height).and_then(|()| check_pixel_budget(width, height))
  {
    return Some(Err(error));
  }

  Some(decode_webp_into(bytes, width, height, true))
}

/// Decodes into a caller-owned buffer sized `width` x `height`; with `scale`,
/// libwebp rescales to those dimensions, otherwise they must be the native ones.
#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
fn decode_webp_into(
  bytes: &[u8],
  width: u32,
  height: u32,
  scale: bool,
) -> ImageResult<ImageBuffer> {
  use crate::error::WebPError;

  let buffer_len = (width as usize)
    .checked_mul(height as usize)
    .and_then(|pixels| pixels.checked_mul(4))
    .ok_or_else(invalid_buffer_error)?;
  let stride = i32::try_from(width)
    .ok()
    .and_then(|w| w.checked_mul(4))
    .ok_or_else(invalid_buffer_error)?;
  let mut image_data = vec![0u8; buffer_len];

  let mut config =
    WebPDecoderConfig::new().map_err(|()| webp_decode_error(WebPError::InvalidEncodedData))?;
  if scale {
    config.options.use_scaling = 1;
    config.options.scaled_width = width as i32;
    config.options.scaled_height = height as i32;
  }
  config.output.colorspace = WEBP_CSP_MODE::MODE_RGBA;
  config.output.is_external_memory = 1;
  config.output.u.RGBA = WebPRGBABuffer {
    rgba: image_data.as_mut_ptr(),
    stride,
    size: buffer_len,
  };

  let status = unsafe {
    // SAFETY: `bytes.as_ptr()` is valid for `bytes.len()` bytes for the duration of the call,
    // and `config.output` points at `image_data`, which outlives the call and is sized for
    // `width` x `height` RGBA. External memory means libwebp allocates no output buffer of
    // its own.
    WebPDecode(bytes.as_ptr(), bytes.len(), &mut config)
  };

  if status != VP8StatusCode::VP8_STATUS_OK {
    return Err(webp_decode_error(WebPError::InvalidEncodedData));
  }

  RgbaImage::from_raw(width, height, image_data)
    .ok_or_else(invalid_buffer_error)
    .and_then(|image| rgba_to_buffer(image, ImageFormat::WebP))
}

/// Whether the `VP8X` header sets the animation flag.
#[cfg(feature = "webp")]
pub(crate) fn is_animated_webp(bytes: &[u8]) -> bool {
  webp_chunks(bytes).any(|(id, payload)| {
    &id == b"VP8X"
      && payload
        .first()
        .is_some_and(|flags| flags & 0b0000_0010 != 0)
  })
}

/// Top-level RIFF chunks of a WebP, as `(id, payload)` pairs.
#[cfg(feature = "webp")]
fn webp_chunks(bytes: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
  let mut offset = 12;
  std::iter::from_fn(move || {
    let header: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let start = offset.checked_add(8)?;
    let payload = bytes.get(start..start.checked_add(size)?)?;
    offset = start.checked_add(size)?.checked_add(size & 1)?;
    Some(([header[0], header[1], header[2], header[3]], payload))
  })
}

#[cfg(feature = "webp")]
pub(crate) fn animated_webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  Ok(decoder.dimensions())
}

/// Per-frame delays in milliseconds, in stream order, read from the `ANMF`
/// headers without decoding any pixels. Stops on the same frame and pixel
/// budgets as [`decode_webp_frames`], so a playback time never selects a frame
/// the decoder drops.
#[cfg(feature = "webp")]
pub(crate) fn webp_frame_infos(bytes: &[u8]) -> ImageResult<Box<[FrameInfo]>> {
  let (width, height) = animated_webp_dimensions(bytes)?;
  let frame_pixels = width as u64 * height as u64;
  let mut total_pixels = 0_u64;
  let mut frames = Vec::new();

  for (id, payload) in webp_chunks(bytes) {
    if &id != b"ANMF" {
      continue;
    }
    if frames.len() >= MAX_ANIMATION_FRAMES {
      break;
    }

    total_pixels += frame_pixels;
    if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
      break;
    }

    // `ANMF`: x, y, width, height and duration as 24-bit values, then flags.
    let Some(header) = payload.get(..16) else {
      break;
    };
    let read_24 = |offset: usize| {
      u32::from_le_bytes([header[offset], header[offset + 1], header[offset + 2], 0])
    };

    frames.push(FrameInfo {
      rect: (
        read_24(0) * 2,
        read_24(3) * 2,
        read_24(6) + 1,
        read_24(9) + 1,
      ),
      duration_ms: read_24(12).max(1),
      blends: header[15] & 0b0000_0010 == 0,
      dispose: if header[15] & 0b0000_0001 == 0 {
        Dispose::Keep
      } else {
        Dispose::Background
      },
    });
  }

  Ok(frames.into())
}

/// Decodes animated WebP frames in stream order, passing each frame past the
/// first `skip` to `push`, up to `limit` pushed frames. Returns whether the
/// stream ended. Mid-stream decode errors and a blown budget truncate the
/// timeline (reported as ended); only a stream with no decodable first frame
/// errors.
///
/// `image-webp` owns the animation canvas, so a read composites onto the frame
/// before it and hands back the whole canvas.
#[cfg(feature = "webp")]
pub(crate) fn decode_webp_frames(
  bytes: &[u8],
  skip: usize,
  limit: Option<usize>,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
  mut push: impl FnMut(Arc<ImageBuffer>),
) -> ImageResult<bool> {
  let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  let (width, height) = decoder.dimensions();
  check_pixel_budget(width, height)?;
  let target = target.filter(|&(w, h, _)| w < width || h < height);

  let has_alpha = decoder.has_alpha();
  let mut canvas = vec![
    0_u8;
    decoder
      .output_buffer_size()
      .ok_or_else(invalid_buffer_error)?
  ];
  let mut total_pixels = 0_u64;
  let mut pushed = 0_usize;

  for index in 0..MAX_ANIMATION_FRAMES {
    if limit.is_some_and(|limit| pushed >= limit) {
      return Ok(false);
    }

    match decoder.read_frame(&mut canvas) {
      Ok(_) => {}
      Err(WebPDecodingError::NoMoreFrames) => return Ok(true),
      Err(error) if index == 0 => return Err(webp_decode_error(error)),
      Err(_) => return Ok(true),
    }

    total_pixels += width as u64 * height as u64;
    if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
      return Ok(true);
    }

    if index < skip {
      continue;
    }

    push(Arc::new(webp_canvas_to_buffer(
      &canvas, width, height, has_alpha, target,
    )?));
    pushed += 1;
  }

  Ok(false)
}

/// The first frame, for the still-image decode paths.
#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
fn decode_animated_webp_first_frame(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  let mut first = None;
  decode_webp_frames(bytes, 0, Some(1), None, |frame| first = Some(frame))?;

  first
    .map(Arc::unwrap_or_clone)
    .ok_or_else(invalid_buffer_error)
}

/// Decodes frame `index` on its own, skipping the frames before it, when the
/// `ANMF` header shows the frame covers the whole canvas and does not blend
/// onto what came before. `None` when the frame depends on its predecessors.
///
/// Blink reaches individual frames through libwebp's demuxer
/// (`WebPDemuxGetFrame`); walking the RIFF chunks keeps this available on wasm,
/// where that demuxer is not.
#[cfg(feature = "webp")]
pub(crate) fn decode_webp_frame_alone(
  bytes: &[u8],
  index: usize,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
) -> Option<ImageBuffer> {
  let read_24 = |bytes: &[u8], offset: usize| {
    Some(u32::from_le_bytes([
      *bytes.get(offset)?,
      *bytes.get(offset + 1)?,
      *bytes.get(offset + 2)?,
      0,
    ]))
  };

  // One walk for both: `VP8X` carries the canvas size, the Nth `ANMF` the frame.
  let mut canvas = None;
  let mut payload = None;
  let mut seen = 0;
  for (id, chunk) in webp_chunks(bytes) {
    match &id {
      b"VP8X" => canvas = Some((read_24(chunk, 4)? + 1, read_24(chunk, 7)? + 1)),
      b"ANMF" if seen == index => {
        payload = Some(chunk);
        break;
      }
      b"ANMF" => seen += 1,
      _ => {}
    }
  }

  let (canvas_width, canvas_height) = canvas?;
  check_pixel_budget(canvas_width, canvas_height).ok()?;
  let payload = payload?;
  let header = payload.get(..16)?;
  let read_24 = |offset: usize| read_24(header, offset).unwrap_or_default();
  let rect = (
    read_24(0) * 2,
    read_24(3) * 2,
    read_24(6) + 1,
    read_24(9) + 1,
  );

  let still = webp_frame_as_still(payload.get(16..)?, rect.2, rect.3)?;
  let frame = decode_webp(&still).ok()?;

  // The `ANMF` rectangle only claims a size; the bitstream is what has one.
  if frame.width() != rect.2 || frame.height() != rect.3 {
    return None;
  }

  // A frame filling the canvas is the canvas; a smaller one lands on a cleared
  // canvas, which is what its predecessors would have left behind.
  let buffer = if covers_canvas(rect, (canvas_width, canvas_height)) {
    frame
  } else {
    place_on_canvas(&frame, rect, (canvas_width, canvas_height))?
  };

  fit_to_target(buffer, target).ok()
}

/// Copies `frame` onto a cleared canvas at `rect`, clipping the part that
/// falls outside.
#[cfg(feature = "webp")]
fn place_on_canvas(
  frame: &ImageBuffer,
  rect: (u32, u32, u32, u32),
  (canvas_width, canvas_height): (u32, u32),
) -> Option<ImageBuffer> {
  let mut canvas = vec![0; canvas_width as usize * canvas_height as usize * 4];
  let stride = canvas_width as usize * 4;
  let span = rect.2.min(canvas_width.saturating_sub(rect.0)) as usize * 4;
  let rows = if span == 0 {
    0
  } else {
    rect.3.min(canvas_height.saturating_sub(rect.1))
  };

  for row in 0..rows {
    let source = row as usize * rect.2 as usize * 4;
    let target = (rect.1 + row) as usize * stride + rect.0 as usize * 4;

    canvas[target..target + span].copy_from_slice(&frame.data()[source..source + span]);
  }
  ImageBuffer::from_premultiplied_rgba(canvas, canvas_width, canvas_height)
}

/// Wraps one frame's bitstream in a RIFF container so the still decoder can
/// read it. A lossy frame carrying a separate `ALPH` chunk needs a `VP8X`
/// header too, which only the animation container had.
#[cfg(feature = "webp")]
fn webp_frame_as_still(bitstream: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
  let mut body = Vec::with_capacity(bitstream.len() + 18);

  if bitstream.get(..4)? == b"ALPH" {
    let (last_width, last_height) = (width.checked_sub(1)?, height.checked_sub(1)?);
    body.extend_from_slice(b"VP8X");
    body.extend_from_slice(&10_u32.to_le_bytes());
    // Alpha flag, then the canvas size as two 24-bit values.
    body.extend_from_slice(&[0b0001_0000, 0, 0, 0]);
    body.extend_from_slice(&last_width.to_le_bytes()[..3]);
    body.extend_from_slice(&last_height.to_le_bytes()[..3]);
  }
  body.extend_from_slice(bitstream);

  let mut still = b"RIFF".to_vec();
  still.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
  still.extend_from_slice(b"WEBP");
  still.extend_from_slice(&body);

  Some(still)
}

/// One composited canvas as a premultiplied buffer, resampled to `target`.
#[cfg(feature = "webp")]
fn webp_canvas_to_buffer(
  canvas: &[u8],
  width: u32,
  height: u32,
  has_alpha: bool,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
) -> ImageResult<ImageBuffer> {
  let rgba = if has_alpha {
    canvas.to_vec()
  } else {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for rgb in canvas.as_chunks::<3>().0 {
      rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], u8::MAX]);
    }
    rgba
  };

  let buffer =
    ImageBuffer::from_rgba_bytes(rgba, width, height).ok_or_else(invalid_buffer_error)?;

  fit_to_target(buffer, target)
}

#[cfg(not(feature = "webp"))]
pub(super) fn webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  header_dimensions(bytes)
}

#[cfg(not(feature = "webp"))]
pub(super) fn decode_webp(_bytes: &[u8]) -> ImageResult<ImageBuffer> {
  Err(format_compiled_out_error())
}

#[cfg(not(all(not(target_arch = "wasm32"), feature = "webp")))]
pub(super) fn decode_webp_scaled(
  _bytes: &[u8],
  _width: u32,
  _height: u32,
) -> Option<ImageResult<ImageBuffer>> {
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(feature = "webp")]
  #[test]
  fn frames_starting_outside_the_canvas_place_nothing() {
    let frame = ImageBuffer::from_premultiplied_rgba(vec![255; 2 * 2 * 4], 2, 2).unwrap();
    let placed = place_on_canvas(&frame, (6, 3, 2, 2), (4, 4)).unwrap();

    assert!(placed.data().iter().all(|byte| *byte == 0));
  }
  use crate::resources::{
    image_decoder::{decode_bitmap_scaled, decode_image},
    image_resampler::resample_premultiplied,
  };

  #[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
  fn encode_test_webp(width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
      for x in 0..width {
        rgba.extend_from_slice(&[(x * 6) as u8, (y * 8) as u8, ((x + y) * 3) as u8, 255]);
      }
    }

    let mut output = std::ptr::null_mut();
    let size = unsafe {
      // SAFETY: `rgba` holds `width * height` RGBA pixels with a `width * 4` stride; libwebp
      // returns an owned buffer freed below with `WebPFree`.
      libwebp_sys::WebPEncodeLosslessRGBA(
        rgba.as_ptr(),
        width as i32,
        height as i32,
        width as i32 * 4,
        &mut output,
      )
    };
    assert!(size > 0);

    unsafe {
      // SAFETY: `output` points to a `size`-byte buffer returned by libwebp.
      let owned = std::slice::from_raw_parts(output, size).to_vec();
      libwebp_sys::WebPFree(output.cast());
      owned
    }
  }

  #[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
  #[test]
  fn webp_scaled_decode_approximates_full_decode() {
    let bytes = encode_test_webp(40, 30);
    let scaled = decode_bitmap_scaled(&bytes, 13, 9, ImageScalingAlgorithm::Auto).unwrap();
    assert_eq!((scaled.width(), scaled.height()), (13, 9));

    let full = decode_image(&bytes).unwrap();
    let resized =
      resample_premultiplied(full.data(), (40, 30), (13, 9), ImageScalingAlgorithm::Auto).unwrap();
    let max_diff = scaled
      .data()
      .iter()
      .zip(resized.data())
      .map(|(a, b)| a.abs_diff(*b))
      .max()
      .unwrap();
    assert!(
      max_diff <= 16,
      "libwebp rescaler drifted {max_diff} from CatmullRom"
    );
  }

  #[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
  #[test]
  fn webp_scaled_decode_skips_upscale() {
    let bytes = encode_test_webp(40, 30);
    let unscaled = decode_bitmap_scaled(&bytes, 80, 60, ImageScalingAlgorithm::Auto).unwrap();
    assert_eq!((unscaled.width(), unscaled.height()), (40, 30));
    assert_eq!(unscaled.data(), decode_image(&bytes).unwrap().data());
  }
}

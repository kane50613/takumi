//! GIF sizing and timelines, composited the way browsers and the `image`
//! crate do; stubs when the decoder is compiled out.

use std::sync::Arc;
#[cfg(feature = "gif")]
use std::{io::Cursor, mem::take};

#[cfg(feature = "gif")]
use gif::{ColorOutput, DecodeOptions, Decoder as GifDecoder, DisposalMethod};
use image::ImageResult;
#[cfg(feature = "gif")]
use image::{ImageError, ImageFormat, error::DecodingError};

#[cfg(not(feature = "gif"))]
use super::format_compiled_out_error;
use super::{DetectedImageFormat, FrameInfo, detect_image_format};
#[cfg(feature = "gif")]
use super::{
  Dispose, MAX_ANIMATION_FRAMES, MAX_ANIMATION_TOTAL_PIXELS, MAX_IMAGE_DIMENSION, fit_to_target,
  invalid_buffer_error, pixel_budget_error,
};
#[cfg(feature = "gif")]
use crate::{geometry::Rect, resources::image_resampler::resample_premultiplied};
use crate::{resources::image_buffer::ImageBuffer, style::ImageScalingAlgorithm};

pub(crate) fn is_gif(bytes: &[u8]) -> bool {
  matches!(detect_image_format(bytes), Some(DetectedImageFormat::Gif))
}

#[cfg(feature = "gif")]
fn gif_decode_error(error: gif::DecodingError) -> ImageError {
  ImageError::Decoding(DecodingError::new(ImageFormat::Gif.into(), error))
}

#[cfg(feature = "gif")]
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
#[cfg(feature = "gif")]
pub(crate) fn gif_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let decoder = gif_decoder(bytes)?;
  Ok((decoder.width() as u32, decoder.height() as u32))
}

/// Per-frame delays in milliseconds, in stream order (first frame included),
/// without decoding any pixels. Uses the same `delay * 10` (min 1ms) rule as
/// [`decode_gif_frames`], so the durations line up with decoded frames.
#[cfg(feature = "gif")]
pub(crate) fn gif_frame_infos(bytes: &[u8]) -> ImageResult<Box<[FrameInfo]>> {
  let mut options = DecodeOptions::new();
  options.skip_frame_decoding(true);
  let mut decoder = options
    .read_info(Cursor::new(bytes))
    .map_err(gif_decode_error)?;

  let (width, height) = (decoder.width() as u32, decoder.height() as u32);
  if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
    return Err(pixel_budget_error(width, height));
  }

  // Stop at the same budgets as `decode_gif_frames`, so the timeline covers
  // exactly the frames that are decodable.
  let frame_pixels = decoder.width() as u64 * decoder.height() as u64;
  let mut total_pixels = 0_u64;
  let mut frames = Vec::new();
  loop {
    if frames.len() >= MAX_ANIMATION_FRAMES {
      break;
    }

    match decoder.read_next_frame() {
      Ok(Some(frame)) => {
        total_pixels += frame_pixels;
        if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
          break;
        }
        frames.push(FrameInfo {
          rect: (
            frame.left as u32,
            frame.top as u32,
            frame.width as u32,
            frame.height as u32,
          ),
          duration_ms: (frame.delay as u32 * 10).max(1),
          // A transparent index lets what is under the frame show through.
          blends: frame.transparent.is_some(),
          dispose: match frame.dispose {
            DisposalMethod::Background => Dispose::Background,
            DisposalMethod::Previous => Dispose::Previous,
            _ => Dispose::Keep,
          },
        });
      }
      Ok(None) => break,
      Err(error) if frames.is_empty() => return Err(gif_decode_error(error)),
      Err(_) => break,
    }
  }

  Ok(frames.into())
}

/// Rows and columns of the rect that fall inside the canvas, plus the rect's
/// unclamped pixel stride.
#[cfg(feature = "gif")]
fn clamped_span(rect: Rect<u32>, canvas_width: u32, canvas_height: u32) -> (u32, usize, usize) {
  let rows = rect.bottom.min(canvas_height).saturating_sub(rect.top);
  let cols = rect.right.min(canvas_width).saturating_sub(rect.left) as usize;
  let stride = (rect.right - rect.left) as usize;
  (rows, cols, stride)
}

/// Overwrites the canvas rect with the frame's non-transparent pixels
/// (straight-alpha RGBA; GIF alpha is 0 or 255).
#[cfg(feature = "gif")]
fn blit_frame(canvas: &mut [u8], canvas_size: (u32, u32), rect: Rect<u32>, pixels: &[u8]) {
  let (rows, cols, stride) = clamped_span(rect, canvas_size.0, canvas_size.1);
  for row in 0..rows {
    let src_row = row as usize * stride * 4;
    let dst_row = ((rect.top + row) * canvas_size.0 + rect.left) as usize * 4;
    for col in 0..cols {
      let src = src_row + col * 4;
      if pixels[src + 3] != 0 {
        let dst = dst_row + col * 4;
        canvas[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
      }
    }
  }
}

#[cfg(feature = "gif")]
fn clear_rect(canvas: &mut [u8], canvas_size: (u32, u32), rect: Rect<u32>) {
  let (rows, cols, _) = clamped_span(rect, canvas_size.0, canvas_size.1);
  for row in 0..rows {
    let dst_row = ((rect.top + row) * canvas_size.0 + rect.left) as usize * 4;
    canvas[dst_row..dst_row + cols * 4].fill(0);
  }
}

/// Decodes GIF frames in stream order, passing each frame past the first
/// `skip` to `push`, up to `limit` pushed frames. Returns whether the stream
/// ended. A mid-stream decode error or a blown [`MAX_ANIMATION_TOTAL_PIXELS`] budget
/// truncates the timeline (reported as ended); only a stream with no decodable
/// first frame errors.
///
/// With `target` set (and smaller than the canvas), pushed frames are resampled
/// to that size; compositing always happens at canvas size.
///
/// Compositing matches the `image` crate (and browsers): frames blend over a
/// transparent canvas, `Keep` persists the composited result, `Background`
/// clears the frame rect, `Previous` restores the pre-frame canvas.
#[cfg(feature = "gif")]
pub(crate) fn decode_gif_frames(
  bytes: &[u8],
  skip: usize,
  limit: Option<usize>,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
  mut push: impl FnMut(Arc<ImageBuffer>),
) -> ImageResult<bool> {
  let mut decoder = gif_decoder(bytes)?;
  let (width, height) = (decoder.width() as u32, decoder.height() as u32);
  let target = target.filter(|&(w, h, _)| w < width || h < height);

  // GIF alpha is 0 or 255, so the canvas is valid premultiplied RGBA as-is:
  // blits copy opaque pixels (premultiply is identity) and cleared pixels are
  // all-zero. Emitted frames skip the premultiply pass entirely.
  let mut canvas = vec![0_u8; width as usize * height as usize * 4];
  let mut scratch = Vec::new();
  let mut total_pixels: u64 = 0;
  let mut pushed = 0_usize;
  let mut index = 0_usize;

  loop {
    // The same cap `gif_frame_infos` stops at, so a decoded frame always has a
    // duration to go with it.
    if index >= MAX_ANIMATION_FRAMES {
      return Ok(false);
    }
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
    if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
      return Ok(true);
    }

    let rect = Rect {
      left: frame.left as u32,
      top: frame.top as u32,
      right: frame.left as u32 + frame.width as u32,
      bottom: frame.top as u32 + frame.height as u32,
    };
    let dispose = frame.dispose;

    scratch.resize(decoder.buffer_size(), 0);
    if let Err(error) = decoder.read_into_buffer(&mut scratch) {
      if current == 0 {
        return Err(gif_decode_error(error));
      }
      return Ok(true);
    }

    // The emitted frame is the canvas after compositing, before disposal.
    // With a target, the composited canvas resamples down instead of cloning.
    let keep = current >= skip;
    let last_needed = keep && limit.is_some_and(|limit| pushed + 1 >= limit);
    let buffer = match dispose {
      DisposalMethod::Any | DisposalMethod::Keep => {
        blit_frame(&mut canvas, (width, height), rect, &scratch);
        if !keep {
          None
        } else if let Some((w, h, algorithm)) = target {
          Some(resample_premultiplied(
            &canvas,
            (width, height),
            (w, h),
            algorithm,
          ))
        } else if last_needed {
          // The canvas is never read again: emit it without cloning.
          Some(ImageBuffer::from_premultiplied_rgba(
            take(&mut canvas),
            width,
            height,
          ))
        } else {
          Some(ImageBuffer::from_premultiplied_rgba(
            canvas.clone(),
            width,
            height,
          ))
        }
      }
      DisposalMethod::Background | DisposalMethod::Previous => {
        let buffer = keep.then(|| {
          let mut out = canvas.clone();
          blit_frame(&mut out, (width, height), rect, &scratch);
          match target {
            Some((w, h, algorithm)) => {
              resample_premultiplied(&out, (width, height), (w, h), algorithm)
            }
            None => ImageBuffer::from_premultiplied_rgba(out, width, height),
          }
        });
        if matches!(dispose, DisposalMethod::Background) {
          clear_rect(&mut canvas, (width, height), rect);
        }
        buffer
      }
    };

    if let Some(buffer) = buffer {
      push(Arc::new(buffer.ok_or_else(invalid_buffer_error)?));
      pushed += 1;
      if limit.is_some_and(|limit| pushed >= limit) {
        return Ok(false);
      }
    }
  }

  Ok(true)
}

/// Decodes frame `index` on its own when the GIF's frame headers show it covers
/// the whole canvas opaquely. Frames before it are advanced past by header,
/// never decoded.
#[cfg(feature = "gif")]
pub(crate) fn decode_gif_frame_alone(
  bytes: &[u8],
  index: usize,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
) -> Option<ImageBuffer> {
  let mut decoder = gif_decoder(bytes).ok()?;
  let (canvas_width, canvas_height) = (decoder.width() as u32, decoder.height() as u32);

  for _ in 0..index {
    decoder.next_frame_info().ok()??;
  }

  let frame = decoder.next_frame_info().ok()??;
  let rect = Rect {
    left: frame.left as u32,
    top: frame.top as u32,
    right: frame.left as u32 + frame.width as u32,
    bottom: frame.top as u32 + frame.height as u32,
  };

  let mut pixels = vec![0; decoder.buffer_size()];
  decoder.read_into_buffer(&mut pixels).ok()?;

  // GIF alpha is 0 or 255, so a frame over a cleared canvas is already valid
  // premultiplied RGBA.
  let buffer = if rect.left == 0
    && rect.top == 0
    && rect.right == canvas_width
    && rect.bottom == canvas_height
  {
    ImageBuffer::from_premultiplied_rgba(pixels, canvas_width, canvas_height)?
  } else {
    let mut canvas = vec![0; canvas_width as usize * canvas_height as usize * 4];
    blit_frame(&mut canvas, (canvas_width, canvas_height), rect, &pixels);
    ImageBuffer::from_premultiplied_rgba(canvas, canvas_width, canvas_height)?
  };

  fit_to_target(buffer, target).ok()
}

#[cfg(not(feature = "gif"))]
pub(crate) fn gif_dimensions(_bytes: &[u8]) -> ImageResult<(u32, u32)> {
  Err(format_compiled_out_error())
}

#[cfg(not(feature = "gif"))]
pub(crate) fn gif_frame_infos(_bytes: &[u8]) -> ImageResult<Box<[FrameInfo]>> {
  Err(format_compiled_out_error())
}

#[cfg(not(feature = "gif"))]
pub(crate) fn decode_gif_frame_alone(
  _bytes: &[u8],
  _index: usize,
  _target: Option<(u32, u32, ImageScalingAlgorithm)>,
) -> Option<ImageBuffer> {
  None
}

#[cfg(not(feature = "gif"))]
pub(crate) fn decode_gif_frames(
  _bytes: &[u8],
  _skip: usize,
  _limit: Option<usize>,
  _target: Option<(u32, u32, ImageScalingAlgorithm)>,
  _push: impl FnMut(Arc<ImageBuffer>),
) -> ImageResult<bool> {
  Err(format_compiled_out_error())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::resources::image_decoder::rgba_to_buffer;

  #[cfg(feature = "gif")]
  fn rgba_patch(width: u16, height: u16, rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat(width as usize * height as usize)
  }

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
  fn encode_test_gif(width: u16, height: u16, frames: &[gif::Frame<'static>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = gif::Encoder::new(&mut bytes, width, height, &[]).unwrap();
    for frame in frames {
      encoder.write_frame(frame).unwrap();
    }
    drop(encoder);
    bytes
  }

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
  fn our_frames(bytes: &[u8], skip: usize) -> Vec<(Vec<u8>, u32)> {
    let infos = gif_frame_infos(bytes).unwrap();
    let mut frames = Vec::new();
    let ended = decode_gif_frames(bytes, skip, None, None, |buffer| {
      frames.push((
        buffer.data().to_vec(),
        infos[skip + frames.len()].duration_ms,
      ));
    })
    .unwrap();
    assert!(ended);
    frames
  }

  #[cfg(feature = "gif")]
  fn assert_matches_reference(bytes: &[u8]) {
    let reference = reference_frames(bytes);
    assert_eq!(our_frames(bytes, 0), reference);
    assert_eq!(our_frames(bytes, 1), reference[1..]);
  }

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
  #[test]
  fn gif_compositing_matches_image_crate_for_interlaced_frames() {
    let mut striped = Vec::new();
    for row in 0..8_u8 {
      let shade = row * 32;
      striped.extend(rgba_patch(8, 1, [shade, 255 - shade, row, 255]));
    }
    let mut interlaced = test_frame(8, 8, (0, 0), striped, DisposalMethod::Keep, 1);
    interlaced.interlaced = true;

    let bytes = encode_test_gif(
      8,
      8,
      &[
        test_frame(
          8,
          8,
          (0, 0),
          rgba_patch(8, 8, [255, 0, 0, 255]),
          DisposalMethod::Keep,
          1,
        ),
        interlaced,
      ],
    );
    assert_matches_reference(&bytes);
  }

  #[cfg(feature = "gif")]
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

  #[cfg(feature = "gif")]
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
    let ended = decode_gif_frames(&bytes, 0, Some(1), None, |frame| frames.push(frame)).unwrap();
    assert!(!ended);
    assert_eq!(frames.len(), 1);
  }
}

//! GIF sizing and timelines, composited the way browsers do.

use std::{io::Cursor, mem::take, sync::Arc};

use gif::{ColorOutput, DecodeOptions, Decoder as GifDecoder, DisposalMethod, Frame};

use super::{
  DecodeError, DecodeResult, DecodeTarget, DetectedImageFormat, Dispose, FrameInfo,
  MAX_ANIMATION_FRAMES, MAX_ANIMATION_TOTAL_PIXELS, check_dimensions, detect_image_format,
  fit_to_target,
};
use crate::{geometry::Rect, resources::image_buffer::ImageBuffer};

pub(crate) fn is_gif(bytes: &[u8]) -> bool {
  matches!(detect_image_format(bytes), Some(DetectedImageFormat::Gif))
}

fn gif_decode_error(error: gif::DecodingError) -> DecodeError {
  DecodeError::format(DetectedImageFormat::Gif, error)
}

fn gif_decoder(bytes: &[u8]) -> DecodeResult<GifDecoder<Cursor<&[u8]>>> {
  let mut options = DecodeOptions::new();
  options.set_color_output(ColorOutput::RGBA);

  read_gif(options, bytes)
}

/// Reads the GIF header under `options`, rejecting a logical screen over the dimension limit.
fn read_gif(options: DecodeOptions, bytes: &[u8]) -> DecodeResult<GifDecoder<Cursor<&[u8]>>> {
  let decoder = options
    .read_info(Cursor::new(bytes))
    .map_err(gif_decode_error)?;
  check_dimensions(u32::from(decoder.width()), u32::from(decoder.height()))?;
  Ok(decoder)
}

/// Rejects a frame over the dimension limit, which the logical screen's check does not cover.
fn check_frame_size(frame: &Frame) -> DecodeResult<()> {
  check_dimensions(u32::from(frame.width), u32::from(frame.height))
}

/// The frame's rectangle within the logical screen.
fn frame_rect(frame: &Frame) -> Rect<u32> {
  Rect {
    left: frame.left as u32,
    top: frame.top as u32,
    right: frame.left as u32 + frame.width as u32,
    bottom: frame.top as u32 + frame.height as u32,
  }
}

/// GIF logical screen dimensions from the header; decodes no frame.
pub(crate) fn gif_dimensions(bytes: &[u8]) -> DecodeResult<(u32, u32)> {
  let decoder = gif_decoder(bytes)?;
  Ok((decoder.width() as u32, decoder.height() as u32))
}

/// Per-frame delays in milliseconds, in stream order (first frame included), without decoding any
/// pixels.
pub(crate) fn gif_frame_infos(bytes: &[u8]) -> DecodeResult<Box<[FrameInfo]>> {
  let mut options = DecodeOptions::new();
  options.skip_frame_decoding(true);
  let mut decoder = read_gif(options, bytes)?;

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
        if let Err(error) = check_frame_size(frame) {
          if frames.is_empty() {
            return Err(error);
          }
          break;
        }

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

/// Rows and columns of the rect that fall inside the canvas, plus the rect's unclamped pixel
/// stride.
fn clamped_span(rect: Rect<u32>, canvas_width: u32, canvas_height: u32) -> (u32, usize, usize) {
  let cols = rect.right.min(canvas_width).saturating_sub(rect.left) as usize;
  let rows = if cols == 0 {
    0
  } else {
    rect.bottom.min(canvas_height).saturating_sub(rect.top)
  };
  let stride = (rect.right - rect.left) as usize;
  (rows, cols, stride)
}

/// Overwrites the canvas rect with the frame's non-transparent pixels (straight-alpha RGBA; GIF
/// alpha is 0 or 255).
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

fn clear_rect(canvas: &mut [u8], canvas_size: (u32, u32), rect: Rect<u32>) {
  let (rows, cols, _) = clamped_span(rect, canvas_size.0, canvas_size.1);
  for row in 0..rows {
    let dst_row = ((rect.top + row) * canvas_size.0 + rect.left) as usize * 4;
    canvas[dst_row..dst_row + cols * 4].fill(0);
  }
}

/// Decodes GIF frames in stream order, passing each frame past the first `skip` to `push`, up to
/// `limit` pushed frames.
pub(crate) fn decode_gif_frames(
  bytes: &[u8],
  skip: usize,
  limit: Option<usize>,
  target: Option<DecodeTarget>,
  mut push: impl FnMut(Arc<ImageBuffer>),
) -> DecodeResult<bool> {
  let mut decoder = gif_decoder(bytes)?;
  let (width, height) = (decoder.width() as u32, decoder.height() as u32);
  let target = target.filter(|target| target.shrinks(width, height));

  // GIF alpha is 0 or 255, so the canvas is already premultiplied.
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

    if let Err(error) = check_frame_size(frame) {
      if current == 0 {
        return Err(error);
      }
      return Ok(true);
    }

    total_pixels += width as u64 * height as u64;
    if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
      return Ok(true);
    }

    let rect = frame_rect(frame);
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
        } else if let Some(target) = target {
          Some(target.resample(&canvas, (width, height)))
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
            Some(target) => target.resample(&out, (width, height)),
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
      push(Arc::new(buffer.ok_or(DecodeError::InvalidBuffer)?));
      pushed += 1;
      if limit.is_some_and(|limit| pushed >= limit) {
        return Ok(false);
      }
    }
  }

  Ok(true)
}

/// Decodes frame `index` on its own when the GIF's frame headers show it covers the whole canvas
/// opaquely.
pub(crate) fn decode_gif_frame_alone(
  bytes: &[u8],
  index: usize,
  target: Option<DecodeTarget>,
) -> Option<ImageBuffer> {
  let mut decoder = gif_decoder(bytes).ok()?;
  let (canvas_width, canvas_height) = (decoder.width() as u32, decoder.height() as u32);

  for _ in 0..index {
    decoder.next_frame_info().ok()??;
  }

  let frame = decoder.next_frame_info().ok()??;
  check_frame_size(frame).ok()?;

  let rect = frame_rect(frame);

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn frames_starting_outside_the_canvas_clear_nothing() {
    let mut canvas = vec![255_u8; 4 * 4 * 4];
    let rect = Rect {
      left: 6,
      right: 8,
      top: 3,
      bottom: 5,
    };

    clear_rect(&mut canvas, (4, 4), rect);
    blit_frame(&mut canvas, (4, 4), rect, &[0; 2 * 2 * 4]);
    assert!(canvas.iter().all(|byte| *byte == 255));
  }

  /// A 1x1 logical screen holding one 65535x65535 frame.
  fn oversized_frame_gif() -> Vec<u8> {
    let mut bytes = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff".to_vec();

    bytes.extend_from_slice(b"\x2c\x00\x00\x00\x00\xff\xff\xff\xff\x00\x02\x02\x44\x01\x00\x3b");
    bytes
  }

  #[test]
  fn frames_over_the_dimension_limit_fail_before_decoding() {
    let bytes = oversized_frame_gif();

    assert!(matches!(
      decode_gif_frames(&bytes, 0, None, None, |_| {}),
      Err(DecodeError::OverBudget(65535, 65535))
    ));
    assert!(matches!(
      gif_frame_infos(&bytes),
      Err(DecodeError::OverBudget(65535, 65535))
    ));
    assert!(decode_gif_frame_alone(&bytes, 0, None).is_none());
  }

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

  /// The GIF's frames composited as the spec describes, premultiplied, with their durations: an
  /// independent check on the decoder's compositing.
  fn reference_frames(bytes: &[u8]) -> Vec<(Vec<u8>, u32)> {
    let mut options = DecodeOptions::new();

    options.set_color_output(ColorOutput::RGBA);

    let mut decoder = options.read_info(Cursor::new(bytes)).unwrap();
    let (width, height) = (usize::from(decoder.width()), usize::from(decoder.height()));
    let mut canvas = vec![0_u8; width * height * 4];
    let mut frames = Vec::new();

    while let Some(frame) = decoder.read_next_frame().unwrap() {
      let previous = canvas.clone();
      let (left, top) = (usize::from(frame.left), usize::from(frame.top));
      let (frame_width, frame_height) = (usize::from(frame.width), usize::from(frame.height));
      let inside = |x: usize, y: usize| x < width && y < height;

      for (index, pixel) in frame.buffer.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = (left + index % frame_width, top + index / frame_width);

        if pixel[3] != 0 && inside(x, y) {
          canvas[(y * width + x) * 4..][..4].copy_from_slice(pixel);
        }
      }

      let buffer =
        ImageBuffer::from_rgba_bytes(canvas.clone(), width as u32, height as u32).unwrap();

      frames.push((buffer.data().to_vec(), (u32::from(frame.delay) * 10).max(1)));

      match frame.dispose {
        DisposalMethod::Background => {
          for y in top..top + frame_height {
            for x in (left..left + frame_width).filter(|&x| inside(x, y)) {
              canvas[(y * width + x) * 4..][..4].fill(0);
            }
          }
        }
        DisposalMethod::Previous => canvas = previous,
        _ => {}
      }
    }

    frames
  }

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

  fn assert_matches_reference(bytes: &[u8]) {
    let reference = reference_frames(bytes);
    assert_eq!(our_frames(bytes, 0), reference);
    assert_eq!(our_frames(bytes, 1), reference[1..]);
  }

  #[test]
  fn gif_compositing_matches_the_reference_for_keep_disposal() {
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
  fn gif_compositing_matches_the_reference_for_transparent_patches() {
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
  fn gif_compositing_matches_the_reference_for_background_disposal() {
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
  fn gif_compositing_matches_the_reference_for_previous_disposal() {
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
  fn gif_compositing_matches_the_reference_for_interlaced_frames() {
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

  #[test]
  fn gif_zero_delay_clamps_to_one_ms() {
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
    let ended = decode_gif_frames(&bytes, 0, Some(1), None, |frame| frames.push(frame)).unwrap();
    assert!(!ended);
    assert_eq!(frames.len(), 1);
  }
}

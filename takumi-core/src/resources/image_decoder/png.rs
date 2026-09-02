//! PNG stills, streamed downscaling, and APNG timelines.

use std::{io::Cursor, sync::Arc};

use image::{ImageError, ImageFormat, ImageResult, codecs::png::PngDecoder, error::DecodingError};
use png::{
  BitDepth, BlendOp, ColorType, Decoder as PngRowDecoder, DisposeOp, FrameControl, Transformations,
};

use super::{
  Dispose, FrameInfo, MAX_ANIMATION_FRAMES, MAX_ANIMATION_TOTAL_PIXELS, MAX_IMAGE_DIMENSION,
  covers_canvas, decode_with_image_crate, fit_to_target, invalid_buffer_error, pixel_budget_error,
};
use crate::{
  resources::{
    image_buffer::{ImageBuffer, premultiply_rgba_in_place},
    image_resampler::{StreamResampler, resample_premultiplied},
  },
  style::ImageScalingAlgorithm,
};

pub(super) const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

pub(crate) fn decode_png(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(PngDecoder::new(Cursor::new(bytes))?, ImageFormat::Png)
}

fn png_decode_error(error: png::DecodingError) -> ImageError {
  ImageError::Decoding(DecodingError::new(ImageFormat::Png.into(), error))
}

/// Streams a non-interlaced PNG through [`StreamResampler`]. `None` means the
/// input isn't eligible (not a PNG, interlaced, unsupported layout, or no
/// downscale) and the caller should decode fully; errors after eligibility are
/// real decode failures.
pub(super) fn decode_png_scaled(
  bytes: &[u8],
  width: u32,
  height: u32,
  algorithm: ImageScalingAlgorithm,
) -> Option<ImageResult<ImageBuffer>> {
  if !bytes.starts_with(&PNG_SIGNATURE) {
    return None;
  }

  let mut decoder = PngRowDecoder::new(Cursor::new(bytes));
  decoder.set_transformations(
    Transformations::EXPAND | Transformations::STRIP_16 | Transformations::ALPHA,
  );
  let mut reader = decoder.read_info().ok()?;

  let info = reader.info();
  let (native_width, native_height) = (info.width, info.height);
  if info.interlaced
    || native_width == 0
    || native_height == 0
    || native_width > MAX_IMAGE_DIMENSION
    || native_height > MAX_IMAGE_DIMENSION
    || (width >= native_width && height >= native_height)
  {
    return None;
  }

  let channels = match reader.output_color_type() {
    (ColorType::Rgba, BitDepth::Eight) => 4,
    (ColorType::GrayscaleAlpha, BitDepth::Eight) => 2,
    _ => return None,
  };

  let mut resampler =
    StreamResampler::new((native_width, native_height), (width, height), algorithm);
  let mut rgba_row = vec![0_u8; native_width as usize * 4];

  loop {
    let row = match reader.next_row() {
      Ok(Some(row)) => row,
      Ok(None) => break,
      Err(error) => return Some(Err(png_decode_error(error))),
    };

    match channels {
      4 => rgba_row.copy_from_slice(row.data()),
      _ => {
        for (rgba, pixel) in rgba_row
          .as_chunks_mut::<4>()
          .0
          .iter_mut()
          .zip(row.data().as_chunks::<2>().0)
        {
          rgba[0] = pixel[0];
          rgba[1] = pixel[0];
          rgba[2] = pixel[0];
          rgba[3] = pixel[1];
        }
      }
    }

    premultiply_rgba_in_place(&mut rgba_row);
    resampler.push_row(&rgba_row);
  }

  Some(resampler.finish().ok_or_else(invalid_buffer_error))
}

/// Whether the PNG carries an `acTL` animation control chunk.
pub(crate) fn is_apng(bytes: &[u8]) -> bool {
  bytes.starts_with(&PNG_SIGNATURE) && png_chunks(bytes).any(|(id, _)| &id == b"acTL")
}

/// Chunks of a PNG stream, as `(type, data)` pairs.
fn png_chunks(bytes: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
  let mut offset = PNG_SIGNATURE.len();
  std::iter::from_fn(move || {
    let header: [u8; 8] = bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
    let start = offset + 8;
    let data = bytes.get(start..start.checked_add(length)?)?;
    offset = start.checked_add(length)?.checked_add(4)?;
    Some(([header[4], header[5], header[6], header[7]], data))
  })
}

pub(crate) fn apng_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let reader = apng_reader(bytes)?;
  let info = reader.info();
  Ok((info.width, info.height))
}

/// Per-frame delays in milliseconds, in stream order, read from the `fcTL`
/// chunks without decoding any pixels. Stops on the same frame and pixel
/// budgets as [`decode_apng_frames`], so a playback time never selects a frame
/// the decoder drops.
pub(crate) fn apng_frame_infos(bytes: &[u8]) -> ImageResult<Box<[FrameInfo]>> {
  let (width, height) = apng_dimensions(bytes)?;
  let frame_pixels = width as u64 * height as u64;
  let mut total_pixels = 0_u64;
  let mut frames = Vec::new();

  for (id, data) in png_chunks(bytes) {
    if &id != b"fcTL" {
      continue;
    }
    if frames.len() >= MAX_ANIMATION_FRAMES {
      break;
    }

    total_pixels += frame_pixels;
    if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
      break;
    }

    // `fcTL`: sequence, width, height, x, y, delay numerator, delay
    // denominator, dispose op, blend op.
    let Some(control) = data.get(..26) else {
      break;
    };
    let read_32 = |offset: usize| {
      u32::from_be_bytes([
        control[offset],
        control[offset + 1],
        control[offset + 2],
        control[offset + 3],
      ])
    };

    frames.push(FrameInfo {
      rect: (read_32(12), read_32(16), read_32(4), read_32(8)),
      duration_ms: apng_delay_ms(
        u16::from_be_bytes([control[20], control[21]]),
        u16::from_be_bytes([control[22], control[23]]),
      ),
      blends: control[25] == 1,
      dispose: match control[24] {
        1 => Dispose::Background,
        2 => Dispose::Previous,
        _ => Dispose::Keep,
      },
    });
  }

  Ok(frames.into())
}

/// An `fcTL` delay fraction in milliseconds. A zero denominator means hundredths
/// of a second, per the APNG spec.
fn apng_delay_ms(numerator: u16, denominator: u16) -> u32 {
  let denominator = if denominator == 0 { 100 } else { denominator };
  ((numerator as u64 * 1000) / denominator as u64).max(1) as u32
}

fn apng_reader(bytes: &[u8]) -> ImageResult<png::Reader<Cursor<&[u8]>>> {
  let mut decoder = PngRowDecoder::new(Cursor::new(bytes));
  decoder.set_transformations(
    Transformations::EXPAND | Transformations::STRIP_16 | Transformations::ALPHA,
  );

  let reader = decoder.read_info().map_err(png_decode_error)?;
  let info = reader.info();
  if info.width > MAX_IMAGE_DIMENSION || info.height > MAX_IMAGE_DIMENSION {
    return Err(pixel_budget_error(info.width, info.height));
  }

  Ok(reader)
}

/// Decodes frame `index` on its own, advancing through the earlier frames by
/// their headers alone, when its `fcTL` shows it covers the whole canvas and
/// replaces rather than blends. Returns `None` when the frame depends on its
/// predecessors.
///
/// This is the narrow case of Blink's dependency walk in
/// `ImageDecoder::FindRequiredPreviousFrame`: a full-canvas, non-blending frame
/// needs no starting state.
pub(crate) fn decode_apng_frame_alone(
  bytes: &[u8],
  index: usize,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
) -> Option<ImageBuffer> {
  let mut reader = apng_reader(bytes).ok()?;
  let (canvas_width, canvas_height) = (reader.info().width, reader.info().height);
  let channels = match reader.output_color_type() {
    (ColorType::Rgba, BitDepth::Eight) => 4,
    (ColorType::GrayscaleAlpha, BitDepth::Eight) => 2,
    _ => return None,
  };

  // A default image no `fcTL` claims sits outside the animation.
  if reader.info().frame_control.is_none() {
    reader.next_frame_info().ok()?;
  }
  for _ in 0..index {
    reader.next_frame_info().ok()?;
  }
  let frame = reader.info().frame_control?;

  let mut subframe = vec![0; reader.output_buffer_size()?];
  reader.next_frame(&mut subframe).ok()?;

  // A full-canvas replacing frame already is the canvas; anything else lands on
  // a cleared one, which is what its predecessors would have left behind.
  let buffer = if channels == 4
    && frame.blend_op == BlendOp::Source
    && covers_canvas(
      (frame.x_offset, frame.y_offset, frame.width, frame.height),
      (canvas_width, canvas_height),
    ) {
    premultiply_rgba_in_place(&mut subframe);
    ImageBuffer::from_premultiplied_rgba(subframe, canvas_width, canvas_height)?
  } else {
    let mut canvas = ApngCanvas::new(canvas_width, canvas_height);
    canvas.composite(frame, &subframe, channels);
    canvas.to_buffer(None).ok()?
  };

  fit_to_target(buffer, target).ok()
}

/// Decodes APNG frames in stream order, passing each frame past the first
/// `skip` to `push`, up to `limit` pushed frames. Returns whether the stream
/// ended. Mid-stream decode errors and a blown budget truncate the timeline
/// (reported as ended); only a stream with no decodable first frame errors.
///
/// A default image no `fcTL` claims is not part of the animation, so it is
/// decoded past and never enters the timeline.
pub(crate) fn decode_apng_frames(
  bytes: &[u8],
  skip: usize,
  limit: Option<usize>,
  target: Option<(u32, u32, ImageScalingAlgorithm)>,
  mut push: impl FnMut(Arc<ImageBuffer>),
) -> ImageResult<bool> {
  let mut reader = apng_reader(bytes)?;
  let (width, height) = (reader.info().width, reader.info().height);
  let target = target.filter(|&(w, h, _)| w < width || h < height);
  let channels = match reader.output_color_type() {
    (ColorType::Rgba, BitDepth::Eight) => 4,
    (ColorType::GrayscaleAlpha, BitDepth::Eight) => 2,
    _ => return Err(invalid_buffer_error()),
  };

  let mut canvas = ApngCanvas::new(width, height);
  let mut subframe = Vec::new();
  let mut total_pixels = 0_u64;
  let mut pushed = 0_usize;
  let mut index = 0_usize;

  // One read past the cap: an unclaimed default image costs a read without
  // taking a slot on the timeline.
  for read in 0..=MAX_ANIMATION_FRAMES {
    if index >= MAX_ANIMATION_FRAMES {
      return Ok(false);
    }
    if limit.is_some_and(|limit| pushed >= limit) {
      return Ok(false);
    }

    if read > 0 && reader.next_frame_info().is_err() {
      break;
    }

    let Some(size) = reader.output_buffer_size() else {
      return Ok(read > 0);
    };
    subframe.resize(size, 0);
    if let Err(error) = reader.next_frame(&mut subframe) {
      if read == 0 {
        return Err(png_decode_error(error));
      }
      return Ok(true);
    }

    // The default image sits outside the animation unless an `fcTL` claims it.
    let Some(frame) = reader.info().frame_control else {
      continue;
    };

    total_pixels += width as u64 * height as u64;
    if total_pixels > MAX_ANIMATION_TOTAL_PIXELS {
      return Ok(true);
    }

    canvas.composite(frame, &subframe, channels);

    let current = index;
    index += 1;
    if current >= skip {
      push(Arc::new(canvas.to_buffer(target)?));
      pushed += 1;
    }

    canvas.dispose(frame);
  }

  Ok(true)
}

/// The straight-alpha RGBA canvas an APNG's frames composite onto, plus the
/// copy `DisposeOp::Previous` restores.
struct ApngCanvas {
  pixels: Vec<u8>,
  restore: Vec<u8>,
  width: u32,
  height: u32,
}

impl ApngCanvas {
  fn new(width: u32, height: u32) -> Self {
    Self {
      pixels: vec![0; width as usize * height as usize * 4],
      restore: Vec::new(),
      width,
      height,
    }
  }

  fn clamped_span(&self, frame: FrameControl) -> (u32, u32) {
    (
      frame.height.min(self.height.saturating_sub(frame.y_offset)),
      frame.width.min(self.width.saturating_sub(frame.x_offset)),
    )
  }

  /// Draws one subframe at its `fcTL` offset.
  fn composite(&mut self, frame: FrameControl, subframe: &[u8], channels: usize) {
    if frame.dispose_op == DisposeOp::Previous {
      self.restore.clear();
      self.restore.extend_from_slice(&self.pixels);
    }

    let (rows, columns) = self.clamped_span(frame);
    for row in 0..rows {
      let source_row = row as usize * frame.width as usize * channels;
      let target_row = ((frame.y_offset + row) * self.width + frame.x_offset) as usize * 4;

      // Replacing RGBA is the same bytes on both sides, so the row copies whole.
      if channels == 4 && frame.blend_op == BlendOp::Source {
        let span = columns as usize * 4;
        self.pixels[target_row..target_row + span]
          .copy_from_slice(&subframe[source_row..source_row + span]);
        continue;
      }

      for column in 0..columns {
        let source = source_row + column as usize * channels;
        let pixel = match channels {
          4 => [
            subframe[source],
            subframe[source + 1],
            subframe[source + 2],
            subframe[source + 3],
          ],
          _ => [
            subframe[source],
            subframe[source],
            subframe[source],
            subframe[source + 1],
          ],
        };

        let target = target_row + column as usize * 4;
        match frame.blend_op {
          BlendOp::Source => self.pixels[target..target + 4].copy_from_slice(&pixel),
          BlendOp::Over => blend_over(&mut self.pixels[target..target + 4], pixel),
        }
      }
    }
  }

  /// Applies the frame's disposal, readying the canvas for the one after it.
  fn dispose(&mut self, frame: FrameControl) {
    match frame.dispose_op {
      DisposeOp::None => {}
      DisposeOp::Background => {
        let (rows, columns) = self.clamped_span(frame);
        for row in 0..rows {
          let start = ((frame.y_offset + row) * self.width + frame.x_offset) as usize * 4;
          self.pixels[start..start + columns as usize * 4].fill(0);
        }
      }
      DisposeOp::Previous => self.pixels.copy_from_slice(&self.restore),
    }
  }

  fn to_buffer(
    &self,
    target: Option<(u32, u32, ImageScalingAlgorithm)>,
  ) -> ImageResult<ImageBuffer> {
    let mut premultiplied = self.pixels.clone();
    premultiply_rgba_in_place(&mut premultiplied);

    match target {
      Some((width, height, algorithm)) => resample_premultiplied(
        &premultiplied,
        (self.width, self.height),
        (width, height),
        algorithm,
      ),
      None => ImageBuffer::from_premultiplied_rgba(premultiplied, self.width, self.height),
    }
    .ok_or_else(invalid_buffer_error)
  }
}

/// Straight-alpha source-over, as the APNG spec defines `BlendOp::Over`.
fn blend_over(target: &mut [u8], source: [u8; 4]) {
  if source[3] == u8::MAX {
    target.copy_from_slice(&source);
    return;
  }
  if source[3] == 0 {
    return;
  }

  let source_alpha = source[3] as u32;
  let target_alpha = target[3] as u32;
  let out_alpha = source_alpha + target_alpha * (255 - source_alpha) / 255;
  for channel in 0..3 {
    let source_part = source[channel] as u32 * source_alpha;
    let target_part = target[channel] as u32 * target_alpha * (255 - source_alpha) / 255;
    target[channel] = ((source_part + target_part) / out_alpha) as u8;
  }
  target[3] = out_alpha as u8;
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::RgbaImage;

  use crate::resources::image_decoder::{decode_bitmap_scaled, decode_image};

  fn assert_streamed_matches_full(bytes: &[u8], width: u32, height: u32) {
    for algorithm in [
      ImageScalingAlgorithm::Auto,
      ImageScalingAlgorithm::Smooth,
      ImageScalingAlgorithm::Pixelated,
    ] {
      let streamed = decode_bitmap_scaled(bytes, width, height, algorithm).unwrap();
      let full = decode_image(bytes).unwrap();
      let resized = resample_premultiplied(
        full.data(),
        (full.width(), full.height()),
        (width, height),
        algorithm,
      )
      .unwrap();
      assert_eq!(streamed.data(), resized.data(), "{algorithm:?}");
    }
  }

  #[test]
  fn png_streaming_matches_full_decode_for_rgba() {
    let mut image = RgbaImage::new(40, 30);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
      *pixel = image::Rgba([(x * 6) as u8, (y * 8) as u8, ((x + y) * 3) as u8, 200]);
    }
    let mut bytes = Vec::new();
    image
      .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
      .unwrap();

    assert_streamed_matches_full(&bytes, 13, 9);
  }

  #[test]
  fn png_streaming_matches_full_decode_for_grayscale_alpha() {
    let mut image = image::GrayAlphaImage::new(32, 24);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
      *pixel = image::LumaA([(x * 8) as u8, (255 - y * 4) as u8]);
    }
    let mut bytes = Vec::new();
    image
      .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
      .unwrap();

    assert_streamed_matches_full(&bytes, 11, 7);
  }

  #[test]
  fn real_png_streaming_matches_full_decode() {
    let bytes = include_bytes!("../../../../assets/images/yeecord.png");
    let full = decode_image(bytes).unwrap();
    assert_streamed_matches_full(bytes, full.width() / 3, full.height() / 3);
  }
}

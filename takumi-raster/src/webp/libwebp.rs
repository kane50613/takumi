use std::{borrow::Borrow, borrow::Cow, io::Write, mem::MaybeUninit, slice};

use image::RgbaImage;
use libwebp_sys::*;
#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::{
  Result,
  error::WebPError,
  webp::{EncodedFrame, FrameRegion, U24_MAX, UniqueFrame, write_riff_container},
  write::{AnimatedWebpOptions, AnimationFrame, Bitmap, Quality},
};

/// libwebp reads `quality` as compression effort when encoding VP8L, not as visual
/// quality: the output is bit-exact at any value.
const LOSSLESS_EFFORT: f32 = 50.0;

fn webp_config(lossless: bool, quality: u8, speed: u8) -> Result<WebPConfig> {
  let mut config = WebPConfig::new_with_preset(
    WebPPreset::WEBP_PRESET_TEXT,
    if lossless {
      LOSSLESS_EFFORT
    } else {
      quality.clamp(0, 100) as f32
    },
  )
  .map_err(|_| WebPError::EncoderSetupFailed)?;

  config.lossless = if lossless { 1 } else { 0 };
  config.method = speed.clamp(0, 6) as i32;
  if unsafe { WebPValidateConfig(&config) } == 0 {
    return Err(WebPError::EncoderSetupFailed.into());
  }

  Ok(config)
}

/// Rejects a canvas the container's 24-bit size fields cannot hold, then builds
/// the encoder config for the animation.
fn animation_config(width: u32, height: u32, options: &AnimatedWebpOptions) -> Result<WebPConfig> {
  if !(1..=U24_MAX + 1).contains(&width) || !(1..=U24_MAX + 1).contains(&height) {
    return Err(
      WebPError::InvalidFrameDimensions {
        width,
        height,
        max: U24_MAX + 1,
      }
      .into(),
    );
  }

  webp_config(
    options.lossless,
    options.quality,
    options.speed.unwrap_or(1),
  )
}

fn import_rgba_picture(
  image: &RgbaImage,
  region: &FrameRegion,
  config: &WebPConfig,
) -> Result<WebPPicture> {
  let mut picture = WebPPicture::new().map_err(|_| WebPError::EncoderSetupFailed)?;

  picture.width = region.width as i32;
  picture.height = region.height as i32;
  // Import() subsamples to YUV420 unless use_argb is set; WebPEncode converts back
  // for VP8L, so a lossless encode would round-trip through chroma subsampling.
  // https://github.com/webmproject/libwebp/blob/main/src/enc/picture_csp_enc.c
  picture.use_argb = config.lossless;

  let stride = image.width() as usize * 4;
  let region_start = region.y as usize * stride + region.x as usize * 4;
  let import_ok = unsafe {
    WebPPictureImportRGBA(
      &mut picture,
      image.as_raw()[region_start..].as_ptr(),
      stride as i32,
    )
  };

  if import_ok == 0 {
    unsafe { WebPPictureFree(&mut picture) };
    return Err(
      WebPError::EncodeFailedWithCode {
        error_code: format!("{:?}", picture.error_code),
      }
      .into(),
    );
  }

  Ok(picture)
}

struct WebPMemoryBuffer {
  writer: WebPMemoryWriter,
}

unsafe impl Send for WebPMemoryBuffer {}

impl WebPMemoryBuffer {
  fn new() -> Self {
    let mut writer = MaybeUninit::<WebPMemoryWriter>::uninit();
    unsafe { WebPMemoryWriterInit(writer.as_mut_ptr()) };
    Self {
      writer: unsafe { writer.assume_init() },
    }
  }

  fn as_mut_ptr(&mut self) -> *mut WebPMemoryWriter {
    &raw mut self.writer
  }
}

impl AsRef<[u8]> for WebPMemoryBuffer {
  fn as_ref(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.writer.mem, self.writer.size) }
  }
}

impl Drop for WebPMemoryBuffer {
  fn drop(&mut self) {
    unsafe { WebPMemoryWriterClear(&raw mut self.writer) };
  }
}

/// Encodes `region` of `image` into a complete WebP file in memory.
fn encode_picture(
  image: &RgbaImage,
  region: &FrameRegion,
  config: &WebPConfig,
) -> Result<WebPMemoryBuffer> {
  let mut picture = import_rgba_picture(image, region, config)?;
  let mut writer = WebPMemoryBuffer::new();
  picture.writer = Some(WebPMemoryWrite);
  picture.custom_ptr = writer.as_mut_ptr().cast();

  let encode_ok = unsafe { WebPEncode(config, &raw mut picture) };
  unsafe { WebPPictureFree(&raw mut picture) };

  if encode_ok == 0 {
    return Err(
      WebPError::EncodeFailedWithCode {
        error_code: format!("{:?}", picture.error_code),
      }
      .into(),
    );
  }

  Ok(writer)
}

fn encode_single_frame<T: Borrow<Bitmap>>(
  frame: &UniqueFrame<T>,
  config: &WebPConfig,
) -> Result<EncodedFrame<WebPMemoryBuffer>> {
  let encoded = encode_picture(
    frame.image.borrow().as_rgba(),
    &frame.placement.region,
    config,
  )?;

  EncodedFrame::new(encoded, frame.placement, frame.duration_ms)
    .ok_or_else(|| WebPError::InvalidEncodedData.into())
}

pub(crate) fn write_webp_lossy(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  quality: Quality,
) -> Result<()> {
  write_webp(image, destination, webp_config(false, quality.get(), 1)?)
}

pub(crate) fn write_webp_lossless(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
) -> Result<()> {
  write_webp(image, destination, webp_config(true, 0, 1)?)
}

fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  config: WebPConfig,
) -> Result<()> {
  let encoded = encode_picture(
    &image,
    &FrameRegion::full(image.width(), image.height()),
    &config,
  )?;

  destination.write_all(encoded.as_ref())?;
  Ok(())
}

fn collect_unique_frames<'a>(
  frames: &'a [AnimationFrame],
  frame_width: u32,
  frame_height: u32,
  options: &AnimatedWebpOptions,
) -> Result<Vec<UniqueFrame<&'a Bitmap>>> {
  let mut unique_frames = Vec::with_capacity(frames.len());
  let mut pending = UniqueFrame::first(&frames[0].image, frames[0].duration_ms, options);

  for frame in &frames[1..] {
    if frame.image.width() != frame_width || frame.image.height() != frame_height {
      return Err(WebPError::MixedFrameDimensions.into());
    }

    unique_frames.extend(pending.push(
      &frame.image,
      frame.duration_ms,
      frame_width,
      frame_height,
      options,
    ));
  }

  unique_frames.push(pending);
  Ok(unique_frames)
}

fn encode_frames(
  unique_frames: &[UniqueFrame<&Bitmap>],
  config: &WebPConfig,
) -> Result<Vec<EncodedFrame<WebPMemoryBuffer>>> {
  #[cfg(feature = "rayon")]
  const MIN_PARALLEL_FRAMES: usize = 4;

  #[cfg(feature = "rayon")]
  if unique_frames.len() >= MIN_PARALLEL_FRAMES {
    return unique_frames
      .par_iter()
      .with_min_len(MIN_PARALLEL_FRAMES)
      .map(|frame| encode_single_frame(frame, config))
      .collect();
  }

  unique_frames
    .iter()
    .map(|frame| encode_single_frame(frame, config))
    .collect()
}

/// Streams frames into an animated WebP a bounded chunk at a time, so peak
/// raw-pixel memory stays fixed while each chunk still encodes in parallel.
/// Produces the same bytes as [`write_animated_webp`] for the same frames.
pub(crate) fn encode_animated_webp<W, I>(
  mut frames: I,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()>
where
  W: Write,
  I: Iterator<Item = Result<AnimationFrame>>,
{
  let Some(first) = frames.next().transpose()? else {
    return Err(WebPError::EmptyAnimation.into());
  };

  let frame_width = first.image.width();
  let frame_height = first.image.height();
  let config = animation_config(frame_width, frame_height, &options)?;
  let chunk_capacity = frames_per_chunk(frame_width, frame_height);
  let mut encoded = Vec::new();
  let mut chunk = Vec::new();
  let mut pending = UniqueFrame::first(first.image, first.duration_ms, &options);

  for frame in frames {
    let frame = frame?;
    if frame.image.width() != frame_width || frame.image.height() != frame_height {
      return Err(WebPError::MixedFrameDimensions.into());
    }

    let Some(finished) = pending.push(
      frame.image,
      frame.duration_ms,
      frame_width,
      frame_height,
      &options,
    ) else {
      continue;
    };

    chunk.push(finished);
    if chunk.len() >= chunk_capacity {
      encode_frame_chunk(&mut chunk, &config, &mut encoded)?;
    }
  }
  chunk.push(pending);
  encode_frame_chunk(&mut chunk, &config, &mut encoded)?;

  write_riff_container(&encoded, frame_width, frame_height, destination, &options)
}

/// Frames to buffer per parallel encode pass. Sized so the buffered raw pixels
/// stay near a fixed budget whatever the frame dimensions, with at least one
/// frame per pass.
fn frames_per_chunk(width: u32, height: u32) -> usize {
  const CHUNK_MEMORY_BUDGET: usize = 64 * 1024 * 1024;
  let frame_bytes = (width as usize)
    .saturating_mul(height as usize)
    .saturating_mul(4)
    .max(1);
  (CHUNK_MEMORY_BUDGET / frame_bytes).max(1)
}

/// Encodes a chunk of unique frames (in parallel when `rayon` is enabled), appends
/// the results in order, and frees the raw frames.
fn encode_frame_chunk(
  chunk: &mut Vec<UniqueFrame<Bitmap>>,
  config: &WebPConfig,
  encoded: &mut Vec<EncodedFrame<WebPMemoryBuffer>>,
) -> Result<()> {
  #[cfg(feature = "rayon")]
  let frames = chunk.par_iter();
  #[cfg(not(feature = "rayon"))]
  let frames = chunk.iter();

  let batch = frames
    .map(|frame| encode_single_frame(frame, config))
    .collect::<Result<Vec<_>>>()?;

  encoded.extend(batch);
  chunk.clear();
  Ok(())
}

/// Encodes a sequence of RGBA frames into an animated WebP.
pub fn write_animated_webp<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()> {
  if frames.is_empty() {
    return Err(WebPError::EmptyAnimation.into());
  }

  let frame_width = frames[0].image.width();
  let frame_height = frames[0].image.height();
  let config = animation_config(frame_width, frame_height, &options)?;
  let unique_frames = collect_unique_frames(&frames, frame_width, frame_height, &options)?;
  let frame_data = encode_frames(&unique_frames, &config)?;

  write_riff_container(
    &frame_data,
    frame_width,
    frame_height,
    destination,
    &options,
  )
}

#[cfg(test)]
mod tests {
  use std::ptr::null_mut;

  use image::Rgba;

  use super::*;
  use crate::write::Bitmap;

  fn decode_rgba(encoded: &[u8]) -> RgbaImage {
    let mut width = 0;
    let mut height = 0;
    let pixels =
      unsafe { WebPDecodeRGBA(encoded.as_ptr(), encoded.len(), &mut width, &mut height) };
    assert!(!pixels.is_null(), "decode failed");

    let len = width as usize * height as usize * 4;
    let raw = unsafe { slice::from_raw_parts(pixels, len) }.to_vec();
    unsafe { WebPFree(pixels.cast()) };

    RgbaImage::from_raw(width as u32, height as u32, raw).unwrap()
  }

  /// YUV420 subsampling destroys saturated chroma edges.
  fn chroma_edges() -> RgbaImage {
    RgbaImage::from_fn(64, 64, |x, y| match (x / 3 + y / 5) % 4 {
      0 => Rgba([255, 0, 0, 255]),
      1 => Rgba([0, 0, 255, 255]),
      2 => Rgba([0, 255, 0, 255]),
      _ => Rgba([255, 0, 255, 255]),
    })
  }

  #[test]
  fn lossless_round_trip_is_bit_exact() {
    let image = chroma_edges();
    let mut encoded = Vec::new();
    write_webp_lossless(Cow::Borrowed(&image), &mut encoded).unwrap();

    let decoded = decode_rgba(&encoded);
    let differing = decoded
      .pixels()
      .zip(image.pixels())
      .filter(|(decoded, source)| decoded != source)
      .count();

    assert_eq!(differing, 0, "lossless encode altered {differing} pixels");
  }

  #[test]
  fn lossy_still_encodes() {
    let image = chroma_edges();
    let mut encoded = Vec::new();
    write_webp_lossy(Cow::Borrowed(&image), &mut encoded, Quality::default()).unwrap();

    let decoded = decode_rgba(&encoded);
    assert_eq!(decoded.dimensions(), image.dimensions());
  }

  fn decode_animation(encoded: &[u8]) -> Vec<RgbaImage> {
    let data = WebPData {
      bytes: encoded.as_ptr(),
      size: encoded.len(),
    };
    let mut options = MaybeUninit::<WebPAnimDecoderOptions>::uninit();
    assert_ne!(
      unsafe { WebPAnimDecoderOptionsInit(options.as_mut_ptr()) },
      0
    );

    let mut options = unsafe { options.assume_init() };
    options.color_mode = WEBP_CSP_MODE::MODE_RGBA;

    let decoder = unsafe { WebPAnimDecoderNew(&data, &options) };
    assert!(!decoder.is_null(), "anim decoder rejected the container");

    let mut info = MaybeUninit::<WebPAnimInfo>::uninit();
    assert_ne!(
      unsafe { WebPAnimDecoderGetInfo(decoder, info.as_mut_ptr()) },
      0
    );

    let info = unsafe { info.assume_init() };
    let canvas_bytes = (info.canvas_width * info.canvas_height * 4) as usize;
    let mut composited = Vec::new();

    while unsafe { WebPAnimDecoderHasMoreFrames(decoder) } != 0 {
      let mut pixels: *mut u8 = null_mut();
      let mut timestamp = 0;
      assert_ne!(
        unsafe { WebPAnimDecoderGetNext(decoder, &mut pixels, &mut timestamp) },
        0
      );

      let raw = unsafe { slice::from_raw_parts(pixels, canvas_bytes) }.to_vec();
      composited.push(RgbaImage::from_raw(info.canvas_width, info.canvas_height, raw).unwrap());
    }

    unsafe { WebPAnimDecoderDelete(decoder) };
    composited
  }

  fn read_le24(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])
  }

  /// (x, y, width, height, flags) of every ANMF chunk in the container.
  fn anmf_rects(encoded: &[u8]) -> Vec<(u32, u32, u32, u32, u8)> {
    let mut rects = Vec::new();
    let mut offset = 12;

    while offset + 8 <= encoded.len() {
      let tag = &encoded[offset..offset + 4];
      let len = u32::from_le_bytes(encoded[offset + 4..offset + 8].try_into().unwrap()) as usize;

      if tag == b"ANMF" {
        let header = &encoded[offset + 8..offset + 24];
        rects.push((
          read_le24(&header[0..3]) * 2,
          read_le24(&header[3..6]) * 2,
          read_le24(&header[6..9]) + 1,
          read_le24(&header[9..12]) + 1,
          header[15],
        ));
      }

      offset += 8 + len + (len & 1);
    }

    rects
  }

  /// A moving 30x20 box over a static gradient, with a transparent border.
  fn moving_box_frames() -> Vec<AnimationFrame> {
    (0..6)
      .map(|frame_index| {
        let box_x = 11 + frame_index * 13;
        let image = RgbaImage::from_fn(120, 80, |x, y| {
          if x < 4 || y < 4 || x >= 116 || y >= 76 {
            return Rgba([0, 0, 0, 0]);
          }
          if x >= box_x && x < box_x + 30 && (25..45).contains(&y) {
            return Rgba([255, 40, 40, 200]);
          }
          Rgba([(x * 2) as u8, (y * 3) as u8, 90, 255])
        });

        AnimationFrame::new(Bitmap::from_rgba(image), 40)
      })
      .collect()
  }

  fn frames_visibly_equal(a: &RgbaImage, b: &RgbaImage) -> bool {
    a.dimensions() == b.dimensions()
      && a
        .pixels()
        .zip(b.pixels())
        .all(|(a, b)| a == b || (a.0[3] == 0 && b.0[3] == 0))
  }

  #[test]
  fn cropped_animation_composites_to_source_frames() {
    let frames = moving_box_frames();
    let mut encoded = Vec::new();
    write_animated_webp(
      Cow::Borrowed(&frames),
      &mut encoded,
      AnimatedWebpOptions::default(),
    )
    .unwrap();

    let composited = decode_animation(&encoded);
    assert_eq!(composited.len(), frames.len());

    for (index, (decoded, source)) in composited.iter().zip(&frames).enumerate() {
      assert!(
        frames_visibly_equal(decoded, source.image.as_rgba()),
        "composited frame {index} deviates from the source frame"
      );
    }
  }

  #[test]
  fn frames_after_the_first_are_cropped_and_do_not_blend() {
    let frames = moving_box_frames();
    let mut encoded = Vec::new();
    write_animated_webp(
      Cow::Borrowed(&frames),
      &mut encoded,
      AnimatedWebpOptions::default(),
    )
    .unwrap();

    let rects = anmf_rects(&encoded);
    assert_eq!(rects.len(), frames.len());
    assert_eq!(rects[0], (0, 0, 120, 80, 0));

    for &(x, y, width, height, flags) in &rects[1..] {
      assert_eq!(x % 2, 0);
      assert_eq!(y % 2, 0);
      assert!(
        width < 120 && height < 80,
        "expected a cropped frame, got {width}x{height}"
      );
      assert_eq!(flags, 0b10, "cropped frames must carry the no-blend flag");
    }
  }

  /// A full-canvas diff region must still replace rather than honour
  /// `blend: true`: blending composes the two alphas, the canvas stops matching
  /// the frame, and every later diff under-covers the real difference.
  #[test]
  fn full_canvas_partial_alpha_frames_replace_under_blend() {
    let frames: Vec<AnimationFrame> = (0..4)
      .map(|frame_index| {
        let image = RgbaImage::from_fn(64, 48, |x, y| {
          Rgba([(x * 3 + frame_index * 40) as u8, (y * 5) as u8, 120, 128])
        });

        AnimationFrame::new(Bitmap::from_rgba(image), 40)
      })
      .collect();

    let mut encoded = Vec::new();
    write_animated_webp(
      Cow::Borrowed(&frames),
      &mut encoded,
      AnimatedWebpOptions::default(),
    )
    .unwrap();

    let composited = decode_animation(&encoded);
    assert_eq!(composited.len(), frames.len());

    for (index, (decoded, source)) in composited.iter().zip(&frames).enumerate() {
      let mismatch = decoded
        .enumerate_pixels()
        .zip(source.image.as_rgba().enumerate_pixels())
        .find(|((_, _, a), (_, _, b))| a != b && !(a.0[3] == 0 && b.0[3] == 0));

      assert!(
        mismatch.is_none(),
        "composited frame {index} deviates from the source frame: {mismatch:?}"
      );
    }
  }

  #[test]
  fn dispose_keeps_full_canvas_frames() {
    let frames = moving_box_frames();
    let mut encoded = Vec::new();
    write_animated_webp(
      Cow::Borrowed(&frames),
      &mut encoded,
      AnimatedWebpOptions::builder().dispose(true).build(),
    )
    .unwrap();

    for &(x, y, width, height, flags) in &anmf_rects(&encoded) {
      assert_eq!((x, y, width, height), (0, 0, 120, 80));
      assert_eq!(flags, 0b01);
    }
  }

  #[test]
  fn invisible_rgb_changes_under_alpha_zero_merge_frames() {
    let visible = RgbaImage::from_pixel(16, 16, Rgba([10, 20, 30, 255]));
    let mut transparent_a = visible.clone();
    let mut transparent_b = visible.clone();
    transparent_a.put_pixel(3, 3, Rgba([1, 2, 3, 0]));
    transparent_b.put_pixel(3, 3, Rgba([9, 9, 9, 0]));

    let frames = vec![
      AnimationFrame::new(Bitmap::from_rgba(transparent_a), 40),
      AnimationFrame::new(Bitmap::from_rgba(transparent_b), 40),
    ];
    let mut encoded = Vec::new();
    write_animated_webp(
      Cow::Borrowed(&frames),
      &mut encoded,
      AnimatedWebpOptions::default(),
    )
    .unwrap();

    let rects = anmf_rects(&encoded);
    assert_eq!(rects.len(), 1);
    assert_eq!(read_le24(&anmf_duration_bytes(&encoded)), 80);
  }

  fn anmf_duration_bytes(encoded: &[u8]) -> [u8; 3] {
    let mut offset = 12;

    while offset + 8 <= encoded.len() {
      let tag = &encoded[offset..offset + 4];
      let len = u32::from_le_bytes(encoded[offset + 4..offset + 8].try_into().unwrap()) as usize;

      if tag == b"ANMF" {
        return encoded[offset + 20..offset + 23].try_into().unwrap();
      }

      offset += 8 + len + (len & 1);
    }

    unreachable!("no ANMF chunk")
  }

  /// An opaque variant of the moving box, since the animated container carries
  /// only the VP8 chunk and lossy alpha would not survive either way.
  fn opaque_box_frames() -> Vec<AnimationFrame> {
    (0..6)
      .map(|frame_index| {
        let box_x = 11 + frame_index * 13;
        let image = RgbaImage::from_fn(120, 80, |x, y| {
          if x >= box_x && x < box_x + 30 && (25..45).contains(&y) {
            return Rgba([255, 40, 40, 255]);
          }
          Rgba([(x * 2) as u8, (y * 3) as u8, 90, 255])
        });

        AnimationFrame::new(Bitmap::from_rgba(image), 40)
      })
      .collect()
  }

  fn worst_channel_error(composited: &[RgbaImage], frames: &[AnimationFrame]) -> u8 {
    let mut worst = 0u8;

    for (decoded, source) in composited.iter().zip(frames) {
      for (decoded_pixel, source_pixel) in decoded.pixels().zip(source.image.as_rgba().pixels()) {
        for channel in 0..3 {
          worst = worst.max(decoded_pixel.0[channel].abs_diff(source_pixel.0[channel]));
        }
      }
    }

    worst
  }

  /// Cropped lossy frames lack the neighbouring pixels a full-canvas encode
  /// would predict from, so pixels inside the rectangle's edge may deviate from
  /// the canvas outside it. Bound that deviation against full-frame encoding.
  #[test]
  fn lossy_cropped_animation_stays_close_to_full_frame_encoding() {
    let frames = opaque_box_frames();
    let lossy = AnimatedWebpOptions::builder().lossless(false).build();

    let mut cropped = Vec::new();
    write_animated_webp(Cow::Borrowed(&frames), &mut cropped, lossy).unwrap();

    // dispose forces full-canvas frames, reproducing the pre-crop behaviour.
    let full_frame = AnimatedWebpOptions::builder()
      .lossless(false)
      .dispose(true)
      .build();
    let mut full = Vec::new();
    write_animated_webp(Cow::Borrowed(&frames), &mut full, full_frame).unwrap();

    let cropped_worst = worst_channel_error(&decode_animation(&cropped), &frames);
    let full_worst = worst_channel_error(&decode_animation(&full), &frames);
    println!("lossy worst channel error: cropped={cropped_worst} full={full_worst}");

    assert!(
      cropped_worst <= full_worst.saturating_add(24),
      "lossy crop deviates by {cropped_worst}, full-frame by {full_worst}"
    );
  }
}

#[cfg(test)]
mod bench {
  use std::time::{Duration, Instant};

  use image::Rgba;

  use super::*;
  use crate::write::Bitmap;

  const WIDTH: u32 = 544;
  const HEIGHT: u32 = 216;
  const FRAMES: usize = 42;

  fn background(x: u32, y: u32) -> Rgba<u8> {
    Rgba([
      ((x * 255) / WIDTH) as u8,
      ((y * 255) / HEIGHT) as u8,
      ((x + y) % 256) as u8,
      255,
    ])
  }

  fn sweep_frames() -> Vec<AnimationFrame> {
    (0..FRAMES)
      .map(|frame_index| {
        let band_start = (frame_index as u32) * 12;
        let image = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
          let mut pixel = background(x, y);

          if x >= band_start && x < band_start + 60 {
            pixel.0[0] = pixel.0[0].saturating_add(80);
            pixel.0[1] = pixel.0[1].saturating_add(80);
            pixel.0[2] = pixel.0[2].saturating_add(80);
          }

          pixel
        });

        AnimationFrame::new(Bitmap::from_rgba(image), 33)
      })
      .collect()
  }

  fn full_change_frames() -> Vec<AnimationFrame> {
    (0..FRAMES)
      .map(|frame_index| {
        let phase = frame_index as u32 * 7;
        let image = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
          Rgba([
            (((x + phase) * 255) / WIDTH) as u8,
            (((y + phase) * 255) / HEIGHT) as u8,
            ((x + y + phase) % 256) as u8,
            255,
          ])
        });

        AnimationFrame::new(Bitmap::from_rgba(image), 33)
      })
      .collect()
  }

  fn run(name: &str, frames: &[AnimationFrame], speed: u8, full_frames: bool) {
    let options = AnimatedWebpOptions::builder()
      .lossless(true)
      .speed(Some(speed))
      .dispose(full_frames)
      .build();

    let mut out = Vec::new();
    let mut best = Duration::MAX;

    for _ in 0..3 {
      out.clear();
      let start = Instant::now();
      write_animated_webp(Cow::Borrowed(frames), &mut out, options).unwrap();
      best = best.min(start.elapsed());
    }

    let mode = if full_frames { "full" } else { "cropped" };
    println!("{name} speed={speed} {mode}: {} bytes, {best:?}", out.len());
  }

  fn run_effort(name: &str, frames: &[AnimationFrame], effort: f32, method: i32) {
    let mut config = WebPConfig::new_with_preset(WebPPreset::WEBP_PRESET_TEXT, effort).unwrap();
    config.lossless = 1;
    config.method = method;

    let options = AnimatedWebpOptions::default();
    let start = Instant::now();
    let unique_frames = collect_unique_frames(frames, WIDTH, HEIGHT, &options).unwrap();
    let encoded = encode_frames(&unique_frames, &config).unwrap();
    let mut out = Vec::new();
    write_riff_container(&encoded, WIDTH, HEIGHT, &mut out, &options).unwrap();
    println!(
      "{name} effort={effort} method={method}: {} bytes, {:?}",
      out.len(),
      start.elapsed()
    );
  }

  #[test]
  #[ignore]
  fn bench_shapes() {
    let config = webp_config(true, 0, 1).unwrap();
    println!("preset exact = {}", config.exact);

    let sweep = sweep_frames();
    let full = full_change_frames();

    for full_frames in [true, false] {
      run("sweep", &sweep, 1, full_frames);
      run("full-change", &full, 1, full_frames);
    }

    for speed in [3, 4, 6] {
      run("sweep", &sweep, speed, false);
      run("full-change", &full, speed, false);
    }

    for effort in [50.0, 75.0, 100.0] {
      run_effort("sweep", &sweep, effort, 1);
      run_effort("full-change", &full, effort, 1);
    }
  }
}

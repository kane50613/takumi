#[cfg(feature = "gif")]
use std::mem::take;
use std::{
  io::{Cursor, Error as IoError, ErrorKind},
  sync::Arc,
};

#[cfg(feature = "gif")]
use gif::{ColorOutput, DecodeOptions, Decoder as GifDecoder, DisposalMethod};
#[cfg(feature = "jpeg")]
use image::codecs::jpeg::JpegDecoder;
use image::{
  DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageResult, Limits, RgbaImage,
  codecs::png::PngDecoder,
  error::{DecodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
};
#[cfg(all(target_arch = "wasm32", feature = "webp"))]
use image_webp::WebPDecoder;
#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
use libwebp_sys::{
  VP8StatusCode, WEBP_CSP_MODE, WebPDecode, WebPDecoderConfig, WebPGetInfo, WebPRGBABuffer,
};
use png::{BitDepth, ColorType, Decoder as PngRowDecoder, Transformations};

#[cfg(feature = "gif")]
use crate::geometry::Rect;
use crate::{
  resources::{
    image_buffer::{ImageBuffer, premultiply_rgba_in_place},
    image_resampler::{StreamResampler, resample_premultiplied},
  },
  style::ImageScalingAlgorithm,
};

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// Maximum decoded image edge length; also the width/height limit fed to the
/// `image` crate decoders.
const MAX_IMAGE_DIMENSION: u32 = 8192;
/// Decoded images above this pixel count are rejected (RGBA cost = 4x).
/// 8192 x 8192 — far above any sane OG-image asset, far below OOM territory.
#[cfg(any(feature = "gif", feature = "webp"))]
const MAX_IMAGE_PIXELS: u64 = MAX_IMAGE_DIMENSION as u64 * MAX_IMAGE_DIMENSION as u64;
/// Total pixels across all GIF frames (frame budget for animations).
#[cfg(feature = "gif")]
const MAX_GIF_TOTAL_PIXELS: u64 = 4 * MAX_IMAGE_PIXELS;

/// Rejects decoded images whose pixel count exceeds [`MAX_IMAGE_PIXELS`].
#[cfg(feature = "webp")]
fn check_pixel_budget(width: u32, height: u32) -> ImageResult<()> {
  if width as u64 * height as u64 > MAX_IMAGE_PIXELS {
    return Err(pixel_budget_error(width, height));
  }

  Ok(())
}

#[cfg(any(
  feature = "gif",
  feature = "webp",
  not(all(feature = "jpeg", feature = "webp"))
))]
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

pub(crate) fn detect_image_format(bytes: &[u8]) -> Option<DetectedImageFormat> {
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
pub(crate) enum DetectedImageFormat {
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
  rgba_to_buffer(DynamicImage::from_decoder(decoder)?.into_rgba8(), format)
}

pub(crate) fn decode_png(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(PngDecoder::new(Cursor::new(bytes))?, ImageFormat::Png)
}

/// The error a decode entry point returns for a format whose feature is off.
#[cfg(not(all(feature = "jpeg", feature = "webp", feature = "gif")))]
fn format_compiled_out_error() -> ImageError {
  ImageError::Unsupported(UnsupportedError::from_format_and_kind(
    ImageFormatHint::Unknown,
    UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
  ))
}

/// Whether these bytes are a format this build cannot decode. Those keep their
/// bytes for a backend that embeds them undecoded; anything else that fails to
/// decode is corrupt, and stays an error.
#[cfg(not(all(feature = "jpeg", feature = "webp")))]
pub(crate) fn decoder_compiled_out(bytes: &[u8]) -> bool {
  match detect_image_format(bytes) {
    #[cfg(not(feature = "jpeg"))]
    Some(DetectedImageFormat::Jpeg) => true,
    #[cfg(not(feature = "webp"))]
    Some(DetectedImageFormat::WebP) => true,
    _ => false,
  }
}

/// Dimensions read from the format header alone, for a format whose decoder is
/// compiled out: layout needs the intrinsic size, and a vector backend embeds
/// the original bytes without ever decoding them.
#[cfg(not(all(feature = "jpeg", feature = "webp")))]
fn header_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let size = imagesize::blob_size(bytes).map_err(|error| {
    ImageError::Decoding(DecodingError::new(
      ImageFormatHint::Unknown,
      IoError::new(ErrorKind::InvalidData, error.to_string()),
    ))
  })?;
  let (width, height) = (size.width as u32, size.height as u32);

  if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
    return Err(pixel_budget_error(width, height));
  }

  Ok((width, height))
}

#[cfg(feature = "jpeg")]
fn decode_jpeg(bytes: &[u8]) -> ImageResult<ImageBuffer> {
  decode_with_image_crate(JpegDecoder::new(Cursor::new(bytes))?, ImageFormat::Jpeg)
}

#[cfg(not(feature = "jpeg"))]
fn decode_jpeg(_bytes: &[u8]) -> ImageResult<ImageBuffer> {
  Err(format_compiled_out_error())
}

#[cfg(feature = "jpeg")]
fn jpeg_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  JpegDecoder::new(Cursor::new(bytes)).map(|d| d.dimensions())
}

#[cfg(not(feature = "jpeg"))]
fn jpeg_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  header_dimensions(bytes)
}

/// Bitmap dimensions from the format header; decodes no pixels. `None` for
/// unsupported or GIF bytes (GIFs carry their own lazy source).
pub(crate) fn bitmap_dimensions(bytes: &[u8]) -> Option<ImageResult<(u32, u32)>> {
  let dimensions = match detect_image_format(bytes)? {
    DetectedImageFormat::Png => PngDecoder::new(Cursor::new(bytes)).map(|d| d.dimensions()),
    DetectedImageFormat::Jpeg => jpeg_dimensions(bytes),
    DetectedImageFormat::WebP => return Some(webp_dimensions(bytes)),
    DetectedImageFormat::Gif => return None,
  };
  Some(dimensions)
}

/// Decodes bitmap bytes scaled to cover `width` x `height`, never upscaling.
/// Non-interlaced PNGs stream row-by-row through the resampler without a
/// full-size buffer; everything else decodes fully and resizes.
pub(crate) fn decode_bitmap_scaled(
  bytes: &[u8],
  width: u32,
  height: u32,
  algorithm: ImageScalingAlgorithm,
) -> ImageResult<ImageBuffer> {
  if let Some(streamed) = decode_png_scaled(bytes, width, height, algorithm) {
    return streamed;
  }

  #[cfg(not(target_arch = "wasm32"))]
  if let Some(scaled) = decode_webp_scaled(bytes, width, height) {
    return scaled;
  }

  let decoded = decode_image(bytes)?;
  if width >= decoded.width() && height >= decoded.height() {
    return Ok(decoded);
  }

  resample_premultiplied(
    decoded.data(),
    (decoded.width(), decoded.height()),
    (width, height),
    algorithm,
  )
  .ok_or_else(invalid_buffer_error)
}

fn png_decode_error(error: png::DecodingError) -> ImageError {
  ImageError::Decoding(DecodingError::new(ImageFormat::Png.into(), error))
}

/// Streams a non-interlaced PNG through [`StreamResampler`]. `None` means the
/// input isn't eligible (not a PNG, interlaced, unsupported layout, or no
/// downscale) and the caller should decode fully; errors after eligibility are
/// real decode failures.
fn decode_png_scaled(
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
        for (rgba, pixel) in rgba_row.chunks_exact_mut(4).zip(row.data().chunks_exact(2)) {
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
pub(crate) fn gif_frame_durations(bytes: &[u8]) -> ImageResult<Box<[u32]>> {
  let mut options = DecodeOptions::new();
  options.skip_frame_decoding(true);
  let mut decoder = options
    .read_info(Cursor::new(bytes))
    .map_err(gif_decode_error)?;

  // Stop at the same cumulative pixel budget as `decode_gif_frames`, so the
  // timing covers exactly the frames that are actually decodable.
  let frame_pixels = decoder.width() as u64 * decoder.height() as u64;
  let mut total_pixels = 0_u64;
  let mut durations = Vec::new();
  loop {
    match decoder.read_next_frame() {
      Ok(Some(frame)) => {
        total_pixels += frame_pixels;
        if total_pixels > MAX_GIF_TOTAL_PIXELS {
          break;
        }
        durations.push((frame.delay as u32 * 10).max(1));
      }
      Ok(None) => break,
      Err(error) if durations.is_empty() => return Err(gif_decode_error(error)),
      Err(_) => break,
    }
  }

  Ok(durations.into())
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
/// ended. A mid-stream decode error or a blown [`MAX_GIF_TOTAL_PIXELS`] budget
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
  mut push: impl FnMut(DecodedGifFrame),
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

    let rect = Rect {
      left: frame.left as u32,
      top: frame.top as u32,
      right: frame.left as u32 + frame.width as u32,
      bottom: frame.top as u32 + frame.height as u32,
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
      let buffer = buffer.ok_or_else(invalid_buffer_error)?;
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

#[cfg(all(target_arch = "wasm32", feature = "webp"))]
fn webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(webp_decode_error)?;
  Ok(decoder.dimensions())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
fn webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
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

#[cfg(all(not(target_arch = "wasm32"), feature = "webp"))]
fn decode_webp(bytes: &[u8]) -> ImageResult<ImageBuffer> {
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
fn decode_webp_scaled(bytes: &[u8], width: u32, height: u32) -> Option<ImageResult<ImageBuffer>> {
  if !matches!(detect_image_format(bytes), Some(DetectedImageFormat::WebP)) {
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
    return Err(webp_decode_error(WebPError::EncodeFailed));
  }

  RgbaImage::from_raw(width, height, image_data)
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

#[cfg(not(feature = "webp"))]
fn webp_dimensions(bytes: &[u8]) -> ImageResult<(u32, u32)> {
  header_dimensions(bytes)
}

#[cfg(not(feature = "webp"))]
fn decode_webp(_bytes: &[u8]) -> ImageResult<ImageBuffer> {
  Err(format_compiled_out_error())
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "webp")))]
fn decode_webp_scaled(
  _bytes: &[u8],
  _width: u32,
  _height: u32,
) -> Option<ImageResult<ImageBuffer>> {
  None
}

#[cfg(not(feature = "gif"))]
pub(crate) fn gif_dimensions(_bytes: &[u8]) -> ImageResult<(u32, u32)> {
  Err(format_compiled_out_error())
}

#[cfg(not(feature = "gif"))]
pub(crate) fn gif_frame_durations(_bytes: &[u8]) -> ImageResult<Box<[u32]>> {
  Err(format_compiled_out_error())
}

#[cfg(not(feature = "gif"))]
pub(crate) fn decode_gif_frames(
  _bytes: &[u8],
  _skip: usize,
  _limit: Option<usize>,
  _target: Option<(u32, u32, ImageScalingAlgorithm)>,
  _push: impl FnMut(DecodedGifFrame),
) -> ImageResult<bool> {
  Err(format_compiled_out_error())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(feature = "webp")]
  #[test]
  fn check_pixel_budget_accepts_budget_edge() {
    assert!(check_pixel_budget(MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION).is_ok());
    assert!(check_pixel_budget(1, 1).is_ok());
  }

  #[cfg(feature = "webp")]
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
    let bytes = include_bytes!("../../../assets/images/yeecord.png");
    let full = decode_image(bytes).unwrap();
    assert_streamed_matches_full(bytes, full.width() / 3, full.height() / 3);
  }

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
    let mut frames = Vec::new();
    let ended = decode_gif_frames(bytes, skip, None, None, |frame| {
      frames.push((frame.buffer.data().to_vec(), frame.duration_ms));
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

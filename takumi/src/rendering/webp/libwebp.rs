use std::{
  borrow::Cow,
  ffi::CStr,
  io::{Error as IoStdError, Write},
  mem::MaybeUninit,
  slice,
};

use image::RgbaImage;
use libwebp_sys::*;
use rayon::prelude::*;

use crate::{Error::IoError, Result};

use super::{super::write::AnimatedWebpOptions, AnimationFrame, U24_MAX};

fn webp_config(quality: u8, speed: u8) -> Result<WebPConfig> {
  let requested_quality = quality.clamp(0, 100);
  let is_lossless = requested_quality == 100;
  let mut config = WebPConfig::new_with_preset(
    WebPPreset::WEBP_PRESET_TEXT,
    if is_lossless {
      20.0
    } else {
      requested_quality as f32
    },
  )
  .map_err(|_| IoError(IoStdError::other("Failed to construct WebP config")))?;

  config.lossless = if is_lossless { 1 } else { 0 };
  config.alpha_compression = if is_lossless { 0 } else { 1 };
  config.method = speed.clamp(0, 6) as i32;
  if unsafe { WebPValidateConfig(&config) } == 0 {
    return Err(IoError(IoStdError::other("Invalid WebP config")));
  }

  Ok(config)
}

fn import_rgba_picture(image: &RgbaImage) -> Result<WebPPicture> {
  let mut picture = WebPPicture::new()
    .map_err(|_| IoError(IoStdError::other("Failed to initialize WebP picture")))?;

  picture.width = image.width() as i32;
  picture.height = image.height() as i32;

  let import_ok = unsafe {
    WebPPictureImportRGBA(
      &mut picture,
      image.as_raw().as_ptr(),
      (image.width() as i32) * 4,
    )
  };

  if import_ok == 0 {
    unsafe { WebPPictureFree(&mut picture) };
    return Err(IoError(IoStdError::other(format!(
      "WebP import error: {:?}",
      picture.error_code
    ))));
  }

  Ok(picture)
}

/// Encodes a single RGBA image with `WebPEncode` and returns the raw VP8/VP8L
/// payload bytes (stripping the single-frame RIFF wrapper) plus the 4-byte
/// chunk tag (`b"VP8 "` or `b"VP8L"`).
///
/// By encoding each frame independently we can parallelise across rayon threads
/// instead of funnelling everything through the serial `WebPAnimEncoderAdd`.
type EncodedFrame = (Vec<u8>, [u8; 4], u32);

fn encode_single_frame(image: &RgbaImage, config: &WebPConfig) -> Result<(Vec<u8>, [u8; 4])> {
  let mut picture = import_rgba_picture(image)?;

  let mut writer = MaybeUninit::<WebPMemoryWriter>::uninit();
  unsafe { WebPMemoryWriterInit(writer.as_mut_ptr()) };
  picture.writer = Some(WebPMemoryWrite);
  picture.custom_ptr = writer.as_mut_ptr().cast();

  let encode_ok = unsafe { WebPEncode(std::ptr::from_ref(config), &raw mut picture) };
  let mut writer = unsafe { writer.assume_init() };

  if encode_ok == 0 {
    let err = IoStdError::other(format!("WebP encode error: {:?}", picture.error_code));
    unsafe {
      WebPMemoryWriterClear(&raw mut writer);
      WebPPictureFree(&raw mut picture);
    }
    return Err(IoError(err));
  }

  let blob = unsafe { slice::from_raw_parts(writer.mem, writer.size) };

  // Scan the RIFF wrapper for the VP8/VP8L chunk and extract just the payload.
  let (tag, payload) = match extract_vp8_payload(blob) {
    Some(result) => result,
    None => {
      unsafe {
        WebPMemoryWriterClear(&raw mut writer);
        WebPPictureFree(&raw mut picture);
      }
      return Err(IoError(IoStdError::other(
        "VP8/VP8L chunk not found in encoded frame",
      )));
    }
  };

  unsafe {
    WebPMemoryWriterClear(&raw mut writer);
    WebPPictureFree(&raw mut picture);
  }

  Ok((payload, tag))
}

/// Scans a single-frame WebP RIFF blob and returns `(4-byte-tag, payload_bytes)`.
fn extract_vp8_payload(buf: &[u8]) -> Option<([u8; 4], Vec<u8>)> {
  if buf.len() < 12 {
    return None;
  }
  let mut i = 12usize; // skip RIFF header
  while i + 8 <= buf.len() {
    let tag: [u8; 4] = buf[i..i + 4].try_into().ok()?;
    let len = u32::from_le_bytes(buf[i + 4..i + 8].try_into().ok()?) as usize;
    if &tag == b"VP8 " || &tag == b"VP8L" {
      let start = i + 8;
      let end = start.checked_add(len)?;
      if end > buf.len() {
        return None;
      }
      return Some((tag, buf[start..end].to_vec()));
    }
    let padding = len & 1;
    i = (i + 8).checked_add(len + padding)?;
  }
  None
}

// ── RIFF animated WebP container assembly ───────────────────────────────────
//
// Layout:
//   RIFF header  12 bytes   "RIFF" + payload_size(u32 LE) + "WEBP"
//   VP8X chunk   18 bytes   "VP8X" + 10(u32 LE) + flags(1) + reserved(3) + cw-1(3) + ch-1(3)
//   ANIM chunk   14 bytes   "ANIM" + 6(u32 LE)  + bgcolor(4) + loop_count(2)
//   ANMF chunks  N × (32 + vp8_len + pad) bytes
//     "ANMF" + anmf_size(u32 LE)
//     x(3) + y(3) + w-1(3) + h-1(3) + duration(3) + flags(1)   ← 16 bytes
//     VP8x_tag(4) + vp8_payload_size(u32 LE) + vp8_payload + [pad]

const VP8X_CHUNK_BYTES: usize = 18; // tag(4)+size(4)+payload(10)
const ANIM_CHUNK_BYTES: usize = 14; // tag(4)+size(4)+payload(6)

#[inline]
fn anmf_chunk_bytes(vp8_len: usize) -> usize {
  // tag(4)+size(4) + frame_header(16) + VP8x_tag(4)+size(4) + payload + padding
  8 + 16 + 8 + vp8_len + (vp8_len & 1)
}

#[inline]
fn write_le24<W: Write>(w: &mut W, v: u32) -> std::io::Result<()> {
  w.write_all(&v.to_le_bytes()[..3])
}

fn write_riff_container<W: Write>(
  destination: &mut W,
  width: u32,
  height: u32,
  loop_count: u16,
  blend: bool,
  dispose: bool,
  frames: &[(&[u8], [u8; 4], u32)], // (vp8_payload, tag, duration_ms)
) -> std::io::Result<()> {
  // bit 1: blend (0 = use alpha-blending, 1 = do not blend)
  // bit 0: dispose (0 = do not dispose, 1 = dispose to background)
  let frame_flags: u8 = (u8::from(!blend) << 1) | u8::from(dispose);

  let frames_total = frames.iter().try_fold(0usize, |acc, (p, _, _)| {
    acc
      .checked_add(anmf_chunk_bytes(p.len()))
      .ok_or_else(|| IoStdError::other("RIFF payload size overflow"))
  })?;
  let riff_payload_usize = 4usize
    .checked_add(VP8X_CHUNK_BYTES)
    .and_then(|v| v.checked_add(ANIM_CHUNK_BYTES))
    .and_then(|v| v.checked_add(frames_total))
    .ok_or_else(|| IoStdError::other("RIFF payload size overflow"))?;
  let riff_payload = u32::try_from(riff_payload_usize)
    .map_err(|_| IoStdError::other("RIFF payload size overflows u32"))?;

  // RIFF header
  destination.write_all(b"RIFF")?;
  destination.write_all(&riff_payload.to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  // VP8X: flags(1)+reserved(3)+canvas_w-1(3)+canvas_h-1(3) = 10 bytes
  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  destination.write_all(b"VP8X")?;
  destination.write_all(&10u32.to_le_bytes())?;
  destination.write_all(&[vp8x_flags, 0, 0, 0])?;
  write_le24(destination, width - 1)?;
  write_le24(destination, height - 1)?;

  // ANIM: bgcolor(4)+loop_count(2) = 6 bytes
  destination.write_all(b"ANIM")?;
  destination.write_all(&6u32.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?; // bgcolor = transparent black
  destination.write_all(&loop_count.to_le_bytes())?;

  // ANMF frames
  for (vp8_payload, vp8_tag, duration_ms) in frames {
    let vp8_len = vp8_payload.len();
    let padding = vp8_len & 1;
    let anmf_payload_size_usize = 16usize
      .checked_add(8)
      .and_then(|v| v.checked_add(vp8_len))
      .and_then(|v| v.checked_add(padding))
      .ok_or_else(|| IoStdError::other("ANMF payload size overflow"))?;
    let anmf_payload_size = u32::try_from(anmf_payload_size_usize)
      .map_err(|_| IoStdError::other("ANMF payload size overflows u32"))?;

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_payload_size.to_le_bytes())?;
    // frame header (16 bytes)
    destination.write_all(&[0u8; 6])?; // x=0, y=0 (each 3 bytes)
    write_le24(destination, width - 1)?; // w-1
    write_le24(destination, height - 1)?; // h-1
    write_le24(destination, (*duration_ms).clamp(0, U24_MAX))?;
    destination.write_all(&[frame_flags])?;
    // VP8 sub-chunk
    destination.write_all(vp8_tag)?;
    let vp8_len_u32 =
      u32::try_from(vp8_len).map_err(|_| IoStdError::other("VP8 payload size overflows u32"))?;
    destination.write_all(&vp8_len_u32.to_le_bytes())?;
    destination.write_all(vp8_payload)?;
    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  destination.flush()
}

// ── Public API ───────────────────────────────────────────────────────────────

pub(crate) fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  quality: Option<u8>,
) -> Result<()> {
  let config = webp_config(quality.unwrap_or(100), 1)?;

  // For static images keep using the old WebPAnimEncoder-free path (WebPEncode directly).
  let mut picture = import_rgba_picture(&image)?;
  let mut writer = MaybeUninit::<WebPMemoryWriter>::uninit();
  unsafe { WebPMemoryWriterInit(writer.as_mut_ptr()) };
  picture.writer = Some(WebPMemoryWrite);
  picture.custom_ptr = writer.as_mut_ptr().cast();

  let encode_ok = unsafe { WebPEncode(&raw const config, &raw mut picture) };
  let mut writer = unsafe { writer.assume_init() };

  if encode_ok == 0 {
    unsafe {
      WebPMemoryWriterClear(&raw mut writer);
      WebPPictureFree(&raw mut picture);
    }
    return Err(IoError(IoStdError::other(format!(
      "WebP encode error: {:?}",
      picture.error_code
    ))));
  }

  let encoded = unsafe { slice::from_raw_parts(writer.mem, writer.size) };
  let write_result = destination.write_all(encoded);
  unsafe {
    WebPMemoryWriterClear(&raw mut writer);
    WebPPictureFree(&raw mut picture);
  }

  write_result?;
  Ok(())
}

/// Encodes a sequence of RGBA frames into an animated WebP using parallel
/// per-frame encoding via `WebPEncode`, then assembles the RIFF container
/// manually.  This bypasses the serial `WebPAnimEncoderAdd` bottleneck.
pub fn encode_animated_webp<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()> {
  if frames.is_empty() {
    return Err(IoError(IoStdError::other(
      "Animation must contain at least one frame",
    )));
  }

  let first_frame = &frames[0];
  let frame_width = first_frame.image.width();
  let frame_height = first_frame.image.height();
  if !(1..=U24_MAX + 1).contains(&frame_width) || !(1..=U24_MAX + 1).contains(&frame_height) {
    return Err(IoError(IoStdError::other(format!(
      "WebP animation frame dimensions must be in 1..={}, got {frame_width}x{frame_height}",
      U24_MAX + 1
    ))));
  }

  let speed = options.speed.unwrap_or(1).clamp(0, 6);
  let config = webp_config(options.quality, speed)?;

  // Step 1: deduplicate consecutive identical frames by pixel-buffer value equality.
  // Each unique entry records (image_ref, duration_ms_for_anmf).
  let mut unique: Vec<(&RgbaImage, u32)> = Vec::new();
  {
    let mut pending_image = &frames[0].image;
    let mut pending_dur = frames[0].duration_ms.clamp(0, U24_MAX);

    for frame in frames.iter().skip(1) {
      if frame.image.width() != frame_width || frame.image.height() != frame_height {
        return Err(IoError(IoStdError::other(
          "All animation frames must have the same dimensions",
        )));
      }
      if frame.image.as_raw() == pending_image.as_raw() {
        pending_dur = pending_dur.saturating_add(frame.duration_ms.clamp(0, U24_MAX));
        continue;
      }
      unique.push((pending_image, pending_dur));
      pending_image = &frame.image;
      pending_dur = frame.duration_ms.clamp(0, U24_MAX);
    }
    unique.push((pending_image, pending_dur));
  }

  // Step 2: encode all unique frames in parallel — each thread owns its own
  // WebPPicture; config is read-only and all its fields are primitives so it
  // is Send + Sync.
  let encoded: Vec<Result<EncodedFrame>> = unique
    .par_iter()
    .map(|(image, dur)| {
      let (payload, tag) = encode_single_frame(image, &config)?;
      Ok((payload, tag, *dur))
    })
    .collect();

  // Collect results, propagating any error.
  let frame_data: Vec<EncodedFrame> = encoded.into_iter().collect::<Result<_>>()?;

  // Step 3: write the RIFF container — O(total_output_bytes), no re-encoding.
  let refs: Vec<(&[u8], [u8; 4], u32)> = frame_data
    .iter()
    .map(|(p, t, d)| (p.as_slice(), *t, *d))
    .collect();

  write_riff_container(
    destination,
    frame_width,
    frame_height,
    options.loop_count.unwrap_or(0),
    options.blend,
    options.dispose,
    &refs,
  )
  .map_err(IoError)?;

  Ok(())
}

// Keep CStr import used only by the dead code path below so the compiler
// does not emit a warning if we ever re-add the animation-encoder path.
#[allow(dead_code)]
fn animation_encoder_error_msg(encoder: *mut WebPAnimEncoder) -> String {
  let ptr = unsafe { WebPAnimEncoderGetError(encoder) };
  if ptr.is_null() {
    return "WebP animation encode error".into();
  }
  unsafe { CStr::from_ptr(ptr) }
    .to_string_lossy()
    .into_owned()
}

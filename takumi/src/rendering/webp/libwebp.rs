use std::{borrow::Cow, ffi::CStr, io::Write, mem::MaybeUninit, slice};

use image::RgbaImage;
use libwebp_sys::*;
use rayon::prelude::*;

use crate::{Result, error::WebPError};

use super::{
  super::write::{AnimatedWebpOptions, AnimationFrame},
  U24_MAX,
};

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
  .map_err(|_| WebPError::ConfigConstruction)?;

  config.lossless = if is_lossless { 1 } else { 0 };
  config.alpha_compression = if is_lossless { 0 } else { 1 };
  config.method = speed.clamp(0, 6) as i32;
  if unsafe { WebPValidateConfig(&config) } == 0 {
    return Err(WebPError::InvalidConfig.into());
  }

  Ok(config)
}

fn import_rgba_picture(image: &RgbaImage) -> Result<WebPPicture> {
  let mut picture = WebPPicture::new().map_err(|_| WebPError::PictureInitialization)?;

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
    return Err(
      WebPError::Import {
        error_code: format!("{:?}", picture.error_code),
      }
      .into(),
    );
  }

  Ok(picture)
}

struct EncodedFrame {
  payload: Vec<u8>,
  tag: [u8; 4],
  duration_ms: u32,
}

fn encode_single_frame(image: &RgbaImage, config: &WebPConfig) -> Result<(Vec<u8>, [u8; 4])> {
  let mut picture = import_rgba_picture(image)?;

  let mut writer = MaybeUninit::<WebPMemoryWriter>::uninit();
  unsafe { WebPMemoryWriterInit(writer.as_mut_ptr()) };
  picture.writer = Some(WebPMemoryWrite);
  picture.custom_ptr = writer.as_mut_ptr().cast();

  let encode_ok = unsafe { WebPEncode(std::ptr::from_ref(config), &raw mut picture) };
  let mut writer = unsafe { writer.assume_init() };

  if encode_ok == 0 {
    unsafe {
      WebPMemoryWriterClear(&raw mut writer);
      WebPPictureFree(&raw mut picture);
    }
    return Err(
      WebPError::EncodeWithCode {
        error_code: format!("{:?}", picture.error_code),
      }
      .into(),
    );
  }

  let blob = unsafe { slice::from_raw_parts(writer.mem, writer.size) };

  let (tag, payload) = match extract_vp8_payload(blob) {
    Some(result) => result,
    None => {
      unsafe {
        WebPMemoryWriterClear(&raw mut writer);
        WebPPictureFree(&raw mut picture);
      }
      return Err(WebPError::MissingVp8ChunkInEncodedFrame.into());
    }
  };

  unsafe {
    WebPMemoryWriterClear(&raw mut writer);
    WebPPictureFree(&raw mut picture);
  }

  Ok((payload, tag))
}

fn extract_vp8_payload(buf: &[u8]) -> Option<([u8; 4], Vec<u8>)> {
  const RIFF_HEADER_SIZE: usize = 12;

  if buf.len() < RIFF_HEADER_SIZE {
    return None;
  }

  let mut offset = RIFF_HEADER_SIZE;
  while offset + 8 <= buf.len() {
    let tag: [u8; 4] = buf[offset..offset + 4].try_into().ok()?;
    let len = u32::from_le_bytes(buf[offset + 4..offset + 8].try_into().ok()?) as usize;
    if &tag == b"VP8 " || &tag == b"VP8L" {
      let payload_start = offset + 8;
      let payload_end = payload_start.checked_add(len)?;
      if payload_end > buf.len() {
        return None;
      }

      return Some((tag, buf[payload_start..payload_end].to_vec()));
    }

    let padding = len & 1;
    offset = (offset + 8).checked_add(len + padding)?;
  }

  None
}

const VP8X_CHUNK_BYTES: usize = 18;
const ANIM_CHUNK_BYTES: usize = 14;

#[inline]
fn anmf_chunk_bytes(vp8_len: usize) -> usize {
  8 + 16 + 8 + vp8_len + (vp8_len & 1)
}

fn write_le24<W: Write>(destination: &mut W, value: u32) -> Result<()> {
  destination.write_all(&value.to_le_bytes()[..3])?;
  Ok(())
}

fn write_riff_container<W: Write>(
  destination: &mut W,
  width: u32,
  height: u32,
  loop_count: u16,
  blend: bool,
  dispose: bool,
  frames: &[(&[u8], [u8; 4], u32)], // (vp8_payload, tag, duration_ms)
) -> Result<()> {
  let frame_flags: u8 = (u8::from(!blend) << 1) | u8::from(dispose);

  let frames_total = frames.iter().try_fold(0usize, |acc, (p, _, _)| {
    acc
      .checked_add(anmf_chunk_bytes(p.len()))
      .ok_or(WebPError::RiffPayloadSizeOverflow)
  })?;
  let riff_payload_usize = 4usize
    .checked_add(VP8X_CHUNK_BYTES)
    .and_then(|v| v.checked_add(ANIM_CHUNK_BYTES))
    .and_then(|v| v.checked_add(frames_total))
    .ok_or(WebPError::RiffPayloadSizeOverflow)?;
  let riff_payload =
    u32::try_from(riff_payload_usize).map_err(|_| WebPError::RiffPayloadSizeTooLarge)?;

  destination.write_all(b"RIFF")?;
  destination.write_all(&riff_payload.to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  destination.write_all(b"VP8X")?;
  destination.write_all(&10u32.to_le_bytes())?;
  destination.write_all(&[vp8x_flags, 0, 0, 0])?;
  write_le24(destination, width - 1)?;
  write_le24(destination, height - 1)?;

  destination.write_all(b"ANIM")?;
  destination.write_all(&6u32.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?;
  destination.write_all(&loop_count.to_le_bytes())?;

  for (vp8_payload, vp8_tag, duration_ms) in frames {
    let vp8_len = vp8_payload.len();
    let padding = vp8_len & 1;
    let anmf_payload_size_usize = 16usize
      .checked_add(8)
      .and_then(|v| v.checked_add(vp8_len))
      .and_then(|v| v.checked_add(padding))
      .ok_or(WebPError::AnmfPayloadSizeOverflow)?;
    let anmf_payload_size =
      u32::try_from(anmf_payload_size_usize).map_err(|_| WebPError::AnmfPayloadSizeTooLarge)?;

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_payload_size.to_le_bytes())?;
    destination.write_all(&[0u8; 6])?;
    write_le24(destination, width - 1)?;
    write_le24(destination, height - 1)?;
    write_le24(destination, (*duration_ms).clamp(0, U24_MAX))?;
    destination.write_all(&[frame_flags])?;
    destination.write_all(vp8_tag)?;
    let vp8_len_u32 = u32::try_from(vp8_len).map_err(|_| WebPError::Vp8PayloadSizeTooLarge)?;
    destination.write_all(&vp8_len_u32.to_le_bytes())?;
    destination.write_all(vp8_payload)?;
    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  destination.flush()?;
  Ok(())
}

pub(crate) fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  quality: Option<u8>,
) -> Result<()> {
  let config = webp_config(quality.unwrap_or(100), 1)?;

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
    return Err(
      WebPError::EncodeWithCode {
        error_code: format!("{:?}", picture.error_code),
      }
      .into(),
    );
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
    return Err(WebPError::EmptyAnimation.into());
  }

  let first_frame = &frames[0];
  let frame_width = first_frame.image.width();
  let frame_height = first_frame.image.height();
  if !(1..=U24_MAX + 1).contains(&frame_width) || !(1..=U24_MAX + 1).contains(&frame_height) {
    return Err(
      WebPError::InvalidFrameDimensions {
        width: frame_width,
        height: frame_height,
        max: U24_MAX + 1,
      }
      .into(),
    );
  }

  let speed = options.speed.unwrap_or(1).clamp(0, 6);
  let config = webp_config(options.quality, speed)?;

  let mut unique: Vec<(&RgbaImage, u32)> = Vec::new();
  {
    let mut pending_image = &frames[0].image;
    let mut pending_dur = frames[0].duration_ms.clamp(0, U24_MAX);

    for frame in frames.iter().skip(1) {
      if frame.image.width() != frame_width || frame.image.height() != frame_height {
        return Err(WebPError::MixedFrameDimensions.into());
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

  let encoded: Vec<Result<EncodedFrame>> = unique
    .par_iter()
    .map(|(image, dur)| {
      let (payload, tag) = encode_single_frame(image, &config)?;
      Ok(EncodedFrame {
        payload,
        tag,
        duration_ms: *dur,
      })
    })
    .collect();

  let frame_data: Vec<EncodedFrame> = encoded.into_iter().collect::<Result<_>>()?;

  let refs: Vec<(&[u8], [u8; 4], u32)> = frame_data
    .iter()
    .map(|frame| (frame.payload.as_slice(), frame.tag, frame.duration_ms))
    .collect();

  write_riff_container(
    destination,
    frame_width,
    frame_height,
    options.loop_count.unwrap_or(0),
    options.blend,
    options.dispose,
    &refs,
  )?;

  Ok(())
}

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

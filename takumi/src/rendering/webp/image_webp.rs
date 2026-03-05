use std::{borrow::Cow, io::Write};

use ::image_webp::WebPEncoder;
use image::RgbaImage;

use crate::{Error::IoError, Result};

use super::{
  super::write::AnimatedWebpOptions, AnimationFrame, U24_MAX, has_any_alpha_pixel,
  strip_alpha_channel,
};

pub(crate) fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  _quality: Option<u8>,
) -> Result<()> {
  let encoder = WebPEncoder::new(destination);
  let width = image.width();
  let height = image.height();
  let has_alpha = has_any_alpha_pixel(&image);

  let image_data = if has_alpha {
    Cow::Borrowed(image.as_raw())
  } else {
    Cow::Owned(strip_alpha_channel(image))
  };

  encoder.encode(
    &image_data,
    width,
    height,
    if has_alpha {
      ::image_webp::ColorType::Rgba8
    } else {
      ::image_webp::ColorType::Rgb8
    },
  )?;

  Ok(())
}

/// Scans the RIFF container and returns (offset, length) of the VP8/VP8L payload.
/// Returns None if the tag is not found or if the buffer is truncated.
fn vp8_payload_coords(buf: &[u8]) -> Option<(usize, usize)> {
  // Skip RIFF header (12 bytes)
  if buf.len() < 12 {
    return None;
  }

  let mut i = 12;
  let buf_len = buf.len();

  // Iterate over chunks
  while i + 8 <= buf_len {
    let tag = &buf[i..i + 4];

    let len = u32::from_le_bytes(buf[i + 4..i + 8].try_into().ok()?) as usize;

    // Check for VP8 (Lossy) or VP8L (Lossless)
    if tag == b"VP8 " || tag == b"VP8L" {
      let start = i + 8;
      let end = start.checked_add(len)?; // Protect against usize overflow

      // Ensure the actual data exists in the buffer.
      if end > buf_len {
        return None;
      }

      return Some((start, len));
    }

    // Calculate next chunk offset (Size + Padding)
    let padding = len & 1;

    let chunk_size = len.checked_add(padding)?;
    i = (i + 8).checked_add(chunk_size)?;
  }

  None
}

// NAME + size (4 bytes)
const BASE_HEADER_SIZE: u32 = 8;

// x (3 bytes) + y (3 bytes) + w (3 bytes) + h (3 bytes) + duration (3 bytes) + flags (1 byte)
const ANMF_HEADER_SIZE: u32 = 16;

// flags (1 byte) + cw (3 bytes) + ch (3 bytes)
const VP8X_HEADER_SIZE: u32 = 10;

// background color (4 bytes) + loop count (2 bytes)
const ANIM_HEADER_SIZE: u32 = 6;

fn estimate_vp8_payload_size(buf: &[u8]) -> Result<u32> {
  let (_, len) = vp8_payload_coords(buf)
    .ok_or_else(|| IoError(std::io::Error::other("VP8/VP8L chunk not found")))?;

  let padding = len & 1;

  // ANMF chunk + VP8L chunk
  Ok(BASE_HEADER_SIZE + ANMF_HEADER_SIZE + BASE_HEADER_SIZE + len as u32 + padding as u32)
}

fn estimate_riff_size<'a, I: Iterator<Item = &'a [u8]>>(frames: I) -> Result<u32> {
  // "WEBP" +  VPX8 chunk + ANIM chunk + [ANMF chunks]
  let mut size = 4 + BASE_HEADER_SIZE + VP8X_HEADER_SIZE + BASE_HEADER_SIZE + ANIM_HEADER_SIZE;

  for frame in frames {
    size += estimate_vp8_payload_size(frame)?;
  }

  Ok(size)
}

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub fn encode_animated_webp<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()> {
  assert_ne!(frames.len(), 0);

  // encode frames losslessly and collect VP8L/VP8 payloads
  let frames_payloads: Vec<(&AnimationFrame, Vec<u8>)> = frames
    .iter()
    .map(|frame| {
      let mut buf = Vec::new();
      WebPEncoder::new(&mut buf)
        .encode(
          &frame.image,
          frame.image.width(),
          frame.image.height(),
          ::image_webp::ColorType::Rgba8,
        )
        .map_err(|_| IoError(std::io::Error::other("WebP encode error")))?;

      Ok((frame, buf))
    })
    .collect::<Result<Vec<(&AnimationFrame, Vec<u8>)>>>()?;

  let riff_size = estimate_riff_size(frames_payloads.iter().map(|(_, buf)| buf.as_slice()))?;

  // RIFF header
  destination.write_all(b"RIFF")?;
  destination.write_all(&riff_size.to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  // VP8X chunk
  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  let cw = (frames[0].image.width() - 1).to_le_bytes();
  let ch = (frames[0].image.height() - 1).to_le_bytes();

  destination.write_all(b"VP8X")?;
  destination.write_all(&VP8X_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[vp8x_flags])?;
  destination.write_all(&[0u8; 3])?;
  destination.write_all(&cw[..3])?;
  destination.write_all(&ch[..3])?;

  // ANIM chunk
  destination.write_all(b"ANIM")?;
  destination.write_all(&ANIM_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?; // bgcolor (4 bytes)
  destination.write_all(&options.loop_count.unwrap_or(0).to_le_bytes())?;

  let blend_flag = if options.blend { 0 } else { 1 };
  let dispose_flag = options.dispose as u8;
  let frame_flags = (blend_flag << 1) | dispose_flag;

  // ANMF frames
  for (frame, vp8_data) in frames_payloads.into_iter() {
    let w_bytes = (frame.image.width() - 1).to_le_bytes();
    let h_bytes = (frame.image.height() - 1).to_le_bytes();

    let (start, len) = vp8_payload_coords(&vp8_data)
      .ok_or_else(|| IoError(std::io::Error::other("VP8/VP8L chunk not found")))?;

    let vp8_payload = &vp8_data[start..start + len];

    let padding = vp8_payload.len() & 1;

    let anmf_size = ANMF_HEADER_SIZE + BASE_HEADER_SIZE + vp8_payload.len() as u32 + padding as u32;

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_size.to_le_bytes())?;

    // frame header (16 bytes)
    destination.write_all(&[0u8; 6])?; // x, y (3 bytes each)
    destination.write_all(&w_bytes[..3])?; // w (3 bytes)
    destination.write_all(&h_bytes[..3])?; // h (3 bytes)
    destination.write_all(&frame.duration_ms.clamp(0, U24_MAX).to_le_bytes()[..3])?; // duration (3 bytes)
    destination.write_all(&[frame_flags])?; // flags (1 byte)

    // VP8L chunk: VP8L payload
    destination.write_all(b"VP8L")?;
    destination.write_all(&(vp8_payload.len() as u32).to_le_bytes())?;
    destination.write_all(vp8_payload)?;

    // padding
    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  destination.flush()?;

  Ok(())
}

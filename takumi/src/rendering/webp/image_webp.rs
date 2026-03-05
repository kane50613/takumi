use std::{borrow::Cow, io::Write};

use image::RgbaImage;
use image_webp::{ColorType, EncoderParams, WebPEncoder};

use crate::{
  Result,
  error::WebPError,
  rendering::{
    webp::{U24_MAX, has_any_alpha_pixel, strip_alpha_channel},
    write::{AnimatedWebpOptions, AnimationFrame},
  },
};

const RIFF_HEADER_SIZE: usize = 12;
const BASE_HEADER_SIZE: u32 = 8;
const ANMF_HEADER_SIZE: u32 = 16;
const VP8X_HEADER_SIZE: u32 = 10;
const ANIM_HEADER_SIZE: u32 = 6;

fn vp8_chunk(buf: &[u8]) -> Option<([u8; 4], usize, usize)> {
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

      return Some((tag, payload_start, len));
    }

    let padding = len & 1;
    offset = (offset + 8).checked_add(len + padding)?;
  }

  None
}

fn vp8_payload_coords(buf: &[u8]) -> Option<(usize, usize)> {
  let (_, payload_start, payload_len) = vp8_chunk(buf)?;
  Some((payload_start, payload_len))
}

fn vp8_chunk_tag(buf: &[u8], payload_start: usize) -> Option<[u8; 4]> {
  let tag_start = payload_start.checked_sub(8)?;
  buf[tag_start..tag_start + 4].try_into().ok()
}

pub(crate) fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  quality: Option<u8>,
) -> Result<()> {
  let quality = quality.unwrap_or(100);
  let mut encoder = WebPEncoder::new(destination);
  let mut params = EncoderParams::default();
  params.use_predictor_transform = quality >= 75;
  encoder.set_params(params);
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
      ColorType::Rgba8
    } else {
      ColorType::Rgb8
    },
  )?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::{vp8_chunk, vp8_chunk_tag};

  #[test]
  fn vp8_chunk_tag_reads_chunk_tag_not_chunk_size() {
    let encoded = [
      b'R', b'I', b'F', b'F', 16, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b' ', 4, 0, 0,
      0, 1, 2, 3, 4,
    ];

    let (tag, payload_start, _) = vp8_chunk(&encoded).expect("expected VP8 chunk");

    assert_eq!(tag, *b"VP8 ");
    assert_eq!(vp8_chunk_tag(&encoded, payload_start), Some(*b"VP8 "));
  }
}

fn estimate_vp8_payload_size(buf: &[u8]) -> Result<u32> {
  let (_, len) = vp8_payload_coords(buf).ok_or(WebPError::MissingVp8Chunk)?;

  let padding = len & 1;
  let len_u32 = u32::try_from(len).map_err(|_| WebPError::Vp8PayloadSizeOverflow)?;
  let padding_u32 = u32::try_from(padding).map_err(|_| WebPError::Vp8PaddingSizeOverflow)?;

  BASE_HEADER_SIZE
    .checked_add(ANMF_HEADER_SIZE)
    .and_then(|size| size.checked_add(BASE_HEADER_SIZE))
    .and_then(|size| size.checked_add(len_u32))
    .and_then(|size| size.checked_add(padding_u32))
    .ok_or(WebPError::EstimatedVp8PayloadSizeOverflow.into())
}

fn estimate_riff_size<'a, I: Iterator<Item = &'a [u8]>>(frames: I) -> Result<u32> {
  let mut size = 4 + BASE_HEADER_SIZE + VP8X_HEADER_SIZE + BASE_HEADER_SIZE + ANIM_HEADER_SIZE;

  for frame in frames {
    size = size
      .checked_add(estimate_vp8_payload_size(frame)?)
      .ok_or(WebPError::EstimatedRiffSizeOverflow)?;
  }

  Ok(size)
}

fn validate_u24_dimension(name: &'static str, value: u32) -> Result<()> {
  if (1..=U24_MAX + 1).contains(&value) {
    return Ok(());
  }

  Err(
    WebPError::InvalidDimension {
      name,
      value,
      max: U24_MAX + 1,
    }
    .into(),
  )
}

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub fn encode_animated_webp<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()> {
  if frames.is_empty() {
    return Err(WebPError::EmptyAnimation.into());
  }

  let canvas_width = frames[0].image.width();
  let canvas_height = frames[0].image.height();
  validate_u24_dimension("WebP canvas width", canvas_width)?;
  validate_u24_dimension("WebP canvas height", canvas_height)?;

  for (index, frame) in frames.iter().enumerate() {
    let frame_width = frame.image.width();
    let frame_height = frame.image.height();
    validate_u24_dimension("WebP frame width", frame_width)?;
    validate_u24_dimension("WebP frame height", frame_height)?;

    if frame_width > canvas_width || frame_height > canvas_height {
      return Err(
        WebPError::FrameExceedsCanvas {
          index,
          frame_width,
          frame_height,
          canvas_width,
          canvas_height,
        }
        .into(),
      );
    }
  }

  let frames_payloads: Vec<(&AnimationFrame, Vec<u8>)> = frames
    .iter()
    .map(|frame| {
      let mut buf = Vec::new();
      let mut encoder = WebPEncoder::new(&mut buf);
      let mut params = EncoderParams::default();
      params.use_predictor_transform = options.quality >= 75;
      encoder.set_params(params);
      encoder
        .encode(
          &frame.image,
          frame.image.width(),
          frame.image.height(),
          ColorType::Rgba8,
        )
        .map_err(|_| WebPError::Encode)?;

      Ok((frame, buf))
    })
    .collect::<Result<Vec<(&AnimationFrame, Vec<u8>)>>>()?;

  let riff_size = estimate_riff_size(frames_payloads.iter().map(|(_, buf)| buf.as_slice()))?;

  destination.write_all(b"RIFF")?;
  destination.write_all(&riff_size.to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  let cw = (canvas_width - 1).to_le_bytes();
  let ch = (canvas_height - 1).to_le_bytes();

  destination.write_all(b"VP8X")?;
  destination.write_all(&VP8X_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[vp8x_flags])?;
  destination.write_all(&[0u8; 3])?;
  destination.write_all(&cw[..3])?;
  destination.write_all(&ch[..3])?;

  destination.write_all(b"ANIM")?;
  destination.write_all(&ANIM_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?;
  destination.write_all(&options.loop_count.unwrap_or(0).to_le_bytes())?;

  let blend_flag = if options.blend { 0 } else { 1 };
  let dispose_flag = options.dispose as u8;
  let frame_flags = (blend_flag << 1) | dispose_flag;

  for (frame, vp8_data) in frames_payloads.into_iter() {
    let w_bytes = (frame.image.width() - 1).to_le_bytes();
    let h_bytes = (frame.image.height() - 1).to_le_bytes();

    let (start, len) = vp8_payload_coords(&vp8_data).ok_or(WebPError::MissingVp8Chunk)?;

    let vp8_payload = &vp8_data[start..start + len];

    let padding = vp8_payload.len() & 1;
    let vp8_payload_len_u32 =
      u32::try_from(vp8_payload.len()).map_err(|_| WebPError::Vp8PayloadSizeOverflow)?;
    let padding_u32 = u32::try_from(padding).map_err(|_| WebPError::Vp8PaddingSizeOverflow)?;
    let anmf_size = ANMF_HEADER_SIZE
      .checked_add(BASE_HEADER_SIZE)
      .and_then(|size| size.checked_add(vp8_payload_len_u32))
      .and_then(|size| size.checked_add(padding_u32))
      .ok_or(WebPError::AnmfChunkSizeOverflow)?;

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_size.to_le_bytes())?;

    destination.write_all(&[0u8; 6])?;
    destination.write_all(&w_bytes[..3])?;
    destination.write_all(&h_bytes[..3])?;
    destination.write_all(&frame.duration_ms.clamp(0, U24_MAX).to_le_bytes()[..3])?;
    destination.write_all(&[frame_flags])?;

    let chunk_tag = vp8_chunk_tag(&vp8_data, start)
      .ok_or(WebPError::MissingVp8ChunkTag)
      .and_then(|tag| {
        if &tag == b"VP8 " || &tag == b"VP8L" {
          Ok(tag)
        } else {
          Err(WebPError::InvalidVp8ChunkTag)
        }
      })?;
    destination.write_all(&chunk_tag)?;
    destination.write_all(&vp8_payload_len_u32.to_le_bytes())?;
    destination.write_all(vp8_payload)?;

    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  destination.flush()?;

  Ok(())
}

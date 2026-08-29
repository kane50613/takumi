use std::{borrow::Cow, io::Write};

use image::RgbaImage;
use image_webp::{ColorType, EncoderParams, WebPEncoder};

use crate::{
  Result,
  error::{Error, WebPError},
  webp::{FramePlacement, U24_MAX, has_any_alpha_pixel, strip_alpha_channel},
  write::{AnimatedWebpOptions, AnimationFrame, Bitmap},
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

pub(crate) fn write_webp_lossless(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
) -> Result<()> {
  let mut encoder = WebPEncoder::new(destination);
  let mut params = EncoderParams::default();
  params.use_predictor_transform = true;
  encoder.set_params(params);
  let width = image.width();
  let height = image.height();
  let has_alpha = has_any_alpha_pixel(&image);

  let image_data = if has_alpha {
    Cow::Borrowed(image.as_raw())
  } else {
    Cow::Owned(strip_alpha_channel(image))
  };

  encoder
    .encode(
      &image_data,
      width,
      height,
      if has_alpha {
        ColorType::Rgba8
      } else {
        ColorType::Rgb8
      },
    )
    .map_err(Error::encode)?;

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
  let (_, len) = vp8_payload_coords(buf).ok_or(WebPError::InvalidEncodedData)?;

  let padding = len & 1;
  let len_u32 = u32::try_from(len).map_err(|_| WebPError::ContainerSizeOverflow)?;
  let padding_u32 = u32::try_from(padding).map_err(|_| WebPError::ContainerSizeOverflow)?;

  BASE_HEADER_SIZE
    .checked_add(ANMF_HEADER_SIZE)
    .and_then(|size| size.checked_add(BASE_HEADER_SIZE))
    .and_then(|size| size.checked_add(len_u32))
    .and_then(|size| size.checked_add(padding_u32))
    .ok_or(WebPError::ContainerSizeOverflow.into())
}

fn estimate_riff_size<'a, I: Iterator<Item = &'a [u8]>>(frames: I) -> Result<u32> {
  let mut size = 4 + BASE_HEADER_SIZE + VP8X_HEADER_SIZE + BASE_HEADER_SIZE + ANIM_HEADER_SIZE;

  for frame in frames {
    size = size
      .checked_add(estimate_vp8_payload_size(frame)?)
      .ok_or(WebPError::ContainerSizeOverflow)?;
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
pub fn write_animated_webp<W: Write>(
  frames: Cow<'_, [AnimationFrame]>,
  destination: &mut W,
  options: AnimatedWebpOptions,
) -> Result<()> {
  encode_animated_webp(
    frames.into_owned().into_iter().map(Ok),
    destination,
    options,
  )
}

/// A frame's placement, duration, and its already-encoded VP8 payload. Small
/// enough to keep for every frame; the raw pixels are dropped after encoding.
struct EncodedAnmf {
  placement: FramePlacement,
  duration_ms: u32,
  vp8: Vec<u8>,
}

/// Streams frames into an animated WebP, encoding each as it arrives so only one
/// raw frame is held at a time. Only the compact VP8 payloads are retained, since
/// the RIFF container needs every frame's size before the first byte is written.
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

  let canvas_width = first.image.width();
  let canvas_height = first.image.height();
  validate_u24_dimension("WebP canvas width", canvas_width)?;
  validate_u24_dimension("WebP canvas height", canvas_height)?;

  let validate_frame = |image: &Bitmap, index: usize| -> Result<()> {
    let width = image.width();
    let height = image.height();
    validate_u24_dimension("WebP frame width", width)?;
    validate_u24_dimension("WebP frame height", height)?;
    if width > canvas_width || height > canvas_height {
      return Err(
        WebPError::FrameExceedsCanvas {
          index,
          frame_width: width,
          frame_height: height,
          canvas_width,
          canvas_height,
        }
        .into(),
      );
    }
    Ok(())
  };

  let encode_frame =
    |image: &Bitmap, placement: FramePlacement, duration_ms: u32| -> Result<EncodedAnmf> {
      let region = placement.region;
      let cropped;
      let source = if region.covers(image.as_rgba()) {
        image.as_rgba()
      } else {
        cropped = region.crop(image.as_rgba());
        &cropped
      };

      let mut buf = Vec::new();
      let mut encoder = WebPEncoder::new(&mut buf);
      let mut params = EncoderParams::default();
      params.use_predictor_transform = true;
      encoder.set_params(params);
      encoder
        .encode(
          source.as_raw(),
          region.width,
          region.height,
          ColorType::Rgba8,
        )
        .map_err(|_| WebPError::EncodeFailed)?;

      Ok(EncodedAnmf {
        placement,
        duration_ms,
        vp8: buf,
      })
    };

  // Merge runs of identical frames, matching the native encoder, so a static
  // stretch encodes and stores once.
  validate_frame(&first.image, 0)?;
  let mut anmfs = Vec::new();
  let mut pending_image = first.image;
  let mut pending_placement = FramePlacement::first(pending_image.as_rgba(), &options);
  let mut pending_duration_ms = first.duration_ms.clamp(0, U24_MAX);

  for (offset, frame) in frames.enumerate() {
    let frame = frame?;
    validate_frame(&frame.image, offset + 1)?;

    let Some(placement) = FramePlacement::next(
      pending_image.as_rgba(),
      frame.image.as_rgba(),
      canvas_width,
      canvas_height,
      &options,
    ) else {
      pending_duration_ms = pending_duration_ms.saturating_add(frame.duration_ms.clamp(0, U24_MAX));
      continue;
    };

    anmfs.push(encode_frame(
      &pending_image,
      pending_placement,
      pending_duration_ms,
    )?);
    pending_image = frame.image;
    pending_placement = placement;
    pending_duration_ms = frame.duration_ms.clamp(0, U24_MAX);
  }
  anmfs.push(encode_frame(
    &pending_image,
    pending_placement,
    pending_duration_ms,
  )?);

  let riff_size = estimate_riff_size(anmfs.iter().map(|anmf| anmf.vp8.as_slice()))?;

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

  let dispose_flag = options.dispose as u8;

  for anmf in anmfs {
    let region = anmf.placement.region;
    let frame_flags = (u8::from(!anmf.placement.blend) << 1) | dispose_flag;
    let x_bytes = (region.x / 2).to_le_bytes();
    let y_bytes = (region.y / 2).to_le_bytes();
    let w_bytes = (region.width - 1).to_le_bytes();
    let h_bytes = (region.height - 1).to_le_bytes();

    let (start, len) = vp8_payload_coords(&anmf.vp8).ok_or(WebPError::InvalidEncodedData)?;
    let vp8_payload = &anmf.vp8[start..start + len];

    let padding = vp8_payload.len() & 1;
    let vp8_payload_len_u32 =
      u32::try_from(vp8_payload.len()).map_err(|_| WebPError::ContainerSizeOverflow)?;
    let padding_u32 = u32::try_from(padding).map_err(|_| WebPError::ContainerSizeOverflow)?;
    let anmf_size = ANMF_HEADER_SIZE
      .checked_add(BASE_HEADER_SIZE)
      .and_then(|size| size.checked_add(vp8_payload_len_u32))
      .and_then(|size| size.checked_add(padding_u32))
      .ok_or(WebPError::ContainerSizeOverflow)?;

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_size.to_le_bytes())?;

    destination.write_all(&x_bytes[..3])?;
    destination.write_all(&y_bytes[..3])?;
    destination.write_all(&w_bytes[..3])?;
    destination.write_all(&h_bytes[..3])?;
    destination.write_all(&anmf.duration_ms.clamp(0, U24_MAX).to_le_bytes()[..3])?;
    destination.write_all(&[frame_flags])?;

    let chunk_tag = vp8_chunk_tag(&anmf.vp8, start)
      .ok_or(WebPError::InvalidEncodedData)
      .and_then(|tag| {
        if &tag == b"VP8 " || &tag == b"VP8L" {
          Ok(tag)
        } else {
          Err(WebPError::InvalidEncodedData)
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

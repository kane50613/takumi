use std::{
  borrow::{Borrow, Cow},
  io::Write,
  mem,
  ops::Range,
};

use image::RgbaImage;
#[cfg(target_arch = "wasm32")]
use image::imageops::crop_imm;

use crate::{
  Result,
  error::{Error, WebPError},
  write::{AnimatedWebpOptions, Bitmap},
};

#[cfg(target_arch = "wasm32")]
mod image_webp;
#[cfg(not(target_arch = "wasm32"))]
mod libwebp;

#[cfg(target_arch = "wasm32")]
pub use image_webp::write_animated_webp;
#[cfg(target_arch = "wasm32")]
pub(crate) use image_webp::{encode_animated_webp, write_webp_lossless};
#[cfg(not(target_arch = "wasm32"))]
pub use libwebp::write_animated_webp;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use libwebp::{encode_animated_webp, write_webp_lossless, write_webp_lossy};

pub(super) const U24_MAX: u32 = 0xffffff;

fn pixels_visibly_equal(a: &[u8; 4], b: &[u8; 4]) -> bool {
  a == b || (a[3] == 0 && b[3] == 0)
}

/// Sub-rectangle of the canvas an ANMF frame covers, with an even origin as the
/// container stores offsets halved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FrameRegion {
  pub x: u32,
  pub y: u32,
  pub width: u32,
  pub height: u32,
}

impl FrameRegion {
  pub(super) fn full(width: u32, height: u32) -> Self {
    Self {
      x: 0,
      y: 0,
      width,
      height,
    }
  }

  /// Bounding box of visibly differing pixels between two equally sized images,
  /// or `None` when every difference is invisible. RGB under alpha 0 never
  /// counts: the encoder is free to rewrite it anyway (`config.exact` is 0).
  pub(super) fn diff(previous: &RgbaImage, current: &RgbaImage) -> Option<Self> {
    let row_bytes = previous.width() as usize * 4;
    let rows = previous
      .as_raw()
      .chunks_exact(row_bytes)
      .zip(current.as_raw().chunks_exact(row_bytes));

    let mut top = None;
    let mut bottom = 0;
    let mut left = previous.width();
    let mut right = 0;

    for (row_index, (previous_row, current_row)) in rows.enumerate() {
      if previous_row == current_row {
        continue;
      }

      let previous_pixels = bytemuck::cast_slice::<u8, [u8; 4]>(previous_row);
      let current_pixels = bytemuck::cast_slice::<u8, [u8; 4]>(current_row);
      let mut differing = previous_pixels
        .iter()
        .zip(current_pixels)
        .enumerate()
        .filter(|(_, (previous_pixel, current_pixel))| {
          !pixels_visibly_equal(previous_pixel, current_pixel)
        });

      let Some((first, _)) = differing.next() else {
        continue;
      };

      let first = first as u32;
      let last = differing
        .next_back()
        .map_or(first, |(index, _)| index as u32);

      top.get_or_insert(row_index as u32);
      bottom = row_index as u32;
      left = left.min(first);
      right = right.max(last);
    }

    let top = top?;
    let x = left & !1;
    let y = top & !1;

    Some(Self {
      x,
      y,
      width: right + 1 - x,
      height: bottom + 1 - y,
    })
  }

  #[cfg(target_arch = "wasm32")]
  pub(super) fn covers(&self, image: &RgbaImage) -> bool {
    self.x == 0 && self.y == 0 && self.width == image.width() && self.height == image.height()
  }

  #[cfg(target_arch = "wasm32")]
  pub(super) fn crop(&self, image: &RgbaImage) -> RgbaImage {
    crop_imm(image, self.x, self.y, self.width, self.height).to_image()
  }
}

/// Where an encoded frame lands on the canvas and whether it blends over it.
#[derive(Debug, Clone, Copy)]
pub(super) struct FramePlacement {
  pub region: FrameRegion,
  pub blend: bool,
}

impl FramePlacement {
  pub(super) fn first(image: &RgbaImage, options: &AnimatedWebpOptions) -> Self {
    Self {
      region: FrameRegion::full(image.width(), image.height()),
      blend: options.blend,
    }
  }

  /// Placement for `current` relative to `previous`, or `None` when the frames
  /// are identical and should merge into one longer frame.
  pub(super) fn next(
    previous: &RgbaImage,
    current: &RgbaImage,
    canvas_width: u32,
    canvas_height: u32,
    options: &AnimatedWebpOptions,
  ) -> Option<Self> {
    let same_dimensions = previous.dimensions() == current.dimensions();
    // Diffing needs the canvas to keep holding the previous frame: dispose clears
    // the rectangle, and a sub-canvas frame never wrote the pixels around it.
    let diffable =
      same_dimensions && current.dimensions() == (canvas_width, canvas_height) && !options.dispose;

    if diffable {
      let region = FrameRegion::diff(previous, current)?;

      // No-blend so the rectangle replaces the canvas; blending would compose the
      // two alphas instead of matching the source frame.
      return Some(Self {
        region,
        blend: false,
      });
    }

    if same_dimensions && previous.as_raw() == current.as_raw() {
      return None;
    }

    Some(Self {
      region: FrameRegion::full(current.width(), current.height()),
      blend: options.blend,
    })
  }
}

/// A run of identical frames merged into one, with where it lands on the canvas.
pub(super) struct UniqueFrame<T> {
  pub image: T,
  pub placement: FramePlacement,
  pub duration_ms: u32,
}

impl<T: Borrow<Bitmap>> UniqueFrame<T> {
  pub(super) fn first(image: T, duration_ms: u32, options: &AnimatedWebpOptions) -> Self {
    Self {
      placement: FramePlacement::first(image.borrow().as_rgba(), options),
      duration_ms: duration_ms.clamp(0, U24_MAX),
      image,
    }
  }

  /// Lengthens this frame when `image` looks the same, or starts the next frame
  /// from `image` and returns this one finished.
  pub(super) fn push(
    &mut self,
    image: T,
    duration_ms: u32,
    canvas_width: u32,
    canvas_height: u32,
    options: &AnimatedWebpOptions,
  ) -> Option<Self> {
    let duration_ms = duration_ms.clamp(0, U24_MAX);
    let Some(placement) = FramePlacement::next(
      self.image.borrow().as_rgba(),
      image.borrow().as_rgba(),
      canvas_width,
      canvas_height,
      options,
    ) else {
      self.duration_ms = self.duration_ms.saturating_add(duration_ms);
      return None;
    };

    Some(mem::replace(
      self,
      Self {
        image,
        placement,
        duration_ms,
      },
    ))
  }
}

/// A frame's encoded WebP file, located down to the VP8 or VP8L chunk an ANMF
/// frame wraps.
pub(super) struct EncodedFrame<B> {
  encoded: B,
  payload_range: Range<usize>,
  tag: [u8; 4],
  placement: FramePlacement,
  duration_ms: u32,
}

impl<B: AsRef<[u8]>> EncodedFrame<B> {
  /// `None` when `encoded` holds no VP8 or VP8L chunk.
  pub(super) fn new(encoded: B, placement: FramePlacement, duration_ms: u32) -> Option<Self> {
    let (tag, payload_range) = vp8_chunk(encoded.as_ref())?;

    Some(Self {
      encoded,
      payload_range,
      tag,
      placement,
      duration_ms,
    })
  }

  fn payload(&self) -> &[u8] {
    &self.encoded.as_ref()[self.payload_range.clone()]
  }
}

/// Finds the VP8 or VP8L chunk of a single-image WebP file.
fn vp8_chunk(buf: &[u8]) -> Option<([u8; 4], Range<usize>)> {
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

      return Some((tag, payload_start..payload_end));
    }

    let padding = len & 1;
    offset = (offset + 8).checked_add(len + padding)?;
  }

  None
}

const VP8X_CHUNK_BYTES: usize = 18;
const ANIM_CHUNK_BYTES: usize = 14;

fn anmf_chunk_bytes(vp8_len: usize) -> Result<usize> {
  8usize
    .checked_add(16)
    .and_then(|v| v.checked_add(8))
    .and_then(|v| v.checked_add(vp8_len))
    .and_then(|v| v.checked_add(vp8_len & 1))
    .ok_or(WebPError::ContainerSizeOverflow.into())
}

fn write_le24<W: Write>(destination: &mut W, value: u32) -> Result<()> {
  destination.write_all(&value.to_le_bytes()[..3])?;
  Ok(())
}

/// Writes the animated WebP container around already-encoded frames.
pub(super) fn write_riff_container<W: Write, B: AsRef<[u8]>>(
  frames: &[EncodedFrame<B>],
  canvas_width: u32,
  canvas_height: u32,
  destination: &mut W,
  options: &AnimatedWebpOptions,
) -> Result<()> {
  let frames_total = frames.iter().try_fold(0usize, |acc, frame| {
    acc
      .checked_add(anmf_chunk_bytes(frame.payload().len())?)
      .ok_or(WebPError::ContainerSizeOverflow)
      .map_err(Error::from)
  })?;
  let riff_payload_usize = 4usize
    .checked_add(VP8X_CHUNK_BYTES)
    .and_then(|v| v.checked_add(ANIM_CHUNK_BYTES))
    .and_then(|v| v.checked_add(frames_total))
    .ok_or(WebPError::ContainerSizeOverflow)?;
  let riff_payload =
    u32::try_from(riff_payload_usize).map_err(|_| WebPError::ContainerSizeOverflow)?;

  destination.write_all(b"RIFF")?;
  destination.write_all(&riff_payload.to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  destination.write_all(b"VP8X")?;
  destination.write_all(&10u32.to_le_bytes())?;
  destination.write_all(&[vp8x_flags, 0, 0, 0])?;
  write_le24(destination, canvas_width - 1)?;
  write_le24(destination, canvas_height - 1)?;

  destination.write_all(b"ANIM")?;
  destination.write_all(&6u32.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?;
  destination.write_all(&options.loop_count.unwrap_or(0).to_le_bytes())?;

  for frame in frames {
    let vp8_payload = frame.payload();
    let vp8_len = vp8_payload.len();
    let padding = vp8_len & 1;
    let anmf_payload_size_usize = 16usize
      .checked_add(8)
      .and_then(|v| v.checked_add(vp8_len))
      .and_then(|v| v.checked_add(padding))
      .ok_or(WebPError::ContainerSizeOverflow)?;
    let anmf_payload_size =
      u32::try_from(anmf_payload_size_usize).map_err(|_| WebPError::ContainerSizeOverflow)?;

    let region = frame.placement.region;
    let frame_flags: u8 = (u8::from(!frame.placement.blend) << 1) | u8::from(options.dispose);

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_payload_size.to_le_bytes())?;
    write_le24(destination, region.x / 2)?;
    write_le24(destination, region.y / 2)?;
    write_le24(destination, region.width - 1)?;
    write_le24(destination, region.height - 1)?;
    write_le24(destination, frame.duration_ms.clamp(0, U24_MAX))?;
    destination.write_all(&[frame_flags])?;
    destination.write_all(&frame.tag)?;
    let vp8_len_u32 = u32::try_from(vp8_len).map_err(|_| WebPError::ContainerSizeOverflow)?;
    destination.write_all(&vp8_len_u32.to_le_bytes())?;
    destination.write_all(vp8_payload)?;
    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  Ok(())
}

pub(super) fn strip_alpha_channel(image: Cow<'_, RgbaImage>) -> Vec<u8> {
  match image {
    Cow::Owned(image) => {
      let mut rgba = image.into_raw();
      let pixels = rgba.len() / 4;

      for pixel_index in 0..pixels {
        let src_offset = pixel_index * 4;
        let dst_offset = pixel_index * 3;
        rgba[dst_offset] = rgba[src_offset];
        rgba[dst_offset + 1] = rgba[src_offset + 1];
        rgba[dst_offset + 2] = rgba[src_offset + 2];
      }

      rgba.truncate(pixels * 3);
      rgba
    }
    Cow::Borrowed(image) => {
      let pixels = bytemuck::cast_slice::<u8, [u8; 4]>(image.as_raw());
      let mut rgb = Vec::with_capacity(pixels.len() * 3);

      for [r, g, b, _] in pixels {
        rgb.extend_from_slice(&[*r, *g, *b]);
      }

      rgb
    }
  }
}

pub(super) fn has_any_alpha_pixel(image: &RgbaImage) -> bool {
  bytemuck::cast_slice::<u8, [u8; 4]>(image.as_raw())
    .iter()
    .any(|[_, _, _, a]| *a != u8::MAX)
}

#[cfg(test)]
mod tests {
  use image::Rgba;

  use super::*;

  #[test]
  fn vp8_chunk_reads_chunk_tag_not_chunk_size() {
    let encoded = [
      b'R', b'I', b'F', b'F', 16, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b' ', 4, 0, 0,
      0, 1, 2, 3, 4,
    ];

    assert_eq!(vp8_chunk(&encoded), Some((*b"VP8 ", 20..24)));
  }

  #[test]
  fn diff_rounds_origin_down_to_even_and_grows_size() {
    let base = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 255]));
    let mut changed = base.clone();
    changed.put_pixel(5, 3, Rgba([255, 0, 0, 255]));
    changed.put_pixel(7, 7, Rgba([0, 255, 0, 255]));

    let region = FrameRegion::diff(&base, &changed).unwrap();
    assert_eq!(
      region,
      FrameRegion {
        x: 4,
        y: 2,
        width: 4,
        height: 6
      }
    );
  }

  #[test]
  fn diff_of_identical_images_is_none() {
    let base = RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255]));

    assert_eq!(FrameRegion::diff(&base, &base.clone()), None);
  }

  #[test]
  fn diff_ignores_rgb_under_alpha_zero() {
    let mut previous = RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255]));
    let mut current = previous.clone();
    previous.put_pixel(0, 0, Rgba([50, 60, 70, 0]));
    current.put_pixel(0, 0, Rgba([7, 8, 9, 0]));
    current.put_pixel(6, 6, Rgba([9, 9, 9, 255]));

    let region = FrameRegion::diff(&previous, &current).unwrap();
    assert_eq!(
      region,
      FrameRegion {
        x: 6,
        y: 6,
        width: 1,
        height: 1
      }
    );
  }
}

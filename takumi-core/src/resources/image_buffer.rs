//! Backend-agnostic decoded-image storage.

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

use crate::style::math::fast_div_255;

/// A decoded image as premultiplied, row-major RGBA bytes.
#[derive(Debug, Clone)]
pub struct ImageBuffer {
  data: Vec<u8>,
  width: u32,
  height: u32,
}

impl ImageBuffer {
  /// Wraps premultiplied RGBA bytes. Returns `None` if `data.len() != width * height * 4`.
  pub fn from_premultiplied_rgba(data: Vec<u8>, width: u32, height: u32) -> Option<Self> {
    let expected = (width as usize)
      .checked_mul(height as usize)?
      .checked_mul(4)?;
    (data.len() == expected).then_some(Self {
      data,
      width,
      height,
    })
  }

  /// Allocates a transparent (all-zero) buffer of the given size.
  #[cfg(test)]
  pub(crate) fn new(width: u32, height: u32) -> Option<Self> {
    let len = (width as usize)
      .checked_mul(height as usize)?
      .checked_mul(4)?;
    Some(Self {
      data: vec![0; len],
      width,
      height,
    })
  }

  /// Builds a premultiplied buffer from straight-alpha RGBA bytes (row-major, 4 bytes/pixel).
  /// Returns `None` if `raw.len() != width * height * 4`.
  pub fn from_rgba_bytes(mut raw: Vec<u8>, width: u32, height: u32) -> Option<Self> {
    if !has_opaque_alpha(&raw) {
      premultiply_rgba_in_place(&mut raw);
    }
    Self::from_premultiplied_rgba(raw, width, height)
  }

  /// The image width in pixels.
  pub fn width(&self) -> u32 {
    self.width
  }

  /// The image height in pixels.
  pub fn height(&self) -> u32 {
    self.height
  }

  /// Consumes the buffer, returning the premultiplied RGBA bytes.
  #[cfg(feature = "svg")]
  pub(crate) fn into_premultiplied_rgba(self) -> Vec<u8> {
    self.data
  }

  /// The premultiplied RGBA bytes, row-major.
  pub fn data(&self) -> &[u8] {
    &self.data
  }

  /// Mutable access to the premultiplied RGBA bytes.
  pub fn data_mut(&mut self) -> &mut [u8] {
    &mut self.data
  }

  /// Encodes the image as straight-alpha PNG bytes, for embedding in an SVG
  /// `<image>` data URL. Returns `None` if encoding fails.
  pub fn encode_png(&self) -> Option<Vec<u8>> {
    let mut straight = self.data.clone();

    unpremultiply_in_place(&mut straight);
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
      .write_image(&straight, self.width, self.height, ExtendedColorType::Rgba8)
      .ok()?;
    Some(out)
  }

  /// Reads the premultiplied RGBA value at `(x, y)`, or `[0; 4]` if out of bounds.
  pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
    if x >= self.width || y >= self.height {
      return [0; 4];
    }
    let index = ((y as usize * self.width as usize) + x as usize) * 4;
    [
      self.data[index],
      self.data[index + 1],
      self.data[index + 2],
      self.data[index + 3],
    ]
  }
}

/// Converts premultiplied RGBA bytes to straight alpha in place.
pub(crate) fn unpremultiply_in_place(data: &mut [u8]) {
  for pixel in data.as_chunks_mut::<4>().0 {
    let alpha = pixel[3];
    if alpha != 0 && alpha != 255 {
      let alpha = alpha as u16;
      pixel[0] = (pixel[0] as u16 * 255 / alpha).min(255) as u8;
      pixel[1] = (pixel[1] as u16 * 255 / alpha).min(255) as u8;
      pixel[2] = (pixel[2] as u16 * 255 / alpha).min(255) as u8;
    }
  }
}

const ALPHA_MASK_U128: u128 =
  u128::from_ne_bytes([0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF]);

#[inline(always)]
fn has_opaque_alpha(raw: &[u8]) -> bool {
  let (chunks, remainder) = raw.as_chunks::<16>();
  for chunk in chunks {
    if u128::from_ne_bytes(*chunk) & ALPHA_MASK_U128 != ALPHA_MASK_U128 {
      return false;
    }
  }
  remainder
    .as_chunks::<4>()
    .0
    .iter()
    .all(|pixel| pixel[3] == u8::MAX)
}

#[inline(always)]
pub(crate) fn premultiply_rgba_in_place(raw: &mut [u8]) {
  for pixel in raw.as_chunks_mut::<4>().0 {
    let alpha = pixel[3];
    if alpha == u8::MAX {
      continue;
    }
    if alpha == 0 {
      pixel[0] = 0;
      pixel[1] = 0;
      pixel[2] = 0;
      continue;
    }
    let alpha_u32 = alpha as u32;
    pixel[0] = fast_div_255(pixel[0] as u32 * alpha_u32);
    pixel[1] = fast_div_255(pixel[1] as u32 * alpha_u32);
    pixel[2] = fast_div_255(pixel[2] as u32 * alpha_u32);
  }
}

#[cfg(test)]
mod tests {
  use super::has_opaque_alpha;

  fn pixel(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    [r, g, b, a]
  }

  fn flatten(pixels: &[[u8; 4]]) -> Vec<u8> {
    pixels.iter().flatten().copied().collect()
  }

  #[test]
  fn empty_slice_is_opaque() {
    assert!(has_opaque_alpha(&[]));
  }

  #[test]
  fn fully_opaque_short_under_16_bytes() {
    let raw = flatten(&[
      pixel(1, 2, 3, 255),
      pixel(4, 5, 6, 255),
      pixel(7, 8, 9, 255),
    ]);
    assert!(has_opaque_alpha(&raw));
  }

  #[test]
  fn fully_opaque_exactly_one_chunk() {
    let raw = flatten(&[pixel(1, 2, 3, 255); 4]);
    assert!(has_opaque_alpha(&raw));
  }

  #[test]
  fn fully_opaque_chunk_plus_tail() {
    let mut raw = flatten(&[pixel(1, 2, 3, 255); 4]);
    raw.extend_from_slice(&[pixel(9, 9, 9, 255), pixel(8, 8, 8, 255)].concat());
    assert!(has_opaque_alpha(&raw));
  }

  #[test]
  fn detects_non_opaque_inside_chunk() {
    let mut pixels = [pixel(1, 2, 3, 255); 4];
    pixels[2][3] = 254;
    assert!(!has_opaque_alpha(&flatten(&pixels)));
  }

  #[test]
  fn detects_non_opaque_in_tail() {
    let mut raw = flatten(&[pixel(1, 2, 3, 255); 4]);
    raw.extend_from_slice(&pixel(0, 0, 0, 0));
    assert!(!has_opaque_alpha(&raw));
  }

  #[test]
  fn detects_first_non_opaque() {
    let mut pixels = [pixel(1, 2, 3, 255); 8];
    pixels[0][3] = 0;
    assert!(!has_opaque_alpha(&flatten(&pixels)));
  }

  #[test]
  fn rgb_values_do_not_affect_result() {
    let raw = flatten(&[pixel(255, 255, 255, 255); 8]);
    assert!(has_opaque_alpha(&raw));
    let raw = flatten(&[pixel(255, 255, 255, 0); 8]);
    assert!(!has_opaque_alpha(&raw));
  }
}

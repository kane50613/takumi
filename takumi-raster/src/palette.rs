//! Images of at most 256 colors, which PNG stores as one palette index per pixel.

use image::RgbaImage;

/// Most colors a PNG palette holds.
const MAX_COLORS: usize = 256;

/// Rows between the ones checked before the whole image is, so most images with more colors stop
/// after a fraction of their pixels.
const SAMPLE_STRIDE: usize = 16;

/// An image's colors and the index of each pixel's.
pub(crate) struct Palette {
  /// One index per pixel into `colors`, row by row.
  pub(crate) indices: Vec<u8>,
  colors: Vec<[u8; 4]>,
}

impl Palette {
  /// The palette of `rgba`, or `None` when it holds more than 256 colors.
  pub(crate) fn of(rgba: &RgbaImage) -> Option<Self> {
    let mut table = ColorTable::default();
    let mut colors = Vec::with_capacity(MAX_COLORS);
    let row_bytes = rgba.width() as usize * 4;

    for row in rgba.as_raw().chunks_exact(row_bytes).step_by(SAMPLE_STRIDE) {
      for pixel in row.as_chunks::<4>().0 {
        table.index(*pixel, &mut colors)?;
      }
    }

    let mut indices = Vec::with_capacity(rgba.as_raw().len() / 4);
    let mut previous = None;

    for &pixel in rgba.as_raw().as_chunks::<4>().0 {
      let index = match previous {
        Some((color, index)) if color == pixel => index,
        _ => table.index(pixel, &mut colors)?,
      };

      previous = Some((pixel, index));
      indices.push(index);
    }
    Some(Self { indices, colors })
  }

  /// The `PLTE` entries: each color's red, green and blue.
  pub(crate) fn rgb(&self) -> Vec<u8> {
    self
      .colors
      .iter()
      .flat_map(|color| [color[0], color[1], color[2]])
      .collect()
  }

  /// The `tRNS` entries, each color's alpha, or `None` when every color is opaque.
  pub(crate) fn alphas(&self) -> Option<Vec<u8>> {
    self
      .colors
      .iter()
      .any(|color| color[3] != u8::MAX)
      .then(|| self.colors.iter().map(|color| color[3]).collect())
  }
}

/// Slots in a [`ColorTable`]: four per color, so a probe rarely runs past one.
const SLOTS: usize = MAX_COLORS * 4;

/// The index given to each color seen so far, by open addressing.
struct ColorTable {
  keys: [u32; SLOTS],
  /// One past each slot's index; zero marks an empty slot.
  entries: [u16; SLOTS],
}

impl Default for ColorTable {
  fn default() -> Self {
    Self {
      keys: [0; SLOTS],
      entries: [0; SLOTS],
    }
  }
}

impl ColorTable {
  /// The index of `color`, appended to `colors` if new, or `None` once that would pass 256.
  fn index(&mut self, color: [u8; 4], colors: &mut Vec<[u8; 4]>) -> Option<u8> {
    let key = u32::from_ne_bytes(color);
    let mut slot = (key.wrapping_mul(0x9E37_79B1) >> (32 - SLOTS.trailing_zeros())) as usize;

    loop {
      match self.entries[slot] {
        0 => {
          if colors.len() == MAX_COLORS {
            return None;
          }
          colors.push(color);
          self.keys[slot] = key;
          self.entries[slot] = colors.len() as u16;
          return Some((colors.len() - 1) as u8);
        }
        entry if self.keys[slot] == key => return Some((entry - 1) as u8),
        _ => slot = (slot + 1) % SLOTS,
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};

  use super::Palette;

  #[test]
  fn indices_map_back_to_every_pixel() {
    let rgba = RgbaImage::from_fn(50, 40, |x, y| {
      Rgba([
        (x % 8 * 30) as u8,
        (y % 8 * 30) as u8,
        7,
        if x % 8 == 3 { 128 } else { 255 },
      ])
    });
    let palette = Palette::of(&rgba).unwrap();
    let rgb = palette.rgb();
    let alphas = palette.alphas().unwrap();

    for (pixel, &index) in rgba.pixels().zip(&palette.indices) {
      let index = usize::from(index);

      assert_eq!(
        pixel.0,
        [
          rgb[index * 3],
          rgb[index * 3 + 1],
          rgb[index * 3 + 2],
          alphas[index]
        ]
      );
    }
  }

  #[test]
  fn opaque_images_carry_no_alphas() {
    let rgba = RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255]));

    assert!(Palette::of(&rgba).unwrap().alphas().is_none());
  }

  #[test]
  fn a_257th_color_rejects_the_image() {
    let rgba = RgbaImage::from_fn(257, 1, |x, _| Rgba([x as u8, (x >> 8) as u8, 0, 255]));

    assert!(Palette::of(&rgba).is_none());
    assert!(
      Palette::of(&RgbaImage::from_fn(256, 1, |x, _| Rgba([
        x as u8, 0, 0, 255
      ])))
      .is_some()
    );
  }
}

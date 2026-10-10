//! An image's rows as PNG scanlines: a filter-type byte, then the row filtered as
//! [PNG](https://www.w3.org/TR/png-3/#9Filters) defines.

use std::{mem::swap, ops::Range};

use image::RgbaImage;

/// The pixels an image's PNG data holds.
#[derive(Clone, Copy)]
pub(crate) enum Pixels<'a> {
  /// RGBA pixels, written with alpha only when `keep_alpha`.
  Rgba {
    image: &'a RgbaImage,
    keep_alpha: bool,
  },
  /// One palette index per pixel, `width` to a row.
  Indexed { indices: &'a [u8], width: usize },
}

/// An image's rows as PNG scanlines, unfiltered or filtered adaptively.
pub(crate) struct Scanlines<'a> {
  pixels: Pixels<'a>,
  adaptive: bool,
}

/// A PNG filter type other than None.
#[derive(Clone, Copy)]
#[repr(u8)]
enum RowFilter {
  Sub = 1,
  Up = 2,
  Average = 3,
  Paeth = 4,
}

impl<'a> Scanlines<'a> {
  /// The scanlines of `pixels`, filtered adaptively when `adaptive`.
  pub(crate) fn new(pixels: Pixels<'a>, adaptive: bool) -> Self {
    Self { pixels, adaptive }
  }

  /// Bytes in one scanline, its filter-type byte included.
  pub(crate) fn len(&self) -> usize {
    self.pixels.row_len() + 1
  }

  /// Appends the scanlines of `rows`. A filtered row reads the one before it, so a range that
  /// starts mid-image writes what the whole image would there.
  pub(crate) fn append(&self, rows: Range<usize>, out: &mut Vec<u8>) {
    if !self.adaptive {
      for row in rows {
        out.push(0);
        self.pixels.extend_row(row, out);
      }
      return;
    }

    let bpp = self.pixels.bytes_per_pixel();
    let row_len = self.pixels.row_len();
    let mut previous = vec![0; row_len];
    let mut current = Vec::with_capacity(row_len);
    let mut best = vec![0; row_len];
    let mut trial = vec![0; row_len];

    if let Some(above) = rows.start.checked_sub(1) {
      previous.clear();
      self.pixels.extend_row(above, &mut previous);
    }
    for row in rows {
      current.clear();
      self.pixels.extend_row(row, &mut current);
      out.push(adaptive_filter(bpp, &previous, &current, &mut best, &mut trial) as u8);
      out.extend_from_slice(&best);
      swap(&mut previous, &mut current);
    }
  }
}

impl<'a> Pixels<'a> {
  /// Bytes each pixel takes in a scanline.
  pub(crate) fn bytes_per_pixel(self) -> usize {
    match self {
      Self::Rgba { keep_alpha, .. } => {
        if keep_alpha {
          4
        } else {
          3
        }
      }
      Self::Indexed { .. } => 1,
    }
  }

  /// Bytes in one row of pixels.
  fn row_len(self) -> usize {
    let width = match self {
      Self::Rgba { image, .. } => image.width() as usize,
      Self::Indexed { width, .. } => width,
    };

    width * self.bytes_per_pixel()
  }

  /// Appends row `row`'s pixels.
  fn extend_row(self, row: usize, out: &mut Vec<u8>) {
    match self {
      Self::Rgba { image, keep_alpha } => {
        let row_bytes = image.width() as usize * 4;
        let pixels = &image.as_raw()[row * row_bytes..(row + 1) * row_bytes];

        if keep_alpha {
          out.extend_from_slice(pixels);
        } else {
          for pixel in pixels.as_chunks::<4>().0 {
            out.extend_from_slice(&pixel[..3]);
          }
        }
      }
      Self::Indexed { indices, width } => {
        out.extend_from_slice(&indices[row * width..(row + 1) * width]);
      }
    }
  }
}

/// Filters `current` into `best` as the png crate's `Filter::Adaptive` does: Up, Sub, Average and
/// Paeth in turn, keeping the last with the least summed magnitude, and stopping at a zero row.
fn adaptive_filter(
  bpp: usize,
  previous: &[u8],
  current: &[u8],
  best: &mut Vec<u8>,
  trial: &mut Vec<u8>,
) -> RowFilter {
  let mut least = u64::MAX;
  let mut choice = RowFilter::Up;

  for filter in [
    RowFilter::Up,
    RowFilter::Sub,
    RowFilter::Average,
    RowFilter::Paeth,
  ] {
    filter.apply(bpp, previous, current, trial);

    let cost = magnitude(trial);

    if cost <= least {
      least = cost;
      choice = filter;
      swap(best, trial);
      if cost == 0 {
        break;
      }
    }
  }
  choice
}

impl RowFilter {
  /// Writes `current` filtered against `previous` into `out`, `bpp` bytes to a pixel.
  fn apply(self, bpp: usize, previous: &[u8], current: &[u8], out: &mut [u8]) {
    let len = current.len();
    let (out_first, out_rest) = out.split_at_mut(bpp);
    let (current_first, current_rest) = current.split_at(bpp);
    let (previous_first, previous_rest) = previous.split_at(bpp);
    let left = &current[..len - bpp];
    let upper_left = &previous[..len - bpp];

    match self {
      Self::Sub => {
        out_first.copy_from_slice(current_first);
        for ((out, &value), &left) in out_rest.iter_mut().zip(current_rest).zip(left) {
          *out = value.wrapping_sub(left);
        }
      }
      Self::Up => {
        for ((out, &value), &up) in out.iter_mut().zip(current).zip(previous) {
          *out = value.wrapping_sub(up);
        }
      }
      Self::Average => {
        for ((out, &value), &up) in out_first.iter_mut().zip(current_first).zip(previous_first) {
          *out = value.wrapping_sub(up >> 1);
        }
        for (((out, &value), &left), &up) in out_rest
          .iter_mut()
          .zip(current_rest)
          .zip(left)
          .zip(previous_rest)
        {
          *out = value.wrapping_sub(((u16::from(left) + u16::from(up)) >> 1) as u8);
        }
      }
      Self::Paeth => {
        for ((out, &value), &up) in out_first.iter_mut().zip(current_first).zip(previous_first) {
          *out = value.wrapping_sub(up);
        }
        for ((((out, &value), &left), &up), &upper_left) in out_rest
          .iter_mut()
          .zip(current_rest)
          .zip(left)
          .zip(previous_rest)
          .zip(upper_left)
        {
          *out = value.wrapping_sub(paeth(left, up, upper_left));
        }
      }
    }
  }
}

/// The Paeth predictor: whichever of `left`, `up` and `upper_left` lies nearest their gradient.
fn paeth(left: u8, up: u8, upper_left: u8) -> u8 {
  let (left16, up16, upper_left16) = (i16::from(left), i16::from(up), i16::from(upper_left));
  let to_left = (up16 - upper_left16).abs();
  let to_up = (left16 - upper_left16).abs();
  let to_upper_left = (left16 + up16 - 2 * upper_left16).abs();

  if to_left <= to_up && to_left <= to_upper_left {
    left
  } else if to_up <= to_upper_left {
    up
  } else {
    upper_left
  }
}

/// The sum of a filtered row's bytes read as signed, which the adaptive choice keeps least.
fn magnitude(row: &[u8]) -> u64 {
  row
    .chunks(32)
    .map(|chunk| {
      chunk
        .iter()
        .map(|&byte| u32::from((byte as i8).unsigned_abs()))
        .sum::<u32>()
    })
    .map(u64::from)
    .sum()
}

#[cfg(test)]
mod tests {
  use std::io::Read;

  use flate2::read::ZlibDecoder;
  use image::RgbaImage;
  use png::{ColorType, Filter};

  use super::{Pixels, Scanlines};

  fn photo(width: u32, height: u32, alpha: bool) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
      let noise = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) as u8;

      image::Rgba([
        (x as u8).wrapping_add(noise / 8),
        (y as u8).wrapping_add(noise / 4),
        noise,
        if alpha { 255 - noise / 2 } else { 255 },
      ])
    })
  }

  /// What the png crate's adaptive filter writes for `rgba`, read back out of its IDAT.
  fn png_crate_scanlines(rgba: &RgbaImage, keep_alpha: bool) -> Vec<u8> {
    let data: Vec<u8> = if keep_alpha {
      rgba.as_raw().clone()
    } else {
      rgba
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
    };
    let mut encoded = Vec::new();
    let mut encoder = png::Encoder::new(&mut encoded, rgba.width(), rgba.height());

    encoder.set_color(if keep_alpha {
      ColorType::Rgba
    } else {
      ColorType::Rgb
    });
    encoder.set_filter(Filter::Adaptive);
    encoder
      .write_header()
      .unwrap()
      .write_image_data(&data)
      .unwrap();

    let mut stream = Vec::new();
    let mut at = 8;

    while at < encoded.len() {
      let len = u32::from_be_bytes(encoded[at..at + 4].try_into().unwrap()) as usize;

      if &encoded[at + 4..at + 8] == b"IDAT" {
        stream.extend_from_slice(&encoded[at + 8..at + 8 + len]);
      }
      at += 12 + len;
    }

    let mut scanlines = Vec::new();

    ZlibDecoder::new(&stream[..])
      .read_to_end(&mut scanlines)
      .unwrap();
    scanlines
  }

  #[test]
  fn adaptive_rows_match_the_png_crate() {
    for keep_alpha in [false, true] {
      let rgba = photo(97, 23, keep_alpha);
      let mut ours = Vec::new();

      Scanlines::new(
        Pixels::Rgba {
          image: &rgba,
          keep_alpha,
        },
        true,
      )
      .append(0..23, &mut ours);
      assert!(ours == png_crate_scanlines(&rgba, keep_alpha));
    }
  }

  #[test]
  fn a_range_from_mid_image_filters_as_the_whole_image_does() {
    let rgba = photo(40, 12, false);
    let scanlines = Scanlines::new(
      Pixels::Rgba {
        image: &rgba,
        keep_alpha: false,
      },
      true,
    );
    let mut whole = Vec::new();
    let mut tail = Vec::new();

    scanlines.append(0..12, &mut whole);
    scanlines.append(5..12, &mut tail);
    assert!(tail == whole[5 * scanlines.len()..]);
  }
}

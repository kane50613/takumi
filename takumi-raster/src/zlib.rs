//! zlib streams compressed a segment at a time, so the segments compress at once, as
//! [pigz](https://zlib.net/pigz/pigz.pdf) does.

use flate2::{Compress, Compression, FlushCompress, Status};
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use simd_adler32::Adler32;

use crate::{Result, error::Error};

/// Bytes of history a segment is primed with: the whole DEFLATE window.
const WINDOW: usize = 32 * 1024;

/// Compresses `data` at `level` into one zlib stream of up to `segments` raw DEFLATE segments,
/// each cut on a multiple of `row` bytes and primed with the window before it.
pub(crate) fn compress_segmented(
  data: &[u8],
  row: usize,
  level: u32,
  segments: usize,
) -> Result<Vec<u8>> {
  let rows = (data.len() / row).div_ceil(segments.max(1)).max(1);
  let starts: Vec<usize> = (0..data.len()).step_by(rows * row).collect();
  let last = starts.len().saturating_sub(1);
  let segment = |(index, &start): (usize, &usize)| {
    let end = (start + rows * row).min(data.len());

    deflate_segment(
      &data[start.saturating_sub(WINDOW)..start],
      &data[start..end],
      level,
      index == last,
    )
  };

  #[cfg(feature = "rayon")]
  let bodies: Vec<Vec<u8>> = starts
    .par_iter()
    .enumerate()
    .map(segment)
    .collect::<Result<_>>()?;
  #[cfg(not(feature = "rayon"))]
  let bodies: Vec<Vec<u8>> = starts
    .iter()
    .enumerate()
    .map(segment)
    .collect::<Result<_>>()?;

  let mut adler = Adler32::new();

  adler.write(data);

  let mut stream = Vec::with_capacity(bodies.iter().map(Vec::len).sum::<usize>() + 6);

  stream.extend_from_slice(&header(level));
  for body in &bodies {
    stream.extend_from_slice(body);
  }
  stream.extend_from_slice(&adler.finish().to_be_bytes());
  Ok(stream)
}

/// Raw DEFLATE of `segment`, primed with `dictionary`. Every segment but the `last` ends on a sync
/// flush, which byte-aligns it and leaves the stream open for the next.
fn deflate_segment(dictionary: &[u8], segment: &[u8], level: u32, last: bool) -> Result<Vec<u8>> {
  let mut compress = Compress::new_with_window_bits(Compression::new(level), false, 15);
  let flush = if last {
    FlushCompress::Finish
  } else {
    FlushCompress::Sync
  };
  let mut body = Vec::with_capacity(segment.len() / 8 + 64);

  if !dictionary.is_empty() {
    compress.set_dictionary(dictionary).map_err(Error::encode)?;
  }
  loop {
    let consumed = compress.total_in() as usize;
    let status = compress
      .compress_vec(&segment[consumed..], &mut body, flush)
      .map_err(Error::encode)?;
    let drained = compress.total_in() as usize == segment.len() && body.len() < body.capacity();

    if status == Status::StreamEnd || (!last && drained) {
      return Ok(body);
    }
    body.reserve(body.capacity());
  }
}

/// The zlib header: DEFLATE with a 32KB window, flagged with the speed `level` traded for size.
fn header(level: u32) -> [u8; 2] {
  let flags = match level {
    0 | 1 => 0x01,
    2..=5 => 0x5E,
    6 => 0x9C,
    _ => 0xDA,
  };

  [0x78, flags]
}

#[cfg(test)]
mod tests {
  use flate2::read::ZlibDecoder;
  use std::io::Read;

  use super::compress_segmented;

  fn inflate(stream: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();

    ZlibDecoder::new(stream).read_to_end(&mut out).unwrap();
    out
  }

  #[test]
  fn segments_inflate_back_to_the_input() {
    let row = 301;
    let data: Vec<u8> = (0..row * 400)
      .map(|index| ((index / row) * 7 + (index % row) % 13) as u8)
      .collect();

    for segments in [1, 2, 3, 8] {
      assert_eq!(
        inflate(&compress_segmented(&data, row, 7, segments).unwrap()),
        data
      );
    }
  }

  #[test]
  fn more_segments_than_rows_still_inflate() {
    let data = vec![9u8; 30];

    assert_eq!(inflate(&compress_segmented(&data, 10, 7, 8).unwrap()), data);
  }
}

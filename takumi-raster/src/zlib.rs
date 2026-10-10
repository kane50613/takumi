//! zlib streams compressed a segment at a time, so the segments compress at once, as
//! [pigz](https://zlib.net/pigz/pigz.pdf) does.

use std::ops::Range;

use flate2::{Compress, Compression, FlushCompress, Status};
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use zlib_rs::adler32::{adler32, adler32_combine};

use crate::{Result, error::Error};

/// Bytes of history a segment is primed with: the whole DEFLATE window.
const WINDOW: usize = 32 * 1024;

/// Bytes of rows a segment writes out and feeds the compressor at a time.
const BATCH: usize = 64 * 1024;

/// A deflated segment: its raw DEFLATE body, and the Adler-32 and length of what it holds.
struct Segment {
  body: Vec<u8>,
  adler: u32,
  len: u64,
}

/// Compresses `rows` rows of `row_len` bytes at `level` into one zlib stream of up to `segments`
/// raw DEFLATE segments, each primed with the window of rows before it. `scanlines` appends the
/// bytes of a range of rows, so no segment holds more than a batch of them at once.
pub(crate) fn compress_segmented(
  rows: usize,
  row_len: usize,
  level: u32,
  segments: usize,
  scanlines: impl Fn(Range<usize>, &mut Vec<u8>) + Sync,
) -> Result<Vec<u8>> {
  let per_segment = rows.div_ceil(segments.max(1)).max(1);
  let starts: Vec<usize> = (0..rows).step_by(per_segment).collect();
  let last = starts.len().saturating_sub(1);
  let segment = |(index, &start): (usize, &usize)| {
    let primed = start.saturating_sub(WINDOW.div_ceil(row_len))..start;

    deflate_segment(
      &scanlines,
      primed,
      start..(start + per_segment).min(rows),
      row_len,
      level,
      index == last,
    )
  };

  #[cfg(feature = "rayon")]
  let segments: Vec<Segment> = starts
    .par_iter()
    .enumerate()
    .map(segment)
    .collect::<Result<_>>()?;
  #[cfg(not(feature = "rayon"))]
  let segments: Vec<Segment> = starts
    .iter()
    .enumerate()
    .map(segment)
    .collect::<Result<_>>()?;

  let adler = segments
    .iter()
    .map(|segment| (segment.adler, segment.len))
    .reduce(|(adler, _), (next, len)| (adler32_combine(adler, next, len), 0))
    .map_or(1, |(adler, _)| adler);
  let mut stream = Vec::with_capacity(
    segments
      .iter()
      .map(|segment| segment.body.len())
      .sum::<usize>()
      + 6,
  );

  stream.extend_from_slice(&header(level));
  for segment in &segments {
    stream.extend_from_slice(&segment.body);
  }
  stream.extend_from_slice(&adler.to_be_bytes());
  Ok(stream)
}

/// Raw DEFLATE of the `rows` `scanlines` writes, primed with the `primed` rows before them. Every
/// segment but the `last` ends on a sync flush, which byte-aligns it and leaves the stream open for
/// the next.
fn deflate_segment(
  scanlines: &impl Fn(Range<usize>, &mut Vec<u8>),
  primed: Range<usize>,
  rows: Range<usize>,
  row_len: usize,
  level: u32,
  last: bool,
) -> Result<Segment> {
  let mut compress = Compress::new_with_window_bits(Compression::new(level), false, 15);
  let mut batch = Vec::new();
  let mut body = Vec::with_capacity(rows.len() * row_len / 8 + 64);
  let mut adler = 1;

  if !primed.is_empty() {
    scanlines(primed, &mut batch);
    compress
      .set_dictionary(&batch[batch.len().saturating_sub(WINDOW)..])
      .map_err(Error::encode)?;
  }
  for start in rows.clone().step_by((BATCH / row_len).max(1)) {
    batch.clear();
    scanlines(
      start..(start + (BATCH / row_len).max(1)).min(rows.end),
      &mut batch,
    );
    adler = adler32(adler, &batch);
    feed(&mut compress, &batch, FlushCompress::None, &mut body)?;
  }

  let flush = if last {
    FlushCompress::Finish
  } else {
    FlushCompress::Sync
  };

  feed(&mut compress, &[], flush, &mut body)?;
  Ok(Segment {
    body,
    adler,
    len: (rows.len() * row_len) as u64,
  })
}

/// Feeds `input` to `compress` under `flush`, growing `body` until the compressor takes all of it
/// and, for a finishing flush, ends the stream.
fn feed(
  compress: &mut Compress,
  input: &[u8],
  flush: FlushCompress,
  body: &mut Vec<u8>,
) -> Result<()> {
  let start = compress.total_in();

  loop {
    if body.len() == body.capacity() {
      body.reserve(body.capacity().max(4096));
    }

    let consumed = (compress.total_in() - start) as usize;
    let status = compress
      .compress_vec(&input[consumed..], body, flush)
      .map_err(Error::encode)?;
    let drained =
      (compress.total_in() - start) as usize == input.len() && body.len() < body.capacity();

    if status == Status::StreamEnd || (drained && !matches!(flush, FlushCompress::Finish)) {
      return Ok(());
    }
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
  use std::io::Read;

  use flate2::read::ZlibDecoder;

  use super::compress_segmented;

  fn inflate(stream: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();

    ZlibDecoder::new(stream).read_to_end(&mut out).unwrap();
    out
  }

  fn compress(data: &[u8], row_len: usize, segments: usize) -> Vec<u8> {
    compress_segmented(data.len() / row_len, row_len, 7, segments, |rows, out| {
      out.extend_from_slice(&data[rows.start * row_len..rows.end * row_len]);
    })
    .unwrap()
  }

  #[test]
  fn segments_inflate_back_to_the_input() {
    let row_len = 301;
    let data: Vec<u8> = (0..row_len * 400)
      .map(|index| ((index / row_len) * 7 + (index % row_len) % 13) as u8)
      .collect();

    for segments in [1, 2, 3, 8] {
      assert_eq!(inflate(&compress(&data, row_len, segments)), data);
    }
  }

  #[test]
  fn more_segments_than_rows_still_inflate() {
    let data = vec![9u8; 30];

    assert_eq!(inflate(&compress(&data, 10, 8)), data);
  }
}

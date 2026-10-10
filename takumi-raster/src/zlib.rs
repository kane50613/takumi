//! zlib streams compressed a segment at a time, so the segments compress at once, as
//! [pigz](https://zlib.net/pigz/pigz.pdf) does.

use std::{ffi::c_int, io, mem::size_of, ops::Range};

use libz_rs_sys::{
  Z_BUF_ERROR, Z_DEFAULT_STRATEGY, Z_DEFLATED, Z_FINISH, Z_NO_FLUSH, Z_OK, Z_STREAM_END,
  Z_SYNC_FLUSH, deflate, deflateEnd, deflateInit2_, deflateSetDictionary, deflateTune, z_stream,
  zlibVersion,
};
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use zlib_rs::adler32::{adler32, adler32_combine};

use crate::{Result, error::Error};

/// Bytes of history a segment is primed with: the whole DEFLATE window.
const WINDOW: usize = 32 * 1024;

/// Bytes of rows a segment writes out and feeds the compressor at a time.
const BATCH: usize = 16 * 1024;

/// zlib's level 7 search (`good`, `lazy`, `nice`, `chain`) with half the hash chain and a nice
/// match of the longest length. On 65 real templates it deflates 30% faster for 0.7% more bytes.
const LEVEL_7_SEARCH: [c_int; 4] = [4, 8, 258, 128];

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
  let mut deflater = RawDeflate::new(level)?;
  let mut batch = Vec::new();
  let mut body = Vec::with_capacity(4096);
  let mut adler = 1;

  if !primed.is_empty() {
    scanlines(primed, &mut batch);
    deflater.set_dictionary(&batch[batch.len().saturating_sub(WINDOW)..])?;
  }
  for start in rows.clone().step_by((BATCH / row_len).max(1)) {
    batch.clear();
    scanlines(
      start..(start + (BATCH / row_len).max(1)).min(rows.end),
      &mut batch,
    );
    adler = adler32(adler, &batch);
    deflater.feed(&batch, Z_NO_FLUSH, &mut body)?;
  }

  deflater.feed(&[], if last { Z_FINISH } else { Z_SYNC_FLUSH }, &mut body)?;
  Ok(Segment {
    body,
    adler,
    len: (rows.len() * row_len) as u64,
  })
}

/// A raw DEFLATE stream on zlib-rs's C API, the one that exposes `deflateTune`.
struct RawDeflate(Box<z_stream>);

impl RawDeflate {
  fn new(level: u32) -> Result<Self> {
    let mut stream = Box::<z_stream>::default();
    // SAFETY: `stream` is a default `z_stream`, which leaves the allocator to zlib-rs, and the
    // version and size are the library's own.
    let code = unsafe {
      deflateInit2_(
        &mut *stream,
        level as c_int,
        Z_DEFLATED,
        -15,
        8,
        Z_DEFAULT_STRATEGY,
        zlibVersion(),
        size_of::<z_stream>() as c_int,
      )
    };

    check(code)?;

    let mut deflater = Self(stream);

    if level == 7 {
      let [good, lazy, nice, chain] = LEVEL_7_SEARCH;

      // SAFETY: the stream was initialized above.
      check(unsafe { deflateTune(&mut *deflater.0, good, lazy, nice, chain) })?;
    }
    Ok(deflater)
  }

  fn set_dictionary(&mut self, dictionary: &[u8]) -> Result<()> {
    // SAFETY: the stream is initialized and `dictionary` is valid for its length.
    check(unsafe { deflateSetDictionary(&mut *self.0, dictionary.as_ptr(), dictionary.len() as _) })
  }

  /// Feeds `input` under `flush`, growing `body` until the stream takes all of it and, for
  /// `Z_FINISH`, ends.
  fn feed(&mut self, input: &[u8], flush: c_int, body: &mut Vec<u8>) -> Result<()> {
    self.0.next_in = input.as_ptr();
    self.0.avail_in = input.len() as _;

    loop {
      if body.len() == body.capacity() {
        body.reserve(body.capacity().max(4096));
      }

      let spare = body.spare_capacity_mut();

      self.0.next_out = spare.as_mut_ptr().cast();
      self.0.avail_out = spare.len() as _;

      // SAFETY: `next_in` covers the unread rest of `input` and `next_out` the spare capacity of
      // `body`, both alive for the call.
      let code = unsafe { deflate(&mut *self.0, flush) };
      let written = spare.len() - self.0.avail_out as usize;

      // SAFETY: zlib-rs initialized the `written` bytes after `body`'s length.
      unsafe { body.set_len(body.len() + written) };

      match code {
        Z_STREAM_END => return Ok(()),
        Z_OK | Z_BUF_ERROR if self.0.avail_in == 0 && self.0.avail_out > 0 && flush != Z_FINISH => {
          return Ok(());
        }
        Z_OK | Z_BUF_ERROR => {}
        code => return check(code),
      }
    }
  }
}

impl Drop for RawDeflate {
  fn drop(&mut self) {
    // SAFETY: the stream was initialized in `new` and is ended once.
    unsafe { deflateEnd(&mut *self.0) };
  }
}

fn check(code: c_int) -> Result<()> {
  if code == Z_OK {
    Ok(())
  } else {
    Err(Error::encode(io::Error::other(format!(
      "zlib error {code}"
    ))))
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

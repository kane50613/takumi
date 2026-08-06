//! Packs the structure tree's dictionaries into an object stream.
//!
//! A tagged document writes one small dictionary per structure element, and on
//! a text-heavy page they outweigh everything else: a third of an invoice, one
//! uncompressed indirect object each, all nearly identical. An object stream
//! holds them in a single compressed stream instead.
//!
//! Their cross-reference rows have to say so, and `pdf-writer` writes only free
//! and occupied rows. [`ObjectStream::patch_xref`] rewrites the rows afterwards,
//! which the fixed row layout of a cross-reference stream makes exact.

use pdf_writer::{Chunk, Filter, Finish, Name, Pdf, Ref};

use crate::krilla::stream::deflate_encode;

/// The objects an object stream holds, and where each one sits in it.
pub(crate) struct ObjectStream {
  stream: i32,
  entries: Vec<(i32, u16)>,
}

/// Moves a chunk's objects into an object stream written to `pdf`.
///
/// Returns `None` when the chunk holds nothing to pack, or when its bytes do
/// not read back as the plain sequence of dictionaries this expects, in which
/// case the caller writes the chunk as it stands.
pub(crate) fn pack(chunk: &Chunk, stream_ref: Ref, pdf: &mut Pdf) -> Option<ObjectStream> {
  let objects = split(chunk)?;

  if objects.len() < 2 {
    return None;
  }

  let mut header = Vec::new();
  let mut bodies = Vec::new();
  let mut entries = Vec::new();

  for (index, (id, body)) in objects.iter().enumerate() {
    let index = u16::try_from(index).ok()?;

    header.extend_from_slice(format!("{} {} ", id.get(), bodies.len()).as_bytes());
    bodies.extend_from_slice(body);
    bodies.push(b'\n');
    entries.push((id.get(), index));
  }

  let first = header.len();

  header.extend_from_slice(&bodies);

  let data = deflate_encode(&header);
  let mut stream = pdf.stream(stream_ref, &data);

  stream.pair(Name(b"Type"), Name(b"ObjStm"));
  stream.pair(Name(b"N"), entries.len() as i32);
  stream.pair(Name(b"First"), first as i32);
  stream.filter(Filter::FlateDecode);
  stream.finish();

  Some(ObjectStream {
    stream: stream_ref.get(),
    entries,
  })
}

impl ObjectStream {
  /// Rewrites the cross-reference rows of the packed objects.
  ///
  /// A cross-reference stream is a table of fixed-width rows indexed by object
  /// number, so a row is found by arithmetic. Each packed object gets a type-2
  /// row naming the stream and its index. The head of the free list, which
  /// pointed at the object numbers this took over, is emptied.
  pub(crate) fn patch_xref(&self, rows: &mut [u8], xref_len: usize) {
    if xref_len == 0 || rows.len() % xref_len != 0 {
      return;
    }

    let row = rows.len() / xref_len;
    // A row is a type byte, then the offset field, then a two-byte field.
    let Some(offset_width) = row.checked_sub(3) else {
      return;
    };

    let mut write = |number: i32, second: u64, third: u16| {
      let Ok(number) = usize::try_from(number) else {
        return;
      };
      let Some(cells) = rows.get_mut(number * row..(number + 1) * row) else {
        return;
      };

      cells[0] = 2;
      cells[1..1 + offset_width].copy_from_slice(&second.to_be_bytes()[8 - offset_width..]);
      cells[1 + offset_width..].copy_from_slice(&third.to_be_bytes());
    };

    for (number, index) in &self.entries {
      write(*number, self.stream as u64, *index);
    }

    // Object 0 heads the free list, which now runs through numbers that are no
    // longer free. Nothing else is free, so the list ends at itself.
    if let Some(cells) = rows.get_mut(..row) {
      cells[0] = 0;
      cells[1..1 + offset_width].fill(0);
    }
  }
}

/// Reads a chunk back as the objects it was written from.
///
/// The objects sit one after another as `<id> 0 obj … endobj`. A text string
/// inside a dictionary can spell `endobj` too, so a candidate end is only
/// accepted when the next object's header follows it.
fn split(chunk: &Chunk) -> Option<Vec<(Ref, &[u8])>> {
  let bytes = chunk.as_bytes();
  let ids = chunk.refs().collect::<Vec<_>>();
  let mut objects = Vec::with_capacity(ids.len());
  let mut pos = 0;

  for (i, id) in ids.iter().enumerate() {
    let header = format!("{} 0 obj", id.get());

    if !bytes[pos..].starts_with(header.as_bytes()) {
      return None;
    }

    let next_header = ids
      .get(i + 1)
      .map(|next| format!("{} 0 obj", next.get()).into_bytes());
    let body_start = pos + header.len();
    let mut search = body_start;

    let (end, next) = loop {
      let found = search + find(&bytes[search..], b"endobj")?;
      let after = skip_whitespace(bytes, found + b"endobj".len());

      let matches = match &next_header {
        Some(next_header) => bytes[after..].starts_with(next_header),
        None => after == bytes.len(),
      };

      if matches {
        break (found, after);
      }

      search = found + 1;
    };

    objects.push((*id, bytes[body_start..end].trim_ascii()));
    pos = next;
  }

  Some(objects)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
  haystack
    .windows(needle.len())
    .position(|window| window == needle)
}

fn skip_whitespace(bytes: &[u8], from: usize) -> usize {
  let mut at = from;

  while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
    at += 1;
  }

  at
}

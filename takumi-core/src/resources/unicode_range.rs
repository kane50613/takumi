use std::sync::Arc;

use parley::fontique::Blob;
use skrifa::{FontRef, MetadataProvider, raw::types::Tag};
use xxhash_rust::xxh3::xxh3_64;

/// An inclusive codepoint range from an `@font-face` `unicode-range`, as Blink's `UnicodeRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnicodeRange {
  from: u32,
  to: u32,
}

impl UnicodeRange {
  /// The codepoints `from..=to`.
  pub const fn new(from: u32, to: u32) -> Self {
    Self { from, to }
  }

  fn contains(self, codepoint: u32) -> bool {
    (self.from..=self.to).contains(&codepoint)
  }
}

/// The codepoints a face may serve, as Blink's `UnicodeRangeSet`. An empty set covers every
/// codepoint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnicodeRangeSet(Vec<UnicodeRange>);

impl FromIterator<UnicodeRange> for UnicodeRangeSet {
  fn from_iter<T: IntoIterator<Item = UnicodeRange>>(ranges: T) -> Self {
    let mut sorted: Vec<UnicodeRange> = ranges
      .into_iter()
      .filter(|range| range.from <= range.to)
      .collect();

    sorted.sort_unstable();

    let mut merged: Vec<UnicodeRange> = Vec::with_capacity(sorted.len());

    for range in sorted {
      match merged.last_mut() {
        Some(last) if range.from <= last.to.saturating_add(1) => {
          last.to = last.to.max(range.to);
        }
        _ => merged.push(range),
      }
    }

    Self(merged)
  }
}

impl UnicodeRangeSet {
  /// Whether `codepoint` falls in the set.
  pub fn contains(&self, codepoint: u32) -> bool {
    if self.0.is_empty() {
      return true;
    }

    let index = self.0.partition_point(|range| range.to < codepoint);

    self
      .0
      .get(index)
      .is_some_and(|range| range.contains(codepoint))
  }

  /// Whether the set covers every codepoint, so the face needs no filtering.
  pub fn is_entire_range(&self) -> bool {
    self.0.is_empty() || (self.0.len() == 1 && self.0[0].from == 0 && self.0[0].to >= 0x10FFFF)
  }

  /// `blob` with its `cmap` restricted to the set; see [`Self::restrict_charmap`].
  pub(crate) fn restrict_blob(&self, blob: Blob<u8>) -> Blob<u8> {
    match self.restrict_charmap(blob.data()) {
      Some(restricted) => {
        let id = xxh3_64(&restricted);

        Blob::from_raw_parts(Arc::new(restricted), id)
      }
      None => blob,
    }
  }

  /// Rewrites a single-face sfnt so its `cmap` maps only codepoints in the set. Blink filters
  /// glyph lookup the same way in `HarfBuzzFace`, so a subset never serves a codepoint outside
  /// its declared range. Returns `None` for a font collection or a font it cannot read, which
  /// keep their full `cmap`. A copied format 14 subtable keeps its non-default variation glyphs.
  fn restrict_charmap(&self, sfnt: &[u8]) -> Option<Vec<u8>> {
    if self.is_entire_range() {
      return None;
    }

    let font = FontRef::new(sfnt).ok()?;
    let mappings = font
      .charmap()
      .mappings()
      .filter(|(codepoint, _)| self.contains(*codepoint))
      .map(|(codepoint, glyph)| (codepoint, glyph.to_u32()));
    let cmap = build_cmap(
      mappings,
      variation_subtable(font.table_data(CMAP)?.as_bytes()),
    );

    replace_table(sfnt, CMAP, &cmap)
  }
}

const CMAP: Tag = Tag::new(b"cmap");
const TABLE_DIRECTORY_LEN: usize = 12;
const TABLE_RECORD_LEN: usize = 16;

/// A `cmap` with a format 12 subtable over `mappings`, ascending by codepoint, plus the font's
/// format 14 subtable when it has one.
fn build_cmap(mappings: impl Iterator<Item = (u32, u32)>, variations: Option<&[u8]>) -> Vec<u8> {
  let mut groups: Vec<(u32, u32, u32)> = Vec::new();

  for (codepoint, glyph) in mappings {
    match groups.last_mut() {
      Some((start, end, start_glyph))
        if codepoint == *end + 1 && glyph == *start_glyph + (codepoint - *start) =>
      {
        *end = codepoint;
      }
      _ => groups.push((codepoint, codepoint, glyph)),
    }
  }

  let records = if variations.is_some() { 2 } else { 1 };
  let header_len = 4 + 8 * records;
  let format12_len = 16 + 12 * groups.len();
  let mut cmap = Vec::with_capacity(header_len + format12_len + variations.map_or(0, <[u8]>::len));

  push_u16(&mut cmap, 0);
  push_u16(&mut cmap, records as u16);

  if variations.is_some() {
    push_u16(&mut cmap, 0);
    push_u16(&mut cmap, 5);
    push_u32(&mut cmap, (header_len + format12_len) as u32);
  }

  push_u16(&mut cmap, 3);
  push_u16(&mut cmap, 10);
  push_u32(&mut cmap, header_len as u32);

  push_u16(&mut cmap, 12);
  push_u16(&mut cmap, 0);
  push_u32(&mut cmap, format12_len as u32);
  push_u32(&mut cmap, 0);
  push_u32(&mut cmap, groups.len() as u32);

  for (start, end, start_glyph) in groups {
    push_u32(&mut cmap, start);
    push_u32(&mut cmap, end);
    push_u32(&mut cmap, start_glyph);
  }

  if let Some(variations) = variations {
    cmap.extend_from_slice(variations);
  }

  cmap
}

/// The Unicode variation sequences subtable (platform 0, encoding 5, format 14) of `cmap`.
fn variation_subtable(cmap: &[u8]) -> Option<&[u8]> {
  let records = read_u16(cmap, 2)?;

  (0..usize::from(records)).find_map(|index| {
    let record = 4 + 8 * index;

    if read_u16(cmap, record)? != 0 || read_u16(cmap, record + 2)? != 5 {
      return None;
    }

    let offset = read_u32(cmap, record + 4)? as usize;

    if read_u16(cmap, offset)? != 14 {
      return None;
    }

    cmap.get(offset..offset + read_u32(cmap, offset + 2)? as usize)
  })
}

/// `sfnt` with the table `tag` swapped for `table`, every table laid out again on 4-byte bounds.
fn replace_table(sfnt: &[u8], tag: Tag, table: &[u8]) -> Option<Vec<u8>> {
  let tables = usize::from(read_u16(sfnt, 4)?);
  let directory_len = TABLE_DIRECTORY_LEN + TABLE_RECORD_LEN * tables;
  let mut out = sfnt.get(..directory_len)?.to_vec();
  let mut replaced = false;

  for index in 0..tables {
    let record = TABLE_DIRECTORY_LEN + TABLE_RECORD_LEN * index;
    let data = if sfnt.get(record..record + 4)? == tag.to_be_bytes() {
      replaced = true;
      out[record + 4..record + 8].copy_from_slice(&checksum(table).to_be_bytes());
      table
    } else {
      let offset = read_u32(sfnt, record + 8)? as usize;

      sfnt.get(offset..offset + read_u32(sfnt, record + 12)? as usize)?
    };

    let offset = out.len() as u32;

    out[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
    out[record + 12..record + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(data);
    out.resize(out.len().next_multiple_of(4), 0);
  }

  replaced.then_some(out)
}

/// The OpenType table checksum: the sum of its big-endian `u32` words, zero-padded.
fn checksum(table: &[u8]) -> u32 {
  table.chunks(4).fold(0_u32, |sum, chunk| {
    let mut word = [0; 4];

    word[..chunk.len()].copy_from_slice(chunk);
    sum.wrapping_add(u32::from_be_bytes(word))
  })
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
  out.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
  out.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
  use std::{fs, path::Path};

  use skrifa::{FontRef, MetadataProvider, raw::TableProvider};

  use super::{UnicodeRange, UnicodeRangeSet};

  fn archivo() -> Vec<u8> {
    fs::read(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf"),
    )
    .unwrap()
  }

  #[test]
  fn ranges_merge_when_they_overlap_or_touch() {
    let set = UnicodeRangeSet::from_iter([
      UnicodeRange::new(0x30, 0x39),
      UnicodeRange::new(0x41, 0x5A),
      UnicodeRange::new(0x3A, 0x40),
      UnicodeRange::new(0x7A, 0x61),
    ]);

    assert_eq!(
      set,
      UnicodeRangeSet::from_iter([UnicodeRange::new(0x30, 0x5A)])
    );
    assert!(set.contains(0x30) && set.contains(0x5A));
    assert!(!set.contains(0x2F) && !set.contains(0x61));
  }

  #[test]
  fn an_empty_set_covers_everything_and_leaves_the_font_alone() {
    let set = UnicodeRangeSet::default();

    assert!(set.contains(0x10FFFF));
    assert!(set.restrict_charmap(&archivo()).is_none());
    assert!(
      UnicodeRangeSet::from_iter([UnicodeRange::new(0, 0x10FFFF)])
        .restrict_charmap(&archivo())
        .is_none()
    );
  }

  #[test]
  fn the_charmap_keeps_only_codepoints_in_range() {
    let original = archivo();
    let restricted = UnicodeRangeSet::from_iter([UnicodeRange::new(0x41, 0x5A)])
      .restrict_charmap(&original)
      .unwrap();
    let before = FontRef::new(&original).unwrap();
    let after = FontRef::new(&restricted).unwrap();

    assert_eq!(after.charmap().map('A'), before.charmap().map('A'));
    assert_eq!(after.charmap().map('Z'), before.charmap().map('Z'));
    assert_eq!(after.charmap().map('a'), None);
    assert_eq!(after.charmap().map(' '), None);
    assert_eq!(
      after.glyf().unwrap().offset_data().as_bytes(),
      before.glyf().unwrap().offset_data().as_bytes()
    );
  }
}

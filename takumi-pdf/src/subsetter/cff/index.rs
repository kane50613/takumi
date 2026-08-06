use crate::subsetter::Error::OverflowError;
use crate::subsetter::Result;
use crate::subsetter::cff::number::U24;
use crate::subsetter::read::{Readable, Reader};
use crate::subsetter::write::{Writeable, Writer};

// Taken from ttf-parser.

pub trait IndexSize: for<'a> Readable<'a> {
  fn to_u32(self) -> u32;
}

impl IndexSize for u16 {
  fn to_u32(self) -> u32 {
    u32::from(self)
  }
}

impl IndexSize for u32 {
  fn to_u32(self) -> u32 {
    self
  }
}

pub fn parse_index<'a, T: IndexSize>(r: &mut Reader<'a>) -> Option<Index<'a>> {
  let count = r.read::<T>()?;
  parse_index_impl(count.to_u32(), r)
}

fn parse_index_impl<'a>(count: u32, r: &mut Reader<'a>) -> Option<Index<'a>> {
  if count == 0 || count == u32::MAX {
    return Some(Index::default());
  }

  let offset_size = r.read::<OffsetSize>()?;
  let offsets_len = (count + 1).checked_mul(offset_size.to_u32())?;
  let offsets = VarOffsets {
    data: r.read_bytes(offsets_len as usize)?,
    offset_size,
  };

  match offsets.last() {
    Some(last_offset) => {
      let data = r.read_bytes(last_offset as usize)?;
      Some(Index { data, offsets })
    }
    None => Some(Index::default()),
  }
}

pub fn skip_index<T: IndexSize>(r: &mut Reader) -> Option<()> {
  let count = r.read::<T>()?;
  skip_index_impl(count.to_u32(), r)
}

fn skip_index_impl(count: u32, r: &mut Reader) -> Option<()> {
  if count == 0 || count == u32::MAX {
    return Some(());
  }

  let offset_size = r.read::<OffsetSize>()?;
  let offsets_len = (count + 1).checked_mul(offset_size.to_u32())?;
  let offsets = VarOffsets {
    data: r.read_bytes(offsets_len as usize)?,
    offset_size,
  };

  if let Some(last_offset) = offsets.last() {
    r.skip_bytes(last_offset as usize);
  }

  Some(())
}

#[derive(Clone, Copy, Debug)]
pub struct VarOffsets<'a> {
  pub data: &'a [u8],
  pub offset_size: OffsetSize,
}

impl VarOffsets<'_> {
  pub fn get(&self, index: u32) -> Option<u32> {
    if index >= self.len() {
      return None;
    }

    let start = index as usize * self.offset_size.to_usize();
    let mut r = Reader::new_at(self.data, start);
    let n: u32 = match self.offset_size {
      OffsetSize::Size1 => u32::from(r.read::<u8>()?),
      OffsetSize::Size2 => u32::from(r.read::<u16>()?),
      OffsetSize::Size3 => r.read::<U24>()?.0,
      OffsetSize::Size4 => r.read::<u32>()?,
    };

    // Offsets are offset by one byte in the font,
    // so we have to shift them back.
    n.checked_sub(1)
  }

  #[inline]
  pub fn last(&self) -> Option<u32> {
    if !self.is_empty() {
      self.get(self.len() - 1)
    } else {
      None
    }
  }

  #[inline]
  pub fn len(&self) -> u32 {
    self.data.len() as u32 / self.offset_size as u32
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Index<'a> {
  pub data: &'a [u8],
  pub offsets: VarOffsets<'a>,
}

impl Default for Index<'_> {
  #[inline]
  fn default() -> Self {
    Index {
      data: b"",
      offsets: VarOffsets {
        data: b"",
        offset_size: OffsetSize::Size1,
      },
    }
  }
}

impl<'a> IntoIterator for Index<'a> {
  type Item = &'a [u8];
  type IntoIter = IndexIter<'a>;

  #[inline]
  fn into_iter(self) -> Self::IntoIter {
    IndexIter {
      data: self,
      offset_index: 0,
    }
  }
}

impl<'a> Index<'a> {
  #[inline]
  pub fn len(&self) -> u32 {
    self.offsets.len().saturating_sub(1)
  }

  pub fn get(&self, index: u32) -> Option<&'a [u8]> {
    let next_index = index.checked_add(1)?;
    let start = self.offsets.get(index)? as usize;
    let end = self.offsets.get(next_index)? as usize;
    self.data.get(start..end)
  }
}

pub struct IndexIter<'a> {
  data: Index<'a>,
  offset_index: u32,
}

impl<'a> Iterator for IndexIter<'a> {
  type Item = &'a [u8];

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.offset_index == self.data.len() {
      return None;
    }

    let index = self.offset_index;
    self.offset_index += 1;
    self.data.get(index)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OffsetSize {
  Size1 = 1,
  Size2 = 2,
  Size3 = 3,
  Size4 = 4,
}

impl OffsetSize {
  #[inline]
  pub fn to_u32(self) -> u32 {
    self as u32
  }
  #[inline]
  pub fn to_usize(self) -> usize {
    self as usize
  }
}

impl Readable<'_> for OffsetSize {
  const SIZE: usize = 1;

  fn read(r: &mut Reader<'_>) -> Option<Self> {
    match r.read::<u8>()? {
      1 => Some(OffsetSize::Size1),
      2 => Some(OffsetSize::Size2),
      3 => Some(OffsetSize::Size3),
      4 => Some(OffsetSize::Size4),
      _ => None,
    }
  }
}

/// An index that owns its data.
pub struct OwnedIndex {
  pub data: Vec<u8>,
  pub header_size: usize,
}

impl Writeable for OwnedIndex {
  fn write(&self, w: &mut Writer) {
    w.extend(&self.data);
  }
}

impl Default for OwnedIndex {
  fn default() -> Self {
    Self {
      data: vec![0, 0],
      header_size: 2,
    }
  }
}

/// Create an index from a vector of data.
pub fn create_index(data: Vec<Vec<u8>>) -> Result<OwnedIndex> {
  let count = u16::try_from(data.len()).map_err(|_| OverflowError)?;
  // + 1 Since we start counting from the preceding byte.
  let offsize = data.iter().map(|v| v.len() as u32).sum::<u32>() + 1;

  // Empty Index only contains the count field
  if count == 0 {
    return Ok(OwnedIndex::default());
  }

  let offset_size = if offsize <= u8::MAX as u32 {
    OffsetSize::Size1
  } else if offsize <= u16::MAX as u32 {
    OffsetSize::Size2
  } else if offsize <= U24::MAX {
    OffsetSize::Size3
  } else {
    OffsetSize::Size4
  };

  let mut w = Writer::new();
  w.write(count);
  w.write(offset_size as u8);

  let mut cur_offset: u32 = 0;

  let mut write_offset = |len| {
    cur_offset += len;

    match offset_size {
      OffsetSize::Size1 => {
        let num = u8::try_from(cur_offset).map_err(|_| OverflowError)?;
        w.write(num);
      }
      OffsetSize::Size2 => {
        let num = u16::try_from(cur_offset).map_err(|_| OverflowError)?;
        w.write(num);
      }
      OffsetSize::Size3 => {
        let num = U24(cur_offset);
        w.write(num);
      }
      OffsetSize::Size4 => w.write(cur_offset),
    }

    Ok(())
  };

  write_offset(1)?;
  for el in &data {
    write_offset(el.len() as u32)?;
  }

  let header_size = w.len();

  for el in &data {
    w.extend(el);
  }

  Ok(OwnedIndex {
    header_size,
    data: w.finish(),
  })
}

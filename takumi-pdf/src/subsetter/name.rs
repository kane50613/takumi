use super::*;
use crate::subsetter::Error::{MalformedFont, SubsetError};
use std::collections::HashMap;

pub fn subset(ctx: &mut Context) -> Result<()> {
  let name = ctx.expect_table(Tag::NAME).ok_or(MalformedFont)?;
  let mut r = Reader::new(name);

  let version = r.read::<u16>().ok_or(MalformedFont)?;

  // From my personal experiments, version 1 isn't used at all, so we
  // don't bother subsetting it.
  if version != 0 {
    ctx.push(Tag::NAME, name);
    return Ok(());
  }

  let table = Table::parse(name).ok_or(MalformedFont)?;
  let subsetted_table = subset_table(&table).ok_or(SubsetError)?;

  let mut w = Writer::new();
  w.write(subsetted_table);

  ctx.push(Tag::NAME, w.finish());
  Ok(())
}

pub fn subset_table<'a>(table: &Table<'a>) -> Option<Table<'a>> {
  let mut names = table
    .names
    .iter()
    .copied()
    .filter(|record| {
      record.is_unicode()
        && [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14].contains(&record.name_id)
    })
    .collect::<Vec<_>>();

  let mut storage = Vec::new();
  let mut cur_storage_offset = 0;

  let mut name_deduplicator: HashMap<&[u8], u16> = HashMap::new();

  for record in &mut names {
    let name = table
      .storage
      .get((record.string_offset as usize)..((record.string_offset + record.length) as usize))?;
    let offset = *name_deduplicator.entry(name).or_insert_with(|| {
      storage.extend(name);
      let offset = cur_storage_offset;
      cur_storage_offset += record.length;
      offset
    });

    record.string_offset = offset;
  }

  Some(Table {
    names,
    storage: Cow::Owned(storage),
  })
}

impl Writeable for Table<'_> {
  fn write(&self, w: &mut Writer) {
    let count = u16::try_from(self.names.len()).unwrap();

    // version
    w.write::<u16>(0);
    // count
    w.write::<u16>(count);
    // storage offset
    w.write::<u16>(u16::SIZE as u16 * 3 + count * NameRecord::SIZE as u16);
    for name in &self.names {
      w.write(name);
    }
    w.extend(&self.storage);
  }
}

impl Writeable for &NameRecord {
  fn write(&self, w: &mut Writer) {
    w.write::<u16>(self.platform_id);
    w.write::<u16>(self.encoding_id);
    w.write::<u16>(self.language_id);
    w.write::<u16>(self.name_id);
    w.write::<u16>(self.length);
    w.write::<u16>(self.string_offset);
  }
}

#[derive(Clone, Debug)]
pub struct Table<'a> {
  pub names: Vec<NameRecord>,
  pub storage: Cow<'a, [u8]>,
}

impl<'a> Table<'a> {
  // The parsing logic was adapted from ttf-parser.
  pub fn parse(data: &'a [u8]) -> Option<Self> {
    let mut r = Reader::new(data);

    let version = r.read::<u16>()?;

    if version != 0 {
      return None;
    }

    let count = r.read::<u16>()?;
    let storage_offset = r.read::<u16>()?;

    let mut names = Vec::with_capacity(count as usize);

    for _ in 0..count {
      names.push(r.read::<NameRecord>()?);
    }

    let storage = Cow::Borrowed(data.get(storage_offset as usize..)?);

    Some(Self { names, storage })
  }
}

impl Readable<'_> for NameRecord {
  const SIZE: usize = u16::SIZE * 6;

  fn read(r: &mut Reader<'_>) -> Option<Self> {
    let platform_id = r.read::<u16>()?;
    let encoding_id = r.read::<u16>()?;
    let language_id = r.read::<u16>()?;
    let name_id = r.read::<u16>()?;
    let length = r.read::<u16>()?;
    let string_offset = r.read::<u16>()?;

    Some(Self {
      platform_id,
      encoding_id,
      language_id,
      name_id,
      length,
      string_offset,
    })
  }
}

#[derive(Clone, Copy, Debug)]
pub struct NameRecord {
  pub platform_id: u16,
  pub encoding_id: u16,
  pub language_id: u16,
  pub name_id: u16,
  pub length: u16,
  pub string_offset: u16,
}

impl NameRecord {
  pub fn is_unicode(&self) -> bool {
    self.platform_id == 0 || (self.platform_id == 3 && [0, 1, 10].contains(&self.encoding_id))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_respects_storage_offset() {
    let mut data = Vec::new();
    data.extend(0u16.to_be_bytes()); // version
    data.extend(1u16.to_be_bytes()); // count
    data.extend(22u16.to_be_bytes()); // storageOffset
    data.extend(3u16.to_be_bytes()); // nameRecord[0].platformID
    data.extend(1u16.to_be_bytes()); // nameRecord[0].encodingID
    data.extend(0x0409u16.to_be_bytes()); // nameRecord[0].languageID
    data.extend(0u16.to_be_bytes()); // nameRecord[0].nameID
    data.extend(4u16.to_be_bytes()); // nameRecord[0].length
    data.extend(0u16.to_be_bytes()); // nameRecord[0].stringOffset
    data.extend(*b"BAD!"); // padding before storageOffset
    data.extend([0, b'O', 0, b'K']); // storage

    let table = Table::parse(&data).unwrap();
    let subset = subset_table(&table).unwrap();

    assert_eq!(subset.storage.as_ref(), &[0, b'O', 0, b'K']);
  }
}

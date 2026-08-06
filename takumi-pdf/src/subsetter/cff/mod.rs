mod argstack;
mod charset;
mod charstring;
mod cid_font;
mod dict;
mod index;
mod number;
mod operator;
mod remapper;
mod sid_font;
mod subroutines;

use super::*;
use crate::subsetter::Error::{OverflowError, SubsetError};
use crate::subsetter::cff::charset::rewrite_charset;
use crate::subsetter::cff::charstring::Decompiler;
use crate::subsetter::cff::cid_font::{CIDMetadata, rewrite_fd_index};
use crate::subsetter::cff::dict::font_dict::{generate_font_dict_index, rewrite_font_dict_index};
use crate::subsetter::cff::dict::private_dict::{rewrite_cid_private_dicts, rewrite_private_dict};
use crate::subsetter::cff::dict::top_dict::{
  TopDictData, parse_top_dict_index, rewrite_top_dict_index,
};
use crate::subsetter::cff::index::{Index, OwnedIndex, create_index, parse_index, skip_index};
use crate::subsetter::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::subsetter::cff::sid_font::SIDMetadata;
use crate::subsetter::cff::subroutines::{SubroutineCollection, SubroutineContainer};
use number::{IntegerNumber, StringId};
use sid_font::generate_fd_index;
use std::cmp::PartialEq;
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub(crate) enum FontKind<'a> {
  Sid(SIDMetadata<'a>),
  Cid(CIDMetadata<'a>),
}

#[derive(Clone)]
pub struct Table<'a> {
  names: &'a [u8],
  top_dict_data: TopDictData,
  strings: Index<'a>,
  global_subrs: Index<'a>,
  char_strings: Index<'a>,
  font_kind: FontKind<'a>,
}

/// An offset that needs to be written after the whole font
/// has been written. location indicates where in the buffer the offset needs to be written to
/// and value indicates the value of the offset.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct DeferredOffset {
  location: usize,
  value: IntegerNumber,
}

const DUMMY_VALUE: IntegerNumber = IntegerNumber(0);
/// This represents a not-existing offset.
const DUMMY_OFFSET: DeferredOffset = DeferredOffset {
  location: 0,
  value: DUMMY_VALUE,
};

impl DeferredOffset {
  fn update_value(&mut self, value: usize) -> Result<()> {
    self.value = IntegerNumber(i32::try_from(value).map_err(|_| OverflowError)?);
    Ok(())
  }

  /// Adjust the location of an offset, if it has been set (i.e. it's not a dummy offset).
  fn adjust_location(&mut self, delta: usize) {
    if *self != DUMMY_OFFSET {
      self.location += delta;
    }
  }

  fn update_location(&mut self, location: usize) {
    self.location = location;
  }

  /// Write the deferred offset into a buffer.
  fn write_into(&self, buffer: &mut [u8]) -> Result<()> {
    let mut w = Writer::new();
    // Always write using 5 bytes, to prevent its size from changing.
    self.value.write_as_5_bytes(&mut w);
    let encoded = w.finish();
    let pos = buffer
      .get_mut(self.location..self.location + 5)
      .ok_or(SubsetError)?;

    pos.copy_from_slice(&encoded);

    Ok(())
  }
}

/// Keeps track of the offsets that need to be written in the font.
#[derive(Debug)]
struct Offsets {
  /// Offset of the charset data.
  charset_offset: DeferredOffset,
  /// Offset of the charstrings data.
  char_strings_offset: DeferredOffset,
  /// Lengths of the private dicts (not strictly an offset, but we still use it for simplicity).
  private_dicts_lens: Vec<DeferredOffset>,
  /// Offset of the private dicts.
  private_dicts_offsets: Vec<DeferredOffset>,
  /// Offset of the fd array.
  fd_array_offset: DeferredOffset,
  /// Offset of the fd select.
  fd_select_offset: DeferredOffset,
}

impl Offsets {
  pub fn new_cid(num_font_dicts: u8) -> Self {
    Self {
      char_strings_offset: DUMMY_OFFSET,
      charset_offset: DUMMY_OFFSET,
      private_dicts_lens: vec![DUMMY_OFFSET; num_font_dicts as usize],
      private_dicts_offsets: vec![DUMMY_OFFSET; num_font_dicts as usize],
      fd_select_offset: DUMMY_OFFSET,
      fd_array_offset: DUMMY_OFFSET,
    }
  }

  pub fn new_sid() -> Self {
    Self::new_cid(1)
  }
}

pub fn subset(ctx: &mut Context<'_>) -> Result<()> {
  let table = Table::parse(ctx)?;

  // Note: The charstrings are already in the new order that they need be written in.
  let (char_strings, fd_remapper) = subset_charstrings(&table, &ctx.mapper)?;

  let sid_remapper = get_sid_remapper(&table, &fd_remapper).ok_or(SubsetError)?;

  let mut offsets = match &table.font_kind {
    FontKind::Sid(_) => Offsets::new_sid(),
    FontKind::Cid(_) => Offsets::new_cid(fd_remapper.len()),
  };

  let mut subsetted_font = {
    let mut w = Writer::new();
    // HEADER
    // We always use OffSize 4 (but as far as I can tell this field is unused anyway).
    w.write([1u8, 0, 4, 4]);
    // Name INDEX
    w.write(table.names);
    // Top DICT INDEX
    rewrite_top_dict_index(&table.top_dict_data, &mut offsets, &sid_remapper, &mut w)?;
    // String INDEX
    let index = create_index(
      sid_remapper
        .sorted_strings()
        .map(|s| Vec::from(s.as_ref()))
        .collect::<Vec<_>>(),
    )?;
    w.extend(&index.data);
    // Global Subr INDEX
    // We desubroutinized, so no global subroutines and thus empty index.
    w.write(OwnedIndex::default());

    // Charsets
    offsets.charset_offset.update_value(w.len())?;
    rewrite_charset(&ctx.mapper, &mut w)?;

    // Private dicts.
    match &table.font_kind {
      FontKind::Sid(sid) => {
        // Since we convert SID-keyed to CID-keyed, we write one private dict with index 0.
        rewrite_private_dict(&mut offsets, sid.private_dict_data, &mut w, 0)?;
      }
      FontKind::Cid(cid) => {
        rewrite_cid_private_dicts(&fd_remapper, &mut offsets, cid, &mut w)?;
      }
    }

    // FDSelect
    offsets.fd_select_offset.update_value(w.len())?;
    match &table.font_kind {
      FontKind::Sid(_) => generate_fd_index(&ctx.mapper, &mut w)?,
      FontKind::Cid(cid_metadata) => {
        rewrite_fd_index(&ctx.mapper, cid_metadata.fd_select, &fd_remapper, &mut w)?
      }
    };

    // FDArray
    offsets.fd_array_offset.update_value(w.len())?;
    match &table.font_kind {
      FontKind::Sid(_) => generate_font_dict_index(&mut offsets, &mut w)?,
      FontKind::Cid(cid_metadata) => rewrite_font_dict_index(
        &fd_remapper,
        &sid_remapper,
        &mut offsets,
        cid_metadata,
        &mut w,
        table.top_dict_data.font_matrix.is_none(),
      )?,
    }

    // Charstrings INDEX
    offsets.char_strings_offset.update_value(w.len())?;
    w.extend(&create_index(char_strings)?.data);

    w.finish()
  };

  // Rewrite the dummy offsets.
  update_offsets(&offsets, subsetted_font.as_mut_slice())?;

  ctx.push(Tag::CFF, subsetted_font);

  Ok(())
}

fn update_offsets(offsets: &Offsets, buffer: &mut [u8]) -> Result<()> {
  let mut write = |offset: DeferredOffset| {
    if offset != DUMMY_OFFSET {
      offset.write_into(buffer)?;
    }
    Ok(())
  };

  // Private dicts offset have already been written correctly, so no need to write them
  // here.

  write(offsets.charset_offset)?;
  write(offsets.char_strings_offset)?;
  write(offsets.fd_select_offset)?;
  write(offsets.fd_array_offset)?;

  Ok(())
}

/// Create the list of bytes that constitute the programs of the charstrings, sorted in the new glyph order.
fn subset_charstrings(
  table: &Table,
  remapper: &GlyphRemapper,
) -> Result<(Vec<Vec<u8>>, FontDictRemapper)> {
  let gsubrs = {
    let subroutines = table.global_subrs.into_iter().collect::<Vec<_>>();
    SubroutineContainer::new(subroutines)
  };

  let lsubrs = {
    match &table.font_kind {
      FontKind::Cid(cid) => {
        let subroutines = cid
          .font_dicts
          .iter()
          .map(|font_dict| font_dict.local_subrs.into_iter().collect::<Vec<_>>())
          .collect::<Vec<_>>();
        SubroutineCollection::new(subroutines)
      }
      FontKind::Sid(sid) => {
        let subroutines = sid.local_subrs.into_iter().collect::<Vec<_>>();
        SubroutineCollection::new(vec![subroutines])
      }
    }
  };

  let mut used_fds = BTreeSet::new();
  let mut char_strings = vec![];

  for old_gid in remapper.remapped_gids() {
    let fd_index = match &table.font_kind {
      FontKind::Cid(cid) => {
        let fd_index = cid
          .fd_select
          .font_dict_index(old_gid)
          .ok_or(MalformedFont)?;
        used_fds.insert(fd_index);
        fd_index
      }
      FontKind::Sid(_) => 0,
    };

    let decompiler = Decompiler::new(
      gsubrs.get_handler(),
      lsubrs.get_handler(fd_index).ok_or(MalformedFont)?,
    );
    let charstring = table
      .char_strings
      .get(old_gid as u32)
      .ok_or(MalformedFont)?;
    char_strings.push(decompiler.decompile(charstring)?);
  }

  let mut fd_remapper = FontDictRemapper::new();

  for fd in used_fds {
    fd_remapper.remap(fd);
  }

  Ok((
    char_strings.iter().map(|p| p.compile()).collect(),
    fd_remapper,
  ))
}

fn get_sid_remapper<'a>(
  table: &Table<'a>,
  fd_remapper: &FontDictRemapper,
) -> Option<SidRemapper<'a>> {
  let mut sid_remapper = SidRemapper::new();
  sid_remapper.remap(&b"Adobe"[..]);
  sid_remapper.remap(&b"Identity"[..]);

  let mut remap_sid = |sid: StringId| {
    if sid.is_standard_string() {
      Some(())
    } else {
      let string = table
        .strings
        .get((sid.0 - StringId::STANDARD_STRING_LEN) as u32)?;
      sid_remapper.remap_with_old_sid(sid, Cow::Borrowed(string));

      Some(())
    }
  };

  if let Some(copyright) = table.top_dict_data.copyright {
    remap_sid(copyright)?;
  }

  if let Some(font_name) = table.top_dict_data.font_name {
    remap_sid(font_name)?;
  }

  if let Some(notice) = table.top_dict_data.notice {
    remap_sid(notice)?;
  }

  if let FontKind::Cid(cid) = &table.font_kind {
    for font_dict in fd_remapper.sorted_iter() {
      let font_dict = cid.font_dicts.get(font_dict as usize)?;

      if let Some(font_name) = font_dict.font_name {
        remap_sid(font_name)?;
      }
    }
  }

  Some(sid_remapper)
}

// The parsing logic was taken from ttf-parser.
impl<'a> Table<'a> {
  pub fn parse(ctx: &mut Context<'a>) -> Result<Self> {
    let cff = ctx.expect_table(Tag::CFF).ok_or(MalformedFont)?;

    let mut r = Reader::new(cff);

    let major = r.read::<u8>().ok_or(MalformedFont)?;

    if major != 1 {
      return Err(Error::CFFError);
    }

    r.skip::<u8>(); // minor
    let header_size = r.read::<u8>().ok_or(MalformedFont)?;

    r.jump(header_size as usize);

    let names_start = r.offset();
    skip_index::<u16>(&mut r).ok_or(MalformedFont)?;
    let names = cff.get(names_start..r.offset()).ok_or(MalformedFont)?;
    let top_dict_data = parse_top_dict_index(&mut r).ok_or(MalformedFont)?;

    let strings = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;
    let global_subrs = parse_index::<u16>(&mut r).ok_or(MalformedFont)?;

    let char_strings_offset = top_dict_data.char_strings.ok_or(MalformedFont)?;
    let char_strings = {
      let mut r = Reader::new_at(cff, char_strings_offset);
      parse_index::<u16>(&mut r).ok_or(MalformedFont)?
    };

    let number_of_glyphs = u16::try_from(char_strings.len())
      .ok()
      .filter(|n| *n > 0)
      .ok_or(MalformedFont)?;

    let font_kind = if top_dict_data.has_ros {
      FontKind::Cid(
        cid_font::parse_cid_metadata(cff, &top_dict_data, number_of_glyphs).ok_or(MalformedFont)?,
      )
    } else {
      FontKind::Sid(sid_font::parse_sid_metadata(cff, &top_dict_data))
    };

    Ok(Self {
      names,
      top_dict_data,
      strings,
      global_subrs,
      char_strings,
      font_kind,
    })
  }
}

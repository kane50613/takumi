use crate::subsetter::Error::{MalformedFont, SubsetError};
use crate::subsetter::GlyphRemapper;
use crate::subsetter::Result;
use crate::subsetter::cff::dict::font_dict;
use crate::subsetter::cff::dict::font_dict::FontDict;
use crate::subsetter::cff::dict::top_dict::TopDictData;
use crate::subsetter::cff::index::{Index, parse_index};
use crate::subsetter::cff::remapper::FontDictRemapper;
use crate::subsetter::read::{LazyArray16, Reader};
use crate::subsetter::write::Writer;

// The parsing logic was taken from ttf-parser.

/// Parse CID metadata from a font.
pub fn parse_cid_metadata<'a>(
  data: &'a [u8],
  top_dict: &TopDictData,
  number_of_glyphs: u16,
) -> Option<CIDMetadata<'a>> {
  let (fd_array_offset, fd_select_offset) = match (top_dict.fd_array, top_dict.fd_select) {
    (Some(a), Some(b)) => (a, b),
    _ => return None,
  };

  let mut metadata = CIDMetadata {
    fd_array: {
      let mut r = Reader::new_at(data, fd_array_offset);
      parse_index::<u16>(&mut r)?
    },
    fd_select: {
      let mut s = Reader::new_at(data, fd_select_offset);
      parse_fd_select(number_of_glyphs, &mut s)?
    },
    ..CIDMetadata::default()
  };

  for font_dict_data in metadata.fd_array {
    metadata
      .font_dicts
      .push(font_dict::parse_font_dict(data, font_dict_data)?);
  }

  Some(metadata)
}

fn parse_fd_select<'a>(number_of_glyphs: u16, r: &mut Reader<'a>) -> Option<FDSelect<'a>> {
  let format = r.read::<u8>()?;
  match format {
    0 => Some(FDSelect::Format0(r.read_array16::<u8>(number_of_glyphs)?)),
    3 => Some(FDSelect::Format3(r.tail()?)),
    _ => None,
  }
}

/// Metadata necessary for processing CID-keyed fonts.
#[derive(Clone, Default, Debug)]
pub struct CIDMetadata<'a> {
  pub font_dicts: Vec<FontDict<'a>>,
  pub fd_array: Index<'a>,
  pub fd_select: FDSelect<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum FDSelect<'a> {
  Format0(LazyArray16<'a, u8>),
  Format3(&'a [u8]),
}

impl Default for FDSelect<'_> {
  fn default() -> Self {
    FDSelect::Format0(LazyArray16::default())
  }
}

impl FDSelect<'_> {
  /// Get the font dict index for a glyph.
  pub fn font_dict_index(&self, glyph_id: u16) -> Option<u8> {
    match self {
      FDSelect::Format0(array) => array.get(glyph_id),
      FDSelect::Format3(data) => {
        let mut r = Reader::new(data);
        let number_of_ranges = r.read::<u16>()?;
        if number_of_ranges == 0 {
          return None;
        }

        let number_of_ranges = number_of_ranges.checked_add(1)?;

        let mut prev_first_glyph = r.read::<u16>()?;
        let mut prev_index = r.read::<u8>()?;
        for _ in 1..number_of_ranges {
          let curr_first_glyph = r.read::<u16>()?;
          if (prev_first_glyph..curr_first_glyph).contains(&glyph_id) {
            return Some(prev_index);
          } else {
            prev_index = r.read::<u8>()?;
          }

          prev_first_glyph = curr_first_glyph;
        }

        None
      }
    }
  }
}

/// Rewrite the FD INDEX for CID-keyed font.
pub fn rewrite_fd_index(
  gid_remapper: &GlyphRemapper,
  fd_select: FDSelect,
  fd_remapper: &FontDictRemapper,
  w: &mut Writer,
) -> Result<()> {
  // We always use format 0, since it's the simplest.
  w.write::<u8>(0);

  for gid in gid_remapper.remapped_gids() {
    let old_fd = fd_select.font_dict_index(gid).ok_or(MalformedFont)?;
    let new_fd = fd_remapper.get(old_fd).ok_or(SubsetError)?;
    w.write(new_fd);
  }

  Ok(())
}

use crate::subsetter::GlyphRemapper;
use crate::subsetter::cff::dict::private_dict::parse_subr_offset;
use crate::subsetter::cff::dict::top_dict::TopDictData;
use crate::subsetter::cff::index::{Index, parse_index};
use crate::subsetter::read::Reader;
use crate::subsetter::write::Writer;

/// Metadata required for handling SID-keyed fonts.
#[derive(Clone, Copy, Default, Debug)]
pub struct SIDMetadata<'a> {
  pub local_subrs: Index<'a>,
  pub private_dict_data: &'a [u8],
}

// The parsing logic was taken from ttf-parser.
pub fn parse_sid_metadata<'a>(data: &'a [u8], top_dict: &TopDictData) -> SIDMetadata<'a> {
  top_dict
    .private
    .clone()
    .and_then(|private_dict_range| {
      let mut metadata = SIDMetadata::default();
      let private_dict_data = data.get(private_dict_range.clone())?;

      metadata.local_subrs = if let Some(subrs_offset) = parse_subr_offset(private_dict_data) {
        let start = private_dict_range.start.checked_add(subrs_offset)?;
        let subrs_data = data.get(start..)?;
        let mut r = Reader::new(subrs_data);
        parse_index::<u16>(&mut r)?
      } else {
        Index::default()
      };

      metadata.private_dict_data = private_dict_data;
      Some(metadata)
    })
    .unwrap_or_default()
}

/// Write the FD INDEX for SID-keyed fonts.
/// They all get mapped to the font DICT 0.
pub fn generate_fd_index(
  gid_remapper: &GlyphRemapper,
  w: &mut Writer,
) -> crate::subsetter::Result<()> {
  // Format
  w.write::<u8>(3);
  // nRanges
  w.write::<u16>(1);
  // first
  w.write::<u16>(0);
  // fd index
  w.write::<u8>(0);
  // sentinel
  w.write::<u16>(gid_remapper.num_gids());
  Ok(())
}

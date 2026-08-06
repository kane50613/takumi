use crate::subsetter::Error::SubsetError;
use crate::subsetter::Result;
use crate::subsetter::cff::Offsets;
use crate::subsetter::cff::cid_font::CIDMetadata;
use crate::subsetter::cff::dict::DictionaryParser;
use crate::subsetter::cff::dict::operators::*;
use crate::subsetter::cff::dict::private_dict::parse_subr_offset;
use crate::subsetter::cff::index::{Index, create_index, parse_index};
use crate::subsetter::cff::number::{Number, StringId};
use crate::subsetter::cff::remapper::{FontDictRemapper, SidRemapper};
use crate::subsetter::read::Reader;
use crate::subsetter::write::Writer;
use std::array;

// The parsing logic was adapted from ttf-parser.

/// A font DICT.
#[derive(Default, Clone, Debug)]
pub struct FontDict<'a> {
  /// The local subroutines that are linked in the font DICT.
  pub local_subrs: Index<'a>,
  /// The underlying data of the private dict.
  pub private_dict: &'a [u8],
  /// The StringID of the font name in this font DICT.
  pub font_name: Option<StringId>,
  /// The font matrix.
  pub font_matrix: Option<[Number; 6]>,
}

pub fn parse_font_dict<'a>(font_data: &'a [u8], font_dict_data: &[u8]) -> Option<FontDict<'a>> {
  let mut font_dict = FontDict::default();

  let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
  let mut dict_parser = DictionaryParser::new(font_dict_data, &mut operands_buffer);
  while let Some(operator) = dict_parser.parse_next() {
    match operator {
      PRIVATE => {
        let private_dict_range = dict_parser.parse_range()?;
        let private_dict_data = font_data.get(private_dict_range.clone())?;
        font_dict.private_dict = private_dict_data;
        font_dict.local_subrs = {
          if let Some(subrs_offset) = parse_subr_offset(private_dict_data) {
            let start = private_dict_range.start.checked_add(subrs_offset)?;
            let subrs_data = font_data.get(start..)?;
            let mut r = Reader::new(subrs_data);
            parse_index::<u16>(&mut r)?
          } else {
            Index::default()
          }
        };
      }
      FONT_NAME => font_dict.font_name = Some(dict_parser.parse_sid()?),
      FONT_MATRIX => font_dict.font_matrix = Some(dict_parser.parse_font_matrix()?),
      _ => {}
    }
  }

  Some(font_dict)
}

/// Rewrite the font DICT INDEX for CID-keyed fonts.
pub fn rewrite_font_dict_index(
  fd_remapper: &FontDictRemapper,
  sid_remapper: &SidRemapper,
  offsets: &mut Offsets,
  metadata: &CIDMetadata,
  w: &mut Writer,
  top_dict_is_missing_font_matrix: bool,
) -> Result<()> {
  let mut dicts = vec![];

  for (new_df, old_df) in fd_remapper.sorted_iter().enumerate() {
    let new_df = new_df as u8;

    let dict = metadata
      .font_dicts
      .get(old_df as usize)
      .ok_or(SubsetError)?;
    let mut w = Writer::new();

    // See comment in `rewrite_top_dict_index`.
    w.write(
      dict
        .font_matrix
        .map(|m| {
          if top_dict_is_missing_font_matrix {
            let scale = [
              Number::from_f32(1000.0),
              Number::zero(),
              Number::zero(),
              Number::from_f32(1000.0),
              Number::zero(),
              Number::zero(),
            ];
            Number::combine(m, scale)
          } else {
            m
          }
        })
        .unwrap_or([
          Number::one(),
          Number::zero(),
          Number::zero(),
          Number::one(),
          Number::zero(),
          Number::zero(),
        ]),
    );
    w.write(FONT_MATRIX);

    // Write the length and offset of the private dict.
    // Private dicts have already been written, so the offsets are already correct.
    // This means that these two offsets are a bit special compared to the others, since
    // we never use the `location` field of the offset and we don't overwrite it like we do
    // for the others.
    offsets
      .private_dicts_lens
      .get(new_df as usize)
      .ok_or(SubsetError)?
      .value
      .write_as_5_bytes(&mut w);

    offsets
      .private_dicts_offsets
      .get_mut(new_df as usize)
      .ok_or(SubsetError)?
      .value
      .write_as_5_bytes(&mut w);
    w.write(PRIVATE);

    if let Some(font_name) = dict.font_name.and_then(|s| sid_remapper.get_new_sid(s)) {
      w.write(Number::from_i32(font_name.0 as i32));
      w.write(FONT_NAME);
    }

    dicts.push(w.finish());
  }

  w.write(create_index(dicts)?);

  Ok(())
}

/// Generate a new font DICT INDEX for SID-keyed fonts.
pub fn generate_font_dict_index(offsets: &mut Offsets, w: &mut Writer) -> Result<()> {
  let mut sub_w = Writer::new();

  // See comment in `rewrite_top_dict_index`
  sub_w.write([
    Number::one(),
    Number::zero(),
    Number::zero(),
    Number::one(),
    Number::zero(),
    Number::zero(),
  ]);
  sub_w.write(FONT_MATRIX);

  // Write the length and offset of the private dict.
  // Private dicts have already been written, so the offsets are already correct.
  // This means that these two offsets are a bit special compared to the others, since
  // we never use the `location` field of the offset and we don't overwrite it like we do
  // for the others.
  offsets
    .private_dicts_lens
    .first()
    .ok_or(SubsetError)?
    .value
    .write_as_5_bytes(&mut sub_w);

  offsets
    .private_dicts_offsets
    .first_mut()
    .ok_or(SubsetError)?
    .value
    .write_as_5_bytes(&mut sub_w);

  sub_w.write(PRIVATE);
  w.write(create_index(vec![sub_w.finish()])?);

  // TODO: Maybe write a font name as well? But shouldn't matter.
  Ok(())
}

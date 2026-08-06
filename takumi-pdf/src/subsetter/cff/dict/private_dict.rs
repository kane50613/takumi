use crate::subsetter::Error::{MalformedFont, SubsetError};
use crate::subsetter::Result;
use crate::subsetter::cff::Offsets;
use crate::subsetter::cff::cid_font::CIDMetadata;
use crate::subsetter::cff::dict::DictionaryParser;
use crate::subsetter::cff::dict::operators::*;
use crate::subsetter::cff::number::Number;
use crate::subsetter::cff::remapper::FontDictRemapper;
use crate::subsetter::write::Writer;
use std::array;

// The parsing logic was adapted from ttf-parser.

/// Parse the subroutine offset from a private dict.
pub fn parse_subr_offset(data: &[u8]) -> Option<usize> {
  let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
  let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

  while let Some(operator) = dict_parser.parse_next() {
    if operator == SUBRS {
      return dict_parser.parse_offset();
    }
  }

  None
}

/// Write the private dicts of a CID font for each font dict.
pub fn rewrite_cid_private_dicts(
  fd_remapper: &FontDictRemapper,
  offsets: &mut Offsets,
  metadata: &CIDMetadata,
  w: &mut Writer,
) -> Result<()> {
  for (new_df, old_df) in fd_remapper.sorted_iter().enumerate() {
    let font_dict = metadata
      .font_dicts
      .get(old_df as usize)
      .ok_or(SubsetError)?;
    rewrite_private_dict(offsets, font_dict.private_dict, w, new_df)?;
  }

  Ok(())
}

pub(crate) fn rewrite_private_dict(
  offsets: &mut Offsets,
  private_dict_data: &[u8],
  w: &mut Writer,
  dict_index: usize,
) -> Result<()> {
  let private_dict_offset = w.len();

  let private_dict_data = {
    let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
    let mut dict_parser = DictionaryParser::new(private_dict_data, &mut operands_buffer);

    let mut sub_w = Writer::new();

    // We just make sure that no subroutine offset gets written, all other operators stay the
    // same.
    while let Some(operator) = dict_parser.parse_next() {
      match operator {
        SUBRS => {
          // We don't have any subroutines, so don't rewrite this DICT entry.
        }
        _ => {
          dict_parser.parse_operands().ok_or(MalformedFont)?;
          let operands = dict_parser.operands();

          sub_w.write(operands);
          sub_w.write(operator);
        }
      }
    }

    sub_w.finish()
  };

  let private_dict_len = private_dict_data.len();

  offsets
    .private_dicts_lens
    .get_mut(dict_index)
    .ok_or(SubsetError)?
    .update_value(private_dict_len)?;

  offsets
    .private_dicts_offsets
    .get_mut(dict_index)
    .ok_or(SubsetError)?
    .update_value(private_dict_offset)?;

  w.extend(&private_dict_data);

  Ok(())
}

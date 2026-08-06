use crate::subsetter::Error::SubsetError;
use crate::subsetter::Result;
use crate::subsetter::cff::dict::DictionaryParser;
use crate::subsetter::cff::index::{create_index, parse_index};
use crate::subsetter::cff::number::{Number, StringId};
use crate::subsetter::cff::remapper::SidRemapper;
use crate::subsetter::cff::{DUMMY_VALUE, Offsets};
use crate::subsetter::read::Reader;
use crate::subsetter::write::Writer;
use std::array;
use std::ops::Range;

// The parsing logic was adapted from ttf-parser.

#[derive(Default, Debug, Clone)]
pub struct TopDictData {
  pub charset: Option<usize>,
  pub char_strings: Option<usize>,
  pub private: Option<Range<usize>>,
  pub fd_array: Option<usize>,
  pub fd_select: Option<usize>,
  pub notice: Option<StringId>,
  pub copyright: Option<StringId>,
  pub font_name: Option<StringId>,
  pub has_ros: bool,
  pub font_matrix: Option<[Number; 6]>,
  pub font_bbox: Option<[Number; 4]>,
}

pub fn parse_top_dict_index(r: &mut Reader) -> Option<TopDictData> {
  use super::operators::*;
  let mut top_dict = TopDictData::default();

  let index = parse_index::<u16>(r)?;

  // The Top DICT INDEX should have only one dictionary in CFF fonts.
  let data = index.get(0)?;

  let mut operands_buffer: [Number; 48] = array::from_fn(|_| Number::zero());
  let mut dict_parser = DictionaryParser::new(data, &mut operands_buffer);

  while let Some(operator) = dict_parser.parse_next() {
    match operator {
      // We only need to preserve the copyrights and font name.
      NOTICE => top_dict.notice = Some(dict_parser.parse_sid()?),
      COPYRIGHT => top_dict.copyright = Some(dict_parser.parse_sid()?),
      FONT_NAME => top_dict.font_name = Some(dict_parser.parse_sid()?),
      CHARSET => top_dict.charset = Some(dict_parser.parse_offset()?),
      // We don't care about encoding since we convert to CID-keyed font anyway.
      ENCODING => {}
      CHAR_STRINGS => top_dict.char_strings = Some(dict_parser.parse_offset()?),
      PRIVATE => top_dict.private = Some(dict_parser.parse_range()?),
      // We will rewrite the ROS, so no need to grab it from here. But we need to
      // register it, so we know we are dealing with a CID-keyed font.
      ROS => top_dict.has_ros = true,
      FD_ARRAY => top_dict.fd_array = Some(dict_parser.parse_offset()?),
      FD_SELECT => top_dict.fd_select = Some(dict_parser.parse_offset()?),
      FONT_MATRIX => top_dict.font_matrix = Some(dict_parser.parse_font_matrix()?),
      FONT_BBOX => top_dict.font_bbox = Some(dict_parser.parse_font_bbox()?),
      _ => {}
    }
  }

  Some(top_dict)
}

/// Rewrite the top dict. Implementation is based on what ghostscript seems to keep when
/// rewriting a font subset.
pub fn rewrite_top_dict_index(
  top_dict_data: &TopDictData,
  offsets: &mut Offsets,
  sid_remapper: &SidRemapper,
  w: &mut Writer,
) -> Result<()> {
  use super::operators::*;

  let mut sub_w = Writer::new();

  // ROS.
  sub_w.write(Number::from_i32(
    sid_remapper.get(b"Adobe").ok_or(SubsetError)?.0 as i32,
  ));
  sub_w.write(Number::from_i32(
    sid_remapper.get(b"Identity").ok_or(SubsetError)?.0 as i32,
  ));
  sub_w.write(Number::zero());
  sub_w.write(ROS);

  // Copyright notices.
  if let Some(copyright) = top_dict_data
    .copyright
    .and_then(|s| sid_remapper.get_new_sid(s))
  {
    sub_w.write(Number::from_i32(copyright.0 as i32));
    sub_w.write(COPYRIGHT);
  }

  if let Some(notice) = top_dict_data
    .notice
    .and_then(|s| sid_remapper.get_new_sid(s))
  {
    sub_w.write(Number::from_i32(notice.0 as i32));
    sub_w.write(NOTICE);
  }

  // Font name.
  if let Some(font_name) = top_dict_data
    .font_name
    .and_then(|s| sid_remapper.get_new_sid(s))
  {
    sub_w.write(Number::from_i32(font_name.0 as i32));
    sub_w.write(FONT_NAME);
  }

  // See https://bugs.ghostscript.com/show_bug.cgi?id=690724#c12 and https://leahneukirchen.org/blog/archive/2022/10/50-blank-pages-or-black-box-debugging-of-pdf-rendering-in-printers.html
  // We assume that if at least one font dict has a matrix, all of them do.
  // Case 1: Top DICT MATRIX is some, FONT DICT MATRIX is some -> supplied, supplied
  // Case 2: Top DICT MATRIX is none, FONT DICT MATRIX is some -> (0.001, 0, 0, 0.001, 0, 0), supplied * 1000
  // Case 3: Top DICT MATRIX is some, FONT DICT MATRIX is none -> supplied, (1, 0, 0, 1, 0, 0)
  // Case 4: Top DICT MATRIX is none, FONT DICT MATRIX is none -> (0.001, 0, 0, 0.001, 0 0), (1, 0, 0, 1, 0, 0)
  sub_w.write(top_dict_data.font_matrix.as_ref().unwrap_or(&[
    Number::from_f32(0.001),
    Number::zero(),
    Number::zero(),
    Number::from_f32(0.001),
    Number::zero(),
    Number::zero(),
  ]));
  sub_w.write(FONT_MATRIX);

  // Write a default font bbox, if it does not exist.
  sub_w.write(top_dict_data.font_bbox.as_ref().unwrap_or(&[
    Number::zero(),
    Number::zero(),
    Number::zero(),
    Number::zero(),
  ]));
  sub_w.write(FONT_BBOX);

  // Note: When writing the offsets, we need to add the current length of w AND sub_w.
  // Charset
  offsets
    .charset_offset
    .update_location(sub_w.len() + w.len());
  DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
  sub_w.write(CHARSET);

  // Charstrings
  offsets
    .char_strings_offset
    .update_location(sub_w.len() + w.len());
  DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
  sub_w.write(CHAR_STRINGS);

  sub_w.write(Number::from_i32(u16::MAX as i32));
  sub_w.write(CID_COUNT);

  // Note: Previously, we wrote those two entries directly after ROS.
  // However, for some reason not known to me, Apple Preview does not like show the CFF font
  // at all if that's the case. This is why we now write the offsets in the very end.

  // FD array.
  offsets
    .fd_array_offset
    .update_location(sub_w.len() + w.len());
  DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
  sub_w.write(FD_ARRAY);

  // FD select.
  offsets
    .fd_select_offset
    .update_location(sub_w.len() + w.len());
  DUMMY_VALUE.write_as_5_bytes(&mut sub_w);
  sub_w.write(FD_SELECT);

  let finished = sub_w.finish();

  // TOP DICT INDEX always has size 1 in CFF.
  let index = create_index(vec![finished])?;

  // The offsets we calculated before were calculated under the assumption
  // that the contents of sub_w will be appended directly to w. However, when we create an index,
  // the INDEX header data will be appended in the beginning, meaning that we need to adjust the offsets
  // to account for that.
  offsets.charset_offset.adjust_location(index.header_size);
  offsets
    .char_strings_offset
    .adjust_location(index.header_size);
  offsets.fd_array_offset.adjust_location(index.header_size);
  offsets.fd_select_offset.adjust_location(index.header_size);

  w.write(index);

  Ok(())
}

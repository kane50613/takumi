pub(crate) mod font_dict;
pub(crate) mod private_dict;
pub(crate) mod top_dict;

// The `DictionaryParser` was taken from ttf-parser.

use crate::subsetter::cff::number::{Number, StringId};
use crate::subsetter::cff::operator::{Operator, TWO_BYTE_OPERATOR_MARK};
use crate::subsetter::read::Reader;
use std::ops::Range;

pub struct DictionaryParser<'a> {
  data: &'a [u8],
  offset: usize,
  operands_offset: usize,
  operands: &'a mut [Number],
  operands_len: u16,
}

impl<'a> DictionaryParser<'a> {
  pub fn new(data: &'a [u8], operands_buffer: &'a mut [Number]) -> Self {
    DictionaryParser {
      data,
      offset: 0,
      operands_offset: 0,
      operands: operands_buffer,
      operands_len: 0,
    }
  }

  pub fn parse_next(&mut self) -> Option<Operator> {
    let mut r = Reader::new_at(self.data, self.offset);
    self.operands_offset = self.offset;
    while !r.at_end() {
      // 0..=21 bytes are operators.
      if is_dict_one_byte_op(r.peak::<u8>()?) {
        let b = r.read::<u8>()?;
        let mut operator = Operator::from_one_byte(b);

        if b == TWO_BYTE_OPERATOR_MARK {
          operator = Operator::from_two_byte(r.read::<u8>()?);
        }

        self.offset = r.offset();
        return Some(operator);
      } else {
        let _ = Number::parse_cff_number(&mut r)?;
      }
    }

    None
  }

  pub fn parse_operands(&mut self) -> Option<()> {
    let mut r = Reader::new_at(self.data, self.operands_offset);
    self.operands_len = 0;
    while !r.at_end() {
      let b = r.peak::<u8>()?;
      // 0..=21 bytes are operators.
      if is_dict_one_byte_op(b) {
        r.read::<u8>()?;
        break;
      } else {
        let op = Number::parse_cff_number(&mut r)?;
        self.operands[usize::from(self.operands_len)] = op;
        self.operands_len += 1;

        if usize::from(self.operands_len) >= self.operands.len() {
          break;
        }
      }
    }

    Some(())
  }

  pub fn operands(&self) -> &[Number] {
    &self.operands[..usize::from(self.operands_len)]
  }

  pub fn parse_sid(&mut self) -> Option<StringId> {
    self.parse_operands()?;
    let operands = self.operands();
    if operands.len() == 1 {
      Some(StringId(u16::try_from(operands[0].as_i32()?).ok()?))
    } else {
      None
    }
  }

  pub fn parse_offset(&mut self) -> Option<usize> {
    self.parse_operands()?;
    let operands = self.operands();
    if operands.len() == 1 {
      usize::try_from(operands[0].as_u32()?).ok()
    } else {
      None
    }
  }

  pub fn parse_font_bbox(&mut self) -> Option<[Number; 4]> {
    self.parse_operands()?;
    let operands = self.operands();
    if operands.len() == 4 {
      Some([operands[0], operands[1], operands[2], operands[3]])
    } else {
      None
    }
  }

  pub fn parse_font_matrix(&mut self) -> Option<[Number; 6]> {
    self.parse_operands()?;
    let operands = self.operands();
    if operands.len() == 6 {
      Some([
        operands[0],
        operands[1],
        operands[2],
        operands[3],
        operands[4],
        operands[5],
      ])
    } else {
      None
    }
  }

  pub fn parse_range(&mut self) -> Option<Range<usize>> {
    self.parse_operands()?;
    let operands = self.operands();
    if operands.len() == 2 {
      let len = usize::try_from(operands[0].as_u32()?).ok()?;
      let start = usize::try_from(operands[1].as_u32()?).ok()?;
      let end = start.checked_add(len)?;
      Some(start..end)
    } else {
      None
    }
  }
}

fn is_dict_one_byte_op(b: u8) -> bool {
  match b {
    0..=27 => true,
    28..=30 => false,  // numbers
    31 => true,        // Reserved
    32..=254 => false, // numbers
    255 => true,       // Reserved
  }
}

/// A subset of the operators for DICT's we care about.
pub(crate) mod operators {
  use crate::subsetter::cff::operator::{Operator, OperatorType, TWO_BYTE_OPERATOR_MARK};

  // TOP DICT OPERATORS
  pub const NOTICE: Operator = Operator(OperatorType::OneByteOperator([1]));
  pub const FONT_BBOX: Operator = Operator(OperatorType::OneByteOperator([5]));
  pub const COPYRIGHT: Operator =
    Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 0]));
  pub const FONT_MATRIX: Operator =
    Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 7]));
  pub const CHARSET: Operator = Operator(OperatorType::OneByteOperator([15]));
  pub const ENCODING: Operator = Operator(OperatorType::OneByteOperator([16]));
  pub const CHAR_STRINGS: Operator = Operator(OperatorType::OneByteOperator([17]));
  pub const PRIVATE: Operator = Operator(OperatorType::OneByteOperator([18]));

  // TOP DICT OPERATORS (CID FONTS)
  pub const ROS: Operator = Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 30]));
  pub const CID_COUNT: Operator =
    Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 34]));
  pub const FD_ARRAY: Operator =
    Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 36]));
  pub const FD_SELECT: Operator =
    Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 37]));
  pub const FONT_NAME: Operator =
    Operator(OperatorType::TwoByteOperator([TWO_BYTE_OPERATOR_MARK, 38]));

  // PRIVATE DICT OPERATORS
  pub const SUBRS: Operator = Operator(OperatorType::OneByteOperator([19]));
}

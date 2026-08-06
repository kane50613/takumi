use crate::subsetter::Error::{CFFError, Unimplemented};
use crate::subsetter::cff::argstack::ArgumentsStack;
use crate::subsetter::cff::number::Number;
use crate::subsetter::cff::operator::Operator;
use crate::subsetter::cff::subroutines::SubroutineHandler;
use crate::subsetter::read::Reader;
use crate::subsetter::write::Writer;
use crate::subsetter::{Error, Result};
use operators::*;
use std::fmt::{Debug, Formatter};

pub type CharString<'a> = &'a [u8];

// Adapted from fonttools.
/// A charstring decompiler.
pub struct Decompiler<'a> {
  gsubr_handler: SubroutineHandler<'a>,
  lsubr_handler: SubroutineHandler<'a>,
  stack: ArgumentsStack,
  hint_count: u16,
  hint_mask_bytes: u16,
}

enum CharStringEnd {
  Return,
  EndChar,
}

impl<'a> Decompiler<'a> {
  /// Create a new charstring decompiler.
  pub fn new(gsubr_handler: SubroutineHandler<'a>, lsubr_handler: SubroutineHandler<'a>) -> Self {
    Self {
      gsubr_handler,
      lsubr_handler,
      stack: ArgumentsStack::new(),
      hint_count: 0,
      hint_mask_bytes: 0,
    }
  }

  /// Decompile a charstring with the given decompiler.
  pub fn decompile(mut self, charstring: CharString<'a>) -> Result<Program<'a>> {
    let mut program = Program::default();
    self.decompile_inner(charstring, &mut program, 1)?;
    Ok(program)
  }

  fn decompile_inner(
    &mut self,
    charstring: CharString<'a>,
    program: &mut Program<'a>,
    depth: u8,
  ) -> Result<CharStringEnd> {
    if depth > 64 {
      return Err(CFFError);
    }

    let mut r = Reader::new(charstring);

    while !r.at_end() {
      // We need to peak instead of read because parsing a number requires
      // access to the whole buffer.
      let op = r.peak::<u8>().ok_or(Error::CFFError)?;

      // Numbers
      if matches!(op, 28 | 32..=255) {
        let number = Number::parse_char_string_number(&mut r).ok_or(CFFError)?;
        self.stack.push(number)?;
        program.push(Instruction::Operand(number));
        continue;
      }

      // No numbers can appear now, so now we can actually read it.
      let op = r.read::<u8>().ok_or(CFFError)?;
      let operator = if op == 12 {
        Operator::from_two_byte(r.read::<u8>().ok_or(CFFError)?)
      } else {
        Operator::from_one_byte(op)
      };

      match operator {
        HFLEX | FLEX | HFLEX1 | FLEX1 => {
          self.stack.pop_all();
          program.push(Instruction::Operator(operator));
        }
        HORIZONTAL_STEM | VERTICAL_STEM | HORIZONTAL_STEM_HINT_MASK | VERTICAL_STEM_HINT_MASK => {
          self.count_hints();
          program.push(Instruction::Operator(operator));
        }
        VERTICAL_MOVE_TO | HORIZONTAL_MOVE_TO | LINE_TO | VERTICAL_LINE_TO | HORIZONTAL_LINE_TO
        | MOVE_TO | CURVE_LINE | LINE_CURVE | VV_CURVE_TO | VH_CURVE_TO | HH_CURVE_TO
        | HV_CURVE_TO | CURVE_TO => {
          self.stack.pop_all();
          program.push(Instruction::Operator(operator));
        }
        RETURN => return Ok(CharStringEnd::Return),
        CALL_GLOBAL_SUBROUTINE => {
          // Pop the subroutine index from the program.
          program.0.pop();

          let biased_index = self.stack.pop().and_then(|n| n.as_i32()).ok_or(CFFError)?;
          let gsubr = self
            .gsubr_handler
            .get_with_biased(biased_index)
            .ok_or(CFFError)?;
          if matches!(
            self.decompile_inner(gsubr, program, depth + 1)?,
            CharStringEnd::EndChar
          ) {
            return Ok(CharStringEnd::EndChar);
          }
        }
        CALL_LOCAL_SUBROUTINE => {
          // Pop the subroutine index from the program.
          program.0.pop();

          let biased_index = self.stack.pop().and_then(|n| n.as_i32()).ok_or(CFFError)?;
          let lsubr = self
            .lsubr_handler
            .get_with_biased(biased_index)
            .ok_or(CFFError)?;
          if matches!(
            self.decompile_inner(lsubr, program, depth + 1)?,
            CharStringEnd::EndChar
          ) {
            return Ok(CharStringEnd::EndChar);
          }
        }
        HINT_MASK | COUNTER_MASK => {
          program.push(Instruction::Operator(operator));
          if self.hint_mask_bytes == 0 {
            // Hintmask can contain implicit stems.
            self.count_hints();
            self.hint_mask_bytes = self.hint_count.div_ceil(8);
          }

          let hint_bytes = r
            .read_bytes(self.hint_mask_bytes as usize)
            .ok_or(CFFError)?;
          program.push(Instruction::HintMask(hint_bytes));
        }
        ENDCHAR => {
          // We don't support seac for now. It's a deprecated feature and Typst for some
          // reason does not support it anyway.
          if self.stack.len() >= 4 {
            return Err(Unimplemented);
          }

          program.push(Instruction::Operator(operator));

          return Ok(CharStringEnd::EndChar);
        }
        _ => {
          return Err(CFFError);
        }
      }
    }

    Ok(CharStringEnd::Return)
  }

  fn count_hints(&mut self) {
    let elements = self.stack.pop_all();
    self.hint_count += elements.len() as u16 / 2;
  }
}

/// A type of instruction in a charstring program.
#[derive(Debug)]
pub enum Instruction<'a> {
  Operand(Number),
  Operator(Operator),
  HintMask(&'a [u8]),
}

/// A charstring program, decompiled into its constituent instructions.
#[derive(Default)]
pub struct Program<'a>(Vec<Instruction<'a>>);

impl Debug for Program<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let mut formatted_strings = vec![];
    let mut str_buffer = vec![];

    for instr in &self.0 {
      match instr {
        Instruction::Operand(op) => str_buffer.push(format!("{}", op.as_f64())),
        Instruction::Operator(op) => {
          str_buffer.push(format!("op({})", op));

          if *op != HINT_MASK && *op != COUNTER_MASK {
            formatted_strings.push(str_buffer.join(" "));
            str_buffer.clear();
          }
        }
        Instruction::HintMask(bytes) => {
          let mut byte_string = String::new();

          for byte in *bytes {
            byte_string.push_str(&format!("{:08b}", *byte));
          }

          str_buffer.push(byte_string);
          formatted_strings.push(str_buffer.join(" "));
          str_buffer.clear();
        }
      }
    }

    write!(f, "{}", formatted_strings.join("\n"))
  }
}

impl<'a> Program<'a> {
  /// Push a new instruction to the program.
  pub fn push(&mut self, instruction: Instruction<'a>) {
    self.0.push(instruction);
  }

  /// Compile the program.
  pub fn compile(&self) -> Vec<u8> {
    let mut w = Writer::new();

    for instr in &self.0 {
      match instr {
        Instruction::Operand(num) => {
          w.write(num);
        }
        Instruction::Operator(op) => {
          w.write(op);
        }
        Instruction::HintMask(hm) => {
          w.write(hm);
        }
      }
    }

    w.finish()
  }
}

#[allow(dead_code)]
pub mod operators {
  use crate::subsetter::cff::operator::Operator;

  pub const HORIZONTAL_STEM: Operator = Operator::from_one_byte(1);
  pub const VERTICAL_STEM: Operator = Operator::from_one_byte(3);
  pub const VERTICAL_MOVE_TO: Operator = Operator::from_one_byte(4);
  pub const LINE_TO: Operator = Operator::from_one_byte(5);
  pub const HORIZONTAL_LINE_TO: Operator = Operator::from_one_byte(6);
  pub const VERTICAL_LINE_TO: Operator = Operator::from_one_byte(7);
  pub const CURVE_TO: Operator = Operator::from_one_byte(8);
  pub const CALL_LOCAL_SUBROUTINE: Operator = Operator::from_one_byte(10);
  pub const RETURN: Operator = Operator::from_one_byte(11);
  pub const ENDCHAR: Operator = Operator::from_one_byte(14);
  pub const HORIZONTAL_STEM_HINT_MASK: Operator = Operator::from_one_byte(18);
  pub const HINT_MASK: Operator = Operator::from_one_byte(19);
  pub const COUNTER_MASK: Operator = Operator::from_one_byte(20);
  pub const MOVE_TO: Operator = Operator::from_one_byte(21);
  pub const HORIZONTAL_MOVE_TO: Operator = Operator::from_one_byte(22);
  pub const VERTICAL_STEM_HINT_MASK: Operator = Operator::from_one_byte(23);
  pub const CURVE_LINE: Operator = Operator::from_one_byte(24);
  pub const LINE_CURVE: Operator = Operator::from_one_byte(25);
  pub const VV_CURVE_TO: Operator = Operator::from_one_byte(26);
  pub const HH_CURVE_TO: Operator = Operator::from_one_byte(27);
  pub const SHORT_INT: Operator = Operator::from_one_byte(28);
  pub const CALL_GLOBAL_SUBROUTINE: Operator = Operator::from_one_byte(29);
  pub const VH_CURVE_TO: Operator = Operator::from_one_byte(30);
  pub const HV_CURVE_TO: Operator = Operator::from_one_byte(31);
  pub const HFLEX: Operator = Operator::from_two_byte(34);
  pub const FLEX: Operator = Operator::from_two_byte(35);
  pub const HFLEX1: Operator = Operator::from_two_byte(36);
  pub const FLEX1: Operator = Operator::from_two_byte(37);
  pub const FIXED_16_16: Operator = Operator::from_one_byte(255);
}

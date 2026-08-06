use crate::subsetter::Error::CFFError;
use crate::subsetter::Result;
use crate::subsetter::cff::number::Number;

const MAX_OPERANDS_LEN: usize = 48;

// Taken from ttf-parser.
/// TODO: Use array instead?
pub struct ArgumentsStack {
  pub data: Vec<Number>,
}

impl ArgumentsStack {
  pub fn new() -> Self {
    Self { data: vec![] }
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.data.len()
  }

  #[inline]
  pub fn push(&mut self, n: Number) -> Result<()> {
    if self.len() == MAX_OPERANDS_LEN {
      Err(CFFError)
    } else {
      self.data.push(n);
      Ok(())
    }
  }

  #[inline]
  pub fn pop(&mut self) -> Option<Number> {
    self.data.pop()
  }

  #[inline]
  pub fn pop_all(&mut self) -> Vec<Number> {
    let mut ret_vec = vec![];
    std::mem::swap(&mut self.data, &mut ret_vec);
    ret_vec
  }
}

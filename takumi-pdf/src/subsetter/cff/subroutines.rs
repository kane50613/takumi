use crate::subsetter::cff::charstring::CharString;

/// A wrapper over a vector of subroutine containers (for local subroutines, where
/// we have a list of subroutines for each font dict).
pub struct SubroutineCollection<'a> {
  subroutines: Vec<SubroutineContainer<'a>>,
}

impl<'a> SubroutineCollection<'a> {
  pub fn new(subroutines: Vec<Vec<CharString<'a>>>) -> Self {
    debug_assert!(subroutines.len() <= 255);
    Self {
      subroutines: subroutines
        .into_iter()
        .map(SubroutineContainer::new)
        .collect(),
    }
  }

  pub fn get_handler(&self, fd_index: u8) -> Option<SubroutineHandler<'_>> {
    self
      .subroutines
      .get(fd_index as usize)
      .map(|s| s.get_handler())
  }
}

/// A wrapper over a vector of charstrings (for global subroutines).
pub struct SubroutineContainer<'a> {
  subroutines: Vec<CharString<'a>>,
}

impl<'a> SubroutineContainer<'a> {
  pub fn new(subroutines: Vec<CharString<'a>>) -> Self {
    Self { subroutines }
  }

  pub fn get_handler(&self) -> SubroutineHandler<'_> {
    SubroutineHandler::new(self.subroutines.as_ref())
  }
}

/// Wrapper over a list of subroutines to allow for convenient access to subroutines
/// given a biased or unbiased index.
#[derive(Clone)]
pub struct SubroutineHandler<'a> {
  subroutines: &'a [CharString<'a>],
  bias: u16,
}

impl<'a> SubroutineHandler<'a> {
  pub fn new(char_strings: &'a [CharString<'a>]) -> Self {
    Self {
      subroutines: char_strings,
      bias: calc_subroutine_bias(char_strings.len() as u32),
    }
  }

  pub fn get_with_biased(&self, index: i32) -> Option<CharString<'a>> {
    self.get_with_unbiased(unapply_bias(index, self.bias)?)
  }

  pub fn get_with_unbiased(&self, index: u32) -> Option<CharString<'a>> {
    self.subroutines.get(index as usize).copied()
  }
}

fn calc_subroutine_bias(len: u32) -> u16 {
  if len < 1240 {
    107
  } else if len < 33900 {
    1131
  } else {
    32768
  }
}

/// Unapply the bias from a biased subroutine offset.
pub fn unapply_bias(index: i32, bias: u16) -> Option<u32> {
  let bias = i32::from(bias);

  u32::try_from(index.checked_add(bias)?).ok()
}

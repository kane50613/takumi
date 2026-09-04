use super::LonghandId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PropertyMask {
  words: [usize; Self::WORD_COUNT],
}

impl PropertyMask {
  const BITS_PER_WORD: usize = usize::BITS as usize;
  const WORD_COUNT: usize = LonghandId::COUNT.div_ceil(Self::BITS_PER_WORD);

  pub const fn new() -> Self {
    Self {
      words: [0; Self::WORD_COUNT],
    }
  }

  pub fn insert(&mut self, property: LonghandId) -> bool {
    let word_index = property.index() / Self::BITS_PER_WORD;
    let bit_index = property.index() % Self::BITS_PER_WORD;
    let bit = 1usize << bit_index;
    let word = &mut self.words[word_index];
    let was_present = (*word & bit) != 0;
    *word |= bit;
    !was_present
  }

  pub fn contains(&self, property: &LonghandId) -> bool {
    let word_index = property.index() / Self::BITS_PER_WORD;
    let bit_index = property.index() % Self::BITS_PER_WORD;
    (self.words[word_index] & (1usize << bit_index)) != 0
  }

  pub(crate) fn union(&mut self, other: &Self) {
    for (word, other_word) in self.words.iter_mut().zip(other.words) {
      *word |= other_word;
    }
  }

  pub fn iter(&self) -> PropertyMaskIter<'_> {
    PropertyMaskIter {
      mask: self,
      word_index: 0,
      current_word: if Self::WORD_COUNT > 0 {
        self.words[0]
      } else {
        0
      },
    }
  }
}

impl Default for PropertyMask {
  fn default() -> Self {
    Self::new()
  }
}

impl Extend<LonghandId> for PropertyMask {
  fn extend<T: IntoIterator<Item = LonghandId>>(&mut self, iter: T) {
    for property in iter {
      self.insert(property);
    }
  }
}

impl FromIterator<LonghandId> for PropertyMask {
  fn from_iter<T: IntoIterator<Item = LonghandId>>(iter: T) -> Self {
    let mut mask = Self::new();
    mask.extend(iter);
    mask
  }
}

pub(crate) struct PropertyMaskIter<'a> {
  mask: &'a PropertyMask,
  word_index: usize,
  current_word: usize,
}

impl Iterator for PropertyMaskIter<'_> {
  type Item = LonghandId;

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if self.current_word != 0 {
        let bit = self.current_word.trailing_zeros() as usize;
        self.current_word &= self.current_word - 1;
        let index = self.word_index * PropertyMask::BITS_PER_WORD + bit;
        if index >= LonghandId::COUNT {
          return None;
        }
        return Some(LonghandId::ALL[index]);
      }
      self.word_index += 1;
      if self.word_index >= PropertyMask::WORD_COUNT {
        return None;
      }
      self.current_word = self.mask.words[self.word_index];
    }
  }
}

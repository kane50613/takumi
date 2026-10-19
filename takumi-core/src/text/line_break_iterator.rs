//! Soft wrap opportunities from ICU4X's UAX #14 segmenter with Chromium's overrides, in place of
//! Blink's LGPL `LazyLineBreakIterator`.

use std::{ops::RangeInclusive, rc::Rc};

use icu_properties::{CodePointMapData, props::LineBreak};
use icu_segmenter::{
  GraphemeClusterSegmenter, LineSegmenter,
  options::{LineBreakOptions, LineBreakStrictness, LineBreakWordOption},
};

/// Whether a line breaks between two ASCII characters where a table overrides UAX #14.
struct AsciiLineBreakTable {
  /// For each `before` character, a bit for each `after` character the table overrides.
  overridden: [u128; 128],
  /// For each `before` character, a bit for each `after` character it breaks before.
  allow: [u128; 128],
}

impl AsciiLineBreakTable {
  const fn new() -> Self {
    Self {
      overridden: [0; 128],
      allow: [0; 128],
    }
  }

  /// Overrides every pair of a character in `before` and one in `after`, a later call winning.
  const fn with_pairs(
    mut self,
    before: RangeInclusive<char>,
    after: RangeInclusive<char>,
    allow: bool,
  ) -> Self {
    let mut mask = 0_u128;
    let mut after_char = *after.start() as u32;

    while after_char <= *after.end() as u32 {
      mask |= 1 << after_char;
      after_char += 1;
    }

    let mut before_char = *before.start() as usize;

    while before_char <= *before.end() as usize {
      self.overridden[before_char] |= mask;
      self.allow[before_char] = if allow {
        self.allow[before_char] | mask
      } else {
        self.allow[before_char] & !mask
      };
      before_char += 1;
    }

    self
  }

  const fn lookup(&self, before: char, after: char) -> Option<bool> {
    let (before, after) = (before as u32, after as u32);

    if before >= 128 || after >= 128 || self.overridden[before as usize] & (1 << after) == 0 {
      return None;
    }

    Some(self.allow[before as usize] & (1 << after) != 0)
  }
}

/// Chromium's breaks between printable ASCII characters, as parley_engine's
/// `CHROMIUM_LINE_BREAK_OVERRIDE` builds them from Chromium's `kLineBreakTable`.
static CHROMIUM_LINE_BREAK_TABLE: AsciiLineBreakTable = {
  const ALL: RangeInclusive<char> = '!'..='\u{7f}';

  AsciiLineBreakTable::new()
    .with_pairs(ALL, ALL, false)
    .with_pairs(ALL, '('..='(', true)
    .with_pairs(ALL, '<'..='<', true)
    .with_pairs(ALL, '['..='[', true)
    .with_pairs(ALL, '{'..='{', true)
    .with_pairs('-'..='-', ALL, true)
    .with_pairs('?'..='?', ALL, true)
    .with_pairs('-'..='-', '$'..='$', false)
    .with_pairs(ALL, '!'..='!', false)
    .with_pairs('?'..='?', '"'..='"', false)
    .with_pairs('?'..='?', '\''..='\'', false)
    .with_pairs(ALL, ')'..=')', false)
    .with_pairs(ALL, ','..=',', false)
    .with_pairs(ALL, '.'..='.', false)
    .with_pairs(ALL, '/'..='/', false)
    .with_pairs('-'..='-', '0'..='9', false)
    .with_pairs(ALL, ':'..=':', false)
    .with_pairs(ALL, ';'..=';', false)
    .with_pairs(ALL, '?'..='?', false)
    .with_pairs(ALL, ']'..=']', false)
    .with_pairs(ALL, '}'..='}', false)
    .with_pairs('$'..='$', ALL, false)
    .with_pairs('\''..='\'', ALL, false)
    .with_pairs('('..='(', ALL, false)
    .with_pairs('/'..='/', ALL, false)
    .with_pairs('0'..='9', ALL, false)
    .with_pairs('<'..='<', ALL, false)
    .with_pairs('@'..='@', ALL, false)
    .with_pairs('A'..='Z', ALL, false)
    .with_pairs('['..='[', ALL, false)
    .with_pairs('^'..='`', ALL, false)
    .with_pairs('a'..='z', ALL, false)
    .with_pairs('{'..='{', ALL, false)
    .with_pairs('\u{7f}'..='\u{7f}', ALL, false)
};

/// Whether Chromium breaks between `before` and `after`, `before_before` preceding them, where it
/// overrides UAX #14: after a space run, around a minus sign, and between the ASCII pairs of its
/// table.
fn chromium_line_break_override(
  before_before: Option<char>,
  before: char,
  after: char,
) -> Option<bool> {
  if before == ' ' && after != ' ' {
    return Some(true);
  }

  // A minus sign before a digit reads as a negative number unless it follows a word.
  if before == '-' && after.is_ascii_digit() {
    return Some(before_before.is_some_and(|character| character.is_ascii_alphanumeric()));
  }

  CHROMIUM_LINE_BREAK_TABLE.lookup(before, after)
}

/// Blink's `LineBreakType`, without `kPhrase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum LineBreakType {
  #[default]
  Normal,
  BreakAll,
  KeepAll,
  /// Break at every grapheme cluster boundary.
  BreakCharacter,
}

/// Blink's `BreakSpaceType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum BreakSpaceType {
  /// Break after a run of spaces.
  #[default]
  AfterSpaceRun,
  /// Break after every space, for `white-space: break-spaces`.
  AfterEverySpace,
}

/// The soft wrap opportunities of a text, as Blink's `LazyLineBreakIterator` finds them.
pub(crate) struct LineBreakIterator<'a> {
  text: &'a str,
  /// Whether a line may break before each byte, and at the end.
  breakable: Rc<[bool]>,
  break_type: LineBreakType,
  break_space: BreakSpaceType,
}

impl<'a> LineBreakIterator<'a> {
  #[cfg(test)]
  pub(crate) fn new(text: &'a str, break_type: LineBreakType, break_space: BreakSpaceType) -> Self {
    Self::with_opportunities(
      text,
      Self::opportunities(text, break_type, break_space),
      break_type,
      break_space,
    )
  }

  /// An iterator over `breakable`, the [`Self::opportunities`] of `text` it was computed for.
  pub(crate) fn with_opportunities(
    text: &'a str,
    breakable: Rc<[bool]>,
    break_type: LineBreakType,
    break_space: BreakSpaceType,
  ) -> Self {
    Self {
      text,
      breakable,
      break_type,
      break_space,
    }
  }

  /// Whether a line may break before each byte of `text`, and at its end.
  pub(crate) fn opportunities(
    text: &str,
    break_type: LineBreakType,
    break_space: BreakSpaceType,
  ) -> Rc<[bool]> {
    let mut breakable = vec![false; text.len() + 1];

    if break_type == LineBreakType::BreakCharacter {
      for position in GraphemeClusterSegmenter::new().segment_str(text) {
        breakable[position] = true;
      }
    } else {
      let mut options = LineBreakOptions::default();

      options.strictness = Some(LineBreakStrictness::Strict);
      options.word_option = Some(match break_type {
        LineBreakType::BreakAll => LineBreakWordOption::BreakAll,
        LineBreakType::KeepAll => LineBreakWordOption::KeepAll,
        LineBreakType::Normal | LineBreakType::BreakCharacter => LineBreakWordOption::Normal,
      });

      for position in LineSegmenter::new_for_non_complex_scripts(options).segment_str(text) {
        breakable[position] = true;
      }

      // Under `word-break: break-all`, Chromium's table allows breaks but no longer prevents them.
      let line_break_override = |before_before, before, after| {
        chromium_line_break_override(before_before, before, after)
          .filter(|&opportunity| opportunity || break_type != LineBreakType::BreakAll)
      };
      let line_break = CodePointMapData::<LineBreak>::new();
      let is_mandatory_break = |character: char| {
        matches!(
          line_break.get(character),
          LineBreak::MandatoryBreak
            | LineBreak::CarriageReturn
            | LineBreak::LineFeed
            | LineBreak::NextLine
        )
      };
      let mut previous: Option<(char, Option<char>)> = None;

      for (position, character) in text.char_indices() {
        let after_break_space = break_space == BreakSpaceType::AfterEverySpace
          && previous.is_some_and(|(previous, _)| matches!(previous, ' ' | '\t' | '\u{3000}'));

        // UAX #14 LB4-LB6: no soft wrap opportunity next to a mandatory break, which breaks on its
        // own.
        breakable[position] = match previous {
          Some((before, _)) if is_mandatory_break(before) || is_mandatory_break(character) => false,
          Some((before, before_before)) => line_break_override(before_before, before, character)
            .unwrap_or(breakable[position] || after_break_space),
          None => false,
        };
        previous = Some((character, previous.map(|(before, _)| before)));
      }
    }

    breakable[0] = false;
    breakable[text.len()] = true;
    breakable.into()
  }

  /// Blink's `BreakType`.
  pub(crate) fn break_type(&self) -> LineBreakType {
    self.break_type
  }

  /// Blink's `GetString`.
  pub(crate) fn text(&self) -> &'a str {
    self.text
  }

  /// Blink's `BreakSpace`.
  pub(crate) fn break_space(&self) -> BreakSpaceType {
    self.break_space
  }

  /// Blink's `IsBreakableSpace`.
  pub(crate) fn is_breakable_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n')
  }

  /// Blink's `IsBreakable`: whether a line may break before `offset`.
  pub(crate) fn is_breakable(&self, offset: usize) -> bool {
    self.breakable.get(offset).copied().unwrap_or(false)
  }

  /// Blink's `NextBreakOpportunity`: the first break at or after `offset`, or `len`.
  pub(crate) fn next_break_opportunity(&self, offset: usize, len: usize) -> usize {
    (offset..len)
      .find(|&position| self.is_breakable(position))
      .unwrap_or(len)
  }

  /// Blink's `PreviousBreakOpportunity`: the last break at or before `offset` and after `min`, or
  /// `min`.
  pub(crate) fn previous_break_opportunity(&self, offset: usize, min: usize) -> usize {
    let position = offset.min(self.text.len());

    ((min + 1)..=position)
      .rev()
      .find(|&position| self.is_breakable(position))
      .unwrap_or(min)
  }
}

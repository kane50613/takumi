//! Soft wrap opportunities from ICU4X's UAX #14 segmenter with Chromium's overrides, in place of
//! Blink's LGPL `LazyLineBreakIterator`.

use std::{ops::RangeInclusive, rc::Rc};

use icu_segmenter::{
  GraphemeClusterSegmenter, LineSegmenter,
  options::{LineBreakOptions, LineBreakStrictness as IcuStrictness, LineBreakWordOption},
};

use crate::text::character::{SOFT_HYPHEN, is_mandatory_break};

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

/// Whether Chromium breaks between `before` and `after` where it overrides UAX #14.
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

/// Blink's `LineBreakStrictness`, from `line-break`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum LineBreakStrictness {
  /// ICU's default rules, which are UAX #14's strict ones.
  #[default]
  Default,
  Normal,
  Strict,
  Loose,
}

/// How a [`LineBreakIterator`] finds its opportunities: Blink's break type, break space,
/// strictness, and soft hyphen switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct BreakOptions {
  pub(crate) break_type: LineBreakType,
  pub(crate) break_space: BreakSpaceType,
  pub(crate) strictness: LineBreakStrictness,
  /// Blink's `EnableSoftHyphen(false)`, from `hyphens: none`: U+00AD neither breaks nor hyphenates.
  pub(crate) disable_soft_hyphen: bool,
}

/// The soft wrap opportunities of a text, as Blink's `LazyLineBreakIterator` finds them.
pub(crate) struct LineBreakIterator<'a> {
  text: &'a str,
  /// Whether a line may break before each byte, and at the end.
  breakable: Rc<[bool]>,
  pub(crate) options: BreakOptions,
}

impl<'a> LineBreakIterator<'a> {
  #[cfg(test)]
  pub(crate) fn new(text: &'a str, break_type: LineBreakType, break_space: BreakSpaceType) -> Self {
    let options = BreakOptions {
      break_type,
      break_space,
      ..BreakOptions::default()
    };

    Self::with_opportunities(text, Self::opportunities(text, options), options)
  }

  /// An iterator over `breakable`, the [`Self::opportunities`] of `text` under `options`.
  pub(crate) fn with_opportunities(
    text: &'a str,
    breakable: Rc<[bool]>,
    options: BreakOptions,
  ) -> Self {
    Self {
      text,
      breakable,
      options,
    }
  }

  /// Whether a line may break before each byte of `text`, and at its end.
  pub(crate) fn opportunities(text: &str, options: BreakOptions) -> Rc<[bool]> {
    let BreakOptions {
      break_type,
      break_space,
      strictness,
      disable_soft_hyphen,
    } = options;
    let mut breakable = vec![false; text.len() + 1];

    if break_type == LineBreakType::BreakCharacter {
      for position in GraphemeClusterSegmenter::new().segment_str(text) {
        breakable[position] = true;
      }
    } else {
      let mut options = LineBreakOptions::default();

      options.strictness = Some(match strictness {
        LineBreakStrictness::Default | LineBreakStrictness::Strict => IcuStrictness::Strict,
        LineBreakStrictness::Normal => IcuStrictness::Normal,
        LineBreakStrictness::Loose => IcuStrictness::Loose,
      });
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
      let mut previous: Option<(char, Option<char>)> = None;

      for (position, character) in text.char_indices() {
        let after_break_space = break_space == BreakSpaceType::AfterEverySpace
          && previous.is_some_and(|(previous, _)| matches!(previous, ' ' | '\t' | '\u{3000}'));

        // UAX #14 LB4-LB6: no soft wrap opportunity next to a mandatory break, which breaks on its
        // own.
        breakable[position] = match previous {
          Some((before, _)) if is_mandatory_break(before) || is_mandatory_break(character) => false,
          Some((SOFT_HYPHEN, _)) if disable_soft_hyphen => false,
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
    self.options.break_type
  }

  /// Blink's `GetString`.
  pub(crate) fn text(&self) -> &'a str {
    self.text
  }

  /// Blink's `BreakSpace`.
  pub(crate) fn break_space(&self) -> BreakSpaceType {
    self.options.break_space
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

#[cfg(test)]
mod tests {
  use super::{BreakOptions, LineBreakIterator, LineBreakStrictness};

  fn breaks(text: &str, options: BreakOptions) -> Vec<usize> {
    let opportunities = LineBreakIterator::opportunities(text, options);

    (1..text.len())
      .filter(|&offset| opportunities[offset])
      .collect()
  }

  /// `line-break: strict` keeps a small kana with the character before it, as UAX #14 classes it
  /// `CJ`; `normal` and `loose` break before it.
  #[test]
  fn strictness_decides_a_break_before_small_kana() {
    let breaks_with = |strictness| {
      breaks(
        "あぁい",
        BreakOptions {
          strictness,
          ..BreakOptions::default()
        },
      )
    };

    assert_eq!(breaks_with(LineBreakStrictness::Default), [6]);
    assert_eq!(breaks_with(LineBreakStrictness::Strict), [6]);
    assert_eq!(breaks_with(LineBreakStrictness::Normal), [3, 6]);
    assert_eq!(breaks_with(LineBreakStrictness::Loose), [3, 6]);
  }

  /// `hyphens: none` takes away the break after a soft hyphen.
  #[test]
  fn a_disabled_soft_hyphen_does_not_break() {
    let text = "ab\u{AD}cd ef";

    assert_eq!(breaks(text, BreakOptions::default()), [4, 7]);
    assert_eq!(
      breaks(
        text,
        BreakOptions {
          disable_soft_hyphen: true,
          ..BreakOptions::default()
        }
      ),
      [7]
    );
  }
}

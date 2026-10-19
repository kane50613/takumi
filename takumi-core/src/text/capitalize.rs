//! Blink's [`Capitalize`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/capitalize.cc),
//! reading ICU4X. Its word breaks skip the dictionaries of complex scripts, which have no case.

use icu_casemap::CaseMapper;
use icu_segmenter::{WordSegmenter, options::WordBreakInvariantOptions};

use crate::text::character::NO_BREAK_SPACE;

/// Blink's `CapitalizeWithPreviousCodePoint`: titlecases the first character of each word.
/// `previous_character` ends the text before, so a word that continues across texts keeps its case.
pub(crate) fn capitalize(string: &str, previous_character: char) -> String {
  // ICU no longer breaks words at a no-break space.
  let as_space = |character| {
    if character == NO_BREAK_SPACE {
      ' '
    } else {
      character
    }
  };
  let string_with_previous: String = std::iter::once(as_space(previous_character))
    .chain(string.chars().map(as_space))
    .collect();
  let case_mapper = CaseMapper::new();
  let mut boundaries =
    WordSegmenter::new_for_non_complex_scripts(WordBreakInvariantOptions::default())
      .segment_str(&string_with_previous)
      .peekable();
  let mut result = String::with_capacity(string.len());

  for ((offset, _), character) in string_with_previous
    .char_indices()
    .skip(1)
    .zip(string.chars())
  {
    while boundaries.next_if(|&boundary| boundary < offset).is_some() {}

    // Blink titlecases a UTF-16 code unit, which leaves a supplementary character as it is.
    if boundaries.next_if_eq(&offset).is_some()
      && character != NO_BREAK_SPACE
      && u32::from(character) <= 0xFFFF
    {
      result.push(case_mapper.simple_titlecase(character));
    } else {
      result.push(character);
    }
  }

  result
}

#[cfg(test)]
mod tests {
  use super::capitalize;

  /// Blink's `CapitalizeTest` data.
  #[test]
  fn capitalize_test_data() {
    assert_eq!(capitalize("", ' '), "");
    assert_eq!(capitalize("hello, world", ' '), "Hello, World");
    assert_eq!(capitalize("bc def", '\u{1D400}'), "bc Def");
  }

  /// Capitalizing leaves the rest of each word's case, and keeps a word whole across an apostrophe.
  #[test]
  fn capitalize_only_the_first_character() {
    assert_eq!(capitalize("hELLO wORLD", ' '), "HELLO WORLD");
    assert_eq!(capitalize("can't stop", ' '), "Can't Stop");
    assert_eq!(capitalize("123abc 456", ' '), "123abc 456");
  }

  /// A word that continues the text before keeps its case.
  #[test]
  fn capitalize_after_the_previous_character() {
    assert_eq!(capitalize("hello", 'a'), "hello");
    assert_eq!(capitalize("hello", '\u{A0}'), "Hello");
  }

  /// A no-break space separates words and stays.
  #[test]
  fn capitalize_after_a_no_break_space() {
    assert_eq!(
      capitalize("\u{A0}hello\u{A0}world", ' '),
      "\u{A0}Hello\u{A0}World"
    );
  }

  /// The titlecase of a digraph is its own letter, and a supplementary letter keeps its case, as
  /// Blink titlecases a UTF-16 code unit.
  #[test]
  fn capitalize_titlecases_a_code_unit() {
    assert_eq!(capitalize("\u{1C6}emal", ' '), "\u{1C5}emal");
    assert_eq!(capitalize("\u{10428}bc", ' '), "\u{10428}bc");
  }
}

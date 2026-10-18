//! Blink's [`SmallCapsIterator`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/small_caps_iterator.cc).

use icu_properties::{
  CodePointMapData, CodePointSetData,
  props::{CanonicalCombiningClass, ChangesWhenUppercased},
};

/// Blink's `SmallCapsBehavior`: whether a run's letters change when uppercased.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SmallCapsBehavior {
  Invalid,
  UppercaseNeeded,
  SameCase,
}

/// Blink's `SmallCapsIterator`: splits text where its letters switch between needing uppercasing
/// and not. A combining mark stays with the letter before it.
pub(crate) struct SmallCapsIterator<'t> {
  text: &'t str,
  offset: usize,
  at_end: bool,
  previous_small_caps_behavior: SmallCapsBehavior,
  current_small_caps_behavior: SmallCapsBehavior,
}

impl<'t> SmallCapsIterator<'t> {
  pub(crate) fn new(text: &'t str) -> Self {
    Self {
      text,
      offset: 0,
      at_end: text.is_empty(),
      previous_small_caps_behavior: SmallCapsBehavior::Invalid,
      current_small_caps_behavior: SmallCapsBehavior::Invalid,
    }
  }

  /// Blink's `Consume`: the end of the next run, in bytes, and its behavior.
  pub(crate) fn consume(&mut self) -> Option<(usize, SmallCapsBehavior)> {
    if self.at_end {
      return None;
    }

    let combining_class = CodePointMapData::<CanonicalCombiningClass>::new();
    let changes_when_uppercased = CodePointSetData::new::<ChangesWhenUppercased>();

    while let Some(ch) = self.text[self.offset..].chars().next() {
      self.previous_small_caps_behavior = self.current_small_caps_behavior;

      // Combining marks combine with the uppercased letters too.
      if combining_class.get(ch) == CanonicalCombiningClass::NotReordered {
        self.current_small_caps_behavior = if changes_when_uppercased.contains(ch) {
          SmallCapsBehavior::UppercaseNeeded
        } else {
          SmallCapsBehavior::SameCase
        };
      }

      if self.previous_small_caps_behavior != self.current_small_caps_behavior
        && self.previous_small_caps_behavior != SmallCapsBehavior::Invalid
      {
        return Some((self.offset, self.previous_small_caps_behavior));
      }

      self.offset += ch.len_utf8();
    }

    self.at_end = true;

    Some((self.text.len(), self.current_small_caps_behavior))
  }
}

#[cfg(test)]
mod tests {
  use super::{SmallCapsBehavior, SmallCapsIterator};

  use SmallCapsBehavior::{SameCase, UppercaseNeeded};

  /// Blink's `SmallCapsIteratorTest::CheckRuns`.
  fn check_runs(runs: &[(&str, SmallCapsBehavior)]) {
    let text: String = runs.iter().map(|(text, _)| *text).collect();
    let mut limit = 0;
    let expected: Vec<(usize, SmallCapsBehavior)> = runs
      .iter()
      .map(|&(text, behavior)| {
        limit += text.len();

        (limit, behavior)
      })
      .collect();
    let mut iterator = SmallCapsIterator::new(&text);
    let actual: Vec<_> = std::iter::from_fn(|| iterator.consume()).collect();

    assert_eq!(actual, expected);
  }

  #[test]
  fn empty() {
    assert_eq!(SmallCapsIterator::new("").consume(), None);
  }

  #[test]
  fn uppercase_a() {
    check_runs(&[("A", SameCase)]);
  }

  #[test]
  fn lowercase_a() {
    check_runs(&[("a", UppercaseNeeded)]);
  }

  #[test]
  fn uppercase_lowercase_a() {
    check_runs(&[("A", SameCase), ("a", UppercaseNeeded)]);
  }

  #[test]
  fn uppercase_punctuation_mixed() {
    check_runs(&[("AAA??", SameCase)]);
  }

  #[test]
  fn lowercase_punctuation_mixed() {
    check_runs(&[("aaa", UppercaseNeeded), ("===", SameCase)]);
  }

  #[test]
  fn lowercase_punctuation_interleaved() {
    check_runs(&[
      ("aaa", UppercaseNeeded),
      ("===", SameCase),
      ("bbb", UppercaseNeeded),
    ]);
  }

  #[test]
  fn japanese() {
    check_runs(&[("\u{307B}\u{3078}\u{3068}", SameCase)]);
  }

  #[test]
  fn armenian() {
    check_runs(&[
      ("\u{561}\u{562}\u{563}\u{564}", UppercaseNeeded),
      ("\u{535}\u{536}\u{537}\u{538}", SameCase),
    ]);
  }

  #[test]
  fn combining_character_sequence() {
    check_runs(&[("e\u{300}u\u{308}", UppercaseNeeded)]);
  }
}

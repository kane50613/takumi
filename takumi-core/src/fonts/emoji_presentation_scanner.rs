//! The emoji presentation grammar of
//! [emoji-segmenter](https://github.com/google/emoji-segmenter/blob/955936be8b391e00835257059607d7c5b72ce744/emoji_presentation_scanner.rl),
//! at the revision Chromium pins. Follows it under the Apache License 2.0 in
//! LICENSE-EMOJI-SEGMENTER.
//!
//! Blink compiles the grammar with Ragel. This matches each token rule by hand and takes the
//! longest match, the earlier rule on a tie, as a Ragel scanner does.

use crate::text::emoji_segmentation_category::EmojiSegmentationCategory as Category;

/// A token of the grammar: its length in characters, whether it shows as emoji, and whether a
/// variation selector chose that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EmojiToken {
  pub(crate) length: usize,
  pub(crate) is_emoji: bool,
  pub(crate) has_vs: bool,
}

/// `scan_emoji_presentation`: the token at the start of `categories`, or an empty one at the end.
pub(crate) fn scan_emoji_presentation(categories: &[Category]) -> EmojiToken {
  let grammar = Grammar(categories);
  // `text_and_emoji_run`, its rules in priority order.
  let rules = [
    (grammar.text_emoji_run_with_vs(), false, true),
    (grammar.emoji_run_with_vs(), true, true),
    (grammar.emoji_run(), true, false),
    (grammar.text_run(), false, false),
  ];
  let mut token = EmojiToken {
    length: 0,
    is_emoji: false,
    has_vs: false,
  };

  for (length, is_emoji, has_vs) in rules {
    if let Some(length) = length
      && length > token.length
    {
      token = EmojiToken {
        length,
        is_emoji,
        has_vs,
      };
    }
  }

  token
}

/// The rules of the grammar, each the length of its longest match at the start, if any.
struct Grammar<'a>(&'a [Category]);

impl Grammar<'_> {
  fn at(&self, index: usize) -> Option<Category> {
    self.0.get(index).copied()
  }

  fn is(&self, index: usize, category: Category) -> bool {
    self.at(index) == Some(category)
  }

  /// `any_emoji`.
  fn any_emoji(&self, index: usize) -> bool {
    matches!(
      self.at(index),
      Some(
        Category::EmojiTextPresentation
          | Category::EmojiEmojiPresentation
          | Category::KeycapBase
          | Category::EmojiModifierBase
          | Category::TagBase
          | Category::Emoji
      )
    )
  }

  /// `emoji_combining_enclosing_circle_backslash_sequence = any_emoji
  /// COMBINING_ENCLOSING_CIRCLE_BACKSLASH`.
  fn emoji_combining_enclosing_circle_backslash_sequence(&self, index: usize) -> Option<usize> {
    (self.any_emoji(index) && self.is(index + 1, Category::CombiningEnclosingCircleBackslash))
      .then_some(2)
  }

  /// `emoji_presentation_sequence = any_emoji VS16`.
  fn emoji_presentation_sequence(&self, index: usize) -> Option<usize> {
    (self.any_emoji(index) && self.is(index + 1, Category::Vs16)).then_some(2)
  }

  /// `emoji_modifier_sequence = EMOJI_MODIFIER_BASE EMOJI_MODIFIER`.
  fn emoji_modifier_sequence(&self, index: usize) -> Option<usize> {
    (self.is(index, Category::EmojiModifierBase) && self.is(index + 1, Category::EmojiModifier))
      .then_some(2)
  }

  /// `emoji_flag_sequence = REGIONAL_INDICATOR REGIONAL_INDICATOR`.
  fn emoji_flag_sequence(&self, index: usize) -> Option<usize> {
    (self.is(index, Category::RegionalIndicator) && self.is(index + 1, Category::RegionalIndicator))
      .then_some(2)
  }

  /// `emoji_tag_sequence = TAG_BASE TAG_SEQUENCE+ TAG_TERM`.
  fn emoji_tag_sequence(&self, index: usize) -> Option<usize> {
    if !self.is(index, Category::TagBase) {
      return None;
    }

    let mut end = index + 1;

    while self.is(end, Category::TagSequence) {
      end += 1;
    }

    (end > index + 1 && self.is(end, Category::TagTerm)).then_some(end + 1 - index)
  }

  /// `emoji_keycap_sequence = KEYCAP_BASE VS16 COMBINING_ENCLOSING_KEYCAP`.
  fn emoji_keycap_sequence(&self, index: usize) -> Option<usize> {
    self.keycap_sequence(index, Category::Vs16)
  }

  /// `text_emoji_keycap_sequence = KEYCAP_BASE VS15 COMBINING_ENCLOSING_KEYCAP`.
  fn text_emoji_keycap_sequence(&self, index: usize) -> Option<usize> {
    self.keycap_sequence(index, Category::Vs15)
  }

  fn keycap_sequence(&self, index: usize, selector: Category) -> Option<usize> {
    (self.is(index, Category::KeycapBase)
      && self.is(index + 1, selector)
      && self.is(index + 2, Category::CombiningEnclosingKeycap))
    .then_some(3)
  }

  /// The ends of `emoji_zwj_element = emoji_presentation_sequence | emoji_modifier_sequence |
  /// any_emoji` at `index`.
  fn emoji_zwj_element_ends(&self, index: usize) -> impl Iterator<Item = usize> {
    [
      self.emoji_presentation_sequence(index),
      self.emoji_modifier_sequence(index),
      self.any_emoji(index).then_some(1),
    ]
    .into_iter()
    .flatten()
    .map(move |length| index + length)
  }

  /// `emoji_zwj_sequence = emoji_zwj_element ( ZWJ emoji_zwj_element )+`.
  fn emoji_zwj_sequence(&self, index: usize) -> Option<usize> {
    let mut ends: Vec<usize> = self.emoji_zwj_element_ends(index).collect();
    let mut longest = None;

    while !ends.is_empty() {
      let mut next_ends: Vec<usize> = ends
        .iter()
        .filter(|&&end| self.is(end, Category::Zwj))
        .flat_map(|&end| self.emoji_zwj_element_ends(end + 1))
        .collect();

      next_ends.sort_unstable();
      next_ends.dedup();
      longest = next_ends.last().copied().max(longest);
      ends = next_ends;
    }

    longest.map(|end| end - index)
  }

  /// `emoji_run = emoji_presentation`.
  fn emoji_run(&self) -> Option<usize> {
    let single = matches!(
      self.at(0),
      Some(Category::EmojiEmojiPresentation | Category::TagBase | Category::EmojiModifierBase)
    );

    [
      single.then_some(1),
      self.emoji_presentation_sequence(0),
      self.emoji_modifier_sequence(0),
      self.emoji_flag_sequence(0),
      self.emoji_tag_sequence(0),
      self.emoji_keycap_sequence(0),
      self.emoji_zwj_sequence(0),
      self.emoji_combining_enclosing_circle_backslash_sequence(0),
    ]
    .into_iter()
    .flatten()
    .max()
  }

  /// `text_emoji_run_with_vs = any_emoji VS15 | text_emoji_keycap_sequence`.
  fn text_emoji_run_with_vs(&self) -> Option<usize> {
    let with_vs15 = (self.any_emoji(0) && self.is(1, Category::Vs15)).then_some(2);

    with_vs15.max(self.text_emoji_keycap_sequence(0))
  }

  /// `emoji_run_with_vs = emoji_presentation_sequence | emoji_keycap_sequence`.
  fn emoji_run_with_vs(&self) -> Option<usize> {
    self
      .emoji_presentation_sequence(0)
      .max(self.emoji_keycap_sequence(0))
  }

  /// `text_run = any`.
  fn text_run(&self) -> Option<usize> {
    (!self.0.is_empty()).then_some(1)
  }
}

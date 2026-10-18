//! Blink's `SymbolsIterator`, which splits text into runs of emoji and text presentation, after
//! [`symbols_iterator.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/symbols_iterator.cc).
//! Follows Blink under the notice in LICENSE-CHROMIUM.
//!
//! Offsets count UTF-8 bytes where Blink's count UTF-16 code units.

use super::{
  emoji_presentation_scanner::{EmojiToken, scan_emoji_presentation},
  font_fallback_priority::FontFallbackPriority,
};
use crate::text::{
  character::maybe_emoji_presentation, emoji_segmentation_category::EmojiSegmentationCategory,
};

/// Blink's `SymbolsIterator`. Positions count characters until they leave as byte offsets.
pub(crate) struct SymbolsIterator {
  /// The byte offset of each character, then of the end.
  offsets: Vec<usize>,
  characters: Vec<char>,
  categories: Vec<EmojiSegmentationCategory>,
  /// Blink's `buffer_iterator_`, which may trail `cursor`.
  buffer_iterator: usize,
  cursor: usize,
  next_token_end: usize,
  next_token_emoji: bool,
  next_token_has_vs: bool,
}

impl SymbolsIterator {
  pub(crate) fn new(text: &str) -> Self {
    let (mut offsets, characters): (Vec<usize>, Vec<char>) = text.char_indices().unzip();

    offsets.push(text.len());

    let categories = characters
      .iter()
      .map(|&ch| EmojiSegmentationCategory::of(ch))
      .collect();
    let mut iterator = Self {
      offsets,
      characters,
      categories,
      buffer_iterator: 0,
      cursor: 0,
      next_token_end: 0,
      next_token_emoji: false,
      next_token_has_vs: false,
    };

    if !iterator.characters.is_empty() {
      iterator.scan_next_token();
    }

    iterator
  }

  /// Blink's `Consume`: the end of the next run and the fonts it falls back through.
  pub(crate) fn consume(&mut self) -> Option<(usize, FontFallbackPriority)> {
    let end = self.characters.len();

    if self.cursor >= end {
      return None;
    }

    let mut current_token_emoji;
    let mut current_has_vs;

    loop {
      self.cursor = self.next_token_end;
      current_token_emoji = self.next_token_emoji;
      current_has_vs = self.next_token_has_vs;

      if self.cursor >= end {
        break;
      }

      if !current_token_emoji && !maybe_emoji_presentation(self.characters[self.buffer_iterator]) {
        self.buffer_iterator += 1;
        self.next_token_end = self.buffer_iterator;
        self.next_token_has_vs = false;
      } else {
        self.buffer_iterator = self.cursor;
        self.scan_next_token();
      }

      if current_token_emoji != self.next_token_emoji || current_has_vs != self.next_token_has_vs {
        break;
      }
    }

    let font_fallback_priority = match (current_token_emoji, current_has_vs) {
      (true, true) => FontFallbackPriority::EmojiEmojiWithVs,
      (false, true) => FontFallbackPriority::EmojiTextWithVs,
      (true, false) => FontFallbackPriority::EmojiEmoji,
      (false, false) => FontFallbackPriority::Text,
    };

    Some((self.offsets[self.cursor], font_fallback_priority))
  }

  /// Scans the token at `buffer_iterator`, which ends the next run.
  fn scan_next_token(&mut self) {
    let EmojiToken {
      length,
      is_emoji,
      has_vs,
    } = scan_emoji_presentation(&self.categories[self.buffer_iterator..]);

    self.next_token_end = self.buffer_iterator + length;
    self.next_token_emoji = is_emoji;
    self.next_token_has_vs = has_vs;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Blink's `SymbolsIteratorTest::CheckRuns`.
  fn check_runs(runs: &[(&str, FontFallbackPriority)]) {
    let mut text = String::new();
    let mut expect = Vec::new();

    for &(run, priority) in runs {
      text.push_str(run);
      expect.push((text.len(), priority));
    }

    let mut iterator = SymbolsIterator::new(&text);
    let mut actual = Vec::new();

    while let Some(run) = iterator.consume() {
      actual.push(run);
    }

    assert_eq!(actual, expect);
  }

  #[test]
  fn empty() {
    assert_eq!(SymbolsIterator::new("").consume(), None);
  }

  #[test]
  fn space() {
    check_runs(&[(" ", FontFallbackPriority::Text)]);
  }

  #[test]
  fn latin() {
    check_runs(&[("Aa", FontFallbackPriority::Text)]);
  }

  #[test]
  fn bmp_emoji() {
    check_runs(&[(
      "\u{231A}\u{231B}\u{231A}\u{231B}\u{231A}\u{231B}\u{231A}\u{231B}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn latin_color_emoji_text_emoji() {
    check_runs(&[
      ("a", FontFallbackPriority::Text),
      ("\u{231A}", FontFallbackPriority::EmojiEmoji),
      ("\u{260E}", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn ignore_vs_in_math() {
    check_runs(&[(
      "\u{2286}\u{2287}\u{2288}\u{FE0E}\u{2299}\u{229A}\u{229A}",
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn ignore_vs15_in_text() {
    check_runs(&[("abcdef\u{FE0E}ghji", FontFallbackPriority::Text)]);
  }

  #[test]
  fn ignore_vs16_in_text() {
    check_runs(&[("abcdef\u{FE0F}ghji", FontFallbackPriority::Text)]);
  }

  #[test]
  fn all_hex_values_text() {
    check_runs(&[("abcdef0123456789ABCDEF", FontFallbackPriority::Text)]);
  }

  #[test]
  fn numbers_and_hash_normal_and_emoji() {
    check_runs(&[
      ("0123456789#*", FontFallbackPriority::Text),
      (
        "0\u{FE0F}\u{20E3}1\u{FE0F}\u{20E3}2\u{FE0F}\u{20E3}3\u{FE0F}\u{20E3}4\u{FE0F}\u{20E3}5\u{FE0F}\u{20E3}6\u{FE0F}\u{20E3}7\u{FE0F}\u{20E3}8\u{FE0F}\u{20E3}9\u{FE0F}\u{20E3}*\u{FE0F}\u{20E3}",
        FontFallbackPriority::EmojiEmojiWithVs,
      ),
      ("0123456789#*", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn vs16on_digits() {
    check_runs(&[
      ("#", FontFallbackPriority::Text),
      ("#\u{FE0F}\u{20E3}", FontFallbackPriority::EmojiEmojiWithVs),
      ("#", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn vs15on_digits() {
    check_runs(&[
      ("#", FontFallbackPriority::Text),
      ("#\u{FE0E}\u{20E3}", FontFallbackPriority::EmojiTextWithVs),
      ("#", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn emoji_vs15_and_vs16() {
    check_runs(&[
      ("\u{1F642}", FontFallbackPriority::EmojiEmoji),
      ("\u{1F642}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{1F642}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
    ]);
  }

  #[test]
  fn emoji_vs_split_starts_with_text() {
    check_runs(&[
      ("abc", FontFallbackPriority::Text),
      ("\u{2614}", FontFallbackPriority::EmojiEmoji),
      ("\u{2603}", FontFallbackPriority::Text),
      ("\u{2614}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
    ]);
  }

  #[test]
  fn emoji_vs_split_starts_with_emoji() {
    check_runs(&[
      ("\u{2614}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("abc", FontFallbackPriority::Text),
      ("\u{2614}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{2614}", FontFallbackPriority::EmojiEmoji),
      ("\u{2614}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
    ]);
  }

  #[test]
  fn emoji_vs_split_with_excessive_vs() {
    check_runs(&[
      ("\u{2603}", FontFallbackPriority::Text),
      ("\u{2603}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{2614}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
      ("\u{2614}", FontFallbackPriority::EmojiEmoji),
    ]);
  }

  #[test]
  fn emoji_and_text_vs_split() {
    check_runs(&[
      ("\u{2603}", FontFallbackPriority::Text),
      ("\u{2603}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
      ("\u{2614}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{2614}", FontFallbackPriority::EmojiEmoji),
    ]);
  }

  #[test]
  fn emoji_text_vs_split() {
    check_runs(&[
      ("\u{2614}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
      ("a\u{2603}bc", FontFallbackPriority::Text),
      (
        "\u{2603}\u{FE0E}\u{2614}\u{FE0E}",
        FontFallbackPriority::EmojiTextWithVs,
      ),
    ]);
  }

  #[test]
  fn zwj_with_vs() {
    check_runs(&[
      ("abc", FontFallbackPriority::Text),
      (
        "\u{26D3}\u{FE0F}\u{200D}\u{1F4A5}\u{FE0F}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}\u{26D3}\u{FE0F}\u{200D}\u{1F4A5}",
        FontFallbackPriority::EmojiEmoji,
      ),
      ("\u{FE0E}", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn multiple_misplaced_vs() {
    check_runs(&[
      ("\u{FE0E}\u{FE0F}", FontFallbackPriority::Text),
      ("\u{1F642}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
      ("\u{1F642}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{1F642}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
      ("\u{FE0E}\u{FE0F}", FontFallbackPriority::Text),
      ("\u{1F642}\u{FE0F}", FontFallbackPriority::EmojiEmojiWithVs),
      ("\u{1F642}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{FE0E}\u{FE0F}", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn single_flag() {
    check_runs(&[("\u{1F1FA}", FontFallbackPriority::Text)]);
  }

  #[test]
  fn combining_circle() {
    check_runs(&[(
      "\u{25CC}\u{301}\u{25CC}\u{300}\u{25CC}\u{308}\u{25CC}\u{302}\u{25CC}\u{304}\u{25CC}\u{30A}",
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn combining_enclosing_circle_backslash() {
    check_runs(&[
      ("A\u{20E0}B\u{20E0}C\u{20E0}", FontFallbackPriority::Text),
      (
        "\u{1F6B7}\u{1F6AF}\u{1F6B1}\u{1F51E}\u{1F4F5}\u{1F6AD}\u{1F6AB}\u{1F399}\u{20E0}\u{1F4F8}\u{20E0}\u{1F52B}\u{20E0}",
        FontFallbackPriority::EmojiEmoji,
      ),
      ("a\u{20E0}b\u{20E0}c\u{20E0}", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn flags_vs_non_flags() {
    check_runs(&[
      ("\u{1F1FA}\u{1F1F8}", FontFallbackPriority::EmojiEmoji),
      ("\u{1F1F8}abc", FontFallbackPriority::Text),
      ("\u{1F1FA}\u{1F1F8}", FontFallbackPriority::EmojiEmoji),
      ("a\u{1F1FF}", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn emoji_vs15() {
    check_runs(&[
      ("\u{2693}\u{FE0E}", FontFallbackPriority::EmojiTextWithVs),
      ("\u{26F5}", FontFallbackPriority::EmojiEmoji),
    ]);
  }

  #[test]
  fn emoji_zws_sequences() {
    check_runs(&[
      (
        "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}",
        FontFallbackPriority::EmojiEmoji,
      ),
      ("abcd", FontFallbackPriority::Text),
      (
        "\u{1F469}\u{200D}\u{1F469}",
        FontFallbackPriority::EmojiEmoji,
      ),
      ("\u{200D}efgh", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn all_emoji_zws_sequences() {
    check_runs(&[(
      "\u{1F48F}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}\u{1F468}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F469}\u{1F491}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}\u{1F468}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F469}\u{1F46A}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F466}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F466}\u{200D}\u{1F466}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F467}\u{1F468}\u{200D}\u{1F468}\u{200D}\u{1F466}\u{1F468}\u{200D}\u{1F468}\u{200D}\u{1F467}\u{1F468}\u{200D}\u{1F468}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{1F468}\u{200D}\u{1F468}\u{200D}\u{1F466}\u{200D}\u{1F466}\u{1F468}\u{200D}\u{1F468}\u{200D}\u{1F467}\u{200D}\u{1F467}\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F466}\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F466}\u{200D}\u{1F466}\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F467}\u{1F441}\u{200D}\u{1F5E8}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn modifier_plus_gender() {
    check_runs(&[(
      "\u{26F9}\u{1F3FB}\u{200D}\u{2642}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn text_member_zwj_sequence() {
    check_runs(&[(
      "\u{1F468}\u{200D}\u{2695}\u{FE0F}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn facepalm_cartwheel_shrug_modifier_female() {
    check_runs(&[(
      "\u{1F926}\u{200D}\u{2640}\u{1F938}\u{200D}\u{2640}\u{1F937}\u{200D}\u{2640}\u{1F937}\u{1F3FE}\u{200D}\u{2640}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn aesculapius_male_femal_emoji() {
    check_runs(&[("a\u{2695}\u{2640}\u{2642}", FontFallbackPriority::Text)]);
  }

  #[test]
  fn eye_speech_bubble() {
    check_runs(&[(
      "\u{1F441}\u{200D}\u{1F5E8}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn modifier() {
    check_runs(&[("\u{1F476}\u{1F3FF}", FontFallbackPriority::EmojiEmoji)]);
  }

  #[test]
  fn dingbats_misc_symbols_modifier() {
    check_runs(&[(
      "\u{26F9}\u{1F3FB}\u{270D}\u{1F3FB}\u{270A}\u{1F3FC}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn extra_zwj_prefix() {
    check_runs(&[
      ("\u{200D}", FontFallbackPriority::Text),
      (
        "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}",
        FontFallbackPriority::EmojiEmoji,
      ),
    ]);
  }

  #[test]
  fn stray_zwj_and_vs() {
    check_runs(&[
      (
        "\u{200D}\u{FE0E}\u{FE0E}\u{FE0E}\u{200D}\u{200D}",
        FontFallbackPriority::Text,
      ),
      (
        "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}",
        FontFallbackPriority::EmojiEmoji,
      ),
      (
        "\u{200D}\u{FE0E}\u{FE0E}\u{FE0E}\u{200D}\u{200D}",
        FontFallbackPriority::Text,
      ),
    ]);
  }

  #[test]
  fn arrows() {
    check_runs(&[(
      "x\u{2192}\u{2190}x\u{2190}\u{2191}\u{2193}\u{2192}",
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn judge_pilot() {
    check_runs(&[(
      "\u{1F468}\u{200D}\u{2696}\u{FE0F}\u{1F469}\u{200D}\u{2696}\u{FE0F}\u{1F468}\u{1F3FC}\u{200D}\u{2696}\u{FE0F}\u{1F469}\u{1F3FC}\u{200D}\u{2696}\u{FE0F}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn emoji_punctuation_text() {
    check_runs(&[(
      "\u{2049}\u{2049}\u{2049}\u{2048}\u{2048}\u{2048}",
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn emoji5_additions_except_flags() {
    check_runs(&[(
      "\u{1F9D4}\u{1F3FB}\u{1F9D4}\u{1F3FC}\u{1F9D4}\u{1F3FD}\u{1F9D4}\u{1F3FE}\u{1F9D4}\u{1F3FF}\u{1F931}\u{1F931}\u{1F3FB}\u{1F931}\u{1F3FC}\u{1F931}\u{1F3FD}\u{1F931}\u{1F3FE}\u{1F931}\u{1F3FF}\u{1F9D9}\u{1F9D9}\u{1F3FB}\u{1F9D9}\u{1F3FC}\u{1F9D9}\u{1F3FD}\u{1F9D9}\u{1F3FE}\u{1F9D9}\u{1F3FF}\u{1F9D9}\u{200D}\u{2640}\u{FE0F}\u{1F9D9}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9D9}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9D9}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9D9}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9D9}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9D9}\u{200D}\u{2642}\u{FE0F}\u{1F9D9}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9D9}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9D9}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9D9}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9D9}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9DA}\u{1F9DA}\u{1F3FB}\u{1F9DA}\u{1F3FC}\u{1F9DA}\u{1F3FD}\u{1F9DA}\u{1F3FE}\u{1F9DA}\u{1F3FF}\u{1F9DA}\u{200D}\u{2640}\u{FE0F}\u{1F9DA}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9DA}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9DA}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9DA}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9DA}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9DA}\u{200D}\u{2642}\u{FE0F}\u{1F9DA}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9DA}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9DA}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9DA}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9DA}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9DB}\u{1F9DB}\u{1F3FB}\u{1F9DB}\u{1F3FC}\u{1F9DB}\u{1F3FD}\u{1F9DB}\u{1F3FE}\u{1F9DB}\u{1F3FF}\u{1F9DB}\u{200D}\u{2640}\u{FE0F}\u{1F9DB}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9DB}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9DB}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9DB}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9DB}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9DB}\u{200D}\u{2642}\u{FE0F}\u{1F9DB}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9DB}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9DB}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9DB}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9DB}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9DC}\u{1F9DC}\u{1F3FB}\u{1F9DC}\u{1F3FC}\u{1F9DC}\u{1F3FD}\u{1F9DC}\u{1F3FE}\u{1F9DC}\u{1F3FF}\u{1F9DC}\u{200D}\u{2640}\u{FE0F}\u{1F9DC}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9DC}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9DC}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9DC}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9DC}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9DC}\u{200D}\u{2642}\u{FE0F}\u{1F9DC}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9DC}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9DC}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9DC}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9DC}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9DD}\u{1F9DD}\u{1F3FB}\u{1F9DD}\u{1F3FC}\u{1F9DD}\u{1F3FD}\u{1F9DD}\u{1F3FE}\u{1F9DD}\u{1F3FF}\u{1F9DD}\u{200D}\u{2640}\u{FE0F}\u{1F9DD}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9DD}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9DD}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9DD}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9DD}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9DD}\u{200D}\u{2642}\u{FE0F}\u{1F9DD}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9DD}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9DD}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9DD}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9DD}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9DE}\u{1F9DE}\u{200D}\u{2640}\u{FE0F}\u{1F9DE}\u{200D}\u{2642}\u{FE0F}\u{1F9DF}\u{1F9DF}\u{200D}\u{2640}\u{FE0F}\u{1F9DF}\u{200D}\u{2642}\u{FE0F}\u{1F9D6}\u{1F9D6}\u{1F3FB}\u{1F9D6}\u{1F3FC}\u{1F9D6}\u{1F3FD}\u{1F9D6}\u{1F3FE}\u{1F9D6}\u{1F3FF}\u{1F9D6}\u{200D}\u{2640}\u{FE0F}\u{1F9D6}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9D6}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9D6}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9D6}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9D6}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9D6}\u{200D}\u{2642}\u{FE0F}\u{1F9D6}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9D6}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9D6}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9D6}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9D6}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9D7}\u{1F9D7}\u{1F3FB}\u{1F9D7}\u{1F3FC}\u{1F9D7}\u{1F3FD}\u{1F9D7}\u{1F3FE}\u{1F9D7}\u{1F3FF}\u{1F9D7}\u{200D}\u{2640}\u{FE0F}\u{1F9D7}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9D7}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9D7}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9D7}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9D7}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9D7}\u{200D}\u{2642}\u{FE0F}\u{1F9D7}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9D7}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9D7}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9D7}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9D7}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9D8}\u{1F9D8}\u{1F3FB}\u{1F9D8}\u{1F3FC}\u{1F9D8}\u{1F3FD}\u{1F9D8}\u{1F3FE}\u{1F9D8}\u{1F3FF}\u{1F9D8}\u{200D}\u{2640}\u{FE0F}\u{1F9D8}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9D8}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9D8}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9D8}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9D8}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9D8}\u{200D}\u{2642}\u{FE0F}\u{1F9D8}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9D8}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9D8}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9D8}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9D8}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F91F}\u{1F91F}\u{1F3FB}\u{1F91F}\u{1F3FC}\u{1F91F}\u{1F3FD}\u{1F91F}\u{1F3FE}\u{1F91F}\u{1F3FF}\u{1F932}\u{1F932}\u{1F3FB}\u{1F932}\u{1F3FC}\u{1F932}\u{1F3FD}\u{1F932}\u{1F3FE}\u{1F932}\u{1F3FF}\u{1F9E0}\u{1F9E1}\u{1F9E3}\u{1F9E4}\u{1F9E5}\u{1F9E6}\u{1F9E2}\u{1F993}\u{1F992}\u{1F994}\u{1F995}\u{1F996}\u{1F997}\u{1F965}\u{1F966}\u{1F968}\u{1F969}\u{1F96A}\u{1F963}\u{1F96B}\u{1F95F}\u{1F960}\u{1F961}\u{1F967}\u{1F964}\u{1F962}\u{1F6F8}\u{1F6F7}\u{1F94C}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn emoji_subdivision_flags() {
    check_runs(&[
      (
        "\u{1F3F4}\u{E0067}\u{E0062}\u{E0077}\u{E006C}\u{E0073}\u{E007F}\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}\u{1F3F4}",
        FontFallbackPriority::EmojiEmoji,
      ),
      ("\u{E0067}\u{E0062}", FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn emoji11_additions() {
    check_runs(&[(
      "\u{1F970}\u{1F975}\u{1F976}\u{1F973}\u{1F974}\u{1F97A}\u{1F468}\u{200D}\u{1F9B0}\u{1F468}\u{1F3FB}\u{200D}\u{1F9B0}\u{1F468}\u{1F3FC}\u{200D}\u{1F9B0}\u{1F468}\u{1F3FD}\u{200D}\u{1F9B0}\u{1F468}\u{1F3FE}\u{200D}\u{1F9B0}\u{1F468}\u{1F3FF}\u{200D}\u{1F9B0}\u{1F468}\u{200D}\u{1F9B1}\u{1F468}\u{1F3FB}\u{200D}\u{1F9B1}\u{1F468}\u{1F3FC}\u{200D}\u{1F9B1}\u{1F468}\u{1F3FD}\u{200D}\u{1F9B1}\u{1F468}\u{1F3FE}\u{200D}\u{1F9B1}\u{1F468}\u{1F3FF}\u{200D}\u{1F9B1}\u{1F468}\u{200D}\u{1F9B3}\u{1F468}\u{1F3FB}\u{200D}\u{1F9B3}\u{1F468}\u{1F3FC}\u{200D}\u{1F9B3}\u{1F468}\u{1F3FD}\u{200D}\u{1F9B3}\u{1F468}\u{1F3FE}\u{200D}\u{1F9B3}\u{1F468}\u{1F3FF}\u{200D}\u{1F9B3}\u{1F468}\u{200D}\u{1F9B2}\u{1F468}\u{1F3FB}\u{200D}\u{1F9B2}\u{1F468}\u{1F3FC}\u{200D}\u{1F9B2}\u{1F468}\u{1F3FD}\u{200D}\u{1F9B2}\u{1F468}\u{1F3FE}\u{200D}\u{1F9B2}\u{1F468}\u{1F3FF}\u{200D}\u{1F9B2}\u{1F469}\u{200D}\u{1F9B0}\u{1F469}\u{1F3FB}\u{200D}\u{1F9B0}\u{1F469}\u{1F3FC}\u{200D}\u{1F9B0}\u{1F469}\u{1F3FD}\u{200D}\u{1F9B0}\u{1F469}\u{1F3FE}\u{200D}\u{1F9B0}\u{1F469}\u{1F3FF}\u{200D}\u{1F9B0}\u{1F469}\u{200D}\u{1F9B1}\u{1F469}\u{1F3FB}\u{200D}\u{1F9B1}\u{1F469}\u{1F3FC}\u{200D}\u{1F9B1}\u{1F469}\u{1F3FD}\u{200D}\u{1F9B1}\u{1F469}\u{1F3FE}\u{200D}\u{1F9B1}\u{1F469}\u{1F3FF}\u{200D}\u{1F9B1}\u{1F469}\u{200D}\u{1F9B3}\u{1F469}\u{1F3FB}\u{200D}\u{1F9B3}\u{1F469}\u{1F3FC}\u{200D}\u{1F9B3}\u{1F469}\u{1F3FD}\u{200D}\u{1F9B3}\u{1F469}\u{1F3FE}\u{200D}\u{1F9B3}\u{1F469}\u{1F3FF}\u{200D}\u{1F9B3}\u{1F469}\u{200D}\u{1F9B2}\u{1F469}\u{1F3FB}\u{200D}\u{1F9B2}\u{1F469}\u{1F3FC}\u{200D}\u{1F9B2}\u{1F469}\u{1F3FD}\u{200D}\u{1F9B2}\u{1F469}\u{1F3FE}\u{200D}\u{1F9B2}\u{1F469}\u{1F3FF}\u{200D}\u{1F9B2}\u{1F9B8}\u{1F9B8}\u{1F3FB}\u{1F9B8}\u{1F3FC}\u{1F9B8}\u{1F3FD}\u{1F9B8}\u{1F3FE}\u{1F9B8}\u{1F3FF}\u{1F9B8}\u{200D}\u{2640}\u{FE0F}\u{1F9B8}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9B8}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9B8}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9B8}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9B8}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9B8}\u{200D}\u{2642}\u{FE0F}\u{1F9B8}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9B8}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9B8}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9B8}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9B8}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9B9}\u{1F9B9}\u{1F3FB}\u{1F9B9}\u{1F3FC}\u{1F9B9}\u{1F3FD}\u{1F9B9}\u{1F3FE}\u{1F9B9}\u{1F3FF}\u{1F9B9}\u{200D}\u{2640}\u{FE0F}\u{1F9B9}\u{1F3FB}\u{200D}\u{2640}\u{FE0F}\u{1F9B9}\u{1F3FC}\u{200D}\u{2640}\u{FE0F}\u{1F9B9}\u{1F3FD}\u{200D}\u{2640}\u{FE0F}\u{1F9B9}\u{1F3FE}\u{200D}\u{2640}\u{FE0F}\u{1F9B9}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}\u{1F9B9}\u{200D}\u{2642}\u{FE0F}\u{1F9B9}\u{1F3FB}\u{200D}\u{2642}\u{FE0F}\u{1F9B9}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}\u{1F9B9}\u{1F3FD}\u{200D}\u{2642}\u{FE0F}\u{1F9B9}\u{1F3FE}\u{200D}\u{2642}\u{FE0F}\u{1F9B9}\u{1F3FF}\u{200D}\u{2642}\u{FE0F}\u{1F9B5}\u{1F9B5}\u{1F3FB}\u{1F9B5}\u{1F3FC}\u{1F9B5}\u{1F3FD}\u{1F9B5}\u{1F3FE}\u{1F9B5}\u{1F3FF}\u{1F9B6}\u{1F9B6}\u{1F3FB}\u{1F9B6}\u{1F3FC}\u{1F9B6}\u{1F3FD}\u{1F9B6}\u{1F3FE}\u{1F9B6}\u{1F3FF}\u{1F9B4}\u{1F9B7}\u{1F9B0}\u{1F9B1}\u{1F9B3}\u{1F9B2}\u{1F97D}\u{1F97C}\u{1F97E}\u{1F97F}\u{1F99D}\u{1F999}\u{1F99B}\u{1F998}\u{1F9A1}\u{1F9A2}\u{1F99A}\u{1F99C}\u{1F99E}\u{1F99F}\u{1F9A0}\u{1F96D}\u{1F96C}\u{1F96F}\u{1F9C2}\u{1F96E}\u{1F9C1}\u{1F9ED}\u{1F9F1}\u{1F6F9}\u{1F9F3}\u{1F9E8}\u{1F9E7}\u{1F94E}\u{1F94F}\u{1F94D}\u{1F9FF}\u{1F9E9}\u{1F9F8}\u{1F9F5}\u{1F9F6}\u{1F9EE}\u{1F9FE}\u{1F9F0}\u{1F9F2}\u{1F9EA}\u{1F9EB}\u{1F9EC}\u{1F9F4}\u{1F9F7}\u{1F9F9}\u{1F9FA}\u{1F9FB}\u{1F9FC}\u{1F9FD}\u{1F9EF}\u{1F3F4}\u{200D}\u{2620}\u{FE0F}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn emoji17_additions() {
    check_runs(&[(
      "\u{1FAEA}\u{1FAEF}\u{1FAC8}\u{1F9D1}\u{200D}\u{1FA70}\u{1F46F}\u{1F93C}\u{1FACD}\u{1F6D8}\u{1FA8A}\u{1FA8E}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn emoji17_bunny_ears() {
    check_runs(&[(
      "\u{1F468}\u{1F3FB}\u{200D}\u{1F430}\u{200D}\u{1F468}\u{1F3FF}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }

  #[test]
  fn emoji17_wrestling() {
    check_runs(&[(
      "\u{1F469}\u{1F3FF}\u{200D}\u{1FAEF}\u{200D}\u{1F469}\u{1F3FB}",
      FontFallbackPriority::EmojiEmoji,
    )]);
  }
}

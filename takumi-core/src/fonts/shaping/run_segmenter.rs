//! Blink's [`RunSegmenter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/run_segmenter.cc).

use icu_properties::props::Script;

use crate::fonts::{
  font_fallback_priority::FontFallbackPriority, script_run_iterator::ScriptRunIterator,
  symbols_iterator::SymbolsIterator,
};

/// Blink's `RunSegmenterRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RunSegmenterRange {
  pub(crate) start: usize,
  pub(crate) end: usize,
  pub(crate) script: Script,
  pub(crate) font_fallback_priority: FontFallbackPriority,
}

/// Blink's `RunSegmenter`.
pub(crate) struct RunSegmenter<'a> {
  buffer_size: usize,
  candidate_range: RunSegmenterRange,
  script_run_iterator: ScriptRunIterator<'a>,
  symbols_iterator: SymbolsIterator,
  last_split: usize,
  script_run_iterator_position: usize,
  symbols_iterator_position: usize,
  at_end: bool,
}

impl<'a> RunSegmenter<'a> {
  pub(crate) fn new(buffer: &'a str) -> Self {
    Self {
      buffer_size: buffer.len(),
      // Every field is set before a range leaves.
      candidate_range: RunSegmenterRange {
        start: 0,
        end: 0,
        script: Script::Unknown,
        font_fallback_priority: FontFallbackPriority::Text,
      },
      script_run_iterator: ScriptRunIterator::new(buffer),
      symbols_iterator: SymbolsIterator::new(buffer),
      last_split: 0,
      script_run_iterator_position: 0,
      symbols_iterator_position: 0,
      at_end: buffer.is_empty(),
    }
  }

  /// Blink's `Consume`.
  pub(crate) fn consume(&mut self) -> Option<RunSegmenterRange> {
    if self.at_end {
      return None;
    }

    Self::consume_iterator_past_last_split(
      || self.script_run_iterator.consume(),
      &mut self.script_run_iterator_position,
      &mut self.candidate_range.script,
      self.last_split,
      self.buffer_size,
    );
    Self::consume_iterator_past_last_split(
      || self.symbols_iterator.consume(),
      &mut self.symbols_iterator_position,
      &mut self.candidate_range.font_fallback_priority,
      self.last_split,
      self.buffer_size,
    );

    self.last_split = self
      .script_run_iterator_position
      .min(self.symbols_iterator_position);
    self.candidate_range.start = self.candidate_range.end;
    self.candidate_range.end = self.last_split;
    self.at_end = self.last_split == self.buffer_size;

    Some(self.candidate_range)
  }

  /// Blink's `ConsumeIteratorPastLastSplit`.
  fn consume_iterator_past_last_split<T>(
    mut consume: impl FnMut() -> Option<(usize, T)>,
    iterator_position: &mut usize,
    segmentation_category: &mut T,
    last_split: usize,
    buffer_size: usize,
  ) {
    if *iterator_position > last_split || *iterator_position >= buffer_size {
      return;
    }

    while let Some((position, category)) = consume() {
      *iterator_position = position;
      *segmentation_category = category;

      if position > last_split {
        return;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Blink's `RunSegmenterTest::CheckRunsHorizontal`.
  fn check_runs(runs: &[(&str, Script, FontFallbackPriority)]) {
    let mut text = String::new();
    let mut expect = Vec::new();

    for &(run, script, font_fallback_priority) in runs {
      let start = text.len();

      text.push_str(run);
      expect.push(RunSegmenterRange {
        start,
        end: text.len(),
        script,
        font_fallback_priority,
      });
    }

    let mut segmenter = RunSegmenter::new(&text);
    let mut actual = Vec::new();

    while let Some(range) = segmenter.consume() {
      actual.push(range);
    }

    assert_eq!(actual, expect);
  }

  #[test]
  fn empty() {
    assert_eq!(RunSegmenter::new("").consume(), None);
  }

  #[test]
  fn latin_punctuation_sideways() {
    check_runs(&[("Abc.;?Xyz", Script::Latin, FontFallbackPriority::Text)]);
  }

  #[test]
  fn one_space() {
    check_runs(&[(" ", Script::Common, FontFallbackPriority::Text)]);
  }

  #[test]
  fn arabic_hangul() {
    check_runs(&[
      ("\u{646}\u{635}", Script::Arabic, FontFallbackPriority::Text),
      (
        "\u{D0A4}\u{C2A4}\u{C758}",
        Script::Hangul,
        FontFallbackPriority::Text,
      ),
    ]);
  }

  #[test]
  fn japanese_hindi_emoji_mix() {
    check_runs(&[
      (
        "\u{767E}\u{5BB6}\u{59D3}",
        Script::Han,
        FontFallbackPriority::Text,
      ),
      (
        "\u{90B}\u{937}\u{93F}\u{92F}\u{94B}\u{902}",
        Script::Devanagari,
        FontFallbackPriority::Text,
      ),
      (
        "\u{1F331}\u{1F332}\u{1F333}\u{1F334}",
        Script::Devanagari,
        FontFallbackPriority::Emoji,
      ),
      (
        "\u{767E}\u{5BB6}\u{59D3}",
        Script::Han,
        FontFallbackPriority::Text,
      ),
      (
        "\u{1F331}\u{1F332}",
        Script::Han,
        FontFallbackPriority::Emoji,
      ),
    ]);
  }

  #[test]
  fn combining_circle() {
    check_runs(&[(
      "\u{25CC}\u{301}\u{25CC}\u{300}\u{25CC}\u{308}\u{25CC}\u{302}\u{25CC}\u{304}\u{25CC}\u{30A}",
      Script::Latin,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn hangul_space() {
    check_runs(&[(
      "\u{D0A4}\u{C2A4}\u{C758} \u{ACE0}\u{C720}\u{C870}\u{AC74}\u{C740}",
      Script::Hangul,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn technical_common_upright() {
    check_runs(&[(
      "\u{2300}\u{2301}\u{2302}",
      Script::Common,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn punctuation_common_sideways() {
    check_runs(&[(
      ".\u{2026}\u{A1}",
      Script::Common,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn japanese_punctuation_mixed_inside() {
    check_runs(&[(
      "\u{3044}\u{308D}\u{306F}\u{306B}.\u{2026}\u{A1}\u{307B}\u{3078}\u{3068}",
      Script::Hiragana,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn punctuation_devanagari_combining() {
    check_runs(&[(
      "\u{915}+\u{947}",
      Script::Devanagari,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn emoji_zwj_sequences() {
    check_runs(&[
      (
        "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}",
        Script::Latin,
        FontFallbackPriority::Emoji,
      ),
      ("abcd", Script::Latin, FontFallbackPriority::Text),
      (
        "\u{1F469}\u{200D}\u{1F469}",
        Script::Latin,
        FontFallbackPriority::Emoji,
      ),
      (
        "\u{200D}\u{200D}efg",
        Script::Latin,
        FontFallbackPriority::Text,
      ),
    ]);
  }

  #[test]
  fn emoji_zwj_sequences_vertical_mixed() {
    check_runs(&[(
      "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F48B}\u{200D}\u{1F468}",
      Script::Common,
      FontFallbackPriority::Emoji,
    )]);
  }

  #[test]
  fn japanese_letterlike_end() {
    check_runs(&[(
      "\u{3044}\u{308D}\u{306F}\u{2110}\u{2112}\u{2110}\u{2112}\u{2110}\u{2112}\u{2110}\u{2112}\u{2110}\u{2112}\u{2110}\u{2112}\u{2110}\u{2112}",
      Script::Hiragana,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn japanese_case() {
    check_runs(&[
      (
        "\u{3044}\u{308D}\u{306F}",
        Script::Hiragana,
        FontFallbackPriority::Text,
      ),
      ("aaAA", Script::Latin, FontFallbackPriority::Text),
      (
        "\u{3044}\u{308D}\u{306F}",
        Script::Hiragana,
        FontFallbackPriority::Text,
      ),
    ]);
  }

  #[test]
  fn dingbats_misc_symbols_modifier() {
    check_runs(&[(
      "\u{26F9}\u{1F3FB}\u{270D}\u{1F3FB}\u{270A}\u{1F3FC}",
      Script::Common,
      FontFallbackPriority::Emoji,
    )]);
  }

  #[test]
  fn armenian_cyrillic_case() {
    check_runs(&[
      (
        "\u{561}\u{562}\u{563}",
        Script::Armenian,
        FontFallbackPriority::Text,
      ),
      (
        "\u{3B1}\u{3B2}\u{3B3}",
        Script::Greek,
        FontFallbackPriority::Text,
      ),
      (
        "\u{531}\u{532}\u{533}",
        Script::Armenian,
        FontFallbackPriority::Text,
      ),
    ]);
  }

  #[test]
  fn emoji_subdivision_flags() {
    check_runs(&[(
      "\u{1F3F4}\u{E0067}\u{E0062}\u{E0077}\u{E006C}\u{E0073}\u{E007F}\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
      Script::Common,
      FontFallbackPriority::Emoji,
    )]);
  }

  #[test]
  fn non_emoji_presentation_symbols() {
    check_runs(&[(
      "\u{2626}\u{262A}\u{2638}\u{271D}\u{2721}\u{2627}\u{2628}\u{2629}\u{262B}\u{262C}\u{2670}\u{2671}\u{271F}\u{2720}",
      Script::Common,
      FontFallbackPriority::Text,
    )]);
  }

  #[test]
  fn cjk_brackets_after_latin_letter() {
    check_runs(&[
      ("A", Script::Latin, FontFallbackPriority::Text),
      (
        "\u{300C}\u{56FD}\u{300D}",
        Script::Han,
        FontFallbackPriority::Text,
      ),
    ]);
  }

  #[test]
  fn cjk_brackets_after_latin_parenthesis() {
    check_runs(&[
      ("A(", Script::Latin, FontFallbackPriority::Text),
      (
        "\u{300C}\u{56FD}\u{300D}",
        Script::Han,
        FontFallbackPriority::Text,
      ),
      (")", Script::Latin, FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn cjk_brackets_with_latin_parenthesis_inside() {
    check_runs(&[
      ("A", Script::Latin, FontFallbackPriority::Text),
      ("\u{300C}\u{56FD}(", Script::Han, FontFallbackPriority::Text),
      ("A", Script::Latin, FontFallbackPriority::Text),
      (")\u{300D}", Script::Han, FontFallbackPriority::Text),
    ]);
  }

  #[test]
  fn cjk_brackets_after_unmatching_latin_parenthesis() {
    check_runs(&[
      ("A((", Script::Latin, FontFallbackPriority::Text),
      (
        "\u{300C}\u{56FD}\u{300D}",
        Script::Han,
        FontFallbackPriority::Text,
      ),
      (")", Script::Latin, FontFallbackPriority::Text),
    ]);
  }
}

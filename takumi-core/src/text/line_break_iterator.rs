//! Soft wrap opportunities from parley's UAX #14 analysis with Chromium's overrides, in place of
//! Blink's LGPL `LazyLineBreakIterator`.

use std::slice::from_ref;

use icu_segmenter::GraphemeClusterSegmenter;
use parley::WordBreak;
use parley_engine::{
  Analysis, AnalysisOptions, Analyzer, LineBreakConfig,
  break_overrides::{CHROMIUM_LINE_BREAK_OVERRIDE, LineBreakContext},
};

/// Chromium's overrides under `word-break: break-all`, where its table of ASCII pairs allows breaks
/// but no longer prevents them.
fn chromium_break_all_override(context: LineBreakContext) -> Option<bool> {
  CHROMIUM_LINE_BREAK_OVERRIDE(context).filter(|&opportunity| opportunity)
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
  breakable: Vec<bool>,
  break_type: LineBreakType,
  break_space: BreakSpaceType,
}

impl<'a> LineBreakIterator<'a> {
  pub(crate) fn new(text: &'a str, break_type: LineBreakType, break_space: BreakSpaceType) -> Self {
    let mut breakable = vec![false; text.len() + 1];

    if break_type == LineBreakType::BreakCharacter {
      for position in GraphemeClusterSegmenter::new().segment_str(text) {
        breakable[position] = true;
      }
    } else {
      let word_break = match break_type {
        LineBreakType::BreakAll => WordBreak::BreakAll,
        LineBreakType::KeepAll => WordBreak::KeepAll,
        LineBreakType::Normal | LineBreakType::BreakCharacter => WordBreak::Normal,
      };
      let config = LineBreakConfig {
        word_break,
        ..LineBreakConfig::default()
      };
      let line_break = [(0..text.len(), config)];
      let whole_text = 0..text.len();
      let options = AnalysisOptions {
        line_break: &line_break,
        break_spaces: match break_space {
          BreakSpaceType::AfterSpaceRun => &[],
          BreakSpaceType::AfterEverySpace => from_ref(&whole_text),
        },
        line_break_override: Some(if word_break == WordBreak::BreakAll {
          &chromium_break_all_override
        } else {
          CHROMIUM_LINE_BREAK_OVERRIDE
        }),
        ..AnalysisOptions::default()
      };
      let mut analysis = Analysis::new();

      Analyzer::new().analyze(text, &options, &mut analysis);

      for ((position, _), info) in text.char_indices().zip(analysis.char_info()) {
        breakable[position] = info.is_soft_wrap_opportunity();
      }
    }

    breakable[0] = false;
    breakable[text.len()] = true;

    Self {
      text,
      breakable,
      break_type,
      break_space,
    }
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

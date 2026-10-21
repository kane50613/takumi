//! Blink's [`JustificationContext`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/justification_opportunity.cc).

use icu_segmenter::GraphemeClusterSegmenter;

use super::character::{
  OBJECT_REPLACEMENT_CHARACTER, is_cjk_ideograph_or_symbol, is_cursive_script,
  is_default_ignorable, treat_as_space,
};
use crate::style::{Direction as TextDirection, TextJustify};

/// Blink's `JustificationContext::Type`: the kind of the character before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Type {
  #[default]
  Normal,
  AtomicInline,
  Cursive,
}

/// Blink's `JustificationContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JustificationContext {
  previous_type: Type,
  is_after_opportunity: bool,
}

impl Default for JustificationContext {
  fn default() -> Self {
    Self {
      previous_type: Type::Normal,
      is_after_opportunity: true,
    }
  }
}

impl JustificationContext {
  pub(crate) fn is_after_opportunity(&self) -> bool {
    self.is_after_opportunity
  }

  pub(crate) fn set_after_opportunity(&mut self, flag: bool) {
    self.is_after_opportunity = flag;
  }

  /// Blink's `CheckOpportunity16`: whether to expand before and after `ch`.
  pub(crate) fn check_opportunity(&mut self, method: TextJustify, ch: char) -> (bool, bool) {
    if is_default_ignorable(ch) {
      return (false, false);
    }

    let character_type = if ch == OBJECT_REPLACEMENT_CHARACTER {
      Type::AtomicInline
    } else if is_cursive_script(ch) {
      Type::Cursive
    } else {
      Type::Normal
    };
    let previous_type = self.previous_type;

    self.previous_type = character_type;

    match method {
      // https://drafts.csswg.org/css-text-4/#valdef-text-justify-none
      TextJustify::None => {
        self.is_after_opportunity = false;

        return (false, false);
      }
      // https://drafts.csswg.org/css-text-4/#valdef-text-justify-inter-character
      TextJustify::InterCharacter => {
        if character_type != Type::Normal {
          let expand_before = !self.is_after_opportunity && previous_type != character_type;

          self.is_after_opportunity = false;

          return (expand_before, false);
        }

        let expand_before = !self.is_after_opportunity;

        self.is_after_opportunity = true;

        return (expand_before, true);
      }
      // https://drafts.csswg.org/css-text-4/#valdef-text-justify-inter-word
      TextJustify::InterWord => {
        if treat_as_space(ch) {
          self.is_after_opportunity = true;

          return (false, true);
        }

        self.is_after_opportunity = false;

        return (false, false);
      }
      // https://drafts.csswg.org/css-text-4/#valdef-text-justify-auto
      TextJustify::Auto => {}
    }

    if treat_as_space(ch) {
      self.is_after_opportunity = true;

      return (false, true);
    }

    // http://www.w3.org/TR/jlreq/#line_adjustment
    if !is_cjk_ideograph_or_symbol(ch) {
      self.is_after_opportunity = false;

      return (false, false);
    }

    let expand_before = !self.is_after_opportunity;

    self.is_after_opportunity = true;

    (expand_before, true)
  }

  /// Blink's `CountOpportunity16`.
  pub(crate) fn count_opportunity(&mut self, method: TextJustify, ch: char) -> usize {
    let (before, after) = self.check_opportunity(method, ch);

    usize::from(before) + usize::from(after)
  }

  /// Blink's `CountOpportunities`, at each grapheme cluster of `text` in `direction`.
  pub(crate) fn count_opportunities(
    &mut self,
    method: TextJustify,
    text: &str,
    direction: TextDirection,
  ) -> usize {
    let starts: Vec<usize> = GraphemeClusterSegmenter::new()
      .segment_str(text)
      .filter(|&start| start < text.len())
      .collect();
    let mut count = 0;
    let mut visit = |start: usize| {
      if let Some(ch) = text[start..].chars().next() {
        count += self.count_opportunity(method, ch);
      }
    };

    if direction == TextDirection::Ltr {
      starts.iter().copied().for_each(&mut visit);
    } else {
      starts.iter().rev().copied().for_each(&mut visit);
    }

    count
  }
}

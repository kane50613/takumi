//! Blink's [`ShapeResultSpacing`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/shape_result_spacing.cc).

use crate::{
  layout_unit::{FixedPoint, InlineLayoutUnit, TextRunLayoutUnit},
  style::Direction as TextDirection,
  text::{
    character::{NO_BREAK_SPACE, char_at, treat_as_space, treat_as_zero_width_space},
    justification_opportunity::JustificationContext,
    text_justify::TextJustify,
  },
};

/// Blink's `ShapeResultSpacing`.
pub(crate) struct ShapeResultSpacing<'a> {
  text: &'a str,
  letter_spacing: TextRunLayoutUnit,
  word_spacing: TextRunLayoutUnit,
  expansion: InlineLayoutUnit,
  expansion_per_opportunity: TextRunLayoutUnit,
  expansion_opportunity_count: usize,
  justification_context: JustificationContext,
  has_spacing: bool,
  is_letter_spacing_applied: bool,
  is_word_spacing_applied: bool,
  allow_tabs: bool,
  allow_word_spacing_anywhere: bool,
}

impl<'a> ShapeResultSpacing<'a> {
  pub(crate) fn new(text: &'a str, allow_word_spacing_anywhere: bool) -> Self {
    Self {
      text,
      letter_spacing: TextRunLayoutUnit::ZERO,
      word_spacing: TextRunLayoutUnit::ZERO,
      expansion: InlineLayoutUnit::ZERO,
      expansion_per_opportunity: TextRunLayoutUnit::ZERO,
      expansion_opportunity_count: 0,
      justification_context: JustificationContext::default(),
      has_spacing: false,
      is_letter_spacing_applied: false,
      is_word_spacing_applied: false,
      allow_tabs: false,
      allow_word_spacing_anywhere,
    }
  }

  pub(crate) fn text(&self) -> &'a str {
    self.text
  }

  pub(crate) fn letter_spacing(&self) -> TextRunLayoutUnit {
    if self.has_spacing {
      self.letter_spacing
    } else {
      TextRunLayoutUnit::ZERO
    }
  }

  pub(crate) fn word_spacing(&self) -> TextRunLayoutUnit {
    if self.has_spacing {
      self.word_spacing
    } else {
      TextRunLayoutUnit::ZERO
    }
  }

  pub(crate) fn has_spacing(&self) -> bool {
    self.has_spacing
  }

  pub(crate) fn is_letter_spacing_applied(&self) -> bool {
    self.is_letter_spacing_applied
  }

  pub(crate) fn is_word_spacing_applied(&self) -> bool {
    self.is_word_spacing_applied
  }

  pub(crate) fn has_expansion(&self) -> bool {
    self.expansion_opportunity_count != 0
  }

  /// Blink's `ExpansionOppotunityCount`.
  pub(crate) fn expansion_opportunity_count(&self) -> usize {
    self.expansion_opportunity_count
  }

  /// Blink's `SetSpacing` from `letter-spacing` and `word-spacing` in pixels.
  pub(crate) fn set_spacing_from(&mut self, letter_spacing: f32, word_spacing: f32) -> bool {
    self.set_spacing(
      TextRunLayoutUnit::from_float(letter_spacing),
      TextRunLayoutUnit::from_float(word_spacing),
    )
  }

  /// Blink's `SetSpacing`.
  pub(crate) fn set_spacing(
    &mut self,
    letter_spacing: TextRunLayoutUnit,
    word_spacing: TextRunLayoutUnit,
  ) -> bool {
    if letter_spacing == TextRunLayoutUnit::ZERO && word_spacing == TextRunLayoutUnit::ZERO {
      self.has_spacing = false;

      return false;
    }

    self.letter_spacing = letter_spacing;
    self.word_spacing = word_spacing;
    self.allow_tabs = true;
    self.has_spacing = true;

    true
  }

  /// Blink's `ExpansionSetup`: prepares to spread `expansion` over the opportunities counted
  /// before the setup drops.
  pub(crate) fn expansion_setup(
    &mut self,
    expansion: InlineLayoutUnit,
    allows_leading_expansion: bool,
    allows_trailing_expansion: bool,
  ) -> ExpansionSetup<'_, 'a> {
    debug_assert!(expansion > InlineLayoutUnit::ZERO);

    let mut justification_context = JustificationContext::default();

    justification_context.set_after_opportunity(!allows_leading_expansion);
    self.expansion = expansion;
    self.expansion_opportunity_count = 0;
    self.justification_context = justification_context;

    ExpansionSetup {
      spacing: self,
      allows_trailing_expansion,
      justification_context,
    }
  }

  /// Blink's `NextExpansion`.
  fn next_expansion(&mut self) -> TextRunLayoutUnit {
    assert!(self.expansion_opportunity_count != 0);

    self.justification_context.set_after_opportunity(true);
    self.expansion_opportunity_count -= 1;

    if self.expansion_opportunity_count == 0 {
      let remaining = self.expansion.to::<TextRunLayoutUnit>();

      self.expansion = InlineLayoutUnit::ZERO;

      return remaining;
    }

    self.expansion -= self.expansion_per_opportunity.into();
    self.expansion_per_opportunity
  }

  /// Blink's `ComputeSpacing`: the space after the character at `index` of the text.
  pub(crate) fn compute_spacing(
    &mut self,
    index: usize,
    is_cursive_script: bool,
  ) -> TextRunLayoutUnit {
    debug_assert!(self.has_spacing);

    let mut character = char_at(self.text, index);
    let treat_as_space = treat_as_space(character) && (character != '\t' || !self.allow_tabs);

    if treat_as_space && character != NO_BREAK_SPACE {
      character = ' ';
    }

    let mut spacing = TextRunLayoutUnit::ZERO;

    // CSS Text 4 §8.2.1: no letter-spacing within cursive script, but still at word separators.
    let apply_letter_spacing = !is_cursive_script || treat_as_space;

    if self.letter_spacing != TextRunLayoutUnit::ZERO
      && !treat_as_zero_width_space(character)
      && apply_letter_spacing
    {
      spacing += self.letter_spacing;
      self.is_letter_spacing_applied = true;
    }

    if treat_as_space
      && (self.allow_word_spacing_anywhere || index != 0 || character == NO_BREAK_SPACE)
    {
      spacing += self.word_spacing;
      self.is_word_spacing_applied = true;
    }

    spacing
  }

  /// Blink's `ComputeExpansion`: the space before and after the character at `index`, taken in
  /// order from the opportunities counted.
  pub(crate) fn compute_expansion(
    &mut self,
    method: TextJustify,
    index: usize,
  ) -> (TextRunLayoutUnit, TextRunLayoutUnit) {
    if !self.has_expansion() || index >= self.text.len() {
      return (TextRunLayoutUnit::ZERO, TextRunLayoutUnit::ZERO);
    }

    debug_assert!(!self.allow_tabs);

    let ch = char_at(self.text, index);

    self.compute_expansion_for(method, ch)
  }

  /// Blink's `ComputeExpansion` for a character outside the text.
  pub(crate) fn compute_expansion_for(
    &mut self,
    method: TextJustify,
    ch: char,
  ) -> (TextRunLayoutUnit, TextRunLayoutUnit) {
    if !self.has_expansion() {
      return (TextRunLayoutUnit::ZERO, TextRunLayoutUnit::ZERO);
    }

    let (opportunity_before, opportunity_after) =
      self.justification_context.check_opportunity(method, ch);

    self.finalize_compute_expansion(opportunity_before, opportunity_after)
  }

  /// Blink's `FinalizeComputeExpansion`.
  fn finalize_compute_expansion(
    &mut self,
    opportunity_before: bool,
    opportunity_after: bool,
  ) -> (TextRunLayoutUnit, TextRunLayoutUnit) {
    let mut spacing_before = TextRunLayoutUnit::ZERO;
    let mut spacing_after = TextRunLayoutUnit::ZERO;

    if opportunity_before {
      spacing_before = self.next_expansion();

      if !self.has_expansion() {
        return (spacing_before, spacing_after);
      }
    }

    if opportunity_after {
      spacing_after = self.next_expansion();
    }

    (spacing_before, spacing_after)
  }
}

/// Blink's `ExpansionSetup`: counts justification opportunities, and spreads the expansion over
/// them when dropped.
pub(crate) struct ExpansionSetup<'s, 'a> {
  spacing: &'s mut ShapeResultSpacing<'a>,
  allows_trailing_expansion: bool,
  justification_context: JustificationContext,
}

impl ExpansionSetup<'_, '_> {
  /// Blink's `CountOpportunities` over `text`.
  pub(crate) fn count_opportunities(
    &mut self,
    method: TextJustify,
    text: &str,
    direction: TextDirection,
  ) {
    self.spacing.expansion_opportunity_count += self
      .justification_context
      .count_opportunities(method, text, direction);
  }

  /// Blink's `CountOpportunities` for one character.
  pub(crate) fn count_opportunity(&mut self, method: TextJustify, ch: char) {
    self.spacing.expansion_opportunity_count +=
      self.justification_context.count_opportunity(method, ch);
  }
}

impl Drop for ExpansionSetup<'_, '_> {
  fn drop(&mut self) {
    let spacing = &mut *self.spacing;

    if self.justification_context.is_after_opportunity()
      && !self.allows_trailing_expansion
      && spacing.expansion_opportunity_count > 0
    {
      spacing.expansion_opportunity_count -= 1;
    }

    if spacing.expansion_opportunity_count != 0 {
      spacing.expansion_per_opportunity =
        (spacing.expansion / spacing.expansion_opportunity_count as i32).to::<TextRunLayoutUnit>();
    }

    spacing.has_spacing |= spacing.has_expansion();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fonts::shaping::{harfbuzz_shaper::HarfBuzzShaper, testing::geist};

  fn expansion_opportunity_count(text: &str, method: TextJustify, expansion: f32) -> usize {
    let mut spacing = ShapeResultSpacing::new(text, false);

    spacing
      .expansion_setup(InlineLayoutUnit::from_float(expansion), false, false)
      .count_opportunities(method, text, TextDirection::Ltr);

    spacing.expansion_opportunity_count()
  }

  #[test]
  fn expansion_opportunity_count_zws() {
    // ZERO WIDTH SPACE is a default ignorable code point.
    assert_eq!(
      expansion_opportunity_count("\u{200B}", TextJustify::Auto, 42.0),
      0
    );
  }

  #[test]
  fn expansion_opportunity_count_bidi_control_and_cjk() {
    // LEFT-TO-RIGHT ISOLATE, default ignorable, between two hiragana.
    assert_eq!(
      expansion_opportunity_count("\u{3042}\u{2066}\u{3043}", TextJustify::Auto, 42.0),
      1
    );
  }

  #[test]
  fn letter_spacing_follows_each_cluster() {
    let (_fonts, font) = geist();
    let text = "abc";
    let mut result = HarfBuzzShaper::new(text.into()).shape_all(&font, TextDirection::Ltr);
    let width = result.width();
    let mut spacing = ShapeResultSpacing::new(text, false);

    assert!(spacing.set_spacing_from(2.0, 0.0));

    result.apply_spacing(&mut spacing, 0);

    assert_eq!(result.width(), width + 6.0);
    assert!(spacing.is_letter_spacing_applied());
  }

  #[test]
  fn word_spacing_skips_a_leading_space() {
    let (_fonts, font) = geist();
    let text = " a b";
    let mut result = HarfBuzzShaper::new(text.into()).shape_all(&font, TextDirection::Ltr);
    let width = result.width();
    let mut spacing = ShapeResultSpacing::new(text, false);

    spacing.set_spacing_from(0.0, 5.0);
    result.apply_spacing(&mut spacing, 0);

    assert_eq!(result.width(), width + 5.0);
  }

  #[test]
  fn expansion_fills_the_spaces_but_the_last() {
    let (_fonts, font) = geist();
    let text = "a b c ";
    let mut result = HarfBuzzShaper::new(text.into()).shape_all(&font, TextDirection::Ltr);
    let width = result.width();
    let mut spacing = ShapeResultSpacing::new(text, false);

    spacing
      .expansion_setup(InlineLayoutUnit::from_int(10), false, false)
      .count_opportunities(TextJustify::Auto, text, TextDirection::Ltr);

    // The trailing space is no opportunity without trailing expansion.
    assert_eq!(spacing.expansion_opportunity_count(), 2);

    result.apply_expansion(TextJustify::Auto, &mut spacing, 0);

    assert_eq!(result.width(), width + 10.0);
  }
}

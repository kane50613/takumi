//! Blink's `unicode::` predicates from [`character.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/character.cc), reading ICU4X.

use icu_properties::{
  CodePointMapData, CodePointSetData,
  props::{
    Emoji, EmojiModifierBase, EmojiPresentation, ExtendedPictographic, GeneralCategory, Script,
  },
};

pub(crate) const ZERO_WIDTH_JOINER: char = '\u{200D}';
pub(crate) const COMBINING_ENCLOSING_CIRCLE_BACKSLASH: char = '\u{20E0}';
pub(crate) const COMBINING_ENCLOSING_KEYCAP: char = '\u{20E3}';
pub(crate) const IDEOGRAPHIC_SPACE: char = '\u{3000}';
pub(crate) const LEFT_CORNER_BRACKET: char = '\u{300C}';
pub(crate) const VARIATION_SELECTOR_15: char = '\u{FE0E}';
pub(crate) const VARIATION_SELECTOR_16: char = '\u{FE0F}';
pub(crate) const WAVING_BLACK_FLAG: char = '\u{1F3F4}';
pub(crate) const CANCEL_TAG: char = '\u{E007F}';

/// Blink's `IsEmoji`.
pub(crate) fn is_emoji(c: char) -> bool {
  CodePointSetData::new::<Emoji>().contains(c)
}

/// Blink's `IsEmojiTextDefault`.
pub(crate) fn is_emoji_text_default(c: char) -> bool {
  is_emoji(c) && !is_emoji_emoji_default(c)
}

/// Blink's `IsEmojiEmojiDefault`.
pub(crate) fn is_emoji_emoji_default(c: char) -> bool {
  CodePointSetData::new::<EmojiPresentation>().contains(c)
}

/// Blink's `IsEmojiModifierBase`.
pub(crate) fn is_emoji_modifier_base(c: char) -> bool {
  CodePointSetData::new::<EmojiModifierBase>().contains(c)
}

/// Blink's `IsEmojiKeycapBase`.
pub(crate) fn is_emoji_keycap_base(c: char) -> bool {
  c.is_ascii_digit() || c == '#' || c == '*'
}

/// Blink's `IsRegionalIndicator`.
pub(crate) fn is_regional_indicator(c: char) -> bool {
  ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// Blink's `IsModifier`.
pub(crate) fn is_modifier(c: char) -> bool {
  ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}

/// Blink's `IsEmojiTagSequence`: the tag characters of
/// [valid emoji tag sequences](https://www.unicode.org/reports/tr51/#valid-emoji-tag-sequences).
pub(crate) fn is_emoji_tag_sequence(c: char) -> bool {
  ('\u{E0030}'..='\u{E0039}').contains(&c) || ('\u{E0061}'..='\u{E007A}').contains(&c)
}

/// Blink's `IsEmojiReserved`.
pub(crate) fn is_emoji_reserved(c: char) -> bool {
  CodePointMapData::<GeneralCategory>::new().get(c) == GeneralCategory::Unassigned
    && CodePointSetData::new::<ExtendedPictographic>().contains(c)
}

/// Blink's `IsEmojiIncludingReserved`.
pub(crate) fn is_emoji_including_reserved(c: char) -> bool {
  is_emoji(c) || is_emoji_reserved(c)
}

/// Blink's `MaybeEmojiPresentation`.
pub(crate) fn maybe_emoji_presentation(c: char) -> bool {
  if c.is_ascii() {
    return is_emoji_keycap_base(c);
  }

  // Blink's ASCII table lists U+00A9 and U+00AE but only spans 128 entries, so neither counts.
  c == ZERO_WIDTH_JOINER
    || ('\u{203C}'..='\u{2B55}').contains(&c)
    || c == VARIATION_SELECTOR_15
    || c == '\u{3030}'
    || c == '\u{303D}'
    || c == '\u{3297}'
    || c == '\u{3299}'
    || c == VARIATION_SELECTOR_16
    || u32::from(c) >= 0x10000
}

/// Blink's `HasLikelyScript`.
pub(crate) fn has_likely_script(c: char) -> bool {
  let mut script = CodePointMapData::<Script>::new().get(c);

  if matches!(script, Script::Inherited | Script::Common) {
    script = get_script_based_on_unicode_block(c);
  }

  !matches!(script, Script::Common | Script::Inherited)
}

/// Blink's `GetScriptBasedOnUnicodeBlock`.
fn get_script_based_on_unicode_block(c: char) -> Script {
  match u32::from(c) {
    0x3000..=0x303F => Script::Han,
    0x3040..=0x30FF => Script::KatakanaOrHiragana,
    0x0E00..=0x0E7F => Script::Thai,
    0x0370..=0x03FF => Script::Greek,
    0x0900..=0x097F => Script::Devanagari,
    0x0530..=0x058F => Script::Armenian,
    0x10A0..=0x10FF => Script::Georgian,
    0x0C80..=0x0CFF => Script::Kannada,
    0x10330..=0x1034F => Script::Gothic,
    _ => Script::Common,
  }
}

/// Blink's `IsCommonOrInheritedScript`.
pub(crate) fn is_common_or_inherited_script(c: char) -> bool {
  matches!(
    CodePointMapData::<Script>::new().get(c),
    Script::Common | Script::Inherited
  )
}

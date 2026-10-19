//! Blink's `unicode::` predicates from [`character.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/character.cc), reading ICU4X.

use std::ops::RangeInclusive;

use icu_properties::{
  CodePointMapData, CodePointSetData,
  props::{
    DefaultIgnorableCodePoint, Emoji, EmojiModifierBase, EmojiPresentation, ExtendedPictographic,
    GeneralCategory, Script,
  },
};

pub(crate) const TAB: char = '\t';
pub(crate) const LINE_FEED: char = '\n';
pub(crate) const FORM_FEED: char = '\u{C}';
pub(crate) const CARRIAGE_RETURN: char = '\r';
pub(crate) const NO_BREAK_SPACE: char = '\u{A0}';
pub(crate) const SOFT_HYPHEN: char = '\u{AD}';
pub(crate) const ZERO_WIDTH_SPACE: char = '\u{200B}';
pub(crate) const ZERO_WIDTH_NON_JOINER: char = '\u{200C}';
pub(crate) const ZERO_WIDTH_JOINER: char = '\u{200D}';
pub(crate) const POP_DIRECTIONAL_FORMATTING: char = '\u{202C}';
pub(crate) const LEFT_TO_RIGHT_OVERRIDE: char = '\u{202D}';
pub(crate) const RIGHT_TO_LEFT_OVERRIDE: char = '\u{202E}';
pub(crate) const OBJECT_REPLACEMENT_CHARACTER: char = '\u{FFFC}';
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

/// Blink's `IsDefaultIgnorable`.
pub(crate) fn is_default_ignorable(c: char) -> bool {
  if u32::from(c) < 0x100 {
    return c == SOFT_HYPHEN;
  }

  CodePointSetData::new::<DefaultIgnorableCodePoint>().contains(c)
}

/// Blink's `TreatAsZeroWidthSpaceInComplexScript`.
pub(crate) fn treat_as_zero_width_space_in_complex_script(c: char) -> bool {
  matches!(c, '\u{C}' | '\r' | OBJECT_REPLACEMENT_CHARACTER) || is_default_ignorable(c)
}

/// Blink's `TreatAsSpace`.
pub(crate) fn treat_as_space(c: char) -> bool {
  matches!(c, ' ' | '\t' | '\n' | NO_BREAK_SPACE)
}

/// Blink's `TreatAsZeroWidthSpace`.
pub(crate) fn treat_as_zero_width_space(c: char) -> bool {
  treat_as_zero_width_space_in_complex_script(c)
    || c == ZERO_WIDTH_NON_JOINER
    || c == ZERO_WIDTH_JOINER
}

/// Blink's `IsCursiveScript`.
pub(crate) fn is_cursive_script(c: char) -> bool {
  matches!(
    CodePointMapData::<Script>::new().get(c),
    Script::Arabic
      | Script::HanifiRohingya
      | Script::Mandaic
      | Script::Mongolian
      | Script::Nko
      | Script::PhagsPa
      | Script::Syriac
  )
}

/// Blink's `String::Is8Bit`, approximated as every character fitting Latin-1.
pub(crate) fn is_8bit(text: &str) -> bool {
  text.chars().all(|c| u32::from(c) <= 0xFF)
}

/// Blink's `MaybeBidiRtlUtf16`: whether a UTF-16 code unit may be right-to-left.
fn maybe_bidi_rtl_utf16(unit: u32) -> bool {
  unit >= 0x0590
    && unit != 0x200B
    && !(0x2010..=0x2029).contains(&unit)
    && !(0x206A..=0xD7FF).contains(&unit)
    && !(0xFF00..=0xFFFF).contains(&unit)
}

/// Blink's `MaybeBidiRtl` for a string, which checks UTF-16 code units: a surrogate of a
/// character past the BMP counts, as there.
pub(crate) fn maybe_bidi_rtl(text: &str) -> bool {
  text
    .chars()
    .any(|c| u32::from(c) >= 0x10000 || maybe_bidi_rtl_utf16(u32::from(c)))
}

/// Blink's `Character::IsCjkIdeographOrSymbol`.
pub(crate) fn is_cjk_ideograph_or_symbol(character: char) -> bool {
  let code = u32::from(character);

  // U+02C7 CARON, a Bopomofo tone mark, is the lowest code point either table holds; Blink
  // returns early below it.
  const LOWEST: u32 = 0x2C7;

  code >= LOWEST
    && (CJK_IDEOGRAPHS_OR_SYMBOLS.binary_search(&code).is_ok()
      || CJK_IDEOGRAPH_OR_SYMBOL_RANGES
        .iter()
        .any(|range| range.contains(&code)))
}

/// Blink's `kIsCjkIdeographOrSymbolArray`, sorted.
pub(crate) const CJK_IDEOGRAPHS_OR_SYMBOLS: &[u32] = &[
  0x2C7, 0x2CA, 0x2CB, 0x2D9, 0x2020, 0x2021, 0x2030, 0x203B, 0x203C, 0x2042, 0x2047, 0x2048,
  0x2049, 0x2051, 0x20DD, 0x20DE, 0x2100, 0x2103, 0x2105, 0x2109, 0x210A, 0x2113, 0x2116, 0x2121,
  0x212B, 0x213B, 0x2150, 0x2151, 0x2152, 0x217F, 0x2189, 0x2307, 0x2312, 0x23CE, 0x2423, 0x25A0,
  0x25A1, 0x25A2, 0x25AA, 0x25AB, 0x25B1, 0x25B2, 0x25B3, 0x25B6, 0x25B7, 0x25BC, 0x25BD, 0x25C0,
  0x25C1, 0x25C6, 0x25C7, 0x25C9, 0x25CB, 0x25CC, 0x25EF, 0x2605, 0x2606, 0x260E, 0x2616, 0x2617,
  0x26A0, 0x2713, 0x271A, 0x273F, 0x2740, 0x2756, 0x2763, 0x2B1A, 0xFE10, 0xFE11, 0xFE12, 0xFE19,
  0xFF1D, 0x1F100, 0x1F200, 0x1F237, 0x1F32C, 0x1F336, 0x1F37D, 0x1F43F, 0x1F54F, 0x1F93B, 0x1F946,
];

/// Blink's `kIsCjkIdeographOrSymbolRanges`.
pub(crate) const CJK_IDEOGRAPH_OR_SYMBOL_RANGES: &[RangeInclusive<u32>] = &[
  0x2E80..=0x2FDF,
  0x31C0..=0x31EF,
  0x3400..=0x4DBF,
  0x4E00..=0x9FFF,
  0xF900..=0xFAFF,
  0x20000..=0x2FFFF,
  0x2156..=0x215A,
  0x2160..=0x216B,
  0x2170..=0x217B,
  0x23BE..=0x23CC,
  0x2460..=0x2492,
  0x249C..=0x24FF,
  0x25CE..=0x25D3,
  0x25E2..=0x25E6,
  0x2600..=0x2603,
  0x2660..=0x266F,
  0x2672..=0x267D,
  0x2776..=0x277F,
  0x2FF0..=0x302D,
  0x3031..=0x312F,
  0x3190..=0x31BF,
  0x3200..=0x33FF,
  0x4DC0..=0x4DFF,
  0xF860..=0xF862,
  0xFE30..=0xFE6F,
  0xFF00..=0xFF0C,
  0xFF0E..=0xFF1A,
  0xFF1F..=0xFFEF,
  0x16FE0..=0x16FFF,
  0x17000..=0x187FF,
  0x18800..=0x18AFF,
  0x1B000..=0x1B0FF,
  0x1B100..=0x1B12F,
  0x1B170..=0x1B2FF,
  0x1F110..=0x1F129,
  0x1F130..=0x1F149,
  0x1F150..=0x1F169,
  0x1F170..=0x1F189,
  0x1F202..=0x1F219,
  0x1F21B..=0x1F22E,
  0x1F230..=0x1F231,
  0x1F23B..=0x1F24F,
  0x1F252..=0x1F2FF,
  0x1F321..=0x1F32A,
  0x1F394..=0x1F39F,
  0x1F3CD..=0x1F3CE,
  0x1F3D4..=0x1F3DF,
  0x1F3F1..=0x1F3F2,
  0x1F3F5..=0x1F3F7,
  0x1F4FD..=0x1F4FE,
  0x1F53E..=0x1F54A,
  0x1F568..=0x1F573,
  0x1F576..=0x1F579,
  0x1F57B..=0x1F58F,
  0x1F591..=0x1F594,
  0x1F597..=0x1F5A3,
  0x1F5A5..=0x1F5E7,
  0x1F5E9..=0x1F5FA,
  0x1F650..=0x1F67F,
  0x1F6C6..=0x1F6CB,
  0x1F6CD..=0x1F6CF,
  0x1F6D3..=0x1F6D4,
  0x1F6D9..=0x1F6DB,
  0x1F6E0..=0x1F6EA,
  0x1F6ED..=0x1F6F3,
  0x1F6FD..=0x1F6FF,
  0x1F900..=0x1F90B,
  0x1FAC9..=0x1FACC,
];

/// Blink's `IsOtherSpaceSeparator`: the ideographic space.
pub(crate) fn is_other_space_separator(c: char) -> bool {
  c == IDEOGRAPHIC_SPACE
}

/// The character starting at byte `offset` of `text`. A byte inside a character stands in for the
/// trailing surrogate Blink would read there, which no predicate takes for a space.
pub(crate) fn char_at(text: &str, offset: usize) -> char {
  if !text.is_char_boundary(offset) {
    return char::REPLACEMENT_CHARACTER;
  }

  text[offset..].chars().next().unwrap_or('\0')
}

/// The character ending at byte `offset` of `text`.
pub(crate) fn char_before(text: &str, offset: usize) -> char {
  text[..offset].chars().next_back().unwrap_or('\0')
}

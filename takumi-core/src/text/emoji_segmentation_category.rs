//! Blink's `EmojiSegmentationCategory`, the classes of the emoji presentation grammar, after
//! [`emoji_segmentation_category_inline_header.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/emoji_segmentation_category_inline_header.h).
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use super::character::{
  CANCEL_TAG, COMBINING_ENCLOSING_CIRCLE_BACKSLASH, COMBINING_ENCLOSING_KEYCAP,
  VARIATION_SELECTOR_15, VARIATION_SELECTOR_16, WAVING_BLACK_FLAG, ZERO_WIDTH_JOINER,
  is_emoji_emoji_default, is_emoji_including_reserved, is_emoji_keycap_base,
  is_emoji_modifier_base, is_emoji_tag_sequence, is_emoji_text_default, is_modifier,
  is_regional_indicator,
};

/// The class of a character in the emoji presentation grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmojiSegmentationCategory {
  Emoji,
  EmojiTextPresentation,
  EmojiEmojiPresentation,
  EmojiModifierBase,
  EmojiModifier,
  RegionalIndicator,
  KeycapBase,
  CombiningEnclosingKeycap,
  CombiningEnclosingCircleBackslash,
  Zwj,
  Vs15,
  Vs16,
  TagBase,
  TagSequence,
  TagTerm,
  /// Any other character.
  MaxCategory,
}

impl EmojiSegmentationCategory {
  /// Blink's `GetEmojiSegmentationCategory`: the narrowest class first, as the grammar needs.
  pub(crate) fn of(codepoint: char) -> Self {
    if codepoint.is_ascii() {
      if is_emoji_keycap_base(codepoint) {
        return Self::KeycapBase;
      }

      return Self::MaxCategory;
    }

    match codepoint {
      COMBINING_ENCLOSING_KEYCAP => Self::CombiningEnclosingKeycap,
      COMBINING_ENCLOSING_CIRCLE_BACKSLASH => Self::CombiningEnclosingCircleBackslash,
      ZERO_WIDTH_JOINER => Self::Zwj,
      VARIATION_SELECTOR_15 => Self::Vs15,
      VARIATION_SELECTOR_16 => Self::Vs16,
      WAVING_BLACK_FLAG => Self::TagBase,
      _ if is_emoji_tag_sequence(codepoint) => Self::TagSequence,
      CANCEL_TAG => Self::TagTerm,
      _ if is_emoji_modifier_base(codepoint) => Self::EmojiModifierBase,
      _ if is_modifier(codepoint) => Self::EmojiModifier,
      _ if is_regional_indicator(codepoint) => Self::RegionalIndicator,
      _ if is_emoji_emoji_default(codepoint) => Self::EmojiEmojiPresentation,
      _ if is_emoji_text_default(codepoint) => Self::EmojiTextPresentation,
      _ if is_emoji_including_reserved(codepoint) => Self::Emoji,
      _ => Self::MaxCategory,
    }
  }
}

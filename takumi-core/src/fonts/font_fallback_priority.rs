//! Blink's `FontFallbackPriority`, after
//! [`font_fallback_priority.h`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_fallback_priority.h).
//! Follows Blink under the notice in LICENSE-CHROMIUM.

/// Which fonts a run of text falls back through: text fonts, or emoji fonts first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FontFallbackPriority {
  /// Text that is not a symbol.
  #[default]
  Text,
  /// Emoji in text presentation.
  EmojiText,
  /// Emoji in text presentation by variation selector 15.
  EmojiTextWithVs,
  /// Emoji in emoji presentation.
  EmojiEmoji,
  /// Emoji in emoji presentation by variation selector 16.
  EmojiEmojiWithVs,
}

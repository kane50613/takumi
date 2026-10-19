//! Blink's `TextJustify`, the `text-justify` methods.

/// Blink's `TextJustify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(dead_code, reason = "TODO: parse text-justify")]
pub(crate) enum TextJustify {
  #[default]
  Auto,
  None,
  InterWord,
  InterCharacter,
}

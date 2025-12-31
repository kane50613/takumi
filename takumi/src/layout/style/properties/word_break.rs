use swash::text::WordBreakStrength;

use crate::layout::style::declare_enum_from_css_impl;

/// Controls how text should be broken at word boundaries.
///
/// Corresponds to CSS word-break property.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum WordBreak {
  /// Normal line breaking behavior—lines may break according to language rules.
  #[default]
  Normal,
  /// Break words at arbitrary points to prevent overflow.
  BreakAll,
  /// Prevents word breaks within words. Useful for languages like Japanese.
  KeepAll,
  /// Allow breaking within long words if necessary to prevent overflow.
  BreakWord,
}

declare_enum_from_css_impl!(
  WordBreak,
  "normal" => WordBreak::Normal,
  "break-all" => WordBreak::BreakAll,
  "keep-all" => WordBreak::KeepAll,
  "break-word" => WordBreak::BreakWord,
);

impl From<WordBreak> for WordBreakStrength {
  fn from(value: WordBreak) -> Self {
    match value {
      WordBreak::Normal | WordBreak::BreakWord => WordBreakStrength::Normal,
      WordBreak::BreakAll => WordBreakStrength::BreakAll,
      WordBreak::KeepAll => WordBreakStrength::KeepAll,
    }
  }
}

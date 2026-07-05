use crate::style::declare_enum_from_css_impl;

/// Controls how text should be broken at word boundaries.
///
/// Corresponds to CSS word-break property.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[non_exhaustive]
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

#[cfg(feature = "unstable")]
impl From<WordBreak> for parley::WordBreak {
  fn from(value: WordBreak) -> Self {
    match value {
      WordBreak::Normal | WordBreak::BreakWord => parley::WordBreak::Normal,
      WordBreak::BreakAll => parley::WordBreak::BreakAll,
      WordBreak::KeepAll => parley::WordBreak::KeepAll,
    }
  }
}

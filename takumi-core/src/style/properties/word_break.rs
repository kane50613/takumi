use crate::style::impl_css_enum;

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

impl_css_enum!(
  WordBreak,
  "normal" => WordBreak::Normal,
  "break-all" => WordBreak::BreakAll,
  "keep-all" => WordBreak::KeepAll,
  "break-word" => WordBreak::BreakWord,
);

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_word_break() {
    for (css, expected) in [
      ("normal", WordBreak::Normal),
      ("break-all", WordBreak::BreakAll),
      ("keep-all", WordBreak::KeepAll),
      ("break-word", WordBreak::BreakWord),
    ] {
      assert_eq!(
        WordBreak::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }
}

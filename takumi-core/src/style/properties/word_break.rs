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

impl WordBreak {
  pub(crate) fn into_parley(self) -> parley::WordBreak {
    match self {
      WordBreak::Normal | WordBreak::BreakWord => parley::WordBreak::Normal,
      WordBreak::BreakAll => parley::WordBreak::BreakAll,
      WordBreak::KeepAll => parley::WordBreak::KeepAll,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{FromCssStr, ToCss};

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

  #[test]
  fn test_word_break_round_trip() {
    for css in ["normal", "break-all", "keep-all", "break-word"] {
      let parsed = WordBreak::from_css_str(css).unwrap();
      let reparsed = WordBreak::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_word_break_invalid() {
    assert!(WordBreak::from_css_str("bogus").is_err());
    assert!(WordBreak::from_css_str("123").is_err());
  }
}

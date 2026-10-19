use crate::style::{impl_css_enum, tw::TailwindPropertyParser};

/// Controls how text should be overflowed.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum OverflowWrap {
  /// Lines break only at allowed break points.
  #[default]
  Normal,
  /// An otherwise unbreakable word breaks anywhere, which min-content sizes count.
  Anywhere,
  /// An otherwise unbreakable word breaks anywhere, which min-content sizes do not count.
  BreakWord,
}

impl TailwindPropertyParser for OverflowWrap {}

impl_css_enum!(
  OverflowWrap,
  "normal" => OverflowWrap::Normal,
  "anywhere" => OverflowWrap::Anywhere,
  "break-word" => OverflowWrap::BreakWord,
);

impl OverflowWrap {
  /// Whether this is `anywhere`.
  pub(crate) fn is_anywhere(self) -> bool {
    self == Self::Anywhere
  }

  /// Whether this is `break-word`.
  pub(crate) fn is_break_word(self) -> bool {
    self == Self::BreakWord
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{FromCssStr, ToCss};

  #[test]
  fn test_parse_overflow_wrap() {
    for (css, expected) in [
      ("normal", OverflowWrap::Normal),
      ("anywhere", OverflowWrap::Anywhere),
      ("break-word", OverflowWrap::BreakWord),
    ] {
      assert_eq!(
        OverflowWrap::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_overflow_wrap_round_trip() {
    for css in ["normal", "anywhere", "break-word"] {
      let parsed = OverflowWrap::from_css_str(css).unwrap();
      let reparsed = OverflowWrap::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_overflow_wrap_invalid() {
    assert!(OverflowWrap::from_css_str("bogus").is_err());
    assert!(OverflowWrap::from_css_str("123").is_err());
  }
}

use crate::style::impl_css_enum;

/// `text-justify`: which opportunities `text-align: justify` stretches.
/// <https://drafts.csswg.org/css-text-4/#text-justify-property>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextJustify {
  /// Spaces, and between CJK characters.
  #[default]
  Auto,
  /// No justification.
  None,
  /// Spaces only.
  InterWord,
  /// Between characters, other than within cursive scripts.
  InterCharacter,
}

impl_css_enum!(
  ident TextJustify,
  "auto" => TextJustify::Auto,
  "none" => TextJustify::None,
  "inter-word" => TextJustify::InterWord,
  "inter-character" | "distribute" => TextJustify::InterCharacter,
);

#[cfg(test)]
mod tests {
  use super::TextJustify;
  use crate::style::{FromCssStr, ToCss};

  #[test]
  fn distribute_is_inter_character() {
    let parsed = TextJustify::from_css_str("distribute").unwrap();

    assert_eq!(parsed, TextJustify::InterCharacter);
    assert_eq!(parsed.to_css_string(), "inter-character");
  }
}

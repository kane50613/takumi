use crate::style::{FontFeature, Tag, impl_css_enum};

/// `font-kerning`. The shaper kerns by default, so only `normal`/`none` emit a `kern` tag.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontKerning {
  /// Shaper default (kerning enabled).
  #[default]
  Auto,
  /// Kerning explicitly enabled (`kern` 1).
  Normal,
  /// Kerning disabled (`kern` 0), including the shaper's fallback kerning.
  None,
}

impl FontKerning {
  pub(crate) fn append_features(&self, out: &mut Vec<FontFeature>) {
    match self {
      Self::Auto => {}
      Self::Normal => out.push(FontFeature::new(Tag::new(b"kern"), 1)),
      Self::None => out.push(FontFeature::new(Tag::new(b"kern"), 0)),
    }
  }
}

impl_css_enum!(
  ident FontKerning,
  "auto" => FontKerning::Auto,
  "normal" => FontKerning::Normal,
  "none" => FontKerning::None,
);

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{FromCssStr, ToCss};

  #[test]
  fn keyword_parsing_is_case_insensitive_and_canonical() {
    let parsed = FontKerning::from_css_str("NORMAL").unwrap();

    assert_eq!(parsed, FontKerning::Normal);
    assert_eq!(parsed.to_css_string(), "normal");
  }

  #[test]
  fn keyword_errors_preserve_ident_and_non_ident_forms() {
    let keyword = FontKerning::from_css_str("kern").unwrap_err();
    let number = FontKerning::from_css_str("1").unwrap_err();
    let string = FontKerning::from_css_str("\"normal\"").unwrap_err();
    let function = FontKerning::from_css_str("kern()").unwrap_err();

    assert_eq!(
      keyword.to_string(),
      "Unexpected token: kern, expected a value of 'auto', 'normal' or 'none'"
    );
    assert_eq!(
      number.to_string(),
      "Basic(UnexpectedToken(Number { has_sign: false, value: 1.0, int_value: Some(1) }))"
    );
    assert_eq!(
      string.to_string(),
      "Basic(UnexpectedToken(QuotedString(\"normal\")))"
    );
    assert_eq!(
      function.to_string(),
      "Basic(UnexpectedToken(Function(\"kern\")))"
    );
  }
}

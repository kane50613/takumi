use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, serialize_string};

use crate::style::{Animatable, FromCss, MakeComputed, ParseResult, ToCss, impl_css_enum};

/// `hyphens`: where a line may break inside a word with a hyphen.
/// <https://drafts.csswg.org/css-text-3/#hyphenation>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Hyphens {
  /// Never, a soft hyphen included.
  None,
  /// At a soft hyphen (U+00AD).
  #[default]
  Manual,
  /// Where the language's hyphenation rules allow. Approximate: takumi has no hyphenation
  /// dictionaries, so it breaks at soft hyphens only, as `manual` does.
  Auto,
}

impl_css_enum!(
  Hyphens,
  "none" => Hyphens::None,
  "manual" => Hyphens::Manual,
  "auto" => Hyphens::Auto,
);

/// `hyphenate-character`: the string a line broken inside a word ends with.
/// <https://drafts.csswg.org/css-text-4/#hyphenate-character>
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum HyphenateCharacter {
  /// U+2010 HYPHEN when the font has it, else U+002D HYPHEN-MINUS.
  #[default]
  Auto,
  /// This string.
  String(Arc<str>),
}

impl MakeComputed for HyphenateCharacter {}

impl Animatable for HyphenateCharacter {}

impl<'i> FromCss<'i> for HyphenateCharacter {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?.clone();

    match &token {
      Token::Ident(ident) if ident.eq_ignore_ascii_case("auto") => Ok(Self::Auto),
      Token::QuotedString(string) => Ok(Self::String(string.as_ref().into())),
      _ => Err(location.new_unexpected_token_error(token)),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["auto", "<string>"];
}

impl ToCss for HyphenateCharacter {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::String(string) => serialize_string(string, dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{HyphenateCharacter, Hyphens};
  use crate::style::{FromCssStr, ToCss};

  #[test]
  fn parses_hyphens_and_hyphenate_character() {
    assert_eq!(Hyphens::from_css_str("none"), Ok(Hyphens::None));
    assert_eq!(
      HyphenateCharacter::from_css_str("\"=\""),
      Ok(HyphenateCharacter::String("=".into()))
    );
    assert_eq!(
      HyphenateCharacter::from_css_str("\"=\"")
        .unwrap()
        .to_css_string(),
      "\"=\""
    );
    assert_eq!(
      HyphenateCharacter::from_css_str("auto"),
      Ok(HyphenateCharacter::Auto)
    );
  }
}

use std::{fmt, str};

use cssparser::{ParseError as CssParseError, Parser, Token, serialize_string};

use crate::style::{Animatable, FromCss, MakeComputed, ParseResult, ToCss};

/// `font-language-override`: the OpenType language system glyphs are picked under, in place of
/// the one the content language maps to.
/// <https://drafts.csswg.org/css-fonts-4/#font-language-override-prop>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FontLanguageOverride {
  /// The content language's language system.
  #[default]
  Normal,
  /// A language system tag, padded with spaces to four characters.
  Tag([u8; 4]),
}

impl MakeComputed for FontLanguageOverride {}

impl Animatable for FontLanguageOverride {}

impl<'i> FromCss<'i> for FontLanguageOverride {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let token = input.next()?.clone();

    match &token {
      Token::Ident(ident) if ident.eq_ignore_ascii_case("normal") => Ok(Self::Normal),
      // An OpenType tag is printable ASCII, spaces only at its end.
      Token::QuotedString(tag)
        if (1..=4).contains(&tag.len())
          && tag
            .trim_end_matches(' ')
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
          && !tag.starts_with(' ') =>
      {
        let mut padded = [b' '; 4];

        padded[..tag.len()].copy_from_slice(tag.as_bytes());

        Ok(Self::Tag(padded))
      }
      _ => Err(CssParseError::unexpected_token()),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["normal", "<string>"];
}

impl ToCss for FontLanguageOverride {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Normal => dest.write_str("normal"),
      Self::Tag(tag) => serialize_string(str::from_utf8(tag).unwrap_or_default().trim_end(), dest),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::FontLanguageOverride;
  use crate::style::{FromCssStr, ToCss};

  #[test]
  fn pads_a_tag_to_four_characters() {
    let parsed = FontLanguageOverride::from_css_str("\"TRK\"").unwrap();

    assert_eq!(parsed, FontLanguageOverride::Tag(*b"TRK "));
    assert_eq!(
      FontLanguageOverride::from_css_str("\"TRK \""),
      Ok(FontLanguageOverride::Tag(*b"TRK "))
    );
    assert_eq!(parsed.to_css_string(), "\"TRK\"");
    assert_eq!(
      FontLanguageOverride::from_css_str("normal"),
      Ok(FontLanguageOverride::Normal)
    );
  }

  #[test]
  fn rejects_an_empty_or_long_tag() {
    for value in ["\"\"", "\"ABCDE\"", "TRK", "\" TRK\"", "\"T RK\""] {
      assert!(
        FontLanguageOverride::from_css_str(value).is_err(),
        "{value}"
      );
    }
  }
}

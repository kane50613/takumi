use std::string::ToString;

use cssparser::{Parser, match_ignore_ascii_case};
use parley::{FontStack, GenericFamily};

use crate::layout::style::{
  CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, tw::TailwindPropertyParser,
};

/// Represents a font family for text rendering.
/// Multi value fallback is supported.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FontFamily(Box<[FontFamilyToken]>);

#[derive(Debug, Clone, PartialEq)]
enum FontFamilyToken {
  Owned(String),
  Generic(GenericFamily),
}

impl MakeComputed for FontFamily {}

impl<'i> FromCss<'i> for FontFamilyToken {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(name) = input.try_parse(|input| input.expect_string().map(ToString::to_string)) {
      return Ok(Self::Owned(name));
    }

    let mut family_name = input.expect_ident()?.to_string();

    while let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      family_name.push(' ');
      family_name.push_str(&ident);
    }

    if let Some(generic) = GenericFamily::parse(&family_name) {
      return Ok(Self::Generic(generic));
    }

    Ok(Self::Owned(family_name))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::FamilyName),
    CssToken::Syntax(CssSyntaxKind::GenericName),
  ];
}

impl<'i> FromCss<'i> for FontFamily {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let list = input.parse_comma_separated(FontFamilyToken::from_css)?;

    Ok(Self(list.into_boxed_slice()))
  }

  const VALID_TOKENS: &'static [CssToken] = FontFamilyToken::VALID_TOKENS;
}

impl TailwindPropertyParser for FontFamily {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sans" => Some(GenericFamily::SansSerif.into()),
      "serif" => Some(GenericFamily::Serif.into()),
      "mono" => Some(GenericFamily::Monospace.into()),
      _ => None,
    }
  }
}

impl Default for FontFamily {
  fn default() -> Self {
    GenericFamily::SansSerif.into()
  }
}

impl<'a> From<FontFamily> for FontStack<'a> {
  fn from(family: FontFamily) -> Self {
    FontStack::List(
      family
        .0
        .into_iter()
        .map(|token| match token {
          FontFamilyToken::Owned(name) => parley::FontFamily::Named(name.into()),
          FontFamilyToken::Generic(generic) => parley::FontFamily::Generic(generic),
        })
        .collect(),
    )
  }
}

impl<'a> From<&'a FontFamily> for FontStack<'a> {
  fn from(family: &'a FontFamily) -> Self {
    FontStack::List(
      family
        .0
        .iter()
        .map(|token| match token {
          FontFamilyToken::Owned(name) => parley::FontFamily::Named(name.into()),
          FontFamilyToken::Generic(generic) => parley::FontFamily::Generic(*generic),
        })
        .collect(),
    )
  }
}

impl From<GenericFamily> for FontFamily {
  fn from(generic: GenericFamily) -> Self {
    Self(Box::new([FontFamilyToken::Generic(generic)]))
  }
}

#[cfg(test)]
mod tests {
  use parley::GenericFamily;

  use super::{FontFamily, FontFamilyToken};
  use crate::layout::style::{FromCss, tw::TailwindPropertyParser};

  #[test]
  fn parses_single_generic_family() {
    assert_eq!(
      FontFamily::from_str("serif"),
      Ok(FontFamily(Box::new([FontFamilyToken::Generic(
        GenericFamily::Serif,
      )])))
    );
  }

  #[test]
  fn parses_fallback_family_list() {
    assert_eq!(
      FontFamily::from_str("\"Inter\", Arial, serif"),
      Ok(FontFamily(Box::new([
        FontFamilyToken::Owned(String::from("Inter")),
        FontFamilyToken::Owned(String::from("Arial")),
        FontFamilyToken::Generic(GenericFamily::Serif),
      ])))
    );
  }

  #[test]
  fn parses_unquoted_multi_word_family_name() {
    assert_eq!(
      FontFamily::from_str("Noto Sans TC"),
      Ok(FontFamily(Box::new([FontFamilyToken::Owned(
        "Noto Sans TC".to_string()
      )])))
    );
  }

  #[test]
  fn parses_tailwind_aliases() {
    assert_eq!(
      FontFamily::parse_tw("sans"),
      Some(FontFamily(Box::new([FontFamilyToken::Generic(
        GenericFamily::SansSerif,
      )])))
    );
    assert_eq!(
      FontFamily::parse_tw("mono"),
      Some(FontFamily(Box::new([FontFamilyToken::Generic(
        GenericFamily::Monospace,
      )])))
    );
    assert_eq!(FontFamily::parse_tw("display"), None);
  }
}

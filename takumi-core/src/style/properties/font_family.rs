use std::{fmt, string::ToString, sync::Arc};

use cssparser::{Parser, match_ignore_ascii_case};
use parley::{FontFamilyName, GenericFamily};

use crate::style::TwNamespace;
use crate::style::{
  CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss, properties::write_css_string,
  tw::TailwindPropertyParser,
};

/// Represents a font family for text rendering.
/// Multi value fallback is supported.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFamily(Arc<[FontFamilyToken]>);

impl Default for FontFamily {
  fn default() -> Self {
    Self::from_parlance_generic(GenericFamily::SansSerif)
  }
}

/// One entry in a font-family fallback list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FontFamilyToken {
  /// A named family.
  Owned(String),
  /// A generic family such as `serif`.
  Generic(GenericFamily),
}

impl MakeComputed for FontFamily {}

impl FontFamily {
  /// Named families ending in `sans-serif`. The trailing generic is the last resort: the
  /// fallback bucket only holds resolved `fontFamilies`, so an unresolved name would otherwise
  /// leave text unrendered.
  pub fn from_names(names: impl IntoIterator<Item = String>) -> Self {
    let mut tokens = names
      .into_iter()
      .map(FontFamilyToken::Owned)
      .collect::<Vec<_>>();
    tokens.push(FontFamilyToken::Generic(GenericFamily::SansSerif));

    Self(tokens.into())
  }

  /// The families as parley `FontFamilyName`s, in declaration order.
  pub(crate) fn names(&self) -> impl Iterator<Item = FontFamilyName<'_>> + Clone {
    self.0.iter().map(|token| match token {
      FontFamilyToken::Owned(name) => FontFamilyName::Named(name.as_str().into()),
      FontFamilyToken::Generic(generic) => FontFamilyName::Generic(*generic),
    })
  }

  pub(crate) fn from_parlance_generic(generic: GenericFamily) -> Self {
    Self(Arc::new([FontFamilyToken::Generic(generic)]))
  }
}

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

    Ok(Self(list.into()))
  }

  const VALID_TOKENS: &'static [CssToken] = FontFamilyToken::VALID_TOKENS;
}

impl TailwindPropertyParser for FontFamily {
  const NAMESPACES: &'static [TwNamespace] = &[TwNamespace::Font];

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sans" => Some(FontFamily::from_parlance_generic(GenericFamily::SansSerif)),
      "serif" => Some(FontFamily::from_parlance_generic(GenericFamily::Serif)),
      "mono" => Some(FontFamily::from_parlance_generic(GenericFamily::Monospace)),
      _ => None,
    }
  }
}

impl ToCss for FontFamilyToken {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Owned(name) => {
        let needs_quoting = name.contains(' ')
          || name.contains('"')
          || name.contains('\\')
          || name.chars().next().is_some_and(|c| c.is_ascii_digit());
        if needs_quoting {
          write_css_string(dest, name)
        } else {
          dest.write_str(name)
        }
      }
      Self::Generic(generic) => dest.write_str(match generic {
        GenericFamily::Serif => "serif",
        GenericFamily::SansSerif => "sans-serif",
        GenericFamily::Monospace => "monospace",
        GenericFamily::Cursive => "cursive",
        GenericFamily::Fantasy => "fantasy",
        GenericFamily::SystemUi => "system-ui",
        GenericFamily::Emoji => "emoji",
        _ => "sans-serif",
      }),
    }
  }
}

impl ToCss for FontFamily {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let mut first = true;
    for token in self.0.iter() {
      if !first {
        dest.write_str(", ")?;
      }
      first = false;
      token.to_css(dest)?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use parley::GenericFamily;

  use super::{FontFamily, FontFamilyToken};
  use crate::style::{FromCssStr, tw::TailwindPropertyParser};

  #[test]
  fn parses_single_generic_family() {
    assert_eq!(
      FontFamily::from_css_str("serif"),
      Ok(FontFamily(Arc::new([FontFamilyToken::Generic(
        GenericFamily::Serif,
      )])))
    );
  }

  #[test]
  fn parses_fallback_family_list() {
    assert_eq!(
      FontFamily::from_css_str("\"Inter\", Arial, serif"),
      Ok(FontFamily(Arc::new([
        FontFamilyToken::Owned(String::from("Inter")),
        FontFamilyToken::Owned(String::from("Arial")),
        FontFamilyToken::Generic(GenericFamily::Serif),
      ])))
    );
  }

  #[test]
  fn parses_unquoted_multi_word_family_name() {
    assert_eq!(
      FontFamily::from_css_str("Noto Sans TC"),
      Ok(FontFamily(Arc::new([FontFamilyToken::Owned(
        "Noto Sans TC".to_string()
      )])))
    );
  }

  #[test]
  fn from_names_appends_sans_serif_last_resort() {
    assert_eq!(
      FontFamily::from_names(["Geist".to_string(), "Inter".to_string()]),
      FontFamily(Arc::new([
        FontFamilyToken::Owned(String::from("Geist")),
        FontFamilyToken::Owned(String::from("Inter")),
        FontFamilyToken::Generic(GenericFamily::SansSerif),
      ]))
    );
  }

  #[test]
  fn parses_tailwind_aliases() {
    assert_eq!(
      FontFamily::parse_tw("sans"),
      Some(FontFamily(Arc::new([FontFamilyToken::Generic(
        GenericFamily::SansSerif,
      )])))
    );
    assert_eq!(
      FontFamily::parse_tw("mono"),
      Some(FontFamily(Arc::new([FontFamilyToken::Generic(
        GenericFamily::Monospace,
      )])))
    );
    assert_eq!(FontFamily::parse_tw("display"), None);
  }
}

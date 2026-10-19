use std::{
  fmt,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use cssparser::{CowRcStr, Parser, Token, match_ignore_ascii_case, serialize_string};

use crate::style::{
  FromCss, MakeComputed, ParseResult, ToCss, is_reserved_custom_ident, tw::Namespace,
  tw::TailwindPropertyParser,
};

/// Blink's `FontDescription::GenericFamilyType`: a CSS generic font family.
/// <https://drafts.csswg.org/css-fonts/#generic-font-families>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GenericFamilyType {
  Serif,
  SansSerif,
  Monospace,
  Cursive,
  Fantasy,
  SystemUi,
  UiSerif,
  UiSansSerif,
  UiMonospace,
  UiRounded,
  Emoji,
  Math,
  FangSong,
}

impl GenericFamilyType {
  const ALL: [(Self, &'static str); 13] = [
    (Self::Serif, "serif"),
    (Self::SansSerif, "sans-serif"),
    (Self::Monospace, "monospace"),
    (Self::Cursive, "cursive"),
    (Self::Fantasy, "fantasy"),
    (Self::SystemUi, "system-ui"),
    (Self::UiSerif, "ui-serif"),
    (Self::UiSansSerif, "ui-sans-serif"),
    (Self::UiMonospace, "ui-monospace"),
    (Self::UiRounded, "ui-rounded"),
    (Self::Emoji, "emoji"),
    (Self::Math, "math"),
    (Self::FangSong, "fangsong"),
  ];

  /// The generic family `keyword` names, in any case.
  pub(crate) fn parse(keyword: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(_, name)| name.eq_ignore_ascii_case(keyword))
      .map(|&(generic, _)| generic)
  }

  /// The CSS keyword.
  pub(crate) fn keyword(self) -> &'static str {
    Self::ALL
      .iter()
      .find(|&&(generic, _)| generic == self)
      .map_or("sans-serif", |&(_, name)| name)
  }
}

/// Represents a font family for text rendering.
/// Multi value fallback is supported.
#[derive(Debug, Clone)]
pub struct FontFamily {
  /// Unique to the stack it was built as, and shared by every clone of it.
  id: u64,
  tokens: Arc<[FontFamilyToken]>,
}

impl PartialEq for FontFamily {
  fn eq(&self, other: &Self) -> bool {
    self.tokens == other.tokens
  }
}

impl Default for FontFamily {
  fn default() -> Self {
    Self::from_generic(GenericFamilyType::SansSerif)
  }
}

/// One entry in a font-family fallback list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FontFamilyToken {
  /// A named family.
  Owned(String),
  /// A generic family such as `serif`.
  Generic(GenericFamilyType),
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
    tokens.push(FontFamilyToken::Generic(GenericFamilyType::SansSerif));

    Self::new(tokens)
  }

  fn new(tokens: impl Into<Arc<[FontFamilyToken]>>) -> Self {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    Self {
      id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
      tokens: tokens.into(),
    }
  }

  /// Names the stack it was built as, which every clone of it shares.
  pub(crate) fn id(&self) -> u64 {
    self.id
  }

  /// The families, in declaration order.
  pub(crate) fn tokens(&self) -> &[FontFamilyToken] {
    &self.tokens
  }

  pub(crate) fn from_generic(generic: GenericFamilyType) -> Self {
    Self::new([FontFamilyToken::Generic(generic)])
  }
}

impl<'i> FromCss<'i> for FontFamilyToken {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(name) = input.try_parse(|input| input.expect_string().map(ToString::to_string)) {
      return Ok(Self::Owned(name));
    }

    let mut family_name = custom_ident(input)?.to_string();

    while let Ok(ident) = input.try_parse(custom_ident) {
      family_name.push(' ');
      family_name.push_str(&ident);
    }

    if let Some(generic) = GenericFamilyType::parse(&family_name) {
      return Ok(Self::Generic(generic));
    }

    Ok(Self::Owned(family_name))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<family-name>", "<generic-name>"];
}

/// One `<custom-ident>` of an unquoted family name.
fn custom_ident<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, CowRcStr<'i>> {
  let location = input.current_source_location();
  let ident = input.expect_ident_cloned()?;

  if is_reserved_custom_ident(&ident) {
    return Err(location.new_unexpected_token_error(Token::Ident(ident)));
  }

  Ok(ident)
}

impl<'i> FromCss<'i> for FontFamily {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let list = input.parse_comma_separated(FontFamilyToken::from_css)?;

    Ok(Self::new(list))
  }

  const VALID_TOKENS: &'static [&'static str] = FontFamilyToken::VALID_TOKENS;
}

impl TailwindPropertyParser for FontFamily {
  const NAMESPACES: &'static [Namespace] = &[Namespace::Font];

  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sans" => Some(FontFamily::from_generic(GenericFamilyType::SansSerif)),
      "serif" => Some(FontFamily::from_generic(GenericFamilyType::Serif)),
      "mono" => Some(FontFamily::from_generic(GenericFamilyType::Monospace)),
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
          serialize_string(name, dest)
        } else {
          dest.write_str(name)
        }
      }
      Self::Generic(generic) => dest.write_str(generic.keyword()),
    }
  }
}

impl ToCss for FontFamily {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.tokens.to_css(dest)
  }
}

#[cfg(test)]
mod tests {
  use super::{FontFamily, FontFamilyToken, GenericFamilyType as GenericFamily};
  use crate::style::{FromCssStr, tw::TailwindPropertyParser};

  #[test]
  fn parses_single_generic_family() {
    assert_eq!(
      FontFamily::from_css_str("serif"),
      Ok(FontFamily::new([FontFamilyToken::Generic(
        GenericFamily::Serif,
      )]))
    );
  }

  #[test]
  fn parses_fallback_family_list() {
    assert_eq!(
      FontFamily::from_css_str("\"Inter\", Arial, serif"),
      Ok(FontFamily::new([
        FontFamilyToken::Owned(String::from("Inter")),
        FontFamilyToken::Owned(String::from("Arial")),
        FontFamilyToken::Generic(GenericFamily::Serif),
      ]))
    );
  }

  #[test]
  fn parses_unquoted_multi_word_family_name() {
    assert_eq!(
      FontFamily::from_css_str("Noto Sans TC"),
      Ok(FontFamily::new([FontFamilyToken::Owned(
        "Noto Sans TC".to_string()
      )]))
    );
  }

  #[test]
  fn from_names_appends_sans_serif_last_resort() {
    assert_eq!(
      FontFamily::from_names(["Geist".to_string(), "Inter".to_string()]),
      FontFamily::new([
        FontFamilyToken::Owned(String::from("Geist")),
        FontFamilyToken::Owned(String::from("Inter")),
        FontFamilyToken::Generic(GenericFamily::SansSerif),
      ])
    );
  }

  #[test]
  fn parses_tailwind_aliases() {
    assert_eq!(
      FontFamily::parse_tw("sans"),
      Some(FontFamily::new([FontFamilyToken::Generic(
        GenericFamily::SansSerif,
      )]))
    );
    assert_eq!(
      FontFamily::parse_tw("mono"),
      Some(FontFamily::new([FontFamilyToken::Generic(
        GenericFamily::Monospace,
      )]))
    );
    assert_eq!(FontFamily::parse_tw("display"), None);
  }
}

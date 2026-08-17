use std::{fmt, sync::Arc};

use cssparser::{Parser, Token};

use crate::style::{
  CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token,
};

/// A 4-byte OpenType tag (for example `wght`, `liga`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Tag([u8; 4]);

impl Tag {
  /// Creates a tag from a 4-byte array.
  pub const fn new(bytes: &[u8; 4]) -> Self {
    Self(*bytes)
  }

  /// Returns this tag as 4 bytes.
  pub const fn to_bytes(self) -> [u8; 4] {
    self.0
  }

  /// Parses a tag from a 4-character ASCII string, matching the OpenType tag grammar
  /// (printable ASCII or space in every position).
  fn parse(s: &str) -> Option<Self> {
    let bytes = s.as_bytes();
    if bytes.len() != 4 || !bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
      return None;
    }
    Some(Self([bytes[0], bytes[1], bytes[2], bytes[3]]))
  }

  pub(crate) fn into_parlance(self) -> parley::setting::Tag {
    parley::setting::Tag::from_bytes(self.0)
  }
}

impl fmt::Display for Tag {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = std::str::from_utf8(&self.0).unwrap_or("????");
    f.write_str(s)
  }
}

pub(crate) fn parse_opentype_tag<'i, T: FromCss<'i>>(
  input: &mut Parser<'i, '_>,
) -> ParseResult<'i, Tag> {
  let location = input.current_source_location();
  let tag_name = input.expect_string()?;
  if tag_name.len() != 4 || !tag_name.is_ascii() {
    return Err(unexpected_token!(
      T,
      location,
      &Token::QuotedString(tag_name.clone()),
    ));
  }
  Tag::parse(tag_name)
    .ok_or_else(|| unexpected_token!(T, location, &Token::QuotedString(tag_name.clone())))
}

/// An OpenType font feature setting (tag + value) from CSS `font-feature-settings`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FontFeature {
  /// The OpenType tag for this setting.
  pub tag: Tag,
  /// The feature value.
  pub value: u16,
}

impl FontFeature {
  /// Creates a new feature setting.
  pub const fn new(tag: Tag, value: u16) -> Self {
    Self { tag, value }
  }

  pub(crate) fn into_parlance(self) -> parley::FontFeature {
    parley::FontFeature {
      tag: self.tag.into_parlance(),
      value: self.value,
    }
  }
}

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
pub(crate) type FontFeatureSettings = Arc<[FontFeature]>;

impl MakeComputed for FontFeatureSettings {}

impl<'i> FromCss<'i> for FontFeatureSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Arc::new([]));
    }

    let list = input.parse_comma_separated(|input| {
      let tag = parse_opentype_tag::<FontFeatureSettings>(input)?;
      let value = if input.is_exhausted() {
        1
      } else {
        let location = input.current_source_location();
        match input.next()? {
          Token::Ident(st) if st.as_ref() == "on" => 1,
          Token::Ident(st) if st.as_ref() == "off" => 0,
          Token::Number {
            value, int_value, ..
          } => int_value.map(|v| v as u16).unwrap_or(*value as u16),
          token => {
            return Err(unexpected_token!(location, token));
          }
        }
      };

      Ok(FontFeature { tag, value })
    })?;

    Ok(list.into())
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for FontFeature {
  // An empty `font-feature-settings` list is the keyword `normal`.
  const EMPTY_LIST_KEYWORD: Option<&'static str> = Some("normal");

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "\"{}\" {}", self.tag, self.value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn empty_settings_serialize_as_normal() {
    assert_eq!(FontFeatureSettings::default().to_css_string(), "normal");
    assert_eq!(
      FontFeatureSettings::from_css_str("normal")
        .unwrap()
        .to_css_string(),
      "normal"
    );
  }
}

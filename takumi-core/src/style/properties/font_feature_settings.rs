use crate::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token};
use parley::{FontFeature, setting::Tag};
use std::fmt;

use crate::style::{CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult};

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

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
pub(crate) type FontFeatureSettings = Box<[FontFeature]>;

impl MakeComputed for FontFeatureSettings {}

impl<'i> FromCss<'i> for FontFeatureSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Box::new([]));
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

    Ok(list.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for parley::FontFeature {
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

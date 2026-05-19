use crate::layout::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token};
use parley::{FontFeature, setting::Tag};
use std::fmt;

use crate::layout::style::{CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult};

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
pub type FontFeatureSettings = Box<[FontFeature]>;

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
      let location = input.current_source_location();
      let tag_name = input.expect_string()?;
      if tag_name.len() != 4 || !tag_name.is_ascii() {
        return Err(unexpected_token!(
          location,
          &Token::QuotedString(tag_name.clone()),
        ));
      }

      let tag = Tag::parse(tag_name)
        .ok_or_else(|| unexpected_token!(location, &Token::QuotedString(tag_name.clone())))?;
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
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "\"{}\" {}", self.tag, self.value)
  }
}

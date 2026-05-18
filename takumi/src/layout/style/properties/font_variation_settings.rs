use crate::layout::style::{CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult};
use crate::layout::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token};
use parley::{FontVariation, setting::Tag};
use std::fmt;

/// Controls variable font axis values via CSS font-variation-settings property.
///
/// This allows fine-grained control over variable font characteristics like weight,
/// width, slant, and other custom axes defined in the font.
pub type FontVariationSettings = Box<[FontVariation]>;

impl MakeComputed for FontVariationSettings {}

impl<'i> FromCss<'i> for FontVariationSettings {
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
      let value = input.expect_number()?;

      Ok(FontVariation { tag, value })
    })?;

    Ok(list.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for parley::FontVariation {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "\"{}\" {}", self.tag, self.value)
  }
}

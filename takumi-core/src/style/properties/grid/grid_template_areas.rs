use std::fmt;

use cssparser::{Parser, Token};

use super::write_space_separated;
use crate::style::{FromCss, MakeComputed, ParseResult, ToCss, unexpected_token};

/// Represents `grid-template-areas` value
///
/// Supports either a 2D matrix of area names (use "." for empty) or a CSS string value
/// like: "a a ." "b b c"
#[derive(Default, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GridTemplateAreas(pub Vec<Vec<String>>);

impl MakeComputed for GridTemplateAreas {}

impl GridTemplateAreas {}

impl<'i> FromCss<'i> for GridTemplateAreas {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      if ident.as_ref() == "none" {
        return Ok(GridTemplateAreas(Vec::new()));
      }
      return Err(unexpected_token!(&Token::Ident(ident)));
    }

    let mut rows: Vec<Vec<String>> = Vec::new();

    while !input.is_exhausted() {
      let row = input.expect_ident_or_string()?;
      let cols: Vec<String> = row.split_whitespace().map(ToString::to_string).collect();
      if cols.is_empty() {
        return Err(unexpected_token!(&Token::Ident("".into()),));
      }
      rows.push(cols);
    }

    if let Some(width) = rows.first().map(Vec::len)
      && rows.iter().any(|r| r.len() != width)
    {
      return Err(unexpected_token!(&Token::Ident("inconsistent-rows".into()),));
    }

    Ok(GridTemplateAreas(rows))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<string>"];
}

impl ToCss for GridTemplateAreas {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.0.is_empty() {
      dest.write_str("none")
    } else {
      let mut first = true;
      for row in &self.0 {
        if !first {
          dest.write_str(" ")?;
        }
        first = false;
        dest.write_str("\"")?;
        write_space_separated(dest, row)?;
        dest.write_str("\"")?;
      }
      Ok(())
    }
  }
}

//! The `page` property: which named page an element's pages use, per CSS
//! Paged Media Level 3 §8.1. Only the paged backend consumes it.

use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token,
};

/// The value of `page`: `auto`, or the name an `@page <name>` rule styles.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PageName {
  /// The page of the nearest ancestor that names one, else the unnamed page.
  #[default]
  Auto,
  /// A named page.
  Named(Arc<str>),
}

impl PageName {
  /// The name this resolves to, or `None` for the unnamed page.
  pub fn name(&self) -> Option<&str> {
    match self {
      Self::Auto => None,
      Self::Named(name) => Some(name),
    }
  }
}

impl MakeComputed for PageName {}

impl<'i> FromCss<'i> for PageName {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::CustomIdent),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(ident) = token else {
      return Err(unexpected_token!(location, token));
    };

    match_ignore_ascii_case! {ident,
      "auto" => Ok(Self::Auto),
      "inherit" | "initial" | "unset" | "revert" | "revert-layer" | "default" => Err(unexpected_token!(location, token)),
      _ => Ok(Self::Named(Arc::from(ident.as_ref()))),
    }
  }
}

impl ToCss for PageName {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Named(name) => dest.write_str(name),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::properties::traits::FromCssStr;

  #[test]
  fn parses_auto_and_a_custom_ident() {
    assert_eq!(PageName::from_css_str("auto"), Ok(PageName::Auto));
    assert_eq!(PageName::from_css_str("AUTO"), Ok(PageName::Auto));
    assert_eq!(
      PageName::from_css_str("cover"),
      Ok(PageName::Named("cover".into()))
    );
    assert!(PageName::from_css_str("inherit").is_err());
    assert!(PageName::from_css_str("\"cover\"").is_err());
    assert!(PageName::from_css_str("1").is_err());
  }
}

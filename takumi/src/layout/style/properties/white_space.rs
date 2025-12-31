use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::layout::style::{
  CssToken, FromCss, ParseResult, TextWrapMode, WhiteSpaceCollapse, tw::TailwindPropertyParser,
};

/// Controls how whitespace should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WhiteSpace {
  /// Controls whether text should be wrapped.
  pub text_wrap_mode: TextWrapMode,
  /// Controls how whitespace should be collapsed.
  pub white_space_collapse: WhiteSpaceCollapse,
}

impl TailwindPropertyParser for WhiteSpace {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "normal" => Some(WhiteSpace::normal()),
      "nowrap" => Some(WhiteSpace::no_wrap()),
      "pre" => Some(WhiteSpace::pre()),
      "pre-wrap" => Some(WhiteSpace::pre_wrap()),
      "pre-line" => Some(WhiteSpace::pre_line()),
      _ => None,
    }
  }
}

impl WhiteSpace {
  /// Creates a `WhiteSpace` instance with `nowrap` behavior.
  pub const fn no_wrap() -> Self {
    Self {
      text_wrap_mode: TextWrapMode::NoWrap,
      white_space_collapse: WhiteSpaceCollapse::Collapse,
    }
  }

  /// Creates a `WhiteSpace` instance with `normal` behavior.
  pub const fn normal() -> Self {
    Self {
      text_wrap_mode: TextWrapMode::Wrap,
      white_space_collapse: WhiteSpaceCollapse::Collapse,
    }
  }

  /// Creates a `WhiteSpace` instance with `pre` behavior.
  pub const fn pre() -> Self {
    Self {
      text_wrap_mode: TextWrapMode::NoWrap,
      white_space_collapse: WhiteSpaceCollapse::Preserve,
    }
  }

  /// Creates a `WhiteSpace` instance with `pre-wrap` behavior.
  pub const fn pre_wrap() -> Self {
    Self {
      text_wrap_mode: TextWrapMode::Wrap,
      white_space_collapse: WhiteSpaceCollapse::Preserve,
    }
  }

  /// Creates a `WhiteSpace` instance with `pre-line` behavior.
  pub const fn pre_line() -> Self {
    Self {
      text_wrap_mode: TextWrapMode::Wrap,
      white_space_collapse: WhiteSpaceCollapse::PreserveBreaks,
    }
  }
}

fn parse_white_space_keyword<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, WhiteSpace> {
  let location = input.current_source_location();
  let ident = input.expect_ident()?;

  match_ignore_ascii_case! {&ident,
    "normal" => Ok(WhiteSpace::normal()),
    "pre" => Ok(WhiteSpace::pre()),
    "pre-wrap" => Ok(WhiteSpace::pre_wrap()),
    "pre-line" => Ok(WhiteSpace::pre_line()),
    _ => Err(location.new_basic_unexpected_token_error(Token::Ident(ident.clone())).into())
  }
}

impl<'i> FromCss<'i> for WhiteSpace {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Try parsing as a keyword first
    if let Ok(ident) = input.try_parse(parse_white_space_keyword) {
      return Ok(ident);
    }

    // Otherwise parse individual components
    let mut text_wrap_mode = TextWrapMode::default();
    let mut white_space_collapse = WhiteSpaceCollapse::default();

    while !input.is_exhausted() {
      if let Ok(value) = input.try_parse(TextWrapMode::from_css) {
        text_wrap_mode = value;
        continue;
      }

      if let Ok(value) = input.try_parse(WhiteSpaceCollapse::from_css) {
        white_space_collapse = value;
        continue;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(WhiteSpace {
      text_wrap_mode,
      white_space_collapse,
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("normal"),
      CssToken::Keyword("pre"),
      CssToken::Keyword("pre-wrap"),
      CssToken::Keyword("pre-line"),
      CssToken::Token("text-wrap-mode"),
      CssToken::Token("white-space-collapse"),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_white_space_no_wrap() {
    assert_eq!(WhiteSpace::from_str("nowrap"), Ok(WhiteSpace::no_wrap()));
  }
}

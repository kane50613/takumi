use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{
  FromCss, ParseResult, TextWrapMode, WhiteSpaceCollapse, tw::TailwindPropertyParser,
};

/// Controls how whitespace should be handled.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(try_from = "WhiteSpaceValue", into = "WhiteSpaceValue")]
#[ts(as = "WhiteSpaceValue")]
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WhiteSpaceKeywords {
  Normal,
  Pre,
  PreWrap,
  PreLine,
}

impl<'i> FromCss<'i> for WhiteSpaceKeywords {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! {&ident,
      "normal" => Ok(WhiteSpaceKeywords::Normal),
      "pre" => Ok(WhiteSpaceKeywords::Pre),
      "pre-wrap" => Ok(WhiteSpaceKeywords::PreWrap),
      "pre-line" => Ok(WhiteSpaceKeywords::PreLine),
      _ => {
        let token = Token::Ident(ident.clone());
        Err(input.new_basic_unexpected_token_error(token).into())
      }
    }
  }
}

impl From<WhiteSpaceKeywords> for WhiteSpace {
  fn from(keyword: WhiteSpaceKeywords) -> Self {
    match keyword {
      WhiteSpaceKeywords::Normal => WhiteSpace::normal(),
      WhiteSpaceKeywords::Pre => WhiteSpace::pre(),
      WhiteSpaceKeywords::PreWrap => WhiteSpace::pre_wrap(),
      WhiteSpaceKeywords::PreLine => WhiteSpace::pre_line(),
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(untagged)]
pub(crate) enum WhiteSpaceValue {
  Keyword(WhiteSpaceKeywords),
  Structured {
    #[serde(rename = "textWrapMode")]
    text_wrap_mode: TextWrapMode,
    #[serde(rename = "whiteSpaceCollapse")]
    white_space_collapse: WhiteSpaceCollapse,
  },
  Css(String),
}

impl TryFrom<WhiteSpaceValue> for WhiteSpace {
  type Error = String;

  fn try_from(value: WhiteSpaceValue) -> Result<Self, Self::Error> {
    match value {
      WhiteSpaceValue::Keyword(keyword) => Ok(keyword.into()),
      WhiteSpaceValue::Structured {
        text_wrap_mode,
        white_space_collapse,
      } => Ok(WhiteSpace {
        text_wrap_mode,
        white_space_collapse,
      }),
      WhiteSpaceValue::Css(css) => Ok(WhiteSpace::from_str(&css).map_err(|e| e.to_string())?),
    }
  }
}

impl<'i> FromCss<'i> for WhiteSpace {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut text_wrap_mode = None;
    let mut white_space_collapse = None;

    while !input.is_exhausted() {
      if let Ok(value) = input.try_parse(WhiteSpaceKeywords::from_css) {
        return Ok(value.into());
      }

      if let Ok(value) = input.try_parse(TextWrapMode::from_css) {
        text_wrap_mode = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(WhiteSpaceCollapse::from_css) {
        white_space_collapse = Some(value);
        continue;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(WhiteSpace {
      text_wrap_mode: text_wrap_mode.unwrap_or_default(),
      white_space_collapse: white_space_collapse.unwrap_or_default(),
    })
  }
}

impl From<WhiteSpace> for WhiteSpaceValue {
  fn from(value: WhiteSpace) -> Self {
    WhiteSpaceValue::Structured {
      text_wrap_mode: value.text_wrap_mode,
      white_space_collapse: value.white_space_collapse,
    }
  }
}

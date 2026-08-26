use std::fmt;

use cssparser::{Parser, serialize_string};

use crate::style::{
  Animatable, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
  declare_enum_from_css_impl, tw::TailwindPropertyParser,
};

/// `block-ellipsis`: `none | auto | <string>`. Inherited.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum BlockEllipsis {
  /// No ellipsis is shown when content is clamped.
  #[default]
  None,
  /// Use the user-agent default ellipsis (`…`).
  Auto,
  /// Use a specific string as the ellipsis.
  String(String),
}

impl MakeComputed for BlockEllipsis {}
impl Animatable for BlockEllipsis {}

impl<'i> FromCss<'i> for BlockEllipsis {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(Self::None);
    }
    if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
      return Ok(Self::Auto);
    }
    Ok(Self::String(input.expect_string_cloned()?.to_string()))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl ToCss for BlockEllipsis {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::None => dest.write_str("none"),
      Self::Auto => dest.write_str("auto"),
      Self::String(value) => serialize_string(value, dest),
    }
  }
}

/// `continue`: `normal | collapse`. Not inherited.
///
/// `collapse` turns the box into a fragmentation context that discards content
/// past `max-lines`; `line-clamp`/`-webkit-line-clamp` set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Continue {
  /// Default fragmentation behavior; content is not clamped.
  #[default]
  Normal,
  /// Discard content past the `max-lines` limit.
  Collapse,
}

declare_enum_from_css_impl!(
  Continue,
  "normal" => Continue::Normal,
  "collapse" => Continue::Collapse,
);

impl Animatable for Continue {}

/// Parsed `line-clamp` shorthand: `none | <integer> || <'block-ellipsis'>`.
///
/// Expands to `max-lines`, `block-ellipsis`, and `continue`. `-webkit-line-clamp`
/// shares this parser via vendor-prefix stripping.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct LineClamp {
  /// `max-lines` longhand.
  pub max_lines: Option<u32>,
  /// `block-ellipsis` longhand.
  pub block_ellipsis: BlockEllipsis,
  /// `continue` longhand.
  pub line_continue: Continue,
}

impl LineClamp {
  fn clamp(max_lines: Option<u32>, block_ellipsis: BlockEllipsis) -> Self {
    Self {
      max_lines,
      block_ellipsis,
      line_continue: Continue::Collapse,
    }
  }
}

impl From<u32> for LineClamp {
  fn from(count: u32) -> Self {
    if count >= 1 {
      Self::clamp(Some(count), BlockEllipsis::Auto)
    } else {
      Self::default()
    }
  }
}

impl MakeComputed for LineClamp {}

impl<'i> FromCss<'i> for LineClamp {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(Self::default());
    }

    let count = input.expect_integer()?;
    if count < 1 {
      return Ok(Self::default());
    }

    let block_ellipsis = if input.is_exhausted() {
      BlockEllipsis::Auto
    } else {
      BlockEllipsis::from_css(input)?
    };

    Ok(Self::clamp(Some(count as u32), block_ellipsis))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Syntax(CssSyntaxKind::Integer),
    CssToken::Syntax(CssSyntaxKind::String),
  ];
}

impl TailwindPropertyParser for LineClamp {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Self::default());
    }
    let count = token.parse::<u32>().ok()?;
    if count == 0 {
      return None;
    }
    Some(Self::clamp(Some(count), BlockEllipsis::Auto))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_block_ellipsis() {
    for (css, expected) in [
      ("none", BlockEllipsis::None),
      ("auto", BlockEllipsis::Auto),
      ("\"foo\"", BlockEllipsis::String("foo".to_string())),
    ] {
      assert_eq!(
        BlockEllipsis::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_block_ellipsis_round_trip() {
    for css in ["none", "auto", "\"foo\""] {
      let parsed = BlockEllipsis::from_css_str(css).unwrap();
      let reparsed = BlockEllipsis::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn test_parse_block_ellipsis_invalid() {
    assert!(BlockEllipsis::from_css_str("123").is_err());
  }

  #[test]
  fn test_parse_line_clamp() {
    for (css, expected) in [
      ("none", LineClamp::default()),
      (
        "3",
        LineClamp {
          max_lines: Some(3),
          block_ellipsis: BlockEllipsis::Auto,
          line_continue: Continue::Collapse,
        },
      ),
      (
        "2 \"foo\"",
        LineClamp {
          max_lines: Some(2),
          block_ellipsis: BlockEllipsis::String("foo".to_string()),
          line_continue: Continue::Collapse,
        },
      ),
      ("0", LineClamp::default()),
    ] {
      assert_eq!(
        LineClamp::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  // LineClamp has no ToCss impl, so no round-trip test.

  #[test]
  fn test_parse_line_clamp_invalid() {
    assert!(LineClamp::from_css_str("bogus").is_err());
    assert!(LineClamp::from_css_str("1.5").is_err());
  }
}

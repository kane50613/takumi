use std::fmt;

use cssparser::{BasicParseErrorKind, ParseError as CssParseError, Parser, serialize_string};

use crate::style::{
  Animatable, FromCss, MakeComputed, ParseResult, ToCss, impl_css_enum, tw::TailwindPropertyParser,
};

/// `block-ellipsis`: `no-ellipsis | auto | <string>`. Inherited.
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
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|i| i.expect_ident_matching("no-ellipsis"))
      .or_else(|_| input.try_parse(|i| i.expect_ident_matching("none")))
      .is_ok()
    {
      return Ok(Self::None);
    }
    if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
      return Ok(Self::Auto);
    }
    Ok(Self::String(input.expect_string_cloned()?.to_string()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "no-ellipsis", "auto", "<string>"];
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

/// `continue`: `normal | collapse | -webkit-legacy`. Not inherited.
///
/// `collapse` turns the box into a fragmentation context that discards content past `max-lines`;
/// `line-clamp` sets it. `-webkit-legacy`, which `-webkit-line-clamp` sets, does so only in a
/// `display: -webkit-box` with `-webkit-box-orient: vertical`.
/// <https://drafts.csswg.org/css-overflow-4/#continue>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Continue {
  /// Default fragmentation behavior; content is not clamped.
  #[default]
  Normal,
  /// Discard content past the `max-lines` limit.
  Collapse,
  /// Discard content past the `max-lines` limit in a vertical legacy `-webkit-box`.
  WebkitLegacy,
}

impl_css_enum!(
  Continue,
  "normal" => Continue::Normal,
  "collapse" => Continue::Collapse,
  "-webkit-legacy" => Continue::WebkitLegacy,
);

/// A CSS `<integer [1,∞]>`.
fn parse_positive_integer<'i>(input: &mut Parser<'i>) -> ParseResult<'i, u32> {
  match u32::from_css(input)? {
    0 => Err(CssParseError::from_basic_kind(
      BasicParseErrorKind::QualifiedRuleInvalid,
    )),
    count => Ok(count),
  }
}

/// The `line-clamp` shorthand of `max-lines`, `block-ellipsis`, and `continue`.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct LineClamp {
  /// `max-lines` longhand, none to clamp by the block size alone.
  pub max_lines: Option<u32>,
  /// Blink's `HasAutoKeyword`, which the `max-lines` longhand drops beside a count.
  pub max_lines_auto: bool,
  /// `block-ellipsis` longhand.
  pub block_ellipsis: BlockEllipsis,
  /// `continue` longhand.
  pub line_continue: Continue,
}

impl LineClamp {
  fn clamp(lines: u32) -> Self {
    Self {
      max_lines: Some(lines),
      max_lines_auto: false,
      block_ellipsis: BlockEllipsis::Auto,
      line_continue: Continue::Collapse,
    }
  }
}

impl From<u32> for LineClamp {
  fn from(count: u32) -> Self {
    if count >= 1 {
      Self::clamp(count)
    } else {
      Self::default()
    }
  }
}

impl MakeComputed for LineClamp {}

impl<'i> FromCss<'i> for LineClamp {
  /// Blink's `AlternativeLineClampShorthand::ParseShorthand`, with a `<string>` ellipsis too.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(Self::default());
    }
    // A count below one turns the clamp off.
    if input
      .try_parse(|i| match i.expect_integer() {
        Ok(count) if count < 1 => Ok(()),
        _ => Err(i.new_error_for_next_token::<()>()),
      })
      .is_ok()
    {
      return Ok(Self::default());
    }

    let mut lines = None;
    let mut auto = false;
    let mut block_ellipsis = None;
    let mut line_continue = Continue::Collapse;

    while !input.is_exhausted() {
      if input
        .try_parse(|i| i.expect_ident_matching("-webkit-legacy"))
        .is_ok()
      {
        line_continue = Continue::WebkitLegacy;
        break;
      }
      if !auto && input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
        auto = true;
      } else if block_ellipsis.is_none()
        && let Ok(value) = input.try_parse(BlockEllipsis::from_css)
      {
        block_ellipsis = Some(value);
      } else if lines.is_none()
        && let Ok(count) = input.try_parse(parse_positive_integer)
      {
        lines = Some(count);
      } else {
        break;
      }
    }

    if lines.is_none() && !auto && block_ellipsis.is_none() {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self {
      max_lines: lines,
      max_lines_auto: auto,
      block_ellipsis: block_ellipsis.unwrap_or(BlockEllipsis::Auto),
      line_continue,
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "<integer>", "auto", "<string>"];
}

/// Parsed `-webkit-line-clamp` legacy shorthand: `none | <integer>`, which expands as
/// `line-clamp` does with `continue: -webkit-legacy`.
/// <https://drafts.csswg.org/css-overflow-4/#webkit-line-clamp>
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WebkitLineClamp(pub LineClamp);

impl MakeComputed for WebkitLineClamp {}

impl<'i> FromCss<'i> for WebkitLineClamp {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(Self::default());
    }

    let count = input.expect_integer()?;

    if count < 1 {
      return Ok(Self::default());
    }

    Ok(Self(LineClamp {
      line_continue: Continue::WebkitLegacy,
      ..LineClamp::clamp(count as u32)
    }))
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "<integer>"];
}

impl TailwindPropertyParser for WebkitLineClamp {}

impl TailwindPropertyParser for LineClamp {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Self::default());
    }
    let count = token.parse::<u32>().ok()?;
    if count == 0 {
      return None;
    }
    Some(Self::clamp(count))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_block_ellipsis() {
    for (css, expected) in [
      ("no-ellipsis", BlockEllipsis::None),
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
    for css in ["none", "no-ellipsis", "auto", "\"foo\""] {
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
      ("3", LineClamp::clamp(3)),
      (
        "2 \"foo\"",
        LineClamp {
          block_ellipsis: BlockEllipsis::String("foo".to_string()),
          ..LineClamp::clamp(2)
        },
      ),
      (
        "auto",
        LineClamp {
          max_lines: None,
          max_lines_auto: true,
          ..LineClamp::clamp(1)
        },
      ),
      (
        "4 auto",
        LineClamp {
          max_lines_auto: true,
          ..LineClamp::clamp(4)
        },
      ),
      (
        "auto no-ellipsis",
        LineClamp {
          max_lines: None,
          max_lines_auto: true,
          block_ellipsis: BlockEllipsis::None,
          line_continue: Continue::Collapse,
        },
      ),
      (
        "2 -webkit-legacy",
        LineClamp {
          line_continue: Continue::WebkitLegacy,
          ..LineClamp::clamp(2)
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

  #[test]
  fn test_parse_line_clamp_invalid() {
    assert!(LineClamp::from_css_str("bogus").is_err());
    assert!(LineClamp::from_css_str("1.5").is_err());
  }
}

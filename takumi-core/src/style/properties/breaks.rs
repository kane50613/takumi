//! CSS fragmentation properties: `break-before`, `break-after`, `break-inside`,
//! `widows`, `orphans`. Only the paged backend consumes them; other backends
//! ignore them.

use std::fmt;

use cssparser::{BasicParseErrorKind, Parser};

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, SizingContext,
  ToCss, declare_enum_from_css_impl, lerp,
};

/// A forced-break value for `break-before` / `break-after`. Pagination has no
/// left and right pages, so the legacy `left` and `right` become a plain page
/// break.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BreakBetween {
  /// No forced break.
  #[default]
  Auto,
  /// Force a page break on this edge of the box.
  Page,
}

impl MakeComputed for BreakBetween {}

impl<'i> FromCss<'i> for BreakBetween {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Keyword("page"),
    CssToken::Keyword("always"),
    CssToken::Keyword("left"),
    CssToken::Keyword("right"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    let cssparser::Token::Ident(ident) = token else {
      return Err(crate::style::unexpected_token!(location, token));
    };

    cssparser::match_ignore_ascii_case! {&ident,
      "auto" => Ok(Self::Auto),
      "page" | "always" | "left" | "right" => Ok(Self::Page),
      _ => Err(crate::style::unexpected_token!(location, token)),
    }
  }
}

impl ToCss for BreakBetween {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(match self {
      Self::Auto => "auto",
      Self::Page => "page",
    })
  }
}

/// A `break-inside` value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BreakInside {
  /// The box may be split across pages.
  #[default]
  Auto,
  /// Keep the whole box on one page.
  Avoid,
}

declare_enum_from_css_impl!(
  BreakInside,
  "auto" => BreakInside::Auto,
  "avoid" => BreakInside::Avoid
);

/// A `widows` / `orphans` value: the fewest lines of a block a page break may
/// leave on either side, per CSS 2 §13.3.2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinLines(u32);

impl Default for MinLines {
  fn default() -> Self {
    Self(2)
  }
}

impl MinLines {
  /// The minimum line count as a usize for solver arithmetic.
  pub fn get(&self) -> usize {
    self.0 as usize
  }
}

impl From<u32> for MinLines {
  fn from(lines: u32) -> Self {
    Self(lines.max(1))
  }
}

impl MakeComputed for MinLines {}

impl Animatable for MinLines {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    self.0 = lerp(from.0 as f32, to.0 as f32, progress).round() as u32;
  }
}

impl<'i> FromCss<'i> for MinLines {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let value = input.expect_integer()?;

    if value < 1 {
      return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    Ok(Self(value as u32))
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Number)];
}

impl ToCss for MinLines {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self.0)
  }
}

/// A `box-decoration-break` value, deciding how box decorations paint across
/// page fragments.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BoxDecorationBreak {
  /// Decorations paint as if the box were unfragmented, then get sliced: the
  /// edge at a break is open.
  #[default]
  Slice,
  /// Every fragment paints its own complete decorations.
  Clone,
}

declare_enum_from_css_impl!(
  BoxDecorationBreak,
  "slice" => BoxDecorationBreak::Slice,
  "clone" => BoxDecorationBreak::Clone
);

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::properties::traits::FromCssStr;

  #[test]
  fn parses_break_between() {
    assert_eq!(BreakBetween::from_css_str("auto"), Ok(BreakBetween::Auto));
    assert_eq!(BreakBetween::from_css_str("page"), Ok(BreakBetween::Page));
    assert!(BreakBetween::from_css_str("column").is_err());
  }

  #[test]
  fn the_legacy_forced_break_keywords_reach_a_page_break() {
    for keyword in ["always", "left", "right"] {
      assert_eq!(BreakBetween::from_css_str(keyword), Ok(BreakBetween::Page));
    }

    assert!(BreakBetween::from_css_str("avoid").is_err());
  }

  #[test]
  fn parses_box_decoration_break() {
    assert_eq!(
      BoxDecorationBreak::from_css_str("slice"),
      Ok(BoxDecorationBreak::Slice)
    );
    assert_eq!(
      BoxDecorationBreak::from_css_str("clone"),
      Ok(BoxDecorationBreak::Clone)
    );
  }

  #[test]
  fn parses_min_lines() {
    assert_eq!(MinLines::from_css_str("1"), Ok(MinLines(1)));
    assert_eq!(MinLines::from_css_str("3"), Ok(MinLines(3)));
    assert!(MinLines::from_css_str("0").is_err());
    assert!(MinLines::from_css_str("-2").is_err());
    assert!(MinLines::from_css_str("2.5").is_err());
    assert!(MinLines::from_css_str("auto").is_err());
  }

  #[test]
  fn min_lines_from_clamps() {
    assert_eq!(MinLines::from(0), MinLines(1));
    assert_eq!(MinLines::from(4), MinLines(4));
    assert_eq!(MinLines::default(), MinLines(2));
  }

  #[test]
  fn parses_break_inside() {
    assert_eq!(BreakInside::from_css_str("auto"), Ok(BreakInside::Auto));
    assert_eq!(BreakInside::from_css_str("avoid"), Ok(BreakInside::Avoid));
    assert!(BreakInside::from_css_str("avoid-page").is_err());
  }
}

use std::fmt;

use cssparser::{Parser, Token};

use crate::style::{
  Animatable, FromCss, MakeComputed, ParseResult, ToCss, tw::TailwindPropertyParser,
  unexpected_token,
};

/// Placement of a grid item along one axis.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum GridPlacement {
  /// Auto-placed.
  Auto,
  /// Span count
  Span(GridPlacementSpan),
  /// Line index (1-based)
  Line(i16),
  /// Named grid area
  Named(String),
  /// The nth line of a name, counted from the end when negative
  NamedLine(i16, String),
  /// A span to the nth line of a name
  NamedSpan(u16, String),
}

impl MakeComputed for GridPlacement {}

impl Animatable for GridPlacement {}

impl Default for GridPlacement {
  fn default() -> Self {
    Self::auto()
  }
}

impl GridPlacement {
  /// Auto placement.
  pub const fn auto() -> Self {
    Self::Auto
  }

  /// Span placement
  pub const fn span(span: u16) -> Self {
    Self::Span(GridPlacementSpan::Span(span))
  }
}

impl TailwindPropertyParser for GridPlacement {
  fn parse_tw(token: &str) -> Option<Self> {
    token.parse::<i16>().map(Self::Line).ok()
  }
}

/// Represents a grid placement span
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GridPlacementSpan {
  /// Span count
  Span(u16),
}

impl<'i> FromCss<'i> for GridPlacementSpan {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    Ok(GridPlacementSpan::Span(
      input.expect_integer()?.max(1) as u16
    ))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<integer>"];
}

impl TailwindPropertyParser for GridPlacementSpan {
  fn parse_tw(token: &str) -> Option<Self> {
    token.parse::<u16>().map(Self::Span).ok()
  }
}

/// Blink's `ConsumeCustomIdentForGridLine`: a `<custom-ident>` other than `auto` or `span`.
fn parse_grid_line_name<'i>(input: &mut Parser<'i>) -> ParseResult<'i, String> {
  let ident = input.expect_ident_cloned()?;

  if ident.eq_ignore_ascii_case("auto") || ident.eq_ignore_ascii_case("span") {
    return Err(unexpected_token!(GridPlacement, &Token::Ident(ident)));
  }
  Ok(ident.to_string())
}

/// Blink's `ConsumeIdent<CSSValueID::kSpan>`.
fn parse_span_keyword<'i>(input: &mut Parser<'i>) -> ParseResult<'i, ()> {
  input.expect_ident_matching("span")?;
  Ok(())
}

impl<'i> FromCss<'i> for GridPlacement {
  /// Blink's `ConsumeGridLine`: `auto | <custom-ident> | [ <integer> && <custom-ident>? ] |
  /// [ span && [ <integer> || <custom-ident> ] ]`.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(GridPlacement::auto());
    }

    let (has_span, integer, name) = if let Ok(integer) = input.try_parse(Parser::expect_integer) {
      let name = input.try_parse(parse_grid_line_name).ok();

      (
        input.try_parse(parse_span_keyword).is_ok(),
        Some(integer),
        name,
      )
    } else if input.try_parse(parse_span_keyword).is_ok() {
      let integer = input.try_parse(Parser::expect_integer).ok();
      let name = input.try_parse(parse_grid_line_name).ok();

      (
        true,
        integer.or_else(|| input.try_parse(Parser::expect_integer).ok()),
        name,
      )
    } else {
      let name = parse_grid_line_name(input)?;
      let integer = input.try_parse(Parser::expect_integer).ok();
      let has_span = input.try_parse(parse_span_keyword).is_ok();

      if !has_span && integer.is_none() {
        return Ok(GridPlacement::Named(name));
      }
      (has_span, integer, Some(name))
    };

    if (has_span && integer.is_none() && name.is_none())
      || integer == Some(0)
      || (has_span && integer.is_some_and(|integer| integer < 0))
    {
      return Err(input.new_error_for_next_token());
    }

    // Blink clamps the integer to the grid's largest line count it allows.
    let integer = integer.map(|integer| integer.clamp(-10_000, 10_000));

    Ok(match (has_span, integer, name) {
      (true, integer, None) => GridPlacement::span(integer.unwrap_or(1) as u16),
      (true, integer, Some(name)) => GridPlacement::NamedSpan(integer.unwrap_or(1) as u16, name),
      (false, Some(integer), None) => GridPlacement::Line(integer as i16),
      (false, integer, name) => {
        GridPlacement::NamedLine(integer.unwrap_or(1) as i16, name.unwrap_or_default())
      }
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["auto", "span", "<number>", "<ident>"];
}

impl ToCss for GridPlacementSpan {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Span(span) => write!(dest, "{span}"),
    }
  }
}

impl ToCss for GridPlacement {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Auto => dest.write_str("auto"),
      Self::Span(span) => {
        dest.write_str("span ")?;
        span.to_css(dest)
      }
      Self::Line(line) => write!(dest, "{line}"),
      Self::Named(name) => dest.write_str(name),
      Self::NamedLine(line, name) => write!(dest, "{line} {name}"),
      Self::NamedSpan(span, name) => write!(dest, "span {span} {name}"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_placement() {
    assert_eq!(
      GridPlacement::from_css_str("auto"),
      Ok(GridPlacement::auto())
    );

    assert_eq!(
      GridPlacement::from_css_str("span 2"),
      Ok(GridPlacement::span(2))
    );

    assert_eq!(
      GridPlacement::from_css_str("span name"),
      Ok(GridPlacement::NamedSpan(1, "name".to_owned()))
    );

    assert_eq!(
      GridPlacement::from_css_str("name -2"),
      Ok(GridPlacement::NamedLine(-2, "name".to_owned()))
    );

    assert_eq!(
      GridPlacement::from_css_str("span 2 name"),
      Ok(GridPlacement::NamedSpan(2, "name".to_owned()))
    );

    assert!(GridPlacement::from_css_str("0").is_err());
    assert!(GridPlacement::from_css_str("span -1").is_err());
    assert!(GridPlacement::from_css_str("span").is_err());

    assert_eq!(GridPlacement::from_css_str("3"), Ok(GridPlacement::Line(3)));

    assert_eq!(
      GridPlacement::from_css_str("-1"),
      Ok(GridPlacement::Line(-1))
    );

    assert_eq!(
      GridPlacement::from_css_str("header"),
      Ok(GridPlacement::Named("header".to_string()))
    );
  }
}

use cssparser::{Parser, Token};

use crate::layout::style::{FromCss, ParseResult, tw::TailwindPropertyParser};

/// Represents a grid placement with serde support
#[derive(Debug, Clone, PartialEq)]
pub enum GridPlacement {
  /// Keyword placement
  Keyword(GridPlacementKeyword),
  /// Span count
  Span(GridPlacementSpan),
  /// Line index (1-based)
  Line(i16),
  /// Named grid area
  Named(String),
}

impl Default for GridPlacement {
  fn default() -> Self {
    Self::auto()
  }
}

impl GridPlacement {
  /// Auto placement
  pub const fn auto() -> Self {
    Self::Keyword(GridPlacementKeyword::Auto)
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

/// Represents a grid placement keyword
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum GridPlacementKeyword {
  /// Auto placement
  #[default]
  Auto,
}

/// Represents a grid placement span
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridPlacementSpan {
  /// Span count
  Span(u16),
}

impl<'i> FromCss<'i> for GridPlacementSpan {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(
      input
        .expect_integer()
        .map(|n| GridPlacementSpan::Span(n.max(1) as u16))?,
    )
  }
}

impl TailwindPropertyParser for GridPlacementSpan {
  fn parse_tw(token: &str) -> Option<Self> {
    token.parse::<u16>().map(Self::Span).ok()
  }
}

// Note: GridPlacement has a custom conversion due to its complex nature
impl From<GridPlacement> for taffy::GridPlacement {
  fn from(placement: GridPlacement) -> Self {
    match placement {
      GridPlacement::Keyword(GridPlacementKeyword::Auto) => taffy::GridPlacement::Auto,
      GridPlacement::Line(line) => taffy::GridPlacement::Line(line.into()),
      GridPlacement::Span(GridPlacementSpan::Span(span)) => taffy::GridPlacement::Span(span),
      GridPlacement::Named(_) => taffy::GridPlacement::Auto,
    }
  }
}

impl<'i> FromCss<'i> for GridPlacement {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      if ident.eq_ignore_ascii_case("auto") {
        return Ok(GridPlacement::auto());
      }

      if ident.eq_ignore_ascii_case("span") {
        // Next token should be a number or ident
        // Try integer first
        if let Ok(span) = input.try_parse(GridPlacementSpan::from_css) {
          return Ok(GridPlacement::Span(span));
        }

        // Try identifier span name (treated as span 1 for named; enum only carries count)
        if let Ok(_name) = input.try_parse(Parser::expect_ident_cloned) {
          return Ok(GridPlacement::span(1));
        }

        // If neither, error
        return Err(input.new_error_for_next_token());
      }

      // Any other ident is a named line
      return Ok(GridPlacement::Named(ident.to_string()));
    }

    // Try a line index (number, may be negative)
    let location = input.current_source_location();
    let token = input.next()?;
    match *token {
      Token::Number {
        int_value, value, ..
      } => {
        let v: i32 = int_value.unwrap_or(value as i32);
        Ok(GridPlacement::Line(v as i16))
      }
      _ => Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_placement() {
    assert_eq!(GridPlacement::from_str("auto"), Ok(GridPlacement::auto()));

    assert_eq!(
      GridPlacement::from_str("span 2"),
      Ok(GridPlacement::span(2))
    );

    assert_eq!(
      GridPlacement::from_str("span name"),
      Ok(GridPlacement::span(1))
    );

    assert_eq!(GridPlacement::from_str("3"), Ok(GridPlacement::Line(3)));

    assert_eq!(GridPlacement::from_str("-1"), Ok(GridPlacement::Line(-1)));

    assert_eq!(
      GridPlacement::from_str("header"),
      Ok(GridPlacement::Named("header".to_string()))
    );
  }
}

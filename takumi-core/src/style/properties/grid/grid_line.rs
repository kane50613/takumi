use cssparser::Parser;

use crate::style::{
  CssSyntaxKind, CssToken, FromCss, GridPlacementSpan, MakeComputed, ParseResult, SizingContext,
  tw::TailwindPropertyParser,
};

use crate::style::GridPlacement;

/// Represents a grid line placement with serde support
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct GridLine {
  /// The start line placement
  pub start: GridPlacement,
  /// The end line placement
  pub end: GridPlacement,
}

impl MakeComputed for GridLine {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.start.make_computed(sizing);
    self.end.make_computed(sizing);
  }
}

impl GridLine {
  /// Create a grid line that spans the entire grid
  pub const fn full() -> Self {
    Self {
      start: GridPlacement::Line(1),
      end: GridPlacement::Line(-1),
    }
  }

  /// Create a grid line with a span placement
  pub const fn span(span: GridPlacementSpan) -> Self {
    Self {
      start: GridPlacement::Span(span),
      end: GridPlacement::Span(span),
    }
  }
}

impl<'i> FromCss<'i> for GridLine {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // First placement is required
    let first = GridPlacement::from_css(input)?;

    // Optional delimiter '/'
    let second = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
      Some(GridPlacement::from_css(input)?)
    } else {
      None
    };

    Ok(GridLine {
      start: first,
      end: second.unwrap_or_default(),
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("span"),
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Syntax(CssSyntaxKind::Ident),
  ];
}

impl TailwindPropertyParser for GridLine {
  fn parse_tw(suffix: &str) -> Option<Self> {
    if suffix.eq_ignore_ascii_case("auto") {
      return Some(GridLine {
        start: GridPlacement::auto(),
        end: GridPlacement::auto(),
      });
    }
    let number = suffix.parse::<i16>().ok()?;

    Some(GridLine {
      start: GridPlacement::Line(number),
      end: GridPlacement::auto(),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_line() {
    assert_eq!(
      GridLine::from_css_str("span 2 / 3"),
      Ok(GridLine {
        start: GridPlacement::span(2),
        end: GridPlacement::Line(3),
      })
    );
  }
}

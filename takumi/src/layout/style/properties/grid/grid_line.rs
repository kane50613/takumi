use cssparser::Parser;

use crate::layout::style::{
  CssSyntaxKind, CssToken, FromCss, GridPlacementKeyword, GridPlacementSpan, MakeComputed,
  ParseResult, tw::TailwindPropertyParser,
};
use crate::rendering::Sizing;

use super::GridPlacement;

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
  fn make_computed(&mut self, sizing: &Sizing) {
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

  /// Create a grid line with only a start placement
  pub const fn start(start: GridPlacement) -> Self {
    Self {
      start,
      end: GridPlacement::auto(),
    }
  }

  /// Create a grid line with only an end placement
  pub const fn end(end: GridPlacement) -> Self {
    Self {
      start: GridPlacement::auto(),
      end,
    }
  }
}

impl From<GridLine> for taffy::Line<taffy::GridPlacement> {
  fn from(line: GridLine) -> Self {
    Self {
      start: line.start.into(),
      end: line.end.into(),
    }
  }
}

impl From<&GridLine> for taffy::Line<taffy::GridPlacement> {
  fn from(line: &GridLine) -> Self {
    Self {
      start: match &line.start {
        GridPlacement::Keyword(GridPlacementKeyword::Auto) => taffy::GridPlacement::Auto,
        GridPlacement::Line(index) => taffy::GridPlacement::Line((*index).into()),
        GridPlacement::Span(GridPlacementSpan::Span(span)) => taffy::GridPlacement::Span(*span),
        GridPlacement::Named(_) => taffy::GridPlacement::Auto,
      },
      end: match &line.end {
        GridPlacement::Keyword(GridPlacementKeyword::Auto) => taffy::GridPlacement::Auto,
        GridPlacement::Line(index) => taffy::GridPlacement::Line((*index).into()),
        GridPlacement::Span(GridPlacementSpan::Span(span)) => taffy::GridPlacement::Span(*span),
        GridPlacement::Named(_) => taffy::GridPlacement::Auto,
      },
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

  #[test]
  fn test_parse_line() {
    assert_eq!(
      GridLine::from_str("span 2 / 3"),
      Ok(GridLine {
        start: GridPlacement::span(2),
        end: GridPlacement::Line(3),
      })
    );
  }
}

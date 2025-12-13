use cssparser::Parser;

use crate::layout::style::{FromCss, GridPlacementSpan, ParseResult, tw::TailwindPropertyParser};

use super::GridPlacement;

/// Represents a grid line placement with serde support
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridLine {
  /// The start line placement
  pub start: GridPlacement,
  /// The end line placement
  pub end: GridPlacement,
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

use cssparser::Parser;

use crate::layout::style::{FromCss, GridPlacementSpan, ParseResult, tw::TailwindPropertyParser};

use super::GridPlacement;

/// Represents a grid line placement with serde support
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridLine {
  /// The start line placement
  pub start: Option<GridPlacement>,
  /// The end line placement
  pub end: Option<GridPlacement>,
}

impl GridLine {
  /// Create a grid line that spans the entire grid
  pub const fn full() -> Self {
    Self {
      start: Some(GridPlacement::Line(1)),
      end: Some(GridPlacement::Line(-1)),
    }
  }

  /// Create a grid line with a span placement
  pub const fn span(span: GridPlacementSpan) -> Self {
    Self {
      start: Some(GridPlacement::Span(span)),
      end: Some(GridPlacement::Span(span)),
    }
  }

  /// Create a grid line with only a start placement
  pub const fn start(start: GridPlacement) -> Self {
    Self {
      start: Some(start),
      end: None,
    }
  }

  /// Create a grid line with only an end placement
  pub const fn end(end: GridPlacement) -> Self {
    Self {
      start: None,
      end: Some(end),
    }
  }
}

impl From<GridLine> for taffy::Line<taffy::GridPlacement> {
  fn from(line: GridLine) -> Self {
    Self {
      start: line.start.unwrap_or_default().into(),
      end: line.end.unwrap_or_default().into(),
    }
  }
}

impl<'i> FromCss<'i> for GridLine {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // First placement is required
    let first = GridPlacement::from_css(input).ok();

    // Optional delimiter '/'
    let second = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
      Some(GridPlacement::from_css(input)?)
    } else {
      None
    };

    if first.is_none() && second.is_none() {
      return Err(input.new_error_for_next_token());
    }

    Ok(GridLine {
      start: first,
      end: second,
    })
  }
}

impl TailwindPropertyParser for GridLine {
  fn parse_tw(suffix: &str) -> Option<Self> {
    let number = suffix.parse::<i16>().ok()?;

    Some(GridLine {
      start: Some(GridPlacement::Line(number)),
      end: None,
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
        start: Some(GridPlacement::span(2)),
        end: Some(GridPlacement::Line(3)),
      })
    );
  }
}

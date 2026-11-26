use cssparser::Parser;

use crate::style::{
  FromCss, GridPlacement, GridPlacementSpan, MakeComputed, ParseMode, ParseResult, SizingContext,
  tw::TailwindPropertyParser,
};

/// Represents a grid line placement
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
  fn from_css_in(input: &mut Parser<'i>, _mode: ParseMode) -> ParseResult<'i, Self> {
    let start = GridPlacement::from_css(input)?;
    // A lone `<custom-ident>` sets the end line too.
    let end = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
      GridPlacement::from_css(input)?
    } else if let GridPlacement::Named(name) = &start {
      GridPlacement::Named(name.clone())
    } else {
      GridPlacement::default()
    };

    Ok(GridLine { start, end })
  }

  const VALID_TOKENS: &'static [&'static str] = &["span", "<number>", "<ident>"];
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

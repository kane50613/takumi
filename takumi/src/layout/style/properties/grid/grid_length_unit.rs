use cssparser::{Parser, Token};
use taffy::CompactLength;

use crate::{
  layout::style::{FromCss, LengthUnit, ParseResult},
  rendering::RenderContext,
};

/// Represents a fraction of the available space
#[derive(Debug, Clone, PartialEq)]
pub enum FrLengthUnit {
  /// A fraction of the available space
  Fr(f32),
}

/// Represents a grid track sizing function with serde support
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridLengthUnit {
  /// A fraction of the available space
  Fr(f32),
  /// A fixed length
  Unit(LengthUnit),
}

impl GridLengthUnit {
  /// Converts the grid track size to a compact length representation.
  pub fn to_compact_length(&self, context: &RenderContext) -> CompactLength {
    match self {
      GridLengthUnit::Fr(fr) => CompactLength::fr(*fr),
      GridLengthUnit::Unit(unit) => unit.to_compact_length(context),
    }
  }
}

// Minimal CSS parsing helpers for grid values (mirror patterns used in other property modules)
impl<'i> FromCss<'i> for GridLengthUnit {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(unit) = input.try_parse(LengthUnit::from_css) {
      return Ok(GridLengthUnit::Unit(unit));
    }

    let location = input.current_source_location();
    let token = input.next()?;

    let Token::Dimension { value, unit, .. } = &token else {
      return Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      );
    };

    if !unit.eq_ignore_ascii_case("fr") {
      return Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      );
    }

    Ok(GridLengthUnit::Fr(*value))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_fr_and_unit() {
    assert_eq!(GridLengthUnit::from_str("1fr"), Ok(GridLengthUnit::Fr(1.0)));

    assert_eq!(
      GridLengthUnit::from_str("10px"),
      Ok(GridLengthUnit::Unit(LengthUnit::Px(10.0)))
    );
  }
}

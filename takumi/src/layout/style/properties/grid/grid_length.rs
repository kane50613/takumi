use cssparser::{Parser, Token};
use taffy::CompactLength;

use crate::{
  layout::style::{CssToken, FromCss, Length, ParseResult},
  rendering::Sizing,
};

/// Represents a fraction of the available space
#[derive(Debug, Clone, PartialEq)]
pub enum FrLength {
  /// A fraction of the available space
  Fr(f32),
}

/// Represents a grid track sizing function with serde support
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridLength {
  /// A fraction of the available space
  Fr(f32),
  /// A fixed length
  Unit(Length),
}

impl GridLength {
  /// Converts the grid track size to a compact length representation.
  pub(crate) fn to_compact_length(self, sizing: &Sizing) -> CompactLength {
    match self {
      GridLength::Fr(fr) => CompactLength::fr(fr),
      GridLength::Unit(unit) => unit.to_compact_length(sizing),
    }
  }
}

// Minimal CSS parsing helpers for grid values (mirror patterns used in other property modules)
impl<'i> FromCss<'i> for GridLength {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(unit) = input.try_parse(Length::from_css) {
      return Ok(GridLength::Unit(unit));
    }

    let location = input.current_source_location();
    let token = input.next()?;

    let Token::Dimension { value, unit, .. } = &token else {
      return Err(Self::unexpected_token_error(location, token));
    };

    if !unit.eq_ignore_ascii_case("fr") {
      return Err(Self::unexpected_token_error(location, token));
    }

    Ok(GridLength::Fr(*value))
  }

  fn valid_tokens() -> &'static [CssToken] {
    Length::<true>::valid_tokens()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_fr_and_unit() {
    assert_eq!(GridLength::from_str("1fr"), Ok(GridLength::Fr(1.0)));

    assert_eq!(
      GridLength::from_str("10px"),
      Ok(GridLength::Unit(Length::Px(10.0)))
    );
  }
}

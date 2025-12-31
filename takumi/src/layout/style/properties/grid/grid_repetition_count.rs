use cssparser::{Parser, Token};

use crate::layout::style::{CssToken, FromCss, ParseResult};

/// Represents grid track repetition keywords
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridRepetitionKeyword {
  /// Automatically fills the available space with as many tracks as possible
  AutoFill,
  /// Automatically fits as many tracks as possible while maintaining minimum size
  AutoFit,
}

/// Represents a grid track repetition pattern
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridRepetitionCount {
  /// Keywords for auto-fill and auto-fit
  Keyword(GridRepetitionKeyword),
  /// Specifies an exact number of track repetitions
  Count(u16),
}

impl From<GridRepetitionCount> for taffy::RepetitionCount {
  fn from(repetition: GridRepetitionCount) -> Self {
    match repetition {
      GridRepetitionCount::Keyword(GridRepetitionKeyword::AutoFill) => {
        taffy::RepetitionCount::AutoFill
      }
      GridRepetitionCount::Keyword(GridRepetitionKeyword::AutoFit) => {
        taffy::RepetitionCount::AutoFit
      }
      GridRepetitionCount::Count(count) => taffy::RepetitionCount::Count(count),
    }
  }
}

impl<'i> FromCss<'i> for GridRepetitionCount {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    if let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      let ident_str = ident.as_ref();
      if ident_str.eq_ignore_ascii_case("auto-fill") {
        return Ok(GridRepetitionCount::Keyword(
          GridRepetitionKeyword::AutoFill,
        ));
      }
      if ident_str.eq_ignore_ascii_case("auto-fit") {
        return Ok(GridRepetitionCount::Keyword(GridRepetitionKeyword::AutoFit));
      }
      return Err(Self::unexpected_token_error(location, &Token::Ident(ident)));
    }

    let token = input.next()?;
    match *token {
      Token::Number {
        int_value, value, ..
      } => {
        // Prefer integer value if provided
        let count: i64 = if let Some(iv) = int_value {
          iv as i64
        } else {
          value as i64
        };
        if count < 0 {
          return Err::<Self, _>(
            location
              .new_basic_unexpected_token_error(token.clone())
              .into(),
          );
        }
        Ok(GridRepetitionCount::Count(count as u16))
      }
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("auto-fill"),
      CssToken::Keyword("auto-fit"),
      CssToken::Token("number"),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_repetition_count() {
    assert_eq!(
      GridRepetitionCount::from_str("auto-fill"),
      Ok(GridRepetitionCount::Keyword(
        GridRepetitionKeyword::AutoFill
      ))
    );

    assert_eq!(
      GridRepetitionCount::from_str("auto-fit"),
      Ok(GridRepetitionCount::Keyword(GridRepetitionKeyword::AutoFit))
    );

    assert_eq!(
      GridRepetitionCount::from_str("3"),
      Ok(GridRepetitionCount::Count(3))
    );
  }
}

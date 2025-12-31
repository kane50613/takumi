use cssparser::{Parser, Token, match_ignore_ascii_case};
use smallvec::SmallVec;

use crate::layout::style::{CssToken, FromCss, Length, ParseResult, tw::TailwindPropertyParser};

/// Parsed `background-size` for one layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundSize {
  /// Scale the image to cover the container (may crop).
  Cover,
  /// Scale the image to be fully contained within the container.
  Contain,
  /// Explicit width and height values.
  Explicit {
    /// Width value for the background image.
    width: Length,
    /// Height value for the background image.
    height: Length,
  },
}

impl TailwindPropertyParser for BackgroundSize {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "cover" => Some(BackgroundSize::Cover),
      "contain" => Some(BackgroundSize::Contain),
      _ => None,
    }
  }
}

impl Default for BackgroundSize {
  fn default() -> Self {
    BackgroundSize::Explicit {
      width: Length::Auto,
      height: Length::Auto,
    }
  }
}

impl<'i> FromCss<'i> for BackgroundSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(width) = input.try_parse(Length::from_css) {
      let height = input.try_parse(Length::from_css).unwrap_or(Length::Auto);

      return Ok(BackgroundSize::Explicit { width, height });
    }

    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! {
      &ident,
      "cover" => Ok(BackgroundSize::Cover),
      "contain" => Ok(BackgroundSize::Contain),
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident.clone()))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("cover"),
      CssToken::Keyword("contain"),
      CssToken::Token("length"),
    ]
  }
}

/// A list of `background-size` values (one per layer).
pub type BackgroundSizes = SmallVec<[BackgroundSize; 4]>;

impl<'i> FromCss<'i> for BackgroundSizes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = SmallVec::new();
    values.push(BackgroundSize::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundSize::from_css(input)?);
    }

    Ok(values)
  }

  fn valid_tokens() -> &'static [CssToken] {
    BackgroundSize::valid_tokens()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use smallvec::smallvec;

  #[test]
  fn parses_cover_keyword() {
    assert_eq!(BackgroundSize::from_str("cover"), Ok(BackgroundSize::Cover));
  }

  #[test]
  fn parses_contain_keyword() {
    assert_eq!(
      BackgroundSize::from_str("contain"),
      Ok(BackgroundSize::Contain)
    );
  }

  #[test]
  fn parses_single_percentage_value_as_both_dimensions() {
    assert_eq!(
      BackgroundSize::from_str("50%\t"),
      Ok(BackgroundSize::Explicit {
        width: Length::Percentage(50.0),
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn parses_single_auto_value_as_both_dimensions() {
    assert_eq!(
      BackgroundSize::from_str("auto"),
      Ok(BackgroundSize::Explicit {
        width: Length::Auto,
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn parses_two_values_mixed_units() {
    assert_eq!(
      BackgroundSize::from_str("100px auto"),
      Ok(BackgroundSize::Explicit {
        width: Length::Px(100.0),
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn errors_on_unknown_identifier() {
    assert!(BackgroundSize::from_str("bogus").is_err());
  }

  #[test]
  fn parses_multiple_layers_with_keywords_and_values() {
    assert_eq!(
      BackgroundSizes::from_str("cover, 50% auto"),
      Ok(smallvec![
        BackgroundSize::Cover,
        BackgroundSize::Explicit {
          width: Length::Percentage(50.0),
          height: Length::Auto,
        }
      ])
    );
  }

  #[test]
  fn parses_multiple_layers_with_single_value_duplication() {
    assert_eq!(
      BackgroundSizes::from_str("25%, contain"),
      Ok(smallvec![
        BackgroundSize::Explicit {
          width: Length::Percentage(25.0),
          height: Length::Auto,
        },
        BackgroundSize::Contain
      ])
    );
  }

  #[test]
  fn errors_on_invalid_first_layer() {
    assert!(BackgroundSizes::from_str("nope").is_err());
  }
}

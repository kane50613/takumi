use cssparser::Parser;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{FromCss, LengthUnit, ParseResult, tw::TailwindPropertyParser};

/// Parsed `background-size` for one layer.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
pub enum BackgroundSize {
  /// Scale the image to cover the container (may crop).
  Cover,
  /// Scale the image to be fully contained within the container.
  Contain,
  /// Explicit width and height values.
  Explicit {
    /// Width value for the background image.
    width: LengthUnit,
    /// Height value for the background image.
    height: LengthUnit,
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
      width: LengthUnit::Auto,
      height: LengthUnit::Auto,
    }
  }
}

impl<'i> FromCss<'i> for BackgroundSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("cover"))
      .is_ok()
    {
      return Ok(BackgroundSize::Cover);
    }

    if input
      .try_parse(|input| input.expect_ident_matching("contain"))
      .is_ok()
    {
      return Ok(BackgroundSize::Contain);
    }

    let first = LengthUnit::from_css(input)?;
    let Ok(second) = input.try_parse(LengthUnit::from_css) else {
      return Ok(BackgroundSize::Explicit {
        width: first,
        height: first,
      });
    };

    Ok(BackgroundSize::Explicit {
      width: first,
      height: second,
    })
  }
}

/// A value representing either a list of parsed sizes or a raw CSS string.
#[derive(Debug, Clone, PartialEq, TS, Deserialize)]
#[serde(untagged)]
pub(crate) enum BackgroundSizesValue {
  /// Parsed sizes for one or more layers.
  Sizes(Vec<BackgroundSize>),
  /// Raw CSS to be parsed at runtime.
  Css(String),
}

/// A list of `background-size` values (one per layer).
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, TS)]
#[ts(as = "BackgroundSizesValue")]
#[serde(try_from = "BackgroundSizesValue")]
pub struct BackgroundSizes(pub Vec<BackgroundSize>);

impl<'i> FromCss<'i> for BackgroundSizes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = Vec::new();
    values.push(BackgroundSize::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundSize::from_css(input)?);
    }

    Ok(Self(values))
  }
}

impl TryFrom<BackgroundSizesValue> for BackgroundSizes {
  type Error = String;

  fn try_from(value: BackgroundSizesValue) -> Result<Self, Self::Error> {
    match value {
      BackgroundSizesValue::Sizes(v) => Ok(Self(v)),
      BackgroundSizesValue::Css(css) => Self::from_str(&css).map_err(|e| e.to_string()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use cssparser::{Parser, ParserInput};

  fn parse_bg_size(input: &str) -> ParseResult<'_, BackgroundSize> {
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);
    BackgroundSize::from_css(&mut parser)
  }

  #[test]
  fn parses_cover_keyword() {
    let result = parse_bg_size("cover").unwrap();
    assert_eq!(result, BackgroundSize::Cover);
  }

  #[test]
  fn parses_contain_keyword() {
    let result = parse_bg_size("contain").unwrap();
    assert_eq!(result, BackgroundSize::Contain);
  }

  #[test]
  fn parses_single_percentage_value_as_both_dimensions() {
    let result = parse_bg_size("50%\t").unwrap();
    assert_eq!(
      result,
      BackgroundSize::Explicit {
        width: LengthUnit::Percentage(50.0),
        height: LengthUnit::Percentage(50.0),
      }
    );
  }

  #[test]
  fn parses_single_auto_value_as_both_dimensions() {
    let result = parse_bg_size("auto").unwrap();
    assert_eq!(
      result,
      BackgroundSize::Explicit {
        width: LengthUnit::Auto,
        height: LengthUnit::Auto,
      }
    );
  }

  #[test]
  fn parses_two_values_mixed_units() {
    let result = parse_bg_size("100px auto").unwrap();
    assert_eq!(
      result,
      BackgroundSize::Explicit {
        width: LengthUnit::Px(100.0),
        height: LengthUnit::Auto,
      }
    );
  }

  #[test]
  fn errors_on_unknown_identifier() {
    let result = parse_bg_size("bogus");
    assert!(result.is_err());
  }

  fn parse_bg_sizes(input: &str) -> Result<BackgroundSizes, String> {
    BackgroundSizes::try_from(BackgroundSizesValue::Css(input.to_string()))
  }

  #[test]
  fn parses_multiple_layers_with_keywords_and_values() {
    let parsed = parse_bg_sizes("cover, 50% auto").unwrap();
    assert_eq!(parsed.0.len(), 2);
    assert_eq!(parsed.0[0], BackgroundSize::Cover);
    assert_eq!(
      parsed.0[1],
      BackgroundSize::Explicit {
        width: LengthUnit::Percentage(50.0),
        height: LengthUnit::Auto,
      }
    );
  }

  #[test]
  fn parses_multiple_layers_with_single_value_duplication() {
    let parsed = parse_bg_sizes("25%, contain").unwrap();
    assert_eq!(parsed.0.len(), 2);
    assert_eq!(
      parsed.0[0],
      BackgroundSize::Explicit {
        width: LengthUnit::Percentage(25.0),
        height: LengthUnit::Percentage(25.0),
      }
    );
    assert_eq!(parsed.0[1], BackgroundSize::Contain);
  }

  #[test]
  fn errors_on_invalid_first_layer() {
    let result = parse_bg_sizes("nope");
    assert!(result.is_err());
  }
}

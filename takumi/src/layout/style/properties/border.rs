use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::{ColorInput, FromCss, ParseResult, properties::LengthUnit};

/// Represents the `border` shorthand which accepts a width, style ("solid"), and an optional color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub(crate) enum BorderValue {
  /// Structured representation when provided as JSON.
  #[serde(rename_all = "camelCase")]
  Structured {
    width: Option<LengthUnit>,
    style: Option<BorderStyle>,
    color: Option<ColorInput>,
  },
  /// Raw CSS string representation.
  Css(String),
}

/// Represents border style options (currently only solid is supported).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum BorderStyle {
  /// Solid border style.
  Solid,
}

/// Parsed `border` value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(try_from = "BorderValue")]
#[ts(as = "BorderValue")]
pub struct Border {
  /// Border width as a `LengthUnit`.
  pub width: Option<LengthUnit>,
  /// Border style (currently only solid is supported).
  pub style: Option<BorderStyle>,
  /// Optional border color.
  pub color: Option<ColorInput>,
}

impl TryFrom<BorderValue> for Border {
  type Error = String;

  fn try_from(value: BorderValue) -> Result<Self, Self::Error> {
    match value {
      BorderValue::Structured {
        width,
        style,
        color,
      } => Ok(Border {
        width,
        style,
        color,
      }),
      BorderValue::Css(s) => Ok(Border::from_str(&s).map_err(|e| e.to_string())?),
    }
  }
}

impl<'i> FromCss<'i> for Border {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut width = None;
    let mut style = None;
    let mut color = None;

    loop {
      if input.is_exhausted() {
        break;
      }

      if let Ok(value) = input.try_parse(LengthUnit::from_css) {
        width = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(BorderStyle::from_css) {
        style = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(ColorInput::from_css) {
        color = Some(value);
        continue;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(Border {
      width,
      style,
      color,
    })
  }
}

impl<'i> FromCss<'i> for BorderStyle {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! {ident,
      "solid" => Ok(BorderStyle::Solid),
      _ => {
        let token = Token::Ident(ident.clone());
        Err(input.new_basic_unexpected_token_error(token).into())
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::Color;

  use super::*;
  use cssparser::{Parser, ParserInput};

  fn parse_border_str(input: &str) -> ParseResult<'_, Border> {
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);

    Border::from_css(&mut parser)
  }

  fn parse_border_style_str(input: &str) -> ParseResult<'_, BorderStyle> {
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);

    BorderStyle::from_css(&mut parser)
  }

  #[test]
  fn test_parse_border_style_solid() {
    let result = parse_border_style_str("solid").unwrap();
    assert_eq!(result, BorderStyle::Solid);
  }

  #[test]
  fn test_parse_border_style_invalid() {
    let result = parse_border_style_str("dashed");
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_border_width_only() {
    let result = parse_border_str("10px").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(10.0)));
    assert_eq!(result.style, None);
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_parse_border_style_only() {
    let result = parse_border_str("solid").unwrap();
    assert_eq!(result.width, None);
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_parse_border_color_only() {
    let result = parse_border_str("red").unwrap();
    assert_eq!(result.width, None);
    assert_eq!(result.style, None);
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_border_width_and_style() {
    let result = parse_border_str("2px solid").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(2.0)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_parse_border_width_style_color() {
    let result = parse_border_str("2px solid red").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(2.0)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_border_style_width_color() {
    let result = parse_border_str("solid 2px red").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(2.0)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_border_color_style_width() {
    let result = parse_border_str("red solid 2px").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(2.0)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_border_rem_units() {
    let result = parse_border_str("1.5rem solid blue").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Rem(1.5)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([0, 0, 255, 255])))
    );
  }

  #[test]
  fn test_parse_border_hex_color() {
    let result = parse_border_str("3px solid #ff0000").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(3.0)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_border_rgb_color() {
    let result = parse_border_str("4px solid rgb(0, 255, 0)").unwrap();
    assert_eq!(result.width, Some(LengthUnit::Px(4.0)));
    assert_eq!(result.style, Some(BorderStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([0, 255, 0, 255])))
    );
  }

  #[test]
  fn test_parse_border_invalid_style() {
    let result = parse_border_str("2px dashed red");
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_border_invalid_color() {
    let result = parse_border_str("2px solid invalid-color");
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_border_empty() {
    let result = parse_border_str("").unwrap();
    assert_eq!(result.width, None);
    assert_eq!(result.style, None);
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_border_value_from_structured() {
    let border_value = BorderValue::Structured {
      width: Some(LengthUnit::Px(5.0)),
      style: Some(BorderStyle::Solid),
      color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
    };

    let border: Border = border_value.try_into().unwrap();
    assert_eq!(border.width, Some(LengthUnit::Px(5.0)));
    assert_eq!(border.style, Some(BorderStyle::Solid));
    assert_eq!(
      border.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_border_value_from_css() {
    let border_value = BorderValue::Css("3px solid blue".to_string());

    let border: Border = border_value.try_into().unwrap();
    assert_eq!(border.width, Some(LengthUnit::Px(3.0)));
    assert_eq!(border.style, Some(BorderStyle::Solid));
    assert_eq!(
      border.color,
      Some(ColorInput::Value(Color([0, 0, 255, 255])))
    );
  }

  #[test]
  fn test_border_value_from_invalid_css() {
    let border_value = BorderValue::Css("invalid border".to_string());

    let result: Result<Border, _> = border_value.try_into();
    assert!(result.is_err());
  }

  #[test]
  fn test_border_default() {
    let border = Border::default();
    assert_eq!(border.width, None);
    assert_eq!(border.style, None);
    assert_eq!(border.color, None);
  }
}

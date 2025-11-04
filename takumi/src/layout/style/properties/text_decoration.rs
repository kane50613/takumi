use cssparser::{Parser, Token, match_ignore_ascii_case};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use ts_rs::TS;

use crate::layout::style::{FromCss, ParseResult, properties::ColorInput};

/// Represents the `text-decoration` shorthand which accepts a line style and an optional color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub(crate) enum TextDecorationValue {
  /// Structured representation when provided as JSON.
  #[serde(rename_all = "camelCase")]
  Structured {
    line: TextDecorationLines,
    style: Option<TextDecorationStyle>,
    color: Option<ColorInput>,
  },
  /// Raw CSS string representation.
  Css(String),
}

/// Represents text decoration line options.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum TextDecorationLine {
  /// Underline text decoration.
  Underline,
  /// Line-through text decoration.
  LineThrough,
  /// Overline text decoration.
  Overline,
}

/// Represents a collection of text decoration lines.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(as = "TextDecorationLinesValue")]
#[serde(try_from = "TextDecorationLinesValue")]
pub struct TextDecorationLines(pub SmallVec<[TextDecorationLine; 3]>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
enum TextDecorationLinesValue {
  #[ts(as = "Vec<TextDecorationLine>")]
  Lines(SmallVec<[TextDecorationLine; 3]>),
  Css(String),
}

impl<'i> FromCss<'i> for TextDecorationLines {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut lines = SmallVec::new();

    while !input.is_exhausted() {
      let line = TextDecorationLine::from_css(input)?;
      lines.push(line);
    }

    Ok(TextDecorationLines(lines))
  }
}

impl TryFrom<TextDecorationLinesValue> for TextDecorationLines {
  type Error = String;

  fn try_from(value: TextDecorationLinesValue) -> Result<Self, Self::Error> {
    match value {
      TextDecorationLinesValue::Lines(lines) => Ok(TextDecorationLines(lines)),
      TextDecorationLinesValue::Css(css) => {
        TextDecorationLines::from_str(&css).map_err(|e| e.to_string())
      }
    }
  }
}

impl TextDecorationLines {
  /// Checks if the text decoration lines contain the specified line.
  pub fn has(&self, target: TextDecorationLine) -> bool {
    self.0.contains(&target)
  }
}

/// Represents text decoration style options (currently only solid is supported).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum TextDecorationStyle {
  /// Solid text decoration style.
  Solid,
}

/// Parsed `text-decoration` value.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(try_from = "TextDecorationValue")]
#[ts(as = "TextDecorationValue")]
pub struct TextDecoration {
  /// Text decoration line style.
  pub line: TextDecorationLines,
  /// Text decoration style (currently only solid is supported).
  pub style: Option<TextDecorationStyle>,
  /// Optional text decoration color.
  pub color: Option<ColorInput>,
}

impl TryFrom<TextDecorationValue> for TextDecoration {
  type Error = String;

  fn try_from(value: TextDecorationValue) -> Result<Self, Self::Error> {
    match value {
      TextDecorationValue::Structured { line, style, color } => {
        Ok(TextDecoration { line, style, color })
      }
      TextDecorationValue::Css(s) => Ok(TextDecoration::from_str(&s).map_err(|e| e.to_string())?),
    }
  }
}

impl<'i> FromCss<'i> for TextDecoration {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut line = TextDecorationLines::default();
    let mut style = None;
    let mut color = None;

    loop {
      if let Ok(value) = input.try_parse(TextDecorationLine::from_css) {
        line.0.push(value);
        continue;
      }

      if let Ok(value) = input.try_parse(TextDecorationStyle::from_css) {
        style = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(ColorInput::from_css) {
        color = Some(value);
        continue;
      }

      if input.is_exhausted() {
        break;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(TextDecoration { line, style, color })
  }
}

impl<'i> FromCss<'i> for TextDecorationLine {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    if let Token::Ident(ident) = token {
      return match_ignore_ascii_case! {ident,
        "underline" => Ok(TextDecorationLine::Underline),
        "line-through" => Ok(TextDecorationLine::LineThrough),
        "overline" => Ok(TextDecorationLine::Overline),
        _ => Err(location.new_basic_unexpected_token_error(token.clone()).into()),
      };
    }

    Err(
      location
        .new_basic_unexpected_token_error(token.clone())
        .into(),
    )
  }
}

impl<'i> FromCss<'i> for TextDecorationStyle {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    if let Token::Ident(ident) = token
      && ident.eq_ignore_ascii_case("solid")
    {
      return Ok(TextDecorationStyle::Solid);
    }

    Err(
      location
        .new_basic_unexpected_token_error(token.clone())
        .into(),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::properties::Color;

  #[test]
  fn test_parse_text_decoration_underline() {
    let result = TextDecoration::from_str("underline").unwrap();
    assert_eq!(result.line.0.as_slice(), &[TextDecorationLine::Underline]);
    assert_eq!(result.style, None);
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_parse_text_decoration_line_through() {
    let result = TextDecoration::from_str("line-through").unwrap();
    assert_eq!(result.line.0.as_slice(), &[TextDecorationLine::LineThrough]);
    assert_eq!(result.style, None);
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_parse_text_decoration_underline_solid() {
    let result = TextDecoration::from_str("underline solid").unwrap();
    assert_eq!(result.line.0.as_slice(), &[TextDecorationLine::Underline]);
    assert_eq!(result.style, Some(TextDecorationStyle::Solid));
    assert_eq!(result.color, None);
  }

  #[test]
  fn test_parse_text_decoration_line_through_solid_red() {
    let result = TextDecoration::from_str("line-through solid red").unwrap();
    assert_eq!(result.line.0.as_slice(), &[TextDecorationLine::LineThrough]);
    assert_eq!(result.style, Some(TextDecorationStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_text_decoration_multiple_lines() {
    let result = TextDecoration::from_str("underline line-through solid red").unwrap();
    assert_eq!(
      result.line.0.as_slice(),
      &[
        TextDecorationLine::Underline,
        TextDecorationLine::LineThrough
      ]
    );
    assert_eq!(result.style, Some(TextDecorationStyle::Solid));
    assert_eq!(
      result.color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_text_decoration_invalid() {
    let result = TextDecoration::from_str("invalid");
    assert!(result.is_err());
  }
}

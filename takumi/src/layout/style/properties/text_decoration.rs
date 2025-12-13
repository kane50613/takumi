use cssparser::{Parser, Token, match_ignore_ascii_case};
use smallvec::SmallVec;

use crate::layout::style::{FromCss, ParseResult, properties::ColorInput};

/// Represents text decoration line options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecorationLine {
  /// Underline text decoration.
  Underline,
  /// Line-through text decoration.
  LineThrough,
  /// Overline text decoration.
  Overline,
}

/// Represents a collection of text decoration lines.
pub type TextDecorationLines = SmallVec<[TextDecorationLine; 3]>;

impl<'i> FromCss<'i> for TextDecorationLines {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut lines = SmallVec::new();

    while !input.is_exhausted() {
      let line = TextDecorationLine::from_css(input)?;
      lines.push(line);
    }

    Ok(lines)
  }
}

/// Represents text decoration style options (currently only solid is supported).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecorationStyle {
  /// Solid text decoration style.
  Solid,
}

/// Parsed `text-decoration` value.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TextDecoration {
  /// Text decoration line style.
  pub line: TextDecorationLines,
  /// Text decoration style (currently only solid is supported).
  pub style: Option<TextDecorationStyle>,
  /// Optional text decoration color.
  pub color: Option<ColorInput>,
}

impl<'i> FromCss<'i> for TextDecoration {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut line = TextDecorationLines::default();
    let mut style = None;
    let mut color = None;

    loop {
      if let Ok(value) = input.try_parse(TextDecorationLine::from_css) {
        line.push(value);
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
    assert_eq!(
      TextDecoration::from_str("underline"),
      Ok(TextDecoration {
        line: smallvec::smallvec![TextDecorationLine::Underline],
        style: None,
        color: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_line_through() {
    assert_eq!(
      TextDecoration::from_str("line-through"),
      Ok(TextDecoration {
        line: smallvec::smallvec![TextDecorationLine::LineThrough],
        style: None,
        color: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_underline_solid() {
    assert_eq!(
      TextDecoration::from_str("underline solid"),
      Ok(TextDecoration {
        line: smallvec::smallvec![TextDecorationLine::Underline],
        style: Some(TextDecorationStyle::Solid),
        color: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_line_through_solid_red() {
    assert_eq!(
      TextDecoration::from_str("line-through solid red"),
      Ok(TextDecoration {
        line: smallvec::smallvec![TextDecorationLine::LineThrough],
        style: Some(TextDecorationStyle::Solid),
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_multiple_lines() {
    assert_eq!(
      TextDecoration::from_str("underline line-through solid red"),
      Ok(TextDecoration {
        line: smallvec::smallvec![
          TextDecorationLine::Underline,
          TextDecorationLine::LineThrough
        ],
        style: Some(TextDecorationStyle::Solid),
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_invalid() {
    let result = TextDecoration::from_str("invalid");
    assert!(result.is_err());
  }
}

use bitflags::bitflags;
use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::{
  layout::style::{
    CssToken, FromCss, Length, MakeComputed, ParseResult, declare_enum_from_css_impl,
    properties::ColorInput, tw::TailwindPropertyParser,
  },
  rendering::Sizing,
};

bitflags! {
  /// Represents a collection of text decoration lines.
  #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
  pub struct TextDecorationLines: u8 {
    /// Underline text decoration.
    const UNDERLINE = 0b001;
    /// Line-through text decoration.
    const LINE_THROUGH = 0b010;
    /// Overline text decoration.
    const OVERLINE = 0b100;
  }
}

impl<'i> FromCss<'i> for TextDecorationLines {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut lines = TextDecorationLines::empty();

    // Parse at least one line decoration
    let first_location = input.current_source_location();
    let first_ident = input.expect_ident()?;
    match_ignore_ascii_case! {first_ident,
      "underline" => lines |= TextDecorationLines::UNDERLINE,
      "line-through" => lines |= TextDecorationLines::LINE_THROUGH,
      "overline" => lines |= TextDecorationLines::OVERLINE,
      _ => return Err(Self::unexpected_token_error(first_location, &Token::Ident(first_ident.clone()))),
    }

    // Parse additional decorations if present
    while !input.is_exhausted() {
      let state = input.state();
      if let Ok(ident) = input.expect_ident() {
        match_ignore_ascii_case! {ident,
          "underline" => lines |= TextDecorationLines::UNDERLINE,
          "line-through" => lines |= TextDecorationLines::LINE_THROUGH,
          "overline" => lines |= TextDecorationLines::OVERLINE,
          _ => {
            input.reset(&state);
            break;
          }
        }
      } else {
        break;
      }
    }

    Ok(lines)
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("underline"),
      CssToken::Keyword("line-through"),
      CssToken::Keyword("overline"),
    ]
  }
}

impl MakeComputed for TextDecorationLines {}

/// Represents text decoration thickness options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecorationThickness {
  /// Use the font's default thickness, fallback to `auto` if not available.
  FromFont,
  /// Use a specific length.
  Length(Length),
}

impl Default for TextDecorationThickness {
  fn default() -> Self {
    Self::Length(Length::Auto)
  }
}

impl MakeComputed for TextDecorationThickness {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SizedTextDecorationThickness {
  FromFont,
  Value(f32),
}

impl<'i> FromCss<'i> for TextDecorationThickness {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("from-font"))
      .is_ok()
    {
      return Ok(Self::FromFont);
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("from-font"), CssToken::Token("length")]
  }
}

impl TailwindPropertyParser for TextDecorationThickness {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(number) = token.parse::<f32>() {
      return Some(Self::Length(Length::Px(number)));
    }

    Self::from_str(token).ok()
  }
}

/// Represents text decoration style options (currently only solid is supported).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextDecorationStyle {
  /// Solid text decoration style.
  #[default]
  Solid,
}

declare_enum_from_css_impl!(
  TextDecorationStyle,
  "solid" => Self::Solid
);

/// Parsed `text-decoration` value.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TextDecoration {
  /// Text decoration line style.
  pub line: TextDecorationLines,
  /// Text decoration style (currently only solid is supported).
  pub style: Option<TextDecorationStyle>,
  /// Optional text decoration color.
  pub color: Option<ColorInput>,
  /// Optional text decoration thickness.
  pub thickness: Option<TextDecorationThickness>,
}

impl MakeComputed for TextDecoration {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.color.make_computed(sizing);
    self.thickness.make_computed(sizing);
  }
}

impl<'i> FromCss<'i> for TextDecoration {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut line = TextDecorationLines::empty();
    let mut style = None;
    let mut color = None;
    let mut thickness = None;

    loop {
      if let Ok(value) = input.try_parse(TextDecorationLines::from_css) {
        line |= value;
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

      if let Ok(value) = input.try_parse(TextDecorationThickness::from_css) {
        thickness = Some(value);
        continue;
      }

      if input.is_exhausted() {
        break;
      }

      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        input.next()?,
      ));
    }

    Ok(TextDecoration {
      line,
      style,
      color,
      thickness,
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("underline"),
      CssToken::Keyword("line-through"),
      CssToken::Keyword("overline"),
      CssToken::Keyword("solid"),
      CssToken::Token("color"),
    ]
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
        line: TextDecorationLines::UNDERLINE,
        style: None,
        color: None,
        thickness: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_line_through() {
    assert_eq!(
      TextDecoration::from_str("line-through"),
      Ok(TextDecoration {
        line: TextDecorationLines::LINE_THROUGH,
        style: None,
        color: None,
        thickness: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_underline_solid() {
    assert_eq!(
      TextDecoration::from_str("underline solid"),
      Ok(TextDecoration {
        line: TextDecorationLines::UNDERLINE,
        style: Some(TextDecorationStyle::Solid),
        color: None,
        thickness: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_line_through_solid_red() {
    assert_eq!(
      TextDecoration::from_str("line-through solid red"),
      Ok(TextDecoration {
        line: TextDecorationLines::LINE_THROUGH,
        style: Some(TextDecorationStyle::Solid),
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        thickness: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_multiple_lines() {
    assert_eq!(
      TextDecoration::from_str("underline line-through solid red"),
      Ok(TextDecoration {
        line: TextDecorationLines::UNDERLINE | TextDecorationLines::LINE_THROUGH,
        style: Some(TextDecorationStyle::Solid),
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        thickness: None,
      })
    );
  }

  #[test]
  fn test_parse_text_decoration_invalid() {
    let result = TextDecoration::from_str("invalid");
    assert!(result.is_err());
  }
}

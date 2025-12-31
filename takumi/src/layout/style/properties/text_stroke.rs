use cssparser::Parser;

use crate::layout::style::{ColorInput, CssToken, FromCss, ParseResult, properties::Length};

/// Parsed `text-stroke` value.
///
/// `color` is optional; when absent the element's `color` property should be used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStroke {
  /// Stroke width.
  pub width: Length<false>,
  /// Optional stroke color.
  pub color: Option<ColorInput>,
}

impl<'i> FromCss<'i> for TextStroke {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Parse width first
    let width = Length::from_css(input)?;
    // Try optional color
    let color = input.try_parse(ColorInput::from_css).ok();

    Ok(TextStroke { width, color })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("length"), CssToken::Token("color")]
  }
}

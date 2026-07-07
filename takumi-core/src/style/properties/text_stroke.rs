use cssparser::Parser;

use crate::style::{
  ColorInput, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult, SizingContext,
};

/// Parsed `text-stroke` value.
///
/// `color` is optional; when absent the element's `color` property should be used.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TextStroke {
  /// Stroke width.
  pub width: Length,
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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Syntax(CssSyntaxKind::Color),
  ];
}

impl MakeComputed for TextStroke {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.width.make_computed(sizing);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{Color, FromCssStr};

  #[test]
  fn test_parse_text_stroke() {
    for (css, expected) in [
      (
        "2px",
        TextStroke {
          width: Length::Px(2.0),
          color: None,
        },
      ),
      (
        "2px red",
        TextStroke {
          width: Length::Px(2.0),
          color: Some(ColorInput::Value(Color::from_rgb(0xff0000))),
        },
      ),
      (
        "1em",
        TextStroke {
          width: Length::Em(1.0),
          color: None,
        },
      ),
    ] {
      assert_eq!(
        TextStroke::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  // TextStroke has no ToCss impl, so no round-trip test.

  #[test]
  fn test_parse_text_stroke_invalid() {
    assert!(TextStroke::from_css_str("bogus").is_err());
    assert!(TextStroke::from_css_str("\"2px\"").is_err());
  }
}

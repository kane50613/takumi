use cssparser::Parser;

use crate::{
  layout::style::{
    ColorInput, CssSyntaxKind, CssToken, FromCss, LengthDefaultsToZero, MakeComputed, ParseResult,
  },
  rendering::Sizing,
};

/// Parsed `text-stroke` value.
///
/// `color` is optional; when absent the element's `color` property should be used.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TextStroke {
  /// Stroke width.
  pub width: LengthDefaultsToZero,
  /// Optional stroke color.
  pub color: Option<ColorInput>,
}

impl<'i> FromCss<'i> for TextStroke {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Parse width first
    let width = LengthDefaultsToZero::from_css(input)?;
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
  fn make_computed(&mut self, sizing: &Sizing) {
    self.width.make_computed(sizing);
  }
}

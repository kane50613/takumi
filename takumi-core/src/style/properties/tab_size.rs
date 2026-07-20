use std::fmt;

use cssparser::{BasicParseErrorKind, Parser};

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, SizingContext,
  ToCss, lerp,
};

/// `tab-size` as a number of spaces. `<length>` values are not supported; preserved tabs
/// expand to `round(n)` spaces before shaping rather than advancing to true tab stops.
/// The representation is private so `<length>` support can land without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabSize(f32);

impl Default for TabSize {
  fn default() -> Self {
    Self(8.0)
  }
}

impl From<f32> for TabSize {
  fn from(spaces: f32) -> Self {
    Self(spaces)
  }
}

impl TabSize {
  pub(crate) fn spaces(&self) -> usize {
    self.0.round().max(0.0) as usize
  }
}

impl MakeComputed for TabSize {}

impl Animatable for TabSize {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    self.0 = lerp(from.0, to.0, progress);
  }
}

impl<'i> FromCss<'i> for TabSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let value = input.expect_number()?;

    if value < 0.0 {
      return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    Ok(Self(value))
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Number)];
}

impl ToCss for TabSize {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    write!(dest, "{}", self.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_tab_size() {
    assert_eq!(TabSize::from_css_str("4"), Ok(TabSize(4.0)));
    assert_eq!(TabSize::from_css_str("2.5"), Ok(TabSize(2.5)));
    assert_eq!(TabSize::from_css_str("0"), Ok(TabSize(0.0)));
  }

  #[test]
  fn test_parse_tab_size_invalid() {
    assert!(TabSize::from_css_str("-1").is_err());
    assert!(TabSize::from_css_str("4px").is_err());
    assert!(TabSize::from_css_str("auto").is_err());
  }

  #[test]
  fn test_tab_size_spaces_rounds() {
    assert_eq!(TabSize(2.5).spaces(), 3);
    assert_eq!(TabSize::default().spaces(), 8);
  }
}

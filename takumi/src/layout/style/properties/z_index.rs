use cssparser::Parser;

use crate::layout::style::{
  Animatable, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult,
};

/// Represents the CSS `z-index` value for stacking order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZIndex {
  /// Uses the default stacking level.
  #[default]
  Auto,
  /// Uses an explicit stacking level.
  Integer(i32),
}

impl ZIndex {
  /// Returns the integer used to sort painting order.
  pub(crate) const fn painting_order_value(self) -> i32 {
    match self {
      Self::Auto => 0,
      Self::Integer(value) => value,
    }
  }
}

impl MakeComputed for ZIndex {}
impl Animatable for ZIndex {}

impl<'i> FromCss<'i> for ZIndex {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(value) = input.try_parse(Parser::expect_integer) {
      return Ok(Self::Integer(value));
    }

    input.expect_ident_matching("auto")?;
    Ok(Self::Auto)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Syntax(CssSyntaxKind::Integer),
  ];
}

#[cfg(test)]
mod tests {
  use super::ZIndex;
  use crate::layout::style::FromCss;

  #[test]
  fn parses_auto() {
    assert_eq!(ZIndex::from_str("auto"), Ok(ZIndex::Auto));
  }

  #[test]
  fn parses_integer() {
    assert_eq!(ZIndex::from_str("-3"), Ok(ZIndex::Integer(-3)));
  }
}

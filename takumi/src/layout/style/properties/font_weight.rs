use cssparser::{Parser, Token, match_ignore_ascii_case};
use parley::style::FontWeight as ParleyFontWeight;

use crate::layout::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, lerp,
  tw::TailwindPropertyParser,
};
use crate::rendering::Sizing;

/// Represents font weight value.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct FontWeight(ParleyFontWeight);

impl MakeComputed for FontWeight {}

impl Animatable for FontWeight {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &Sizing,
    _current_color: Color,
  ) {
    *self = FontWeight::from(lerp(from.value(), to.value(), progress));
  }
}

impl<'i> FromCss<'i> for FontWeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Number { value, .. } => Ok((*value).into()),
      Token::Ident(ident) => match_ignore_ascii_case! { ident,
        "normal" => Ok(400.0.into()),
        "bold" => Ok(700.0.into()),
        _ => Err(Self::unexpected_token_error(location, token)),
      },
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Keyword("normal"),
    CssToken::Keyword("bold"),
  ];
}

impl TailwindPropertyParser for FontWeight {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {&token,
      "thin" => Some(100.0.into()),
      "extralight" => Some(200.0.into()),
      "light" => Some(300.0.into()),
      "normal" => Some(400.0.into()),
      "medium" => Some(500.0.into()),
      "semibold" => Some(600.0.into()),
      "bold" => Some(700.0.into()),
      "extrabold" => Some(800.0.into()),
      "black" => Some(900.0.into()),
      _ => None,
    }
  }
}

impl FontWeight {
  pub(crate) fn value(self) -> f32 {
    self.0.value()
  }
}

impl From<FontWeight> for ParleyFontWeight {
  fn from(value: FontWeight) -> Self {
    value.0
  }
}

impl From<f32> for FontWeight {
  fn from(value: f32) -> Self {
    FontWeight(ParleyFontWeight::new(value))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_numeric_font_weight() {
    assert_eq!(FontWeight::from_str("700"), Ok(700.0.into()));
  }
}

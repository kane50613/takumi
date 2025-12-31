use cssparser::{Parser, match_ignore_ascii_case};
use parley::style::FontWeight as ParleyFontWeight;

use crate::layout::style::{CssToken, FromCss, ParseResult, tw::TailwindPropertyParser};

/// Represents font weight value.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct FontWeight(ParleyFontWeight);

impl<'i> FromCss<'i> for FontWeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let Some(value) = ParleyFontWeight::parse(input.current_line()) else {
      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        input.next()?,
      ));
    };

    Ok(FontWeight(value))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("number"),
      CssToken::Keyword("normal"),
      CssToken::Keyword("bold"),
    ]
  }
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

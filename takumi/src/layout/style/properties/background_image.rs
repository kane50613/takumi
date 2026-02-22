use std::sync::Arc;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::layout::style::{
  ConicGradient, CssToken, FromCss, LinearGradient, MakeComputed, NoiseV1, ParseResult,
  RadialGradient, tw::TailwindPropertyParser,
};
use crate::rendering::Sizing;

/// Background image variants supported by Takumi.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum BackgroundImage {
  /// No background image.
  #[default]
  None,
  /// CSS linear-gradient(...)
  Linear(LinearGradient),
  /// CSS radial-gradient(...)
  Radial(RadialGradient),
  /// CSS conic-gradient(...)
  Conic(ConicGradient),
  /// Custom noise-v1(...)
  Noise(NoiseV1),
  /// Load external image resource.
  Url(Arc<str>),
}

impl MakeComputed for BackgroundImage {
  fn make_computed(&mut self, sizing: &Sizing) {
    match self {
      BackgroundImage::Linear(gradient) => gradient.make_computed(sizing),
      BackgroundImage::Radial(gradient) => gradient.make_computed(sizing),
      BackgroundImage::Conic(gradient) => gradient.make_computed(sizing),
      _ => {}
    }
  }
}

impl TailwindPropertyParser for BackgroundImage {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "none" => Some(BackgroundImage::None),
      _ => None,
    }
  }
}

impl<'i> FromCss<'i> for BackgroundImage {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, BackgroundImage> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(BackgroundImage::None);
    }

    if let Ok(url) = input.try_parse(Parser::expect_url) {
      return Ok(BackgroundImage::Url((&*url).into()));
    }

    let location = input.current_source_location();
    let start = input.state();
    let function = input.expect_function()?.to_owned();

    input.reset(&start);

    match_ignore_ascii_case! {&function,
      "linear-gradient" => Ok(BackgroundImage::Linear(LinearGradient::from_css(input)?)),
      "radial-gradient" => Ok(BackgroundImage::Radial(RadialGradient::from_css(input)?)),
      "conic-gradient" => Ok(BackgroundImage::Conic(ConicGradient::from_css(input)?)),
      "noise-v1" => Ok(BackgroundImage::Noise(NoiseV1::from_css(input)?)),
      _ => Err(Self::unexpected_token_error(location, &Token::Function(function))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("url()"),
      CssToken::Token("linear-gradient()"),
      CssToken::Token("radial-gradient()"),
      CssToken::Token("conic-gradient()"),
      CssToken::Token("noise-v1()"),
      CssToken::Keyword("none"),
    ]
  }
}

/// A collection of background images.
pub type BackgroundImages = Box<[BackgroundImage]>;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_tailwind_none() {
    assert_eq!(
      BackgroundImage::parse_tw("none"),
      Some(BackgroundImage::None)
    );
  }

  #[test]
  fn test_parse_tailwind_arbitrary_url() {
    assert_eq!(
      BackgroundImage::parse_tw_with_arbitrary("[url(https://example.com/bg.png)]"),
      Some(BackgroundImage::Url("https://example.com/bg.png".into()))
    );
  }
}

use std::sync::Arc;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use smallvec::SmallVec;

use crate::layout::style::{
  CssToken, FromCss, LinearGradient, NoiseV1, ParseResult, RadialGradient,
  tw::TailwindPropertyParser,
};

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
  /// Custom noise-v1(...)
  Noise(NoiseV1),
  /// Load external image resource.
  Url(Arc<str>),
}

impl TailwindPropertyParser for BackgroundImage {
  fn parse_tw(_token: &str) -> Option<Self> {
    // TODO: Implement
    None
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
      "noise-v1" => Ok(BackgroundImage::Noise(NoiseV1::from_css(input)?)),
      _ => Err(Self::unexpected_token_error(location, &Token::Function(function))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("url()"),
      CssToken::Token("linear-gradient()"),
      CssToken::Token("radial-gradient()"),
      CssToken::Token("noise-v1()"),
      CssToken::Keyword("none"),
    ]
  }
}

/// A collection of background images.
pub type BackgroundImages = SmallVec<[BackgroundImage; 4]>;

impl<'i> FromCss<'i> for BackgroundImages {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut images = SmallVec::new();

    images.push(BackgroundImage::from_css(input)?);

    while input.expect_comma().is_ok() {
      images.push(BackgroundImage::from_css(input)?);
    }

    Ok(images)
  }

  fn valid_tokens() -> &'static [CssToken] {
    BackgroundImage::valid_tokens()
  }
}

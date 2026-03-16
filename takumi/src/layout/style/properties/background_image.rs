use std::sync::Arc;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::layout::style::{
  Animatable, ConicGradient, CssDescriptorKind, CssToken, FromCss, LinearGradient,
  ListInterpolationStrategy, MakeComputed, ParseResult, RadialGradient, tw::TailwindPropertyParser,
};
use crate::rendering::Sizing;

/// Background image variants supported by Takumi.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
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

impl Animatable for BackgroundImage {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::RepeatToLcm
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
      "linear-gradient" | "repeating-linear-gradient" => Ok(BackgroundImage::Linear(LinearGradient::from_css(input)?)),
      "radial-gradient" | "repeating-radial-gradient" => Ok(BackgroundImage::Radial(RadialGradient::from_css(input)?)),
      "conic-gradient" | "repeating-conic-gradient" => Ok(BackgroundImage::Conic(ConicGradient::from_css(input)?)),
      _ => Err(Self::unexpected_token_error(location, &Token::Function(function))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Descriptor(CssDescriptorKind::UrlFn),
    CssToken::Descriptor(CssDescriptorKind::LinearGradientFn),
    CssToken::Descriptor(CssDescriptorKind::RepeatingLinearGradientFn),
    CssToken::Descriptor(CssDescriptorKind::RadialGradientFn),
    CssToken::Descriptor(CssDescriptorKind::RepeatingRadialGradientFn),
    CssToken::Descriptor(CssDescriptorKind::ConicGradientFn),
    CssToken::Descriptor(CssDescriptorKind::RepeatingConicGradientFn),
    CssToken::Keyword("none"),
  ];
}

/// A collection of background images.
pub type BackgroundImages = Box<[BackgroundImage]>;

impl<'i> FromCss<'i> for BackgroundImages {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut images = Vec::new();

    images.push(BackgroundImage::from_css(input)?);

    while input.expect_comma().is_ok() {
      images.push(BackgroundImage::from_css(input)?);
    }

    Ok(images.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundImage::VALID_TOKENS;
}

#[cfg(test)]
mod tests {
  use crate::layout::style::RadialSize;

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

  #[test]
  fn test_parse_background_images_radial_explicit_radii() {
    let images = BackgroundImages::from_str(
      "radial-gradient(ellipse 60% 60% at 50% 50%, rgba(255, 53, 53, 0.10) 0%, transparent 70%), radial-gradient(ellipse 30% 30% at 50% 50%, rgba(255, 53, 53, 0.06) 0%, transparent 55%)",
    );

    assert!(match images {
      Ok(images) => matches!(
        images.as_ref(),
        [BackgroundImage::Radial(first), BackgroundImage::Radial(second)]
          if matches!(first.size, RadialSize::Explicit { .. })
            && matches!(second.size, RadialSize::Explicit { .. })
      ),
      Err(_) => false,
    });
  }

  #[test]
  fn test_parse_repeating_gradients_in_background_images() {
    let images = BackgroundImages::from_str(
      "repeating-linear-gradient(90deg, red 0px 5px, blue 5px 10px), repeating-radial-gradient(circle 20px, red 0px 5px, blue 5px 10px), repeating-conic-gradient(from 0deg, red 0deg 90deg, blue 90deg 180deg)",
    );

    assert!(match images {
      Ok(images) => matches!(
        images.as_ref(),
        [BackgroundImage::Linear(linear), BackgroundImage::Radial(radial), BackgroundImage::Conic(conic)]
          if linear.repeating && radial.repeating && conic.repeating
      ),
      Err(_) => false,
    });
  }

  #[test]
  fn test_linear_gradient_from_css_repeating() {
    let mut input = cssparser::ParserInput::new("repeating-linear-gradient(90deg, red, blue)");
    let mut parser = cssparser::Parser::new(&mut input);
    let gradient = LinearGradient::from_css(&mut parser).unwrap();
    assert!(gradient.repeating);
    assert!((*gradient.angle - 90.0).abs() < 1e-3);
    assert_eq!(gradient.stops.len(), 2);
  }

  #[test]
  fn test_radial_gradient_from_css_repeating() {
    let mut input = cssparser::ParserInput::new("repeating-radial-gradient(circle, red, blue)");
    let mut parser = cssparser::Parser::new(&mut input);
    let gradient = RadialGradient::from_css(&mut parser).unwrap();
    assert!(gradient.repeating);
    assert_eq!(gradient.stops.len(), 2);
  }

  #[test]
  fn test_conic_gradient_from_css_repeating() {
    let mut input = cssparser::ParserInput::new("repeating-conic-gradient(from 45deg, red, blue)");
    let mut parser = cssparser::Parser::new(&mut input);
    let gradient = ConicGradient::from_css(&mut parser).unwrap();
    assert!(gradient.repeating);
    assert!((*gradient.from_angle - 45.0).abs() < 1e-3);
    assert_eq!(gradient.stops.len(), 2);
  }
}

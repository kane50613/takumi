use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, ConicGradient, CssDescriptorKind, CssToken, FromCss, LinearGradient,
  ListInterpolationStrategy, MakeComputed, ParseResult, RadialGradient, SizingContext, ToCss,
  properties::write_css_string, tw::TailwindPropertyParser, unexpected_token,
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
  /// CSS conic-gradient(...)
  Conic(ConicGradient),
  /// Load external image resource.
  Url(Arc<str>),
}

impl BackgroundImage {
  /// Whether the layer has anything to paint. `none` is a placeholder that
  /// keeps the other `background-*` lists aligned, so it paints nothing.
  pub fn paints(&self) -> bool {
    !matches!(self, Self::None)
  }
}

impl MakeComputed for BackgroundImage {
  fn make_computed(&mut self, sizing: &SizingContext) {
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
      _ => Err(unexpected_token!(location, &Token::Function(function))),
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

/// Parses a comma-separated list of `parse_item`, requiring at least one.
pub(crate) fn parse_comma_list<'i, T>(
  input: &mut Parser<'i, '_>,
  mut parse_item: impl FnMut(&mut Parser<'i, '_>) -> ParseResult<'i, T>,
) -> ParseResult<'i, Box<[T]>> {
  let mut items = Vec::new();
  items.push(parse_item(input)?);
  while input.expect_comma().is_ok() {
    items.push(parse_item(input)?);
  }
  Ok(items.into_boxed_slice())
}

/// An ordered list of [`BackgroundImage`] values.
pub type BackgroundImages = Box<[BackgroundImage]>;

impl<'i> FromCss<'i> for BackgroundImages {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_list(input, BackgroundImage::from_css)
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundImage::VALID_TOKENS;
}

impl ToCss for BackgroundImage {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::None => dest.write_str("none"),
      Self::Linear(linear) => linear.to_css(dest),
      Self::Radial(radial) => radial.to_css(dest),
      Self::Conic(conic) => conic.to_css(dest),
      Self::Url(url) => {
        dest.write_str("url(")?;
        write_css_string(dest, url)?;
        dest.write_char(')')
      }
    }
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::Theme;
  use crate::style::{
    Angle, Color, ConicGradient, FromCssStr, GradientStop, Length, LinearGradient,
    LinearGradientDirection, PositionValue, RadialGradient, RadialShape, RadialSize, SpacePair,
    StopPosition,
  };

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
      BackgroundImage::parse_tw_with_arbitrary(
        "[url(https://example.com/bg.png)]",
        &Theme::default(),
        BackgroundImage::NAMESPACES,
      ),
      Some(BackgroundImage::Url("https://example.com/bg.png".into()))
    );
  }

  #[test]
  fn test_parse_background_images_radial_explicit_radii() {
    let images = BackgroundImages::from_css_str(
      "radial-gradient(ellipse 60% 60% at 50% 50%, rgba(255, 53, 53, 0.10) 0%, transparent 70%), radial-gradient(ellipse 30% 30% at 50% 50%, rgba(255, 53, 53, 0.06) 0%, transparent 55%)",
    );

    assert_eq!(
      images,
      Ok(
        [
          BackgroundImage::Radial(
            RadialGradient::builder()
              .size(RadialSize::Explicit {
                radius_x: Length::Percentage(0.6 * 100.0),
                radius_y: Length::Percentage(0.6 * 100.0),
              })
              .center(PositionValue(SpacePair::from_pair(
                Length::Percentage(0.5 * 100.0).into(),
                Length::Percentage(0.5 * 100.0).into(),
              )))
              .stops([
                GradientStop::ColorHint {
                  color: Color([255, 53, 53, 26]).into(),
                  hint: Some(StopPosition(Length::Percentage(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::transparent().into(),
                  hint: Some(StopPosition(Length::Percentage(70.0))),
                },
              ])
              .build(),
          ),
          BackgroundImage::Radial(
            RadialGradient::builder()
              .size(RadialSize::Explicit {
                radius_x: Length::Percentage(0.3 * 100.0),
                radius_y: Length::Percentage(0.3 * 100.0),
              })
              .center(PositionValue(SpacePair::from_pair(
                Length::Percentage(0.5 * 100.0).into(),
                Length::Percentage(0.5 * 100.0).into(),
              )))
              .stops([
                GradientStop::ColorHint {
                  color: Color([255, 53, 53, 15]).into(),
                  hint: Some(StopPosition(Length::Percentage(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::transparent().into(),
                  hint: Some(StopPosition(Length::Percentage(55.0))),
                },
              ])
              .build(),
          ),
        ]
        .into()
      )
    );
  }

  #[test]
  fn test_parse_repeating_gradients_in_background_images() {
    let images = BackgroundImages::from_css_str(
      "repeating-linear-gradient(90deg, red 0px 5px, blue 5px 10px), repeating-radial-gradient(circle 20px, red 0px 5px, blue 5px 10px), repeating-conic-gradient(from 0deg, red 0deg 90deg, blue 90deg 180deg)",
    );

    assert_eq!(
      images,
      Ok(
        [
          BackgroundImage::Linear(
            LinearGradient::builder()
              .repeating(true)
              .direction(LinearGradientDirection::Angle(Angle::new(90.0)))
              .stops([
                GradientStop::ColorHint {
                  color: Color::from_rgb(0xff0000).into(),
                  hint: Some(StopPosition(Length::Px(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0xff0000).into(),
                  hint: Some(StopPosition(Length::Px(5.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0x0000ff).into(),
                  hint: Some(StopPosition(Length::Px(5.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0x0000ff).into(),
                  hint: Some(StopPosition(Length::Px(10.0))),
                },
              ])
              .build(),
          ),
          BackgroundImage::Radial(
            RadialGradient::builder()
              .repeating(true)
              .shape(RadialShape::Circle)
              .size(RadialSize::Explicit {
                radius_x: Length::Px(20.0),
                radius_y: Length::Px(20.0),
              })
              .stops([
                GradientStop::ColorHint {
                  color: Color::from_rgb(0xff0000).into(),
                  hint: Some(StopPosition(Length::Px(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0xff0000).into(),
                  hint: Some(StopPosition(Length::Px(5.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0x0000ff).into(),
                  hint: Some(StopPosition(Length::Px(5.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0x0000ff).into(),
                  hint: Some(StopPosition(Length::Px(10.0))),
                },
              ])
              .build(),
          ),
          BackgroundImage::Conic(
            ConicGradient::builder()
              .repeating(true)
              .from_angle(Angle::zero())
              .stops([
                GradientStop::ColorHint {
                  color: Color::from_rgb(0xff0000).into(),
                  hint: Some(StopPosition(Length::Percentage(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0xff0000).into(),
                  hint: Some(StopPosition(Length::Percentage(25.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0x0000ff).into(),
                  hint: Some(StopPosition(Length::Percentage(25.0))),
                },
                GradientStop::ColorHint {
                  color: Color::from_rgb(0x0000ff).into(),
                  hint: Some(StopPosition(Length::Percentage(50.0))),
                },
              ])
              .build(),
          ),
        ]
        .into()
      )
    );
  }
}

use cssparser::Parser;

use crate::layout::style::*;
use crate::rendering::Sizing;

/// Parsed `background` shorthand value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Background {
  /// Background color.
  pub color: Option<ColorInput<false>>,
  /// Background image.
  pub image: BackgroundImage,
  /// Background position.
  pub position: BackgroundPosition,
  /// Background size.
  pub size: BackgroundSize,
  /// Background repeat.
  pub repeat: BackgroundRepeat,
  /// Background clip.
  pub clip: BackgroundClip,
  /// Background blend mode.
  pub blend_mode: BlendMode,
}

impl MakeComputed for Background {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.image.make_computed(sizing);
    self.position.make_computed(sizing);
    self.size.make_computed(sizing);
  }
}

impl<'i> FromCss<'i> for Background {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut color = None;
    let mut image = None;
    let mut position = None;
    let mut size = None;
    let mut repeat = None;
    let mut clip = None;
    let mut blend_mode = None;

    loop {
      if input.is_exhausted() {
        break;
      }

      let state = input.state();
      if input.try_parse(Parser::expect_comma).is_ok() {
        input.reset(&state);
        break;
      }

      // Try to parse background-color
      if color.is_none()
        && let Ok(value) = input.try_parse(ColorInput::from_css)
      {
        color = Some(value);
        continue;
      }

      // Try to parse background-position (and optionally background-size with /)
      if position.is_none()
        && let Ok(value) = input.try_parse(BackgroundPosition::from_css)
      {
        position = Some(value);

        size = input
          .try_parse(|input| {
            input.expect_delim('/')?;
            BackgroundSize::from_css(input)
          })
          .ok();

        continue;
      }

      // Try to parse background-image
      if image.is_none()
        && let Ok(value) = input.try_parse(BackgroundImage::from_css)
      {
        image = Some(value);
        continue;
      }

      // Try to parse background-repeat
      if repeat.is_none()
        && let Ok(value) = input.try_parse(BackgroundRepeat::from_css)
      {
        repeat = Some(value);
        continue;
      }

      // Try to parse background-clip
      if clip.is_none()
        && let Ok(value) = input.try_parse(BackgroundClip::from_css)
      {
        clip = Some(value);
        continue;
      }

      // Try to parse background-blend-mode
      if blend_mode.is_none()
        && let Ok(value) = input.try_parse(BlendMode::from_css)
      {
        blend_mode = Some(value);
        continue;
      }

      // If we can't parse anything, it's an error
      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        input.next()?,
      ));
    }

    Ok(Background {
      color,
      image: image.unwrap_or_default(),
      position: position.unwrap_or_default(),
      size: size.unwrap_or_default(),
      repeat: repeat.unwrap_or_default(),
      clip: clip.unwrap_or_default(),
      blend_mode: blend_mode.unwrap_or_default(),
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("color"),
      CssToken::Token("image"),
      CssToken::Token("position"),
      CssToken::Token("repeat"),
      CssToken::Token("clip"),
      CssToken::Token("blend-mode"),
    ]
  }
}

/// A list of background properties (one per layer).
pub type Backgrounds = Box<[Background]>;

impl<'i> FromCss<'i> for Backgrounds {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut backgrounds = Vec::new();
    backgrounds.push(Background::from_css(input)?);

    while input.expect_comma().is_ok() {
      backgrounds.push(Background::from_css(input)?);
    }

    Ok(backgrounds.into_boxed_slice())
  }

  fn valid_tokens() -> &'static [CssToken] {
    Background::valid_tokens()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_background_color_only() {
    assert_eq!(
      Background::from_str("red"),
      Ok(Background {
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_color_and_clip() {
    assert_eq!(
      Background::from_str("red border-box"),
      Ok(Background {
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        clip: BackgroundClip::BorderBox,
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_with_position_and_size() {
    assert_eq!(
      Background::from_str("center/cover"),
      Ok(Background {
        position: BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Center),
        )),
        size: BackgroundSize::Cover,
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_full() {
    assert_eq!(
      Background::from_str("no-repeat center/80% url(../img/image.png)"),
      Ok(Background {
        image: BackgroundImage::Url("../img/image.png".into()),
        position: BackgroundPosition(SpacePair::from_pair(
          PositionComponent::KeywordX(PositionKeywordX::Center),
          PositionComponent::KeywordY(PositionKeywordY::Center),
        )),
        size: BackgroundSize::Explicit {
          width: Length::Percentage(80.0),
          height: Length::Auto,
        },
        repeat: BackgroundRepeat::no_repeat(),
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_empty() {
    assert_eq!(Background::from_str(""), Ok(Background::default()));
  }

  #[test]
  fn test_parse_background_invalid() {
    assert!(Background::from_str("invalid-value").is_err());
  }

  #[test]
  fn test_parse_backgrounds_multiple_gradients() {
    assert_eq!(
      Backgrounds::from_str(
        "radial-gradient(circle at 80% 20%, #FF3D00 0%, transparent 40%), radial-gradient(circle at 20% 80%, #00E5FF 0%, transparent 40%)",
      ),
      Ok(
        vec![
          Background {
            image: BackgroundImage::Radial(RadialGradient {
              shape: RadialShape::Circle,
              size: RadialSize::FarthestCorner,
              center: BackgroundPosition(SpacePair::from_pair(
                Length::Percentage(80.0).into(),
                Length::Percentage(20.0).into(),
              )),
              interpolation: ColorInterpolationMethod::default(),
              stops: [
                GradientStop::ColorHint {
                  color: Color([255, 61, 0, 255]).into(),
                  hint: Some(StopPosition(Length::Percentage(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::transparent().into(),
                  hint: Some(StopPosition(Length::Percentage(40.0))),
                },
              ]
              .into(),
            }),
            ..Default::default()
          },
          Background {
            image: BackgroundImage::Radial(RadialGradient {
              shape: RadialShape::Circle,
              size: RadialSize::FarthestCorner,
              center: BackgroundPosition(SpacePair::from_pair(
                Length::Percentage(20.0).into(),
                Length::Percentage(80.0).into(),
              )),
              interpolation: ColorInterpolationMethod::default(),
              stops: [
                GradientStop::ColorHint {
                  color: Color([0, 229, 255, 255]).into(),
                  hint: Some(StopPosition(Length::Percentage(0.0))),
                },
                GradientStop::ColorHint {
                  color: Color::transparent().into(),
                  hint: Some(StopPosition(Length::Percentage(40.0))),
                },
              ]
              .into(),
            }),
            ..Default::default()
          },
        ]
        .into_boxed_slice()
      )
    );
  }
}

use cssparser::Parser;

use crate::style::{unexpected_token, *};

/// Parsed `background` shorthand value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Background {
  /// Background color.
  pub color: Option<ColorInput>,
  /// Background image.
  pub image: BackgroundImage,
  /// Background position.
  pub position: PositionValue,
  /// Background size.
  pub size: BackgroundSize,
  /// Background repeat.
  pub repeat: BackgroundRepeat,
  /// Background clip.
  pub clip: BackgroundClip,
  /// Background origin.
  pub origin: BackgroundOrigin,
  /// Background blend mode.
  pub blend_mode: BlendMode,
}

impl MakeComputed for Background {
  fn make_computed(&mut self, sizing: &SizingContext) {
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
    let mut origin = None;
    let mut clip = None;
    let mut box_count = 0u8;
    let mut blend_mode = None;

    while !input.is_exhausted() && !next_is_comma(input) {
      // Try to parse background-color
      if color.is_none()
        && let Ok(value) = input.try_parse(ColorInput::from_css)
      {
        color = Some(value);
        continue;
      }

      // Try to parse background-position (and optionally background-size with /)
      if position.is_none()
        && let Ok(value) = input.try_parse(PositionValue::from_css)
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

      // A `<box>` value: the first sets origin (and clip), a second overrides
      // clip only. https://drafts.csswg.org/css-backgrounds/#background
      if box_count < 2
        && let Ok(value) = input.try_parse(BackgroundClip::from_css)
      {
        if box_count == 0 {
          origin = Option::<BackgroundOrigin>::from(value);
        }
        clip = Some(value);
        box_count += 1;
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
      return Err(unexpected_token!(
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
      origin: origin.unwrap_or_default(),
      blend_mode: blend_mode.unwrap_or_default(),
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Color),
    CssToken::Syntax(CssSyntaxKind::Image),
    CssToken::Syntax(CssSyntaxKind::Position),
    CssToken::Syntax(CssSyntaxKind::Repeat),
    CssToken::Syntax(CssSyntaxKind::Clip),
    CssToken::Descriptor(CssDescriptorKind::BlendMode),
  ];
}

/// A list of background properties (one per layer).
pub(crate) type Backgrounds = Box<[Background]>;

impl<'i> FromCss<'i> for Backgrounds {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(
      input
        .parse_comma_separated(Background::from_css)?
        .into_boxed_slice(),
    )
  }

  const VALID_TOKENS: &'static [CssToken] = Background::VALID_TOKENS;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_background_color_only() {
    assert_eq!(
      Background::from_css_str("red"),
      Ok(Background {
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_color_and_clip() {
    // A single `<box>` sets both origin and clip.
    assert_eq!(
      Background::from_css_str("red border-box"),
      Ok(Background {
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
        clip: BackgroundClip::BorderBox,
        origin: BackgroundOrigin::BorderBox,
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_two_boxes_origin_then_clip() {
    assert_eq!(
      Background::from_css_str("content-box border-box"),
      Ok(Background {
        origin: BackgroundOrigin::ContentBox,
        clip: BackgroundClip::BorderBox,
        ..Default::default()
      })
    );
  }

  #[test]
  fn test_parse_background_default_origin_is_padding_box() {
    assert_eq!(Background::default().origin, BackgroundOrigin::PaddingBox);
    assert_eq!(
      Background::from_css_str("red").unwrap().origin,
      BackgroundOrigin::PaddingBox
    );
  }

  #[test]
  fn test_parse_background_with_position_and_size() {
    assert_eq!(
      Background::from_css_str("center/cover"),
      Ok(Background {
        position: PositionValue(SpacePair::from_pair(
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
      Background::from_css_str("no-repeat center/80% url(../img/image.png)"),
      Ok(Background {
        image: BackgroundImage::Url("../img/image.png".into()),
        position: PositionValue(SpacePair::from_pair(
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
    assert_eq!(Background::from_css_str(""), Ok(Background::default()));
  }

  #[test]
  fn test_parse_background_invalid() {
    assert!(Background::from_css_str("invalid-value").is_err());
  }

  #[test]
  fn test_parse_backgrounds_multiple_gradients() {
    assert_eq!(
      Backgrounds::from_css_str(
        "radial-gradient(circle at 80% 20%, #FF3D00 0%, transparent 40%), radial-gradient(circle at 20% 80%, #00E5FF 0%, transparent 40%)",
      ),
      Ok(
        vec![
          Background {
            image: BackgroundImage::Radial(
              RadialGradient::builder()
                .shape(RadialShape::Circle)
                .center(PositionValue(SpacePair::from_pair(
                  Length::Percentage(80.0).into(),
                  Length::Percentage(20.0).into(),
                )))
                .stops([
                  GradientStop::ColorHint {
                    color: Color([255, 61, 0, 255]).into(),
                    hint: Some(StopPosition(Length::Percentage(0.0))),
                  },
                  GradientStop::ColorHint {
                    color: Color::transparent().into(),
                    hint: Some(StopPosition(Length::Percentage(40.0))),
                  },
                ])
                .build(),
            ),
            ..Default::default()
          },
          Background {
            image: BackgroundImage::Radial(
              RadialGradient::builder()
                .shape(RadialShape::Circle)
                .center(PositionValue(SpacePair::from_pair(
                  Length::Percentage(20.0).into(),
                  Length::Percentage(80.0).into(),
                )))
                .stops([
                  GradientStop::ColorHint {
                    color: Color([0, 229, 255, 255]).into(),
                    hint: Some(StopPosition(Length::Percentage(0.0))),
                  },
                  GradientStop::ColorHint {
                    color: Color::transparent().into(),
                    hint: Some(StopPosition(Length::Percentage(40.0))),
                  },
                ])
                .build(),
            ),
            ..Default::default()
          },
        ]
        .into_boxed_slice()
      )
    );
  }
}

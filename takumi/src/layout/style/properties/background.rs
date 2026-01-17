use cssparser::Parser;

use crate::layout::style::*;

/// Parsed `background` shorthand value.
///
/// The background shorthand allows setting multiple background properties at once:
/// - color
/// - image
/// - position (optionally followed by / size)
/// - repeat
/// - clip
///
/// Example: `background: red url(image.png) center/cover no-repeat border-box`
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
}

impl<'i> FromCss<'i> for Background {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut color = None;
    let mut image = None;
    let mut position = None;
    let mut size = None;
    let mut repeat = None;
    let mut clip = None;

    loop {
      if input.is_exhausted() {
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
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("color"),
      CssToken::Token("image"),
      CssToken::Token("position"),
      CssToken::Token("repeat"),
      CssToken::Token("clip"),
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
}

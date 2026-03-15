use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::Size;

use crate::{
  layout::style::{
    Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, ListInterpolationStrategy,
    MakeComputed, ParseResult, tw::TailwindPropertyParser,
  },
  rendering::Sizing,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoBackgroundAxis {
  Width,
  Height,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedBackgroundSize {
  pub(crate) width: u32,
  pub(crate) height: u32,
  pub(crate) intrinsic_size: Option<(f32, f32)>,
  pub(crate) auto_axis: Option<AutoBackgroundAxis>,
}

/// Parsed `background-size` for one layer.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum BackgroundSize {
  /// Scale the image to cover the container (may crop).
  Cover,
  /// Scale the image to be fully contained within the container.
  Contain,
  /// Explicit width and height values.
  Explicit {
    /// Width value for the background image.
    width: Length,
    /// Height value for the background image.
    height: Length,
  },
}

impl TailwindPropertyParser for BackgroundSize {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "cover" => Some(BackgroundSize::Cover),
      "contain" => Some(BackgroundSize::Contain),
      _ => None,
    }
  }
}

impl Default for BackgroundSize {
  fn default() -> Self {
    BackgroundSize::Explicit {
      width: Length::Auto,
      height: Length::Auto,
    }
  }
}

impl<'i> FromCss<'i> for BackgroundSize {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(width) = input.try_parse(Length::from_css) {
      let height = input.try_parse(Length::from_css).unwrap_or(Length::Auto);

      return Ok(BackgroundSize::Explicit { width, height });
    }

    let location = input.current_source_location();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! {
      &ident,
      "cover" => Ok(BackgroundSize::Cover),
      "contain" => Ok(BackgroundSize::Contain),
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident.clone()))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("cover"),
    CssToken::Keyword("contain"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl MakeComputed for BackgroundSize {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Self::Explicit { width, height } = self {
      width.make_computed(sizing);
      height.make_computed(sizing);
    }
  }
}

impl Animatable for BackgroundSize {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::RepeatToLcm
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &Sizing,
    current_color: Color,
  ) {
    *self = match (*from, *to) {
      (
        BackgroundSize::Explicit {
          width: from_width,
          height: from_height,
        },
        BackgroundSize::Explicit {
          width: to_width,
          height: to_height,
        },
      ) => {
        let mut width = from_width;
        width.interpolate(&from_width, &to_width, progress, sizing, current_color);
        let mut height = from_height;
        height.interpolate(&from_height, &to_height, progress, sizing, current_color);
        BackgroundSize::Explicit { width, height }
      }
      _ => {
        if progress >= 0.5 {
          *to
        } else {
          *from
        }
      }
    };
  }
}

impl BackgroundSize {
  pub(crate) fn resolve(
    self,
    area: Size<u32>,
    sizing: &Sizing,
    intrinsic_size: Option<(f32, f32)>,
  ) -> ResolvedBackgroundSize {
    match self {
      BackgroundSize::Explicit { width, height } => {
        if width != Length::Auto && height != Length::Auto {
          return ResolvedBackgroundSize {
            width: width.to_px(sizing, area.width as f32).max(0.0) as u32,
            height: height.to_px(sizing, area.height as f32).max(0.0) as u32,
            intrinsic_size: None,
            auto_axis: None,
          };
        }

        let (resolved_width, resolved_height) =
          resolve_auto_background_size(width, height, area, sizing, intrinsic_size);

        ResolvedBackgroundSize {
          width: resolved_width,
          height: resolved_height,
          intrinsic_size,
          auto_axis: match (width == Length::Auto, height == Length::Auto) {
            (true, false) => Some(AutoBackgroundAxis::Width),
            (false, true) => Some(AutoBackgroundAxis::Height),
            _ => None,
          },
        }
      }
      BackgroundSize::Cover => {
        let Some((intrinsic_width, intrinsic_height)) = intrinsic_size else {
          return ResolvedBackgroundSize {
            width: 0,
            height: 0,
            intrinsic_size: None,
            auto_axis: None,
          };
        };

        if intrinsic_width == 0.0 || intrinsic_height == 0.0 {
          return ResolvedBackgroundSize {
            width: 0,
            height: 0,
            intrinsic_size: Some((intrinsic_width, intrinsic_height)),
            auto_axis: None,
          };
        }

        let scale_x = area.width as f32 / intrinsic_width;
        let scale_y = area.height as f32 / intrinsic_height;
        let scale = scale_x.max(scale_y);

        ResolvedBackgroundSize {
          width: (intrinsic_width * scale).round() as u32,
          height: (intrinsic_height * scale).round() as u32,
          intrinsic_size: Some((intrinsic_width, intrinsic_height)),
          auto_axis: None,
        }
      }
      BackgroundSize::Contain => {
        let Some((intrinsic_width, intrinsic_height)) = intrinsic_size else {
          return ResolvedBackgroundSize {
            width: 0,
            height: 0,
            intrinsic_size: None,
            auto_axis: None,
          };
        };

        if intrinsic_width == 0.0 || intrinsic_height == 0.0 {
          return ResolvedBackgroundSize {
            width: 0,
            height: 0,
            intrinsic_size: Some((intrinsic_width, intrinsic_height)),
            auto_axis: None,
          };
        }

        let scale_x = area.width as f32 / intrinsic_width;
        let scale_y = area.height as f32 / intrinsic_height;
        let scale = scale_x.min(scale_y);

        ResolvedBackgroundSize {
          width: (intrinsic_width * scale).round() as u32,
          height: (intrinsic_height * scale).round() as u32,
          intrinsic_size: Some((intrinsic_width, intrinsic_height)),
          auto_axis: None,
        }
      }
    }
  }
}

fn resolve_auto_background_size(
  width: Length,
  height: Length,
  area: Size<u32>,
  sizing: &Sizing,
  intrinsic_size: Option<(f32, f32)>,
) -> (u32, u32) {
  match (width == Length::Auto, height == Length::Auto) {
    (true, true) => {
      let Some((intrinsic_width, intrinsic_height)) = intrinsic_size else {
        return (area.width, area.height);
      };

      (
        intrinsic_width.round() as u32,
        intrinsic_height.round() as u32,
      )
    }
    (true, false) => {
      let fixed_height = height.to_px(sizing, area.height as f32).max(0.0);
      let Some((intrinsic_width, intrinsic_height)) = intrinsic_size else {
        return (area.width, fixed_height as u32);
      };
      if intrinsic_width == 0.0 || intrinsic_height == 0.0 {
        return (0, 0);
      }

      let scale_factor = fixed_height / intrinsic_height;
      (
        (intrinsic_width * scale_factor).round() as u32,
        fixed_height as u32,
      )
    }
    (false, true) => {
      let fixed_width = width.to_px(sizing, area.width as f32).max(0.0);
      let Some((intrinsic_width, intrinsic_height)) = intrinsic_size else {
        return (fixed_width as u32, area.height);
      };
      if intrinsic_width == 0.0 || intrinsic_height == 0.0 {
        return (0, 0);
      }

      let scale_factor = fixed_width / intrinsic_width;
      (
        fixed_width as u32,
        (intrinsic_height * scale_factor).round() as u32,
      )
    }
    (false, false) => unreachable!(),
  }
}

/// A list of `background-size` values (one per layer).
pub type BackgroundSizes = Box<[BackgroundSize]>;

impl<'i> FromCss<'i> for BackgroundSizes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = Vec::new();
    values.push(BackgroundSize::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BackgroundSize::from_css(input)?);
    }

    Ok(values.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundSize::VALID_TOKENS;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_cover_keyword() {
    assert_eq!(BackgroundSize::from_str("cover"), Ok(BackgroundSize::Cover));
  }

  #[test]
  fn parses_contain_keyword() {
    assert_eq!(
      BackgroundSize::from_str("contain"),
      Ok(BackgroundSize::Contain)
    );
  }

  #[test]
  fn parses_single_percentage_value_as_both_dimensions() {
    assert_eq!(
      BackgroundSize::from_str("50%\t"),
      Ok(BackgroundSize::Explicit {
        width: Length::Percentage(50.0),
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn parses_single_auto_value_as_both_dimensions() {
    assert_eq!(
      BackgroundSize::from_str("auto"),
      Ok(BackgroundSize::Explicit {
        width: Length::Auto,
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn parses_two_values_mixed_units() {
    assert_eq!(
      BackgroundSize::from_str("100px auto"),
      Ok(BackgroundSize::Explicit {
        width: Length::Px(100.0),
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn errors_on_unknown_identifier() {
    assert!(BackgroundSize::from_str("bogus").is_err());
  }

  #[test]
  fn parses_multiple_layers_with_keywords_and_values() {
    assert_eq!(
      BackgroundSizes::from_str("cover, 50% auto"),
      Ok(
        [
          BackgroundSize::Cover,
          BackgroundSize::Explicit {
            width: Length::Percentage(50.0),
            height: Length::Auto,
          }
        ]
        .into()
      )
    );
  }

  #[test]
  fn parses_multiple_layers_with_single_value_duplication() {
    assert_eq!(
      BackgroundSizes::from_str("25%, contain"),
      Ok(
        [
          BackgroundSize::Explicit {
            width: Length::Percentage(25.0),
            height: Length::Auto,
          },
          BackgroundSize::Contain
        ]
        .into()
      )
    );
  }

  #[test]
  fn errors_on_invalid_first_layer() {
    assert!(BackgroundSizes::from_str("nope").is_err());
  }
}

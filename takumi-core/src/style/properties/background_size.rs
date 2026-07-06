use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use super::{background_image::parse_comma_list, background_size_resolve::*};
use crate::{
  geometry::Size,
  style::{
    Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, ListInterpolationStrategy,
    MakeComputed, ParseResult, SizingContext, ToCss, tw::TailwindPropertyParser, unexpected_token,
  },
};

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
    match_ignore_ascii_case! {token,
      "auto" => Some(BackgroundSize::Explicit {
        width: Length::Auto,
        height: Length::Auto,
      }),
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
      _ => Err(unexpected_token!(location, &Token::Ident(ident.clone()))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("cover"),
    CssToken::Keyword("contain"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl MakeComputed for BackgroundSize {
  fn make_computed(&mut self, sizing: &SizingContext) {
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
    sizing: &SizingContext,
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
  /// Resolves this value against the positioning area and intrinsic sizing.
  pub fn resolve(
    self,
    area: Size<u32>,
    sizing: &SizingContext,
    intrinsic: IntrinsicSizing,
  ) -> ResolvedBackgroundSize {
    match self {
      BackgroundSize::Explicit { width, height } => {
        if width != Length::Auto && height != Length::Auto {
          return ResolvedBackgroundSize {
            width: width.to_px(sizing, area.width as f32).max(0.0) as u32,
            height: height.to_px(sizing, area.height as f32).max(0.0) as u32,
            intrinsic_ratio: None,
            auto_axis: None,
          };
        }

        let (resolved_width, resolved_height) =
          resolve_auto_background_size(width, height, area, sizing, intrinsic);

        ResolvedBackgroundSize {
          width: resolved_width,
          height: resolved_height,
          intrinsic_ratio: intrinsic.ratio,
          auto_axis: match (width == Length::Auto, height == Length::Auto) {
            (true, false) => Some(AutoBackgroundAxis::Width),
            (false, true) => Some(AutoBackgroundAxis::Height),
            _ => None,
          },
        }
      }
      // cover/contain scale the intrinsic ratio to the area; with no ratio the
      // area is filled (§5.3).
      BackgroundSize::Cover | BackgroundSize::Contain => {
        let Some(ratio) = intrinsic.ratio.filter(|ratio| *ratio > 0.0) else {
          return ResolvedBackgroundSize {
            width: area.width,
            height: area.height,
            intrinsic_ratio: intrinsic.ratio,
            auto_axis: None,
          };
        };

        let (width, height) = fit_ratio_to_area(ratio, area, matches!(self, BackgroundSize::Cover));

        ResolvedBackgroundSize {
          width,
          height,
          intrinsic_ratio: Some(ratio),
          auto_axis: None,
        }
      }
    }
  }
}

/// An ordered list of [`BackgroundSize`] values.
pub type BackgroundSizes = Box<[BackgroundSize]>;

impl<'i> FromCss<'i> for BackgroundSizes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_list(input, BackgroundSize::from_css)
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundSize::VALID_TOKENS;
}

impl ToCss for BackgroundSize {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Cover => dest.write_str("cover"),
      Self::Contain => dest.write_str("contain"),
      Self::Explicit { width, height } => {
        if *height == Length::Auto {
          width.to_css(dest)
        } else {
          width.to_css(dest)?;
          dest.write_str(" ")?;
          height.to_css(dest)
        }
      }
    }
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_cover_keyword() {
    assert_eq!(
      BackgroundSize::from_css_str("cover"),
      Ok(BackgroundSize::Cover)
    );
  }

  #[test]
  fn parses_contain_keyword() {
    assert_eq!(
      BackgroundSize::from_css_str("contain"),
      Ok(BackgroundSize::Contain)
    );
  }

  #[test]
  fn parses_single_percentage_value_as_both_dimensions() {
    assert_eq!(
      BackgroundSize::from_css_str("50%\t"),
      Ok(BackgroundSize::Explicit {
        width: Length::Percentage(50.0),
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn parses_single_auto_value_as_both_dimensions() {
    assert_eq!(
      BackgroundSize::from_css_str("auto"),
      Ok(BackgroundSize::Explicit {
        width: Length::Auto,
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn parses_two_values_mixed_units() {
    assert_eq!(
      BackgroundSize::from_css_str("100px auto"),
      Ok(BackgroundSize::Explicit {
        width: Length::Px(100.0),
        height: Length::Auto,
      })
    );
  }

  #[test]
  fn errors_on_unknown_identifier() {
    assert!(BackgroundSize::from_css_str("bogus").is_err());
  }

  #[test]
  fn parses_multiple_layers_with_keywords_and_values() {
    assert_eq!(
      BackgroundSizes::from_css_str("cover, 50% auto"),
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
      BackgroundSizes::from_css_str("25%, contain"),
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
    assert!(BackgroundSizes::from_css_str("nope").is_err());
  }
}

use crate::layout::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token, match_ignore_ascii_case};
use std::fmt;
use taffy::Size;

use crate::{
  layout::style::SizingContext,
  layout::style::{
    Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, ListInterpolationStrategy,
    MakeComputed, ParseResult, tw::TailwindPropertyParser,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoBackgroundAxis {
  Width,
  Height,
}

/// Intrinsic sizing of an image, per CSS Images Level 3 §5.3. `width`/`height`
/// are set only where the image has an intrinsic dimension on that axis (a
/// non-percentage SVG `width`/`height`); a `viewBox`-only SVG has `ratio` alone.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct IntrinsicSizing {
  pub(crate) width: Option<f32>,
  pub(crate) height: Option<f32>,
  pub(crate) ratio: Option<f32>,
}

impl IntrinsicSizing {
  pub(crate) fn from_dimensions(width: f32, height: f32) -> Self {
    Self {
      width: Some(width),
      height: Some(height),
      ratio: (height != 0.0).then_some(width / height),
    }
  }

  /// §5.3 default sizing algorithm with `area` as the default object size
  /// (Blink's `ConcreteObjectSize`).
  fn concrete_size(self, area_width: f32, area_height: f32) -> (u32, u32) {
    let round = |width: f32, height: f32| (width.round() as u32, height.round() as u32);

    match (self.width, self.height) {
      (Some(width), Some(height)) => round(width, height),
      (Some(width), None) => match self.ratio {
        Some(ratio) if ratio != 0.0 => round(width, width / ratio),
        _ => round(width, area_height),
      },
      (None, Some(height)) => match self.ratio {
        Some(ratio) => round(height * ratio, height),
        None => round(area_width, height),
      },
      (None, None) => match self.ratio {
        // Contain the intrinsic ratio within the default object size.
        Some(ratio) if ratio != 0.0 => {
          let solution_width = area_height * ratio;
          if solution_width <= area_width {
            round(solution_width, area_height)
          } else {
            round(area_width, area_width / ratio)
          }
        }
        _ => round(area_width, area_height),
      },
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedBackgroundSize {
  pub(crate) width: u32,
  pub(crate) height: u32,
  pub(crate) intrinsic_ratio: Option<f32>,
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
  pub(crate) fn resolve(
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

/// Scale a rectangle of the given aspect ratio (`width / height`) to cover or
/// contain `area`, returning rounded device-pixel dimensions.
fn fit_ratio_to_area(ratio: f32, area: Size<u32>, cover: bool) -> (u32, u32) {
  let area_width = area.width as f32;
  let area_height = area.height as f32;
  let width_at_area_height = area_height * ratio;

  let (width, height) = if (width_at_area_height >= area_width) == cover {
    (width_at_area_height, area_height)
  } else {
    (area_width, area_width / ratio)
  };

  (width.round() as u32, height.round() as u32)
}

fn resolve_auto_background_size(
  width: Length,
  height: Length,
  area: Size<u32>,
  sizing: &SizingContext,
  intrinsic: IntrinsicSizing,
) -> (u32, u32) {
  let area_width = area.width as f32;
  let area_height = area.height as f32;

  match (width == Length::Auto, height == Length::Auto) {
    (true, true) => intrinsic.concrete_size(area_width, area_height),
    // One axis definite: the auto axis follows the ratio, else that axis'
    // intrinsic dimension, else the area (§5.3).
    (true, false) => {
      let fixed_height = height.to_px(sizing, area_height).max(0.0);
      let resolved_width = match intrinsic.ratio {
        Some(ratio) => fixed_height * ratio,
        None => intrinsic.width.unwrap_or(area_width),
      };
      (resolved_width.round() as u32, fixed_height.round() as u32)
    }
    (false, true) => {
      let fixed_width = width.to_px(sizing, area_width).max(0.0);
      let resolved_height = match intrinsic.ratio {
        Some(ratio) if ratio != 0.0 => fixed_width / ratio,
        Some(_) => 0.0,
        None => intrinsic.height.unwrap_or(area_height),
      };
      (fixed_width.round() as u32, resolved_height.round() as u32)
    }
    (false, false) => (0, 0),
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

  // `auto auto` (the default `background-size`/`mask-size`) sizing, per the CSS
  // Images Level 3 §5.3 default sizing algorithm.

  #[test]
  fn auto_size_uses_intrinsic_dimensions_when_present() {
    let intrinsic = IntrinsicSizing::from_dimensions(102.0, 38.0);
    assert_eq!(intrinsic.concrete_size(1200.0, 630.0), (102, 38));
  }

  #[test]
  fn auto_size_contains_ratio_only_image_within_area() {
    // viewBox-only: contained within the area, not sized to the viewBox (which
    // would tile under the default `repeat`).
    let intrinsic = IntrinsicSizing {
      width: None,
      height: None,
      ratio: Some(1.0),
    };
    assert_eq!(intrinsic.concrete_size(400.0, 400.0), (400, 400));
    assert_eq!(intrinsic.concrete_size(1200.0, 630.0), (630, 630));
  }

  #[test]
  fn auto_size_fills_area_without_intrinsic_information() {
    // No dimensions and no ratio (e.g. a gradient): fill the positioning area.
    assert_eq!(
      IntrinsicSizing::default().concrete_size(1200.0, 630.0),
      (1200, 630)
    );
  }

  #[test]
  fn contain_and_cover_scale_ratio_to_area() {
    let area = Size {
      width: 800,
      height: 400,
    };
    // A 1:1 ratio: `contain` is the largest square fitting the area, `cover`
    // the smallest square covering it.
    assert_eq!(fit_ratio_to_area(1.0, area, false), (400, 400));
    assert_eq!(fit_ratio_to_area(1.0, area, true), (800, 800));
  }
}

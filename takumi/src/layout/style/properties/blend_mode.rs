use cssparser::Parser;

use crate::layout::style::tw::TailwindPropertyParser;
use crate::layout::style::{
  Animatable, CssToken, FromCss, ListInterpolationStrategy, ParseResult, declare_enum_from_css_impl,
};

/// A list of blend modes.
pub type BlendModes = Box<[BlendMode]>;

impl<'i> FromCss<'i> for BlendModes {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut values = Vec::new();
    values.push(BlendMode::from_css(input)?);

    while input.expect_comma().is_ok() {
      values.push(BlendMode::from_css(input)?);
    }

    Ok(values.into_boxed_slice())
  }

  const VALID_TOKENS: &'static [CssToken] = BlendMode::VALID_TOKENS;
}

/// Defines the blending mode for an element.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BlendMode {
  /// The final color is the top color, regardless of what the bottom color is.
  #[default]
  Normal,
  /// The final color is the result of multiplying the top and bottom colors.
  Multiply,
  /// The final color is the result of inverting the colors, multiplying them, and inverting that value.
  Screen,
  /// The final color is the result of multiply if the bottom color is darker, or screen if the bottom color is lighter.
  Overlay,
  /// The final color is composed of the darkest values of each color channel.
  Darken,
  /// The final color is composed of the lightest values of each color channel.
  Lighten,
  /// The final color is the result of dividing the bottom color by the inverse of the top color.
  ColorDodge,
  /// The final color is the result of inverting the bottom color, dividing the value by the top color, and inverting that value.
  ColorBurn,
  /// The final color is the result of multiply if the top color is darker, or screen if the top color is lighter.
  HardLight,
  /// The final color is similar to hard-light, but softer.
  SoftLight,
  /// The final color is the result of subtracting the darker of the two colors from the lighter one.
  Difference,
  /// The final color is similar to difference, but with less contrast.
  Exclusion,
  /// The final color has the hue of the top color, while using the saturation and luminosity of the bottom color.
  Hue,
  /// The final color has the saturation of the top color, while using the hue and luminosity of the bottom color.
  Saturation,
  /// The final color has the hue and saturation of the top color, while using the luminosity of the bottom color.
  Color,
  /// The final color has the luminosity of the top color, while using the hue and saturation of the bottom color.
  Luminosity,
  /// The final color is the result of adding the source and backdrop colors, clamped to maximum.
  PlusLighter,
  /// The final color is the result of adding the source and backdrop colors, clamped to minimum.
  PlusDarker,
}

declare_enum_from_css_impl!(
  BlendMode,
  "normal" => BlendMode::Normal,
  "multiply" => BlendMode::Multiply,
  "screen" => BlendMode::Screen,
  "overlay" => BlendMode::Overlay,
  "darken" => BlendMode::Darken,
  "lighten" => BlendMode::Lighten,
  "color-dodge" => BlendMode::ColorDodge,
  "color-burn" => BlendMode::ColorBurn,
  "hard-light" => BlendMode::HardLight,
  "soft-light" => BlendMode::SoftLight,
  "difference" => BlendMode::Difference,
  "exclusion" => BlendMode::Exclusion,
  "hue" => BlendMode::Hue,
  "saturation" => BlendMode::Saturation,
  "color" => BlendMode::Color,
  "luminosity" => BlendMode::Luminosity,
  "plus-lighter" => BlendMode::PlusLighter,
  "plus-darker" => BlendMode::PlusDarker
);

impl TailwindPropertyParser for BlendMode {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl Animatable for BlendMode {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::RepeatToLcm
  }
}

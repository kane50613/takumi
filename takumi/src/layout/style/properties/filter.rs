use std::ops::Deref;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::{
  Pixel, RgbaImage,
  imageops::colorops::{contrast_in_place, huerotate_in_place},
};
use smallvec::SmallVec;

use crate::layout::style::{Angle, FromCss, LengthUnit, ParseResult, PercentageNumber, TextShadow};

/// Represents a single CSS filter operation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
  /// Brightness multiplier (1 = unchanged). Accepts number or percentage
  Brightness(PercentageNumber),
  /// Contrast multiplier (1 = unchanged). Accepts number or percentage
  Contrast(PercentageNumber),
  /// Grayscale amount (0..1). Accepts number or percentage
  Grayscale(PercentageNumber),
  /// Saturate multiplier (1 = unchanged). Accepts number or percentage
  Saturate(PercentageNumber),
  /// Hue rotation in degrees
  HueRotate(Angle),
  /// Invert amount (0..1). Accepts number or percentage
  Invert(PercentageNumber),
  /// Opacity amount (0..1). Accepts number or percentage
  Opacity(PercentageNumber),
  /// Blur radius in pixels
  Blur(LengthUnit),
  /// Drop shadow effect with offset, blur, and color (reuses TextShadow parsing)
  DropShadow(TextShadow),
}

#[derive(Debug, Clone, PartialEq, Default)]
/// A list of filter operations
pub struct Filters(SmallVec<[Filter; 4]>);

impl Deref for Filters {
  type Target = SmallVec<[Filter; 4]>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Filters {
  pub(crate) fn apply_to(&self, image: &mut RgbaImage) {
    for filter in self.0.iter() {
      match *filter {
        Filter::Brightness(PercentageNumber(value)) => {
          for pixel in image.pixels_mut() {
            for channel in pixel.0.iter_mut().take(3) {
              *channel = ((*channel) as f32 * value).clamp(0.0, 255.0) as u8;
            }
          }
        }
        Filter::Contrast(PercentageNumber(value)) => {
          let amount = value * 100.0 - 100.0;
          contrast_in_place(image, amount);
        }
        Filter::Grayscale(PercentageNumber(amount)) => {
          for pixel in image.pixels_mut() {
            let lum = pixel.to_luma().0[0] as f32;

            for channel in pixel.0.iter_mut().take(3) {
              *channel =
                ((*channel as f32 * (1.0 - amount)) + (lum * amount)).clamp(0.0, 255.0) as u8;
            }
          }
        }
        Filter::HueRotate(angle) => {
          huerotate_in_place(image, *angle as i32);
        }
        Filter::Saturate(PercentageNumber(value)) => {
          for pixel in image.pixels_mut() {
            let lum = pixel.to_luma().0[0] as f32;

            for channel in pixel.0.iter_mut().take(3) {
              *channel = (lum * (1.0 - value) + *channel as f32 * value).clamp(0.0, 255.0) as u8;
            }
          }
        }
        Filter::Invert(PercentageNumber(amount)) => {
          for pixel in image.pixels_mut() {
            for channel in pixel.0.iter_mut().take(3) {
              let inverted = u8::MAX.saturating_sub(*channel);
              *channel = ((*channel as f32 * (1.0 - amount)) + (inverted as f32 * amount))
                .clamp(0.0, 255.0) as u8;
            }
          }
        }
        Filter::Opacity(PercentageNumber(value)) => {
          for alpha in image.as_mut().iter_mut().skip(3).step_by(4) {
            *alpha = ((*alpha) as f32 * value).clamp(0.0, 255.0) as u8;
          }
        }
        // Blur and DropShadow require node-level rendering and are handled separately
        Filter::Blur(_) | Filter::DropShadow(_) => {}
      }
    }
  }

  /// Returns true if any filter requires node-level rendering (blur or drop-shadow)
  pub(crate) fn requires_node_level_rendering(&self) -> bool {
    self
      .0
      .iter()
      .any(|f| matches!(f, Filter::Blur(_) | Filter::DropShadow(_)))
  }

  /// Returns the blur radius if a blur filter is present, otherwise None
  pub(crate) fn get_blur(&self) -> Option<LengthUnit> {
    self.0.iter().find_map(|f| {
      if let Filter::Blur(radius) = f {
        Some(*radius)
      } else {
        None
      }
    })
  }

  /// Returns all drop shadow filters
  pub(crate) fn get_drop_shadows(&self) -> impl Iterator<Item = &TextShadow> {
    self.0.iter().filter_map(|f| {
      if let Filter::DropShadow(shadow) = f {
        Some(shadow)
      } else {
        None
      }
    })
  }
}

impl<'i> FromCss<'i> for Filters {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut filters = SmallVec::new();

    while !input.is_exhausted() {
      let filter = Filter::from_css(input)?;
      filters.push(filter);
    }

    Ok(Filters(filters))
  }
}

impl<'i> FromCss<'i> for Filter {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let token = parser.next()?;

    let Token::Function(function) = token else {
      return Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      );
    };

    match_ignore_ascii_case! {function,
      "brightness" => parser.parse_nested_block(|input| {
        Ok(Filter::Brightness(PercentageNumber::from_css(input)?))
      }),
      "opacity" => parser.parse_nested_block(|input| {
        Ok(Filter::Opacity(PercentageNumber::from_css(input)?))
      }),
      "contrast" => parser.parse_nested_block(|input| {
        Ok(Filter::Contrast(PercentageNumber::from_css(input)?))
      }),
      "grayscale" => parser.parse_nested_block(|input| {
        Ok(Filter::Grayscale(PercentageNumber::from_css(input)?))
      }),
      "hue-rotate" => parser.parse_nested_block(|input| {
        Ok(Filter::HueRotate(Angle::from_css(input)?))
      }),
      "invert" => parser.parse_nested_block(|input| {
        Ok(Filter::Invert(PercentageNumber::from_css(input)?))
      }),
      "saturate" => parser.parse_nested_block(|input| {
        Ok(Filter::Saturate(PercentageNumber::from_css(input)?))
      }),
      "blur" => parser.parse_nested_block(|input| {
        // blur() can have an optional radius, defaults to 0
        let radius = input
          .try_parse(LengthUnit::from_css)
          .unwrap_or(LengthUnit::zero());
        Ok(Filter::Blur(radius))
      }),
      "drop-shadow" => parser.parse_nested_block(|input| {
        // drop-shadow uses the same syntax as text-shadow
        Ok(Filter::DropShadow(TextShadow::from_css(input)?))
      }),
      _ => Err(location.new_basic_unexpected_token_error(Token::Function(function.clone())).into()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{Color, ColorInput, LengthUnit::Px};

  #[test]
  fn test_parse_blur_filter() {
    assert_eq!(Filter::from_str("blur(5px)"), Ok(Filter::Blur(Px(5.0))));
  }

  #[test]
  fn test_parse_blur_filter_zero() {
    assert_eq!(
      Filter::from_str("blur()"),
      Ok(Filter::Blur(LengthUnit::zero()))
    );
  }

  #[test]
  fn test_parse_drop_shadow_filter() {
    assert_eq!(
      Filter::from_str("drop-shadow(2px 4px 6px red)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Px(6.0),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      }))
    );
  }

  #[test]
  fn test_parse_drop_shadow_color_first() {
    assert_eq!(
      Filter::from_str("drop-shadow(red 2px 4px)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: LengthUnit::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      }))
    );
  }

  #[test]
  fn test_parse_drop_shadow_no_blur() {
    assert_eq!(
      Filter::from_str("drop-shadow(2px 4px)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: LengthUnit::zero(),
        color: ColorInput::CurrentColor,
      }))
    );
  }

  #[test]
  fn test_filters_requires_node_level_rendering() {
    assert!(
      Filters::from_str("blur(5px)").is_ok_and(|filters| filters.requires_node_level_rendering())
    );

    assert!(
      Filters::from_str("grayscale(50%)")
        .is_ok_and(|filters| !filters.requires_node_level_rendering())
    );

    assert!(
      Filters::from_str("drop-shadow(2px 4px)")
        .is_ok_and(|filters| filters.requires_node_level_rendering())
    );
  }
}

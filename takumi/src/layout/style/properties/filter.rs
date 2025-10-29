use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::{
  Pixel, RgbaImage,
  imageops::colorops::{contrast_in_place, huerotate_in_place},
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use taffy::Point;
use ts_rs::TS;

use crate::{
  layout::style::{Affine, Angle, FromCss, LengthUnit, ParseResult, PercentageNumber},
  rendering::{BorderProperties, RenderContext, apply_fast_blur, overlay_image},
};

/// Represents a single CSS filter operation
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "kebab-case")]
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
  /// Blurs the image.
  Blur(LengthUnit),
}

/// A list of filters
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TS)]
#[serde(untagged)]
pub(crate) enum FiltersValue {
  /// Structured set of filters
  #[ts(as = "Vec<Filter>")]
  Structured(SmallVec<[Filter; 4]>),
  /// Raw CSS string to be parsed
  Css(String),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TS, Default)]
#[ts(as = "FiltersValue")]
#[serde(try_from = "FiltersValue")]
/// A list of filter operations
pub struct Filters(pub SmallVec<[Filter; 4]>);

impl Filters {
  pub(crate) fn apply_to(&self, image: &mut RgbaImage, context: &RenderContext) {
    let mut radius = 0.0;

    for filter in self.0.iter() {
      match *filter {
        Filter::Blur(length) => {
          radius += length.resolve_to_px(context, image.width() as f32).max(0.0);
        }
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
      }
    }

    let mut extended_image = RgbaImage::new(
      image.width() + (radius * 2.0) as u32,
      image.height() + (radius * 2.0) as u32,
    );

    overlay_image(
      &mut extended_image,
      image,
      Point {
        x: radius as i32,
        y: radius as i32,
      },
      BorderProperties::default(),
      Affine::identity(),
      context.style.image_rendering,
    );

    apply_fast_blur(&mut extended_image, radius);

    *image = extended_image;
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

impl TryFrom<FiltersValue> for Filters {
  type Error = String;

  fn try_from(value: FiltersValue) -> Result<Self, Self::Error> {
    match value {
      FiltersValue::Structured(filters) => Ok(Filters(filters)),
      FiltersValue::Css(css) => Filters::from_str(&css).map_err(|e| e.to_string()),
    }
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
        Ok(Filter::Blur(LengthUnit::from_css(input)?))
      }),
      _ => Err(location.new_basic_unexpected_token_error(Token::Function(function.clone())).into()),
    }
  }
}

use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::{Pixel, Rgba, RgbaImage, imageops::colorops::huerotate_in_place};
use smallvec::SmallVec;
use taffy::Size;

use crate::{
  layout::style::{Angle, Color, FromCss, LengthUnit, ParseResult, PercentageNumber, TextShadow},
  rendering::{SizedShadow, Sizing, apply_fast_blur, blend_pixel},
};

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

/// A list of filter operations
pub type Filters = SmallVec<[Filter; 2]>;

/// Categorizes filters for batch processing
enum FilterCategory<'f> {
  /// Pixel filters that can potentially be batched
  Pixel(&'f Filter),
  /// Complex filters that need special handling (blur, drop-shadow, hue-rotate)
  Complex(&'f Filter),
}

impl Filter {
  fn categorize(&self) -> FilterCategory<'_> {
    match self {
      Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {
        FilterCategory::Complex(self)
      }
      _ => FilterCategory::Pixel(self),
    }
  }
}

/// Applies a single pixel filter inline - used for single filter optimization
#[inline(always)]
fn apply_single_pixel_filter(pixel: &mut Rgba<u8>, filter: &Filter) {
  match *filter {
    Filter::Brightness(PercentageNumber(value)) => {
      for channel in pixel.0.iter_mut().take(3) {
        *channel = ((*channel) as f32 * value).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Contrast(PercentageNumber(value)) => {
      for channel in pixel.0.iter_mut().take(3) {
        *channel = ((*channel as f32 - 128.0) * value + 128.0).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Grayscale(PercentageNumber(amount)) => {
      let lum = pixel.to_luma().0[0] as f32;
      for channel in pixel.0.iter_mut().take(3) {
        *channel = ((*channel as f32 * (1.0 - amount)) + (lum * amount)).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Saturate(PercentageNumber(value)) => {
      let lum = pixel.to_luma().0[0] as f32;
      for channel in pixel.0.iter_mut().take(3) {
        *channel = (lum * (1.0 - value) + *channel as f32 * value).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Invert(PercentageNumber(amount)) => {
      for channel in pixel.0.iter_mut().take(3) {
        let inverted = u8::MAX.saturating_sub(*channel);
        *channel =
          ((*channel as f32 * (1.0 - amount)) + (inverted as f32 * amount)).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Opacity(PercentageNumber(value)) => {
      pixel.0[3] = ((pixel.0[3]) as f32 * value).clamp(0.0, 255.0) as u8;
    }
    // Complex filters are not handled here
    Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {}
  }
}

/// Applies batched pixel filters in a single pass over the image
fn apply_batched_pixel_filters(image: &mut RgbaImage, filters: &[&Filter]) {
  if filters.is_empty() {
    return;
  }

  // Single filter fast path
  if filters.len() == 1 {
    let filter = filters[0];
    for pixel in image.pixels_mut() {
      if pixel.0[3] == 0 {
        continue;
      }
      apply_single_pixel_filter(pixel, filter);
    }
    return;
  }

  // Multiple filters: apply all in one pass
  for pixel in image.pixels_mut() {
    if pixel.0[3] == 0 {
      continue;
    }
    for &filter in filters {
      apply_single_pixel_filter(pixel, filter);
    }
  }
}

pub(crate) fn apply_filters<'f, F: Iterator<Item = &'f Filter>>(
  image: &mut RgbaImage,
  sizing: &Sizing,
  current_color: Color,
  opacity: u8,
  filters: F,
) {
  // Collect filters and batch consecutive pixel filters
  let mut pending_pixel_filters: SmallVec<[&Filter; 8]> = SmallVec::new();

  for filter in filters {
    match filter.categorize() {
      FilterCategory::Pixel(f) => {
        // Accumulate pixel filters for batch processing
        pending_pixel_filters.push(f);
      }
      FilterCategory::Complex(f) => {
        // Flush any pending pixel filters first
        if !pending_pixel_filters.is_empty() {
          apply_batched_pixel_filters(image, &pending_pixel_filters);
          pending_pixel_filters.clear();
        }

        // Apply complex filter
        match *f {
          Filter::HueRotate(angle) => {
            huerotate_in_place(image, *angle as i32);
          }
          Filter::Blur(blur) => {
            apply_fast_blur(image, blur.to_px(sizing, 1.0));
          }
          Filter::DropShadow(drop_shadow) => {
            let size = Size {
              width: image.width() as f32,
              height: image.height() as f32,
            };
            let shadow =
              SizedShadow::from_text_shadow(drop_shadow, sizing, current_color, opacity, size);
            apply_drop_shadow_filter(image, &shadow);
          }
          _ => unreachable!(),
        }
      }
    }
  }

  // Flush remaining pixel filters
  if !pending_pixel_filters.is_empty() {
    apply_batched_pixel_filters(image, &pending_pixel_filters);
  }
}

/// Applies a drop-shadow filter effect to an image.
/// This renders the shadow based on the source image's alpha channel.
///
/// The drop-shadow filter creates a shadow that follows the shape of the source
/// image's alpha channel. The process is:
/// 1. Create a shadow image large enough to contain the original + blur + offset
/// 2. Copy the source alpha channel, filling with shadow color
/// 3. Apply blur to the shadow
/// 4. Composite: draw shadow at offset, then draw original on top
fn apply_drop_shadow_filter(canvas: &mut RgbaImage, shadow: &SizedShadow) {
  let canvas_width = canvas.width();
  let canvas_height = canvas.height();

  if canvas_width == 0 || canvas_height == 0 {
    return;
  }

  // Calculate the padding needed for blur
  let blur_padding = (shadow.blur_radius.ceil() as i32).max(0);

  // Calculate the offset as integers
  let offset_x = shadow.offset_x as i32;
  let offset_y = shadow.offset_y as i32;

  // Calculate the required size for the composited result
  // We need space for: shadow (original + blur padding + offset) and original
  let min_x = (-blur_padding + offset_x).min(0);
  let min_y = (-blur_padding + offset_y).min(0);
  let max_x = (canvas_width as i32 + blur_padding + offset_x).max(canvas_width as i32);
  let max_y = (canvas_height as i32 + blur_padding + offset_y).max(canvas_height as i32);

  let result_width = (max_x - min_x) as u32;
  let result_height = (max_y - min_y) as u32;

  // The origin offset for placing content in the result image
  let origin_x = -min_x;
  let origin_y = -min_y;

  // Create shadow image with enough space for blur
  let shadow_width = canvas_width + (blur_padding as u32 * 2);
  let shadow_height = canvas_height + (blur_padding as u32 * 2);
  let mut shadow_image = RgbaImage::new(shadow_width, shadow_height);

  // Copy the source alpha channel and fill with shadow color
  let shadow_color: Rgba<u8> = shadow.color.into();
  for y in 0..canvas_height {
    for x in 0..canvas_width {
      let src_pixel = canvas.get_pixel(x, y);
      let alpha = src_pixel.0[3];
      if alpha > 0 {
        // Place at center of shadow image (offset by blur_padding)
        let dest_x = x + blur_padding as u32;
        let dest_y = y + blur_padding as u32;
        shadow_image.put_pixel(
          dest_x,
          dest_y,
          Rgba([
            shadow_color.0[0],
            shadow_color.0[1],
            shadow_color.0[2],
            // Blend shadow alpha with source alpha
            ((shadow_color.0[3] as u32 * alpha as u32) / 255) as u8,
          ]),
        );
      }
    }
  }

  // Apply blur to the shadow
  apply_fast_blur(&mut shadow_image, shadow.blur_radius);

  // Create the result image
  let mut result = RgbaImage::new(result_width, result_height);

  // Draw the shadow at its offset position
  let shadow_dest_x = origin_x + offset_x - blur_padding;
  let shadow_dest_y = origin_y + offset_y - blur_padding;
  for y in 0..shadow_height {
    for x in 0..shadow_width {
      let dest_x = shadow_dest_x + x as i32;
      let dest_y = shadow_dest_y + y as i32;
      if dest_x >= 0 && dest_x < result_width as i32 && dest_y >= 0 && dest_y < result_height as i32
      {
        let shadow_pixel = shadow_image.get_pixel(x, y);
        if shadow_pixel.0[3] > 0 {
          blend_pixel(
            result.get_pixel_mut(dest_x as u32, dest_y as u32),
            *shadow_pixel,
          );
        }
      }
    }
  }

  // Draw the original image on top
  for y in 0..canvas_height {
    for x in 0..canvas_width {
      let dest_x = (origin_x + x as i32) as u32;
      let dest_y = (origin_y + y as i32) as u32;
      let src_pixel = *canvas.get_pixel(x, y);
      if src_pixel.0[3] > 0 {
        blend_pixel(result.get_pixel_mut(dest_x, dest_y), src_pixel);
      }
    }
  }

  // Copy the result back to the canvas area
  // The canvas should remain the same size, so we crop/extend as needed
  for y in 0..canvas_height {
    for x in 0..canvas_width {
      let src_x = (origin_x + x as i32) as u32;
      let src_y = (origin_y + y as i32) as u32;
      if src_x < result_width && src_y < result_height {
        *canvas.get_pixel_mut(x, y) = *result.get_pixel(src_x, src_y);
      }
    }
  }
}

impl<'i> FromCss<'i> for Filters {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut filters = SmallVec::new();

    while !input.is_exhausted() {
      let filter = Filter::from_css(input)?;
      filters.push(filter);
    }

    Ok(filters)
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
}

use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::{
  Pixel, Rgba, RgbaImage,
  imageops::{colorops::huerotate_in_place, crop_imm},
};
use smallvec::SmallVec;
use taffy::{Point, Size};

use crate::{
  layout::style::{
    Affine, Angle, Color, CssToken, FromCss, Length, ParseResult, PercentageNumber, TextShadow,
    tw::TailwindPropertyParser,
  },
  rendering::{
    BlurType, BorderProperties, Canvas, RenderContext, SizedShadow, Sizing, apply_blur,
    blend_pixel, fast_div_255,
  },
};

/// Lookup table for a single 8-bit channel transition.
pub(crate) type TransferTable = [u8; 256];

/// Builds a LUT for the Brightness filter.
pub(crate) fn build_brightness_table(value: f32) -> TransferTable {
  let mut table = [0u8; 256];
  for (i, entry) in table.iter_mut().enumerate() {
    *entry = (i as f32 * value).clamp(0.0, 255.0) as u8;
  }
  table
}

/// Builds a LUT for the Contrast filter.
pub(crate) fn build_contrast_table(value: f32) -> TransferTable {
  let mut table = [0u8; 256];
  for (i, entry) in table.iter_mut().enumerate() {
    *entry = ((i as f32 - 128.0) * value + 128.0).clamp(0.0, 255.0) as u8;
  }
  table
}

/// Builds a LUT for the Invert filter.
pub(crate) fn build_invert_table(amount: f32) -> TransferTable {
  let mut table = [0u8; 256];
  for (i, entry) in table.iter_mut().enumerate() {
    let inverted = 255 - i as u8;
    *entry = ((i as f32 * (1.0 - amount)) + (inverted as f32 * amount)).clamp(0.0, 255.0) as u8;
  }
  table
}

/// Builds a LUT for the Opacity filter (applied to alpha channel).
pub(crate) fn build_opacity_table(value: f32) -> TransferTable {
  let mut table = [0u8; 256];
  for (i, entry) in table.iter_mut().enumerate() {
    *entry = (i as f32 * value).clamp(0.0, 255.0) as u8;
  }
  table
}

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
  /// Sepia amount (0..1). Accepts number or percentage
  Sepia(PercentageNumber),
  /// Opacity amount (0..1). Accepts number or percentage
  Opacity(PercentageNumber),
  /// Blur radius in pixels
  Blur(Length),
  /// Drop shadow effect with offset, blur, and color (reuses TextShadow parsing)
  DropShadow(TextShadow),
}

/// A list of filter operations
pub type Filters = Vec<Filter>;

impl TailwindPropertyParser for Filters {
  fn parse_tw(_token: &str) -> Option<Self> {
    None
  }
}

impl Filter {
  pub(crate) fn categorize(&self) -> FilterCategory<'_> {
    match self {
      Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {
        FilterCategory::Complex(self)
      }
      _ => FilterCategory::Pixel(self),
    }
  }

  /// Returns a LUT if this filter is a simple 1D channel transfer.
  /// Returns (RGB_LUT, Alpha_LUT).
  pub(crate) fn transfer_tables(&self) -> (Option<TransferTable>, Option<TransferTable>) {
    match *self {
      Filter::Brightness(PercentageNumber(v)) => (Some(build_brightness_table(v)), None),
      Filter::Contrast(PercentageNumber(v)) => (Some(build_contrast_table(v)), None),
      Filter::Invert(PercentageNumber(v)) => (Some(build_invert_table(v)), None),
      Filter::Opacity(PercentageNumber(v)) => (None, Some(build_opacity_table(v))),
      _ => (None, None),
    }
  }
}

/// Category of filters for optimization purposes.
pub(crate) enum FilterCategory<'f> {
  /// Pixel filters that can potentially be batched
  Pixel(&'f Filter),
  /// Complex filters that need special handling (blur, drop-shadow, hue-rotate)
  Complex(&'f Filter),
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
    Filter::Sepia(PercentageNumber(amount)) => {
      // Sepia tone matrix coefficients
      let r = pixel.0[0] as f32;
      let g = pixel.0[1] as f32;
      let b = pixel.0[2] as f32;

      let sepia_r = (r * 0.393 + g * 0.769 + b * 0.189).clamp(0.0, 255.0);
      let sepia_g = (r * 0.349 + g * 0.686 + b * 0.168).clamp(0.0, 255.0);
      let sepia_b = (r * 0.272 + g * 0.534 + b * 0.131).clamp(0.0, 255.0);

      pixel.0[0] = (r * (1.0 - amount) + sepia_r * amount).clamp(0.0, 255.0) as u8;
      pixel.0[1] = (g * (1.0 - amount) + sepia_g * amount).clamp(0.0, 255.0) as u8;
      pixel.0[2] = (b * (1.0 - amount) + sepia_b * amount).clamp(0.0, 255.0) as u8;
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

  // Pre-calculate LUTs for each filter once
  let luts: SmallVec<[(Option<TransferTable>, Option<TransferTable>); 4]> =
    filters.iter().map(|f| f.transfer_tables()).collect();

  for pixel in image.pixels_mut() {
    if pixel.0[3] == 0 {
      continue;
    }

    for (i, &filter) in filters.iter().enumerate() {
      let (rgb_lut, alpha_lut) = &luts[i];

      if rgb_lut.is_none() && alpha_lut.is_none() {
        // Fallback for matrix filters (grayscale, sepia, etc.)
        apply_single_pixel_filter(pixel, filter);
      } else {
        if let Some(t) = rgb_lut {
          pixel.0[0] = t[pixel.0[0] as usize];
          pixel.0[1] = t[pixel.0[1] as usize];
          pixel.0[2] = t[pixel.0[2] as usize];
        }
        if let Some(t) = alpha_lut {
          pixel.0[3] = t[pixel.0[3] as usize];
        }
      }
    }
  }
}

pub(crate) fn apply_filters<'f, F: Iterator<Item = &'f Filter>>(
  image: &mut RgbaImage,
  sizing: &Sizing,
  current_color: Color,
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
            apply_blur(image, blur.to_px(sizing, 1.0), BlurType::Filter);
          }
          Filter::DropShadow(drop_shadow) => {
            let size = Size {
              width: image.width() as f32,
              height: image.height() as f32,
            };
            let shadow = SizedShadow::from_text_shadow(drop_shadow, sizing, current_color, size);
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

/// Applies backdrop-filter effects to the area behind an element.
///
/// This extracts the region of the canvas that will be covered by the element,
/// applies the specified filters to it, and composites it back to the canvas.
pub(crate) fn apply_backdrop_filter(
  canvas: &mut Canvas,
  border: BorderProperties,
  layout_size: Size<f32>,
  transform: Affine,
  context: &RenderContext,
) {
  let filters = &context.style.backdrop_filter;

  let drop_shadow_filtered = filters
    .iter()
    .filter(|f| !matches!(f, Filter::DropShadow(_)));

  if drop_shadow_filtered.clone().count() == 0 {
    return;
  }

  let canvas_size = canvas.size();
  if canvas_size.width == 0 || canvas_size.height == 0 {
    return;
  }

  // Generate the mask for the element's shape (with border-radius)
  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, layout_size, Point::ZERO);

  let (mask, placement) = canvas.mask_memory.render(&paths, Some(transform), None);

  if placement.width == 0 || placement.height == 0 {
    return;
  }

  // Calculate the region to extract (clamped to canvas bounds)
  let region_x = (placement.left).clamp(0, canvas_size.width as i32) as u32;
  let region_y = (placement.top).clamp(0, canvas_size.height as i32) as u32;
  let region_right =
    (placement.left + placement.width as i32).clamp(0, canvas_size.width as i32) as u32;
  let region_bottom =
    (placement.top + placement.height as i32).clamp(0, canvas_size.height as i32) as u32;

  if region_x >= region_right || region_y >= region_bottom {
    return;
  }

  let region_width = region_right - region_x;
  let region_height = region_bottom - region_y;

  // Extract the region from the canvas using crop_imm
  let mut backdrop_image = crop_imm(
    &canvas.image,
    region_x,
    region_y,
    region_width,
    region_height,
  )
  .to_image();

  apply_filters(
    &mut backdrop_image,
    &context.sizing,
    context.current_color,
    drop_shadow_filtered,
  );

  // Composite the filtered backdrop back to the canvas, respecting the mask
  let mask_offset_x = (region_x as i32 - placement.left) as u32;
  let mask_offset_y = (region_y as i32 - placement.top) as u32;

  let canvas_width = canvas.image.width();
  let canvas_raw = canvas.image.as_mut();
  let backdrop_raw = backdrop_image.as_raw();

  for y in 0..region_height {
    let mask_y = mask_offset_y + y;
    if mask_y >= placement.height {
      continue;
    }

    let canvas_y = region_y + y;
    let canvas_y_offset = (canvas_y * canvas_width + region_x) as usize * 4;
    let backdrop_y_offset = (y * region_width) as usize * 4;
    let mask_y_offset = (mask_y * placement.width + mask_offset_x) as usize;

    for x in 0..region_width {
      let mask_idx = mask_y_offset + x as usize;
      let alpha = mask[mask_idx];

      if alpha == 0 {
        continue;
      }

      let c_idx = canvas_y_offset + x as usize * 4;
      let b_idx = backdrop_y_offset + x as usize * 4;

      if alpha == 255 {
        canvas_raw[c_idx] = backdrop_raw[b_idx];
        canvas_raw[c_idx + 1] = backdrop_raw[b_idx + 1];
        canvas_raw[c_idx + 2] = backdrop_raw[b_idx + 2];
        canvas_raw[c_idx + 3] = backdrop_raw[b_idx + 3];
      } else {
        let src_a = alpha as u16;
        let inv_a = 255 - src_a;

        canvas_raw[c_idx] =
          fast_div_255(backdrop_raw[b_idx] as u16 * src_a + canvas_raw[c_idx] as u16 * inv_a);
        canvas_raw[c_idx + 1] = fast_div_255(
          backdrop_raw[b_idx + 1] as u16 * src_a + canvas_raw[c_idx + 1] as u16 * inv_a,
        );
        canvas_raw[c_idx + 2] = fast_div_255(
          backdrop_raw[b_idx + 2] as u16 * src_a + canvas_raw[c_idx + 2] as u16 * inv_a,
        );
      }
    }
  }
}

/// Applies a drop-shadow filter effect to an image.
fn apply_drop_shadow_filter(canvas: &mut RgbaImage, shadow: &SizedShadow) {
  let (canvas_width, canvas_height) = canvas.dimensions();
  if canvas_width == 0 || canvas_height == 0 {
    return;
  }

  let blur_radius = shadow.blur_radius;
  let padding = blur_radius.ceil() as u32;

  let shadow_width = canvas_width + 2 * padding;
  let shadow_height = canvas_height + 2 * padding;
  let mut shadow_image = RgbaImage::new(shadow_width, shadow_height);

  let offset_x = shadow.offset_x.round() as i32;
  let offset_y = shadow.offset_y.round() as i32;

  // Populate shadow image with source alpha and shadow color
  let shadow_color: Rgba<u8> = shadow.color.into();
  let [sr, sg, sb, sa] = shadow_color.0;

  let canvas_raw = canvas.as_raw();
  let shadow_raw = shadow_image.as_mut();

  for y in 0..canvas_height {
    let src_y_idx = (y * canvas_width) as usize * 4;
    let dest_y = y as i32 + offset_y + padding as i32;

    if dest_y < 0 || dest_y >= shadow_height as i32 {
      continue;
    }

    let dest_y_offset = (dest_y as usize * shadow_width as usize) * 4;

    for x in 0..canvas_width {
      let alpha = canvas_raw[src_y_idx + x as usize * 4 + 3];
      if alpha == 0 {
        continue;
      }

      let dest_x = x as i32 + offset_x + padding as i32;
      if dest_x >= 0 && dest_x < shadow_width as i32 {
        let d_idx = dest_y_offset + dest_x as usize * 4;
        shadow_raw[d_idx] = sr;
        shadow_raw[d_idx + 1] = sg;
        shadow_raw[d_idx + 2] = sb;
        shadow_raw[d_idx + 3] = fast_div_255(sa as u16 * alpha as u16);
      }
    }
  }

  // Apply blur to the shadow image
  apply_blur(&mut shadow_image, blur_radius, BlurType::Shadow);

  // Draw source element OVER the blurred shadow
  // Since we already copied the source alpha to shadow_image, we can blend in-place
  for (x, y, canvas_pixel) in canvas.enumerate_pixels_mut() {
    let mut final_px = *shadow_image.get_pixel(x + padding, y + padding);
    blend_pixel(&mut final_px, *canvas_pixel);
    *canvas_pixel = final_px;
  }
}

impl<'i> FromCss<'i> for Filters {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut filters = Vec::new();

    while !input.is_exhausted() {
      let filter = Filter::from_css(input)?;
      filters.push(filter);
    }

    Ok(filters)
  }

  fn valid_tokens() -> &'static [CssToken] {
    Filter::valid_tokens()
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
      "sepia" => parser.parse_nested_block(|input| {
        Ok(Filter::Sepia(PercentageNumber::from_css(input)?))
      }),
      "blur" => parser.parse_nested_block(|input| {
        // blur() can have an optional radius, defaults to 0
        let radius = input
          .try_parse(Length::from_css)
          .unwrap_or(Length::zero());
        Ok(Filter::Blur(radius))
      }),
      "drop-shadow" => parser.parse_nested_block(|input| {
        // drop-shadow uses the same syntax as text-shadow
        Ok(Filter::DropShadow(TextShadow::from_css(input)?))
      }),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("brightness()"),
      CssToken::Token("opacity()"),
      CssToken::Token("contrast()"),
      CssToken::Token("grayscale()"),
      CssToken::Token("hue-rotate()"),
      CssToken::Token("invert()"),
      CssToken::Token("saturate()"),
      CssToken::Token("sepia()"),
      CssToken::Token("blur()"),
      CssToken::Token("drop-shadow()"),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{Color, ColorInput, Length::Px};

  #[test]
  fn test_parse_blur_filter() {
    assert_eq!(Filter::from_str("blur(5px)"), Ok(Filter::Blur(Px(5.0))));
  }

  #[test]
  fn test_parse_blur_filter_zero() {
    assert_eq!(Filter::from_str("blur()"), Ok(Filter::Blur(Length::zero())));
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
        blur_radius: Length::zero(),
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
        blur_radius: Length::zero(),
        color: ColorInput::CurrentColor,
      }))
    );
  }

  #[test]
  fn test_apply_filters_lut_batching() {
    let mut image = RgbaImage::new(1, 1);
    image.put_pixel(0, 0, Rgba([100, 150, 200, 255]));

    let filters = [
      Filter::Brightness(PercentageNumber(1.2)), // 100 * 1.2 = 120, 150 * 1.2 = 180, 200 * 1.2 = 240
      Filter::Invert(PercentageNumber(1.0)),     // 120 -> 135, 180 -> 75, 240 -> 15
      Filter::Opacity(PercentageNumber(0.5)),    // 255 * 0.5 = 127
    ];

    let viewport = crate::layout::Viewport::new(Some(100), Some(100));
    let sizing = Sizing {
      viewport,
      font_size: 16.0,
    };
    apply_filters(&mut image, &sizing, Color::black(), filters.iter());

    let pixel = image.get_pixel(0, 0);
    // Rough verification of the math
    assert_eq!(pixel.0[0], 135);
    assert_eq!(pixel.0[1], 75);
    assert_eq!(pixel.0[2], 15);
    assert_eq!(pixel.0[3], 127);
  }
}

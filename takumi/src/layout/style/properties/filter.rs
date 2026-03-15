use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::{Rgba, RgbaImage, imageops::colorops::huerotate_in_place};
use smallvec::SmallVec;
use taffy::{Point, Size};

use crate::{
  Result,
  layout::style::{
    Affine, Angle, Animatable, BlendMode, Color, CssDescriptorKind, CssToken, FromCss, Length,
    ListInterpolationStrategy, MakeComputed, ParseResult, PercentageNumber, TextShadow,
    tw::TailwindPropertyParser,
  },
  rendering::{
    BlurFormat, BlurType, BorderProperties, BufferPool, Canvas, RenderContext, SizedShadow, Sizing,
    apply_blur, blend_pixel, fast_div_255,
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
#[non_exhaustive]
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

impl MakeComputed for Filter {
  fn make_computed(&mut self, sizing: &Sizing) {
    match self {
      Filter::Blur(length) => length.make_computed(sizing),
      Filter::DropShadow(shadow) => shadow.make_computed(sizing),
      _ => {}
    }
  }
}

impl Animatable for Filter {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::PadToLongestWithNeutral
  }

  fn neutral_value_like(other: &Self) -> Option<Self> {
    Some(match *other {
      Filter::Brightness(_) => Filter::Brightness(PercentageNumber(1.0)),
      Filter::Contrast(_) => Filter::Contrast(PercentageNumber(1.0)),
      Filter::Grayscale(_) => Filter::Grayscale(PercentageNumber(0.0)),
      Filter::Saturate(_) => Filter::Saturate(PercentageNumber(1.0)),
      Filter::HueRotate(_) => Filter::HueRotate(Angle::zero()),
      Filter::Invert(_) => Filter::Invert(PercentageNumber(0.0)),
      Filter::Sepia(_) => Filter::Sepia(PercentageNumber(0.0)),
      Filter::Opacity(_) => Filter::Opacity(PercentageNumber(1.0)),
      Filter::Blur(_) => Filter::Blur(Length::zero()),
      Filter::DropShadow(_) => Filter::DropShadow(TextShadow {
        offset_x: Length::zero(),
        offset_y: Length::zero(),
        blur_radius: Length::zero(),
        color: Color::transparent().into(),
      }),
    })
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
      (Filter::Brightness(from), Filter::Brightness(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Brightness(value)
      }
      (Filter::Contrast(from), Filter::Contrast(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Contrast(value)
      }
      (Filter::Grayscale(from), Filter::Grayscale(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Grayscale(value)
      }
      (Filter::Saturate(from), Filter::Saturate(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Saturate(value)
      }
      (Filter::HueRotate(from), Filter::HueRotate(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::HueRotate(value)
      }
      (Filter::Invert(from), Filter::Invert(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Invert(value)
      }
      (Filter::Sepia(from), Filter::Sepia(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Sepia(value)
      }
      (Filter::Opacity(from), Filter::Opacity(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Opacity(value)
      }
      (Filter::Blur(from), Filter::Blur(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::Blur(value)
      }
      (Filter::DropShadow(from), Filter::DropShadow(to)) => {
        let mut value = from;
        value.interpolate(&from, &to, progress, sizing, current_color);
        Filter::DropShadow(value)
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

/// Calculates the luma of an RGB pixel.
#[inline(always)]
fn get_luma(pixel: &[u8]) -> f32 {
  pixel[0] as f32 * 0.2126 + pixel[1] as f32 * 0.7152 + pixel[2] as f32 * 0.0722
}

/// Applies a single pixel filter inline - used for single filter optimization
#[inline(always)]
fn apply_single_pixel_filter(pixel: &mut [u8], filter: &Filter) {
  match *filter {
    Filter::Brightness(PercentageNumber(value)) => {
      for channel in pixel.iter_mut().take(3) {
        *channel = ((*channel) as f32 * value).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Contrast(PercentageNumber(value)) => {
      for channel in pixel.iter_mut().take(3) {
        *channel = ((*channel as f32 - 128.0) * value + 128.0).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Grayscale(PercentageNumber(amount)) => {
      let lum = get_luma(pixel);
      for channel in pixel.iter_mut().take(3) {
        *channel = ((*channel as f32 * (1.0 - amount)) + (lum * amount)).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Saturate(PercentageNumber(value)) => {
      let lum = get_luma(pixel);
      for channel in pixel.iter_mut().take(3) {
        *channel = (lum * (1.0 - value) + *channel as f32 * value).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Invert(PercentageNumber(amount)) => {
      for channel in pixel.iter_mut().take(3) {
        let inverted = u8::MAX.saturating_sub(*channel);
        *channel =
          ((*channel as f32 * (1.0 - amount)) + (inverted as f32 * amount)).clamp(0.0, 255.0) as u8;
      }
    }
    Filter::Sepia(PercentageNumber(amount)) => {
      // Sepia tone matrix coefficients
      let r = pixel[0] as f32;
      let g = pixel[1] as f32;
      let b = pixel[2] as f32;

      let sepia_r = (r * 0.393 + g * 0.769 + b * 0.189).clamp(0.0, 255.0);
      let sepia_g = (r * 0.349 + g * 0.686 + b * 0.168).clamp(0.0, 255.0);
      let sepia_b = (r * 0.272 + g * 0.534 + b * 0.131).clamp(0.0, 255.0);

      pixel[0] = (r * (1.0 - amount) + sepia_r * amount).clamp(0.0, 255.0) as u8;
      pixel[1] = (g * (1.0 - amount) + sepia_g * amount).clamp(0.0, 255.0) as u8;
      pixel[2] = (b * (1.0 - amount) + sepia_b * amount).clamp(0.0, 255.0) as u8;
    }
    Filter::Opacity(PercentageNumber(value)) => {
      pixel[3] = ((pixel[3]) as f32 * value).clamp(0.0, 255.0) as u8;
    }
    // Complex filters are not handled here
    Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {}
  }
}

/// Filter prepared for batch execution
enum PreparedFilter<'a> {
  Matrix(&'a Filter),
  RgbLut(Box<TransferTable>),
  AlphaLut(Box<TransferTable>),
  BothLut(Box<TransferTable>, Box<TransferTable>),
}

/// Applies batched pixel filters in a single pass over the image
fn apply_batched_pixel_filters(image: &mut RgbaImage, filters: &[&Filter]) {
  if filters.is_empty() {
    return;
  }

  // Pre-calculate LUTs and categorize filters
  let prepared: SmallVec<[PreparedFilter; 4]> = filters
    .iter()
    .map(|&f| match f.transfer_tables() {
      (Some(rgb), Some(alpha)) => PreparedFilter::BothLut(Box::new(rgb), Box::new(alpha)),
      (Some(rgb), None) => PreparedFilter::RgbLut(Box::new(rgb)),
      (None, Some(alpha)) => PreparedFilter::AlphaLut(Box::new(alpha)),
      (None, None) => PreparedFilter::Matrix(f),
    })
    .collect();

  for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(image.as_mut()) {
    if pixel[3] == 0 {
      continue;
    }

    for p in &prepared {
      match p {
        PreparedFilter::Matrix(f) => apply_single_pixel_filter(pixel, f),
        PreparedFilter::RgbLut(t) => {
          pixel[0] = t[pixel[0] as usize];
          pixel[1] = t[pixel[1] as usize];
          pixel[2] = t[pixel[2] as usize];
        }
        PreparedFilter::AlphaLut(t) => {
          pixel[3] = t[pixel[3] as usize];
        }
        PreparedFilter::BothLut(rgb, alpha) => {
          pixel[0] = rgb[pixel[0] as usize];
          pixel[1] = rgb[pixel[1] as usize];
          pixel[2] = rgb[pixel[2] as usize];
          pixel[3] = alpha[pixel[3] as usize];
        }
      }
    }
  }
}

pub(crate) fn apply_filters<'f, F: Iterator<Item = &'f Filter>>(
  image: &mut RgbaImage,
  sizing: &Sizing,
  current_color: Color,
  buffer_pool: &mut BufferPool,
  filters: F,
) -> Result<()> {
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
            apply_blur(
              BlurFormat::Rgba(image),
              blur.to_px(sizing, 1.0),
              BlurType::Filter,
              buffer_pool,
            )?;
          }
          Filter::DropShadow(drop_shadow) => {
            let size = Size {
              width: image.width() as f32,
              height: image.height() as f32,
            };
            let shadow = SizedShadow::from_text_shadow(drop_shadow, sizing, current_color, size);
            apply_drop_shadow_filter(image, &shadow, buffer_pool)?;
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

  Ok(())
}

/// Applies backdrop-filter effects to the area behind an element.
///
/// This extracts the region of the canvas that will be covered by the element,
/// applies the specified filters to it, and composites it back to the canvas.
#[allow(clippy::needless_range_loop)]
pub(crate) fn apply_backdrop_filter(
  canvas: &mut Canvas,
  border: BorderProperties,
  layout_size: Size<f32>,
  transform: Affine,
  context: &RenderContext,
) -> Result<()> {
  let filters = &context.style.backdrop_filter;

  if filters.iter().all(|f| matches!(f, Filter::DropShadow(_))) {
    return Ok(());
  }

  let drop_shadow_filtered = filters
    .iter()
    .filter(|f| !matches!(f, Filter::DropShadow(_)));

  let canvas_size = canvas.size();
  if canvas_size.width == 0 || canvas_size.height == 0 {
    return Ok(());
  }

  // Generate the mask for the element's shape (with border-radius)
  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, layout_size, Point::ZERO);

  // Render the mask — this borrows mask_memory only for the duration of the
  // composite loop below. apply_filters borrows buffer_pool separately, so
  // there is no conflict and no clone is needed.
  let (mask_data, placement) =
    canvas
      .mask_memory
      .render(&paths, Some(transform), None, &mut canvas.buffer_pool);

  if placement.width == 0 || placement.height == 0 {
    return Ok(());
  }

  // Calculate the region to extract (clamped to canvas bounds)
  let region_x = (placement.left).clamp(0, canvas_size.width as i32) as u32;
  let region_y = (placement.top).clamp(0, canvas_size.height as i32) as u32;
  let region_right =
    (placement.left + placement.width as i32).clamp(0, canvas_size.width as i32) as u32;
  let region_bottom =
    (placement.top + placement.height as i32).clamp(0, canvas_size.height as i32) as u32;

  if region_x >= region_right || region_y >= region_bottom {
    return Ok(());
  }

  let region_width = region_right - region_x;
  let region_height = region_bottom - region_y;

  // Extract the region from the canvas using the pool to avoid allocations
  let mut backdrop_image = canvas
    .buffer_pool
    .acquire_image(region_width, region_height)?;

  {
    let canvas_width = canvas.image.width() as usize;
    let canvas_raw = canvas.image.as_raw();
    let backdrop_raw = backdrop_image.as_mut();

    let row_bytes = region_width as usize * 4;
    for (y, dest_row) in backdrop_raw.chunks_exact_mut(row_bytes).enumerate() {
      let src_y = region_y as usize + y;
      let src_start = (src_y * canvas_width + region_x as usize) * 4;
      dest_row.copy_from_slice(&canvas_raw[src_start..src_start + row_bytes]);
    }
  }

  apply_filters(
    &mut backdrop_image,
    &context.sizing,
    context.current_color,
    &mut canvas.buffer_pool,
    drop_shadow_filtered,
  )?;

  // Composite the filtered backdrop back to the canvas, respecting the mask.
  // mask_memory borrow (mask_data) ends here — canvas.image borrow begins.
  let mask_offset_x = (region_x as i32 - placement.left) as u32;
  let mask_offset_y = (region_y as i32 - placement.top) as u32;

  let canvas_width = canvas.image.width() as usize;
  let canvas_raw = canvas.image.as_mut();
  let backdrop_raw = backdrop_image.as_raw();

  let region_row_bytes = region_width as usize * 4;

  for y in 0..region_height {
    let mask_y = mask_offset_y + y;
    if mask_y >= placement.height {
      break;
    }

    let canvas_y = region_y + y;
    let canvas_start = (canvas_y as usize * canvas_width + region_x as usize) * 4;
    let canvas_row = &mut canvas_raw[canvas_start..canvas_start + region_row_bytes];

    let backdrop_start = (y * region_width) as usize * 4;
    let backdrop_row = &backdrop_raw[backdrop_start..backdrop_start + region_row_bytes];

    let mask_start = (mask_y * placement.width + mask_offset_x) as usize;
    let mask_row = &mask_data[mask_start..mask_start + region_width as usize];

    assert_eq!(canvas_row.len(), region_row_bytes);
    assert_eq!(backdrop_row.len(), region_row_bytes);
    assert_eq!(mask_row.len(), region_width as usize);

    for x in 0..region_width as usize {
      let alpha = mask_row[x];
      if alpha == 0 {
        continue;
      }

      let px_idx = x * 4;
      let b_chunk = &backdrop_row[px_idx..px_idx + 4];
      let c_chunk = &mut canvas_row[px_idx..px_idx + 4];

      if alpha == 255 {
        c_chunk.copy_from_slice(b_chunk);
      } else {
        let src_a = alpha as u32;
        let inv_a = 255 - src_a;

        c_chunk[0] = fast_div_255(b_chunk[0] as u32 * src_a + c_chunk[0] as u32 * inv_a);
        c_chunk[1] = fast_div_255(b_chunk[1] as u32 * src_a + c_chunk[1] as u32 * inv_a);
        c_chunk[2] = fast_div_255(b_chunk[2] as u32 * src_a + c_chunk[2] as u32 * inv_a);
      }
    }
  }

  canvas.buffer_pool.release_image(backdrop_image);

  Ok(())
}

/// Applies a drop-shadow filter effect to an image.
fn apply_drop_shadow_filter(
  canvas: &mut RgbaImage,
  shadow: &SizedShadow,
  buffer_pool: &mut BufferPool,
) -> Result<()> {
  let (canvas_width, canvas_height) = canvas.dimensions();
  if canvas_width == 0 || canvas_height == 0 {
    return Ok(());
  }

  let blur_radius = shadow.blur_radius;
  let padding = blur_radius.ceil() as u32;

  let shadow_width = canvas_width + 2 * padding;
  let shadow_height = canvas_height + 2 * padding;

  let mut shadow_alpha = buffer_pool.acquire_dirty((shadow_width * shadow_height) as usize);

  let offset_x = shadow.offset_x.round() as i32;
  let offset_y = shadow.offset_y.round() as i32;

  // Populate shadow alpha with source alpha
  let shadow_color: Rgba<u8> = shadow.color.into();
  let [sr, sg, sb, sa] = shadow_color.0;

  let canvas_raw = canvas.as_raw();

  for y in 0..shadow_height {
    let dest_y_offset = (y * shadow_width) as usize;
    let src_y = y as i32 - offset_y - padding as i32;

    if src_y >= 0 && src_y < canvas_height as i32 {
      let src_y_idx = src_y as usize * canvas_width as usize * 4;

      for x in 0..shadow_width {
        let src_x = x as i32 - offset_x - padding as i32;

        if src_x >= 0 && src_x < canvas_width as i32 {
          let alpha = canvas_raw[src_y_idx + src_x as usize * 4 + 3];
          if alpha > 0 {
            shadow_alpha[dest_y_offset + x as usize] = fast_div_255(sa as u32 * alpha as u32);
          } else {
            shadow_alpha[dest_y_offset + x as usize] = 0;
          }
        } else {
          shadow_alpha[dest_y_offset + x as usize] = 0;
        }
      }
    } else {
      for x in 0..shadow_width {
        shadow_alpha[dest_y_offset + x as usize] = 0;
      }
    }
  }

  // Apply blur to the shadow alpha
  apply_blur(
    BlurFormat::Alpha {
      data: &mut shadow_alpha,
      width: shadow_width,
      height: shadow_height,
    },
    blur_radius,
    BlurType::Shadow,
    buffer_pool,
  )?;

  // Draw source element OVER the blurred shadow
  for y in 0..canvas_height {
    for x in 0..canvas_width {
      let sa_px = shadow_alpha[((y + padding) * shadow_width + (x + padding)) as usize];
      if sa_px > 0 {
        let canvas_pixel = canvas.get_pixel_mut(x, y);
        let mut final_px = Rgba([sr, sg, sb, sa_px]);
        blend_pixel(&mut final_px, *canvas_pixel, BlendMode::Normal);
        *canvas_pixel = final_px;
      }
    }
  }

  buffer_pool.release(shadow_alpha);
  Ok(())
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

  const VALID_TOKENS: &'static [CssToken] = Filter::VALID_TOKENS;
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

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Descriptor(CssDescriptorKind::BrightnessFn),
    CssToken::Descriptor(CssDescriptorKind::OpacityFn),
    CssToken::Descriptor(CssDescriptorKind::ContrastFn),
    CssToken::Descriptor(CssDescriptorKind::GrayscaleFn),
    CssToken::Descriptor(CssDescriptorKind::HueRotateFn),
    CssToken::Descriptor(CssDescriptorKind::InvertFn),
    CssToken::Descriptor(CssDescriptorKind::SaturateFn),
    CssToken::Descriptor(CssDescriptorKind::SepiaFn),
    CssToken::Descriptor(CssDescriptorKind::BlurFn),
    CssToken::Descriptor(CssDescriptorKind::DropShadowFn),
  ];
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{
    Result,
    layout::{
      Viewport,
      style::{CalcArena, Color, ColorInput, Length::Px},
    },
  };

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
  fn test_apply_filters_lut_batching() -> Result<()> {
    let mut image = RgbaImage::new(1, 1);
    image.put_pixel(0, 0, Rgba([100, 150, 200, 255]));

    let filters = [
      Filter::Brightness(PercentageNumber(1.2)), // 100 * 1.2 = 120, 150 * 1.2 = 180, 200 * 1.2 = 240
      Filter::Invert(PercentageNumber(1.0)),     // 120 -> 135, 180 -> 75, 240 -> 15
      Filter::Opacity(PercentageNumber(0.5)),    // 255 * 0.5 = 127
    ];

    let viewport = Viewport::new(Some(100), Some(100));
    let sizing = Sizing {
      viewport,
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let mut buffer_pool = BufferPool::default();
    apply_filters(
      &mut image,
      &sizing,
      Color::black(),
      &mut buffer_pool,
      filters.iter(),
    )?;

    let pixel = image.get_pixel(0, 0);
    // Rough verification of the math
    assert_eq!(pixel.0[0], 135);
    assert_eq!(pixel.0[1], 75);
    assert_eq!(pixel.0[2], 15);
    assert_eq!(pixel.0[3], 127);

    Ok(())
  }
}

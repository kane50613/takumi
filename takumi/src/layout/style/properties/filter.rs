use std::fmt;

use crate::layout::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::Rgba;
use smallvec::SmallVec;
use taffy::{Point, Size};
use tiny_skia::PixmapMut;

use crate::{
  Result,
  layout::style::{
    Affine, Angle, Animatable, Color, CssDescriptorKind, CssToken, FromCss, Length,
    ListInterpolationStrategy, MakeComputed, ParseResult, PercentageNumber, SizingContext,
    TextShadow, tw::TailwindPropertyParser,
  },
  rendering::{
    BlurFormat, BlurType, BorderProperties, BufferPool, Canvas, Placement, RenderContext,
    SizedShadow, apply_blur, apply_blur_rgba_bytes, fast_div_255, render_mask,
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
  fn make_computed(&mut self, sizing: &SizingContext) {
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
    sizing: &SizingContext,
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
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Filters::default());
    }
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

  /// Returns the 1D channel transfer table for this filter, if any.
  pub(crate) fn transfer_table(&self) -> Option<TransferChannel> {
    match *self {
      Filter::Brightness(PercentageNumber(v)) => {
        Some(TransferChannel::Rgb(build_brightness_table(v)))
      }
      Filter::Contrast(PercentageNumber(v)) => Some(TransferChannel::Rgb(build_contrast_table(v))),
      Filter::Invert(PercentageNumber(v)) => Some(TransferChannel::Rgb(build_invert_table(v))),
      Filter::Opacity(PercentageNumber(v)) => Some(TransferChannel::Alpha(build_opacity_table(v))),
      _ => None,
    }
  }
}

/// A 1D channel transfer for a filter, tagged with which channels it touches.
pub(crate) enum TransferChannel {
  /// Applies to R, G, B (leaves alpha untouched).
  Rgb(TransferTable),
  /// Applies to alpha only.
  Alpha(TransferTable),
}

/// Composes `next` after `existing` so that `existing[i] = next[existing[i]]`,
/// collapsing two LUTs into a single equivalent LUT.
#[inline]
fn compose_transfer_table(existing: &mut TransferTable, next: &TransferTable) {
  for entry in existing.iter_mut() {
    *entry = next[*entry as usize];
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
}

/// Builds an execution plan that fuses consecutive RGB or alpha transfer tables
/// into a single composed LUT, so each LUT-only run costs one lookup per channel
/// regardless of how many filters it represents.
fn prepare_pixel_filters<'a>(filters: &[&'a Filter]) -> SmallVec<[PreparedFilter<'a>; 4]> {
  let mut prepared: SmallVec<[PreparedFilter; 4]> = SmallVec::new();
  let mut pending_rgb: Option<TransferTable> = None;
  let mut pending_alpha: Option<TransferTable> = None;

  for &filter in filters {
    match filter.transfer_table() {
      Some(TransferChannel::Rgb(table)) => match &mut pending_rgb {
        Some(existing) => compose_transfer_table(existing, &table),
        slot @ None => *slot = Some(table),
      },
      Some(TransferChannel::Alpha(table)) => match &mut pending_alpha {
        Some(existing) => compose_transfer_table(existing, &table),
        slot @ None => *slot = Some(table),
      },
      None => {
        if let Some(table) = pending_rgb.take() {
          prepared.push(PreparedFilter::RgbLut(Box::new(table)));
        }
        if let Some(table) = pending_alpha.take() {
          prepared.push(PreparedFilter::AlphaLut(Box::new(table)));
        }
        prepared.push(PreparedFilter::Matrix(filter));
      }
    }
  }

  if let Some(table) = pending_rgb {
    prepared.push(PreparedFilter::RgbLut(Box::new(table)));
  }
  if let Some(table) = pending_alpha {
    prepared.push(PreparedFilter::AlphaLut(Box::new(table)));
  }

  prepared
}

/// Applies batched pixel filters in a single pass over the image
fn apply_batched_pixel_filters(data: &mut [u8], filters: &[&Filter]) {
  if filters.is_empty() {
    return;
  }

  let prepared = prepare_pixel_filters(filters);

  for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(data) {
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
      }
    }
  }
}

fn apply_hue_rotate_rgba_bytes(data: &mut [u8], angle_degrees: i32) {
  let radians = (angle_degrees as f32).to_radians();
  let cos = radians.cos();
  let sin = radians.sin();

  let m00 = 0.213 + cos * 0.787 - sin * 0.213;
  let m01 = 0.715 - cos * 0.715 - sin * 0.715;
  let m02 = 0.072 - cos * 0.072 + sin * 0.928;

  let m10 = 0.213 - cos * 0.213 + sin * 0.143;
  let m11 = 0.715 + cos * 0.285 + sin * 0.140;
  let m12 = 0.072 - cos * 0.072 - sin * 0.283;

  let m20 = 0.213 - cos * 0.213 - sin * 0.787;
  let m21 = 0.715 - cos * 0.715 + sin * 0.715;
  let m22 = 0.072 + cos * 0.928 + sin * 0.072;

  for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(data) {
    if pixel[3] == 0 {
      continue;
    }

    let r = pixel[0] as f32;
    let g = pixel[1] as f32;
    let b = pixel[2] as f32;

    pixel[0] = (r * m00 + g * m01 + b * m02).clamp(0.0, 255.0) as u8;
    pixel[1] = (r * m10 + g * m11 + b * m12).clamp(0.0, 255.0) as u8;
    pixel[2] = (r * m20 + g * m21 + b * m22).clamp(0.0, 255.0) as u8;
  }
}

#[inline(always)]
fn composite_pixel_under(dst: &mut [u8], under_rgb: [u8; 3], under_alpha: u8) {
  if under_alpha == 0 || dst[3] == 255 {
    return;
  }

  if dst[3] == 0 {
    dst[0] = under_rgb[0];
    dst[1] = under_rgb[1];
    dst[2] = under_rgb[2];
    dst[3] = under_alpha;
    return;
  }

  let dst_alpha = dst[3] as u32;
  let under_alpha = under_alpha as u32;
  let result_alpha = dst_alpha + under_alpha - u32::from(fast_div_255(dst_alpha * under_alpha));
  if result_alpha == 0 {
    return;
  }

  let inverse_dst_alpha = 255 - dst_alpha;
  for (channel, src) in dst.iter_mut().take(3).zip(under_rgb) {
    let dst_premul = *channel as u32 * dst_alpha;
    let src_premul = src as u32 * under_alpha;
    let result_premul = dst_premul + (src_premul * inverse_dst_alpha + 127) / 255;
    *channel = ((result_premul + result_alpha / 2) / result_alpha).min(255) as u8;
  }
  dst[3] = result_alpha.min(255) as u8;
}

fn find_nonzero_bounds<T>(
  pixels: &[T],
  width: u32,
  height: u32,
  mut alpha_of: impl FnMut(&T) -> u8,
) -> Option<Placement> {
  let mut min_x = width;
  let mut min_y = height;
  let mut max_x = 0;
  let mut max_y = 0;
  let mut has_alpha = false;

  for (y, row) in pixels
    .chunks_exact(width as usize)
    .take(height as usize)
    .enumerate()
  {
    for (x, pixel) in row.iter().enumerate() {
      if alpha_of(pixel) == 0 {
        continue;
      }
      has_alpha = true;
      min_x = min_x.min(x as u32);
      min_y = min_y.min(y as u32);
      max_x = max_x.max(x as u32);
      max_y = max_y.max(y as u32);
    }
  }

  has_alpha.then_some(Placement {
    left: min_x as i32,
    top: min_y as i32,
    width: max_x - min_x + 1,
    height: max_y - min_y + 1,
  })
}

fn mask_bounds(mask: &[u8], width: u32, height: u32) -> Option<Placement> {
  find_nonzero_bounds(mask, width, height, |alpha| *alpha)
}

fn backdrop_filter_padding(filters: &[Filter], sizing: &SizingContext) -> i32 {
  filters
    .iter()
    .filter_map(|filter| match filter {
      Filter::Blur(radius) => {
        Some((radius.to_px(sizing, 1.0) * BlurType::Filter.extent_multiplier()).ceil() as i32)
      }
      _ => None,
    })
    .max()
    .unwrap_or(0)
}

fn backdrop_region(
  mask_placement: Placement,
  mask_bounds: Placement,
  padding: i32,
  canvas_size: Size<u32>,
) -> Option<Placement> {
  mask_bounds
    .translate(mask_placement.left, mask_placement.top)
    .inflate(padding)?
    .clamp_to(canvas_size)
}

fn composite_backdrop_with_mask(
  canvas_row: &mut [u8],
  backdrop_row: &[u8],
  mask_row: &[u8],
  mask_start_x: usize,
  region_width: usize,
) {
  let mask_end_x = (mask_start_x + region_width).min(mask_row.len());
  for (x, &alpha) in mask_row[mask_start_x..mask_end_x].iter().enumerate() {
    if alpha == 0 {
      continue;
    }

    let px_idx = x * 4;
    let src = &backdrop_row[px_idx..px_idx + 4];
    let dst = &mut canvas_row[px_idx..px_idx + 4];

    if alpha == 255 {
      dst.copy_from_slice(src);
      continue;
    }

    let src_alpha = alpha as u32;
    let inverse_alpha = 255 - src_alpha;
    dst[0] = fast_div_255(src[0] as u32 * src_alpha + dst[0] as u32 * inverse_alpha);
    dst[1] = fast_div_255(src[1] as u32 * src_alpha + dst[1] as u32 * inverse_alpha);
    dst[2] = fast_div_255(src[2] as u32 * src_alpha + dst[2] as u32 * inverse_alpha);
  }
}

pub(crate) fn apply_filters_to_pixmap<'f, F: Iterator<Item = &'f Filter>>(
  pixmap: &mut PixmapMut<'_>,
  sizing: &SizingContext,
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
          let raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
          apply_batched_pixel_filters(raw, &pending_pixel_filters);
          pending_pixel_filters.clear();
        }

        // Apply complex filter
        match *f {
          Filter::HueRotate(angle) => {
            let raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
            apply_hue_rotate_rgba_bytes(raw, *angle as i32);
          }
          Filter::Blur(blur) => {
            let width = pixmap.width();
            let height = pixmap.height();
            let raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
            apply_blur_rgba_bytes(
              raw,
              width,
              height,
              blur.to_px(sizing, 1.0),
              BlurType::Filter,
              buffer_pool,
            )?;
          }
          Filter::DropShadow(drop_shadow) => {
            let size = Size {
              width: pixmap.width() as f32,
              height: pixmap.height() as f32,
            };
            let shadow = SizedShadow::from_text_shadow(drop_shadow, sizing, current_color, size);
            apply_drop_shadow_filter(pixmap, &shadow, buffer_pool)?;
          }
          _ => {}
        }
      }
    }
  }

  // Flush remaining pixel filters
  if !pending_pixel_filters.is_empty() {
    let raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
    apply_batched_pixel_filters(raw, &pending_pixel_filters);
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

  // Render the mask for compositing.
  let (mask_data, placement) = render_mask(&paths, Some(transform), None, &mut canvas.buffer_pool);

  if placement.width == 0 || placement.height == 0 {
    canvas.buffer_pool.release(mask_data);
    return Ok(());
  }

  let Some(mask_bounds) = mask_bounds(&mask_data, placement.width, placement.height) else {
    canvas.buffer_pool.release(mask_data);
    return Ok(());
  };

  let padding = backdrop_filter_padding(filters, &context.sizing);
  let Some(region) = backdrop_region(placement, mask_bounds, padding, canvas_size) else {
    canvas.buffer_pool.release(mask_data);
    return Ok(());
  };

  let region_width = region.width;
  let region_height = region.height;
  let region_row_bytes = region_width as usize * 4;
  let backdrop_len = region_row_bytes * region_height as usize;

  // Extract the region from the canvas using pooled raw bytes.
  let mut backdrop_raw = canvas.buffer_pool.acquire_dirty(backdrop_len);

  canvas.with_pixmap_ref_and_pool(|pixmap, _| {
    let canvas_width = pixmap.width() as usize;
    let canvas_raw: &[u8] = bytemuck::cast_slice(pixmap.pixels());
    for (y, dest_row) in backdrop_raw.chunks_exact_mut(region_row_bytes).enumerate() {
      let src_y = region.top as usize + y;
      let src_start = (src_y * canvas_width + region.left as usize) * 4;
      dest_row.copy_from_slice(&canvas_raw[src_start..src_start + region_row_bytes]);
    }
  });

  let Some(mut backdrop_pixmap) =
    PixmapMut::from_bytes(&mut backdrop_raw, region_width, region_height)
  else {
    canvas.buffer_pool.release(backdrop_raw);
    canvas.buffer_pool.release(mask_data);
    return Ok(());
  };

  apply_filters_to_pixmap(
    &mut backdrop_pixmap,
    &context.sizing,
    context.current_color,
    &mut canvas.buffer_pool,
    drop_shadow_filtered,
  )?;

  // Composite the filtered backdrop back to the canvas, respecting the mask.
  let mask_offset_x = region.left - placement.left;
  let mask_offset_y = region.top - placement.top;
  let x_start = (-mask_offset_x).max(0) as usize;
  let x_end = (placement.width as i32 - mask_offset_x)
    .min(region.width as i32)
    .max(0) as usize;
  let visible_width = x_end.saturating_sub(x_start);

  canvas.with_pixmap_and_pool(|pixmap, _| {
    let canvas_width = pixmap.width() as usize;
    let canvas_raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

    for y in 0..region_height {
      let mask_y = mask_offset_y + y as i32;
      if mask_y < 0 {
        continue;
      }
      if mask_y >= placement.height as i32 {
        break;
      }

      let canvas_y = region.top as usize + y as usize;
      let canvas_start = (canvas_y * canvas_width + region.left as usize) * 4;
      let canvas_row = &mut canvas_raw[canvas_start..canvas_start + region_row_bytes];

      let backdrop_start = (y * region_width) as usize * 4;
      let backdrop_row = &backdrop_raw[backdrop_start..backdrop_start + region_row_bytes];
      let mask_row_start = mask_y as usize * placement.width as usize;
      let mask_row = &mask_data[mask_row_start..mask_row_start + placement.width as usize];
      composite_backdrop_with_mask(
        &mut canvas_row[x_start * 4..(x_start + visible_width) * 4],
        &backdrop_row[x_start * 4..(x_start + visible_width) * 4],
        mask_row,
        (mask_offset_x + x_start as i32) as usize,
        visible_width,
      );
    }
  });

  canvas.buffer_pool.release(backdrop_raw);
  canvas.buffer_pool.release(mask_data);

  Ok(())
}

/// Applies a drop-shadow filter effect to an image.
fn apply_drop_shadow_filter(
  pixmap: &mut PixmapMut<'_>,
  shadow: &SizedShadow,
  buffer_pool: &mut BufferPool,
) -> Result<()> {
  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  if canvas_width == 0 || canvas_height == 0 {
    return Ok(());
  }

  let blur_radius = shadow.blur_radius;
  let padding = blur_radius.ceil() as u32;

  let offset_x = shadow.offset_x.round() as i32;
  let offset_y = shadow.offset_y.round() as i32;

  let shadow_color: Rgba<u8> = shadow.color.into();
  let [sr, sg, sb, sa] = shadow_color.0;
  let shadow_rgb = [sr, sg, sb];
  let source_pixels = pixmap.as_ref().pixels();
  let Some(source_bounds) =
    find_nonzero_bounds(source_pixels, canvas_width, canvas_height, |pixel| {
      pixel.alpha()
    })
  else {
    return Ok(());
  };

  if sa == 0 {
    return Ok(());
  }

  let shadow_width = source_bounds.width + 2 * padding;
  let shadow_height = source_bounds.height + 2 * padding;

  let mut shadow_alpha = buffer_pool.acquire_dirty((shadow_width * shadow_height) as usize);
  shadow_alpha.fill(0);

  for y in 0..source_bounds.height {
    let src_row = (source_bounds.top as u32 + y) as usize * canvas_width as usize;
    let dst_row = (y + padding) as usize * shadow_width as usize + padding as usize;
    for x in 0..source_bounds.width {
      let alpha = source_pixels[src_row + (source_bounds.left as u32 + x) as usize].alpha();
      shadow_alpha[dst_row + x as usize] = fast_div_255(sa as u32 * alpha as u32);
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

  let dest_left = source_bounds.left + offset_x - padding as i32;
  let dest_top = source_bounds.top + offset_y - padding as i32;
  let dest_right = dest_left + shadow_width as i32;
  let dest_bottom = dest_top + shadow_height as i32;

  let start_x = dest_left.max(0);
  let start_y = dest_top.max(0);
  let end_x = dest_right.min(canvas_width as i32);
  let end_y = dest_bottom.min(canvas_height as i32);
  if start_x >= end_x || start_y >= end_y {
    buffer_pool.release(shadow_alpha);
    return Ok(());
  }

  let canvas_data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  for y in start_y..end_y {
    let shadow_y = (y - dest_top) as u32;
    for x in start_x..end_x {
      let shadow_x = (x - dest_left) as u32;
      let sa_px = shadow_alpha[(shadow_y * shadow_width + shadow_x) as usize];
      if sa_px > 0 {
        let pixel_index = ((y as u32 * canvas_width + x as u32) * 4) as usize;
        composite_pixel_under(
          &mut canvas_data[pixel_index..pixel_index + 4],
          shadow_rgb,
          sa_px,
        );
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
      _ => Err(unexpected_token!(location, token)),
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

impl ToCss for Filter {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    macro_rules! write_fn {
      ($name:expr, $v:expr) => {{
        dest.write_str($name)?;
        dest.write_char('(')?;
        $v.to_css(dest)?;
        dest.write_char(')')
      }};
    }
    match self {
      Self::Brightness(v) => write_fn!("brightness", v),
      Self::Contrast(v) => write_fn!("contrast", v),
      Self::Grayscale(v) => write_fn!("grayscale", v),
      Self::Saturate(v) => write_fn!("saturate", v),
      Self::HueRotate(v) => write_fn!("hue-rotate", v),
      Self::Invert(v) => write_fn!("invert", v),
      Self::Sepia(v) => write_fn!("sepia", v),
      Self::Opacity(v) => write_fn!("opacity", v),
      Self::Blur(v) => write_fn!("blur", v),
      Self::DropShadow(v) => write_fn!("drop-shadow", v),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use image::RgbaImage;
  use tiny_skia::PixmapMut;

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

    let viewport = Viewport::new((100, 100));
    let sizing = SizingContext {
      viewport,
      container_size: Size::NONE,
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let mut buffer_pool = BufferPool::default();
    let width = image.width();
    let height = image.height();
    let Some(mut pixmap) = PixmapMut::from_bytes(image.as_mut(), width, height) else {
      return Ok(());
    };
    apply_filters_to_pixmap(
      &mut pixmap,
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

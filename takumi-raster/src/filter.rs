use smallvec::SmallVec;
#[cfg(feature = "svg")]
use takumi_core::{Error, resources::image::apply_svg_filter, style::FilterReference};
use takumi_core::{
  filter::ColorMatrix,
  geometry::{Point, Size},
  paint::compose_transfer_table,
};
use tiny_skia::{Mask as TinyMask, PixmapMut};

use crate::{
  BlurFormat, BlurType, BorderProperties, Canvas, Placement, RenderContext, Result, SizedShadow,
  apply_blur, apply_blur_rgba_bytes,
  canvas::demultiply_rgba_in_place,
  checked_area, fast_div_255, intersect_alpha_masks, premultiply_rgba_pixel, render_mask,
  style::{
    Affine, Angle, Color, Filter, FilterCategory, LUMA_WEIGHTS, PercentageNumber, SEPIA_WEIGHTS,
    SizingContext, TransferChannel, TransferTable,
  },
};

/// Calculates the luma of an RGB pixel.
#[inline(always)]
fn get_luma(pixel: &[u8]) -> f32 {
  pixel[0] as f32 * LUMA_WEIGHTS[0]
    + pixel[1] as f32 * LUMA_WEIGHTS[1]
    + pixel[2] as f32 * LUMA_WEIGHTS[2]
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

      let sepia_r = (r * SEPIA_WEIGHTS[0][0] + g * SEPIA_WEIGHTS[0][1] + b * SEPIA_WEIGHTS[0][2])
        .clamp(0.0, 255.0);
      let sepia_g = (r * SEPIA_WEIGHTS[1][0] + g * SEPIA_WEIGHTS[1][1] + b * SEPIA_WEIGHTS[1][2])
        .clamp(0.0, 255.0);
      let sepia_b = (r * SEPIA_WEIGHTS[2][0] + g * SEPIA_WEIGHTS[2][1] + b * SEPIA_WEIGHTS[2][2])
        .clamp(0.0, 255.0);

      pixel[0] = (r * (1.0 - amount) + sepia_r * amount).clamp(0.0, 255.0) as u8;
      pixel[1] = (g * (1.0 - amount) + sepia_g * amount).clamp(0.0, 255.0) as u8;
      pixel[2] = (b * (1.0 - amount) + sepia_b * amount).clamp(0.0, 255.0) as u8;
    }
    Filter::Opacity(PercentageNumber(value)) => {
      pixel[3] = ((pixel[3]) as f32 * value).clamp(0.0, 255.0) as u8;
    }
    // Complex filters are not handled here
    Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {}
    _ => {}
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

/// Runs `transform` on the pixel's straight-alpha channels, leaving it
/// premultiplied again. Filter Effects defines every colour filter on
/// non-premultiplied colour, and a canvas pixel is premultiplied.
#[inline(always)]
fn on_straight_alpha(pixel: &mut [u8; 4], transform: impl FnOnce(&mut [u8; 4])) {
  let opaque = pixel[3] == u8::MAX;

  if !opaque {
    demultiply_rgba_in_place(pixel);
  }

  transform(pixel);

  if !opaque || pixel[3] != u8::MAX {
    *pixel = premultiply_rgba_pixel(pixel[0], pixel[1], pixel[2], pixel[3]);
  }
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

    on_straight_alpha(pixel, |pixel| {
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
    });
  }
}

/// Rotates every visible pixel's hue by `angle`, through the matrix Filter
/// Effects defines for it.
fn apply_hue_rotate_rgba_bytes(data: &mut [u8], angle: Angle) {
  let Some(matrix) = ColorMatrix::from_filter(&Filter::HueRotate(angle)) else {
    return;
  };

  for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(data) {
    if pixel[3] == 0 {
      continue;
    }
    on_straight_alpha(pixel, |pixel| {
      let channel = |value: u8| f32::from(value) / 255.0;
      let out = matrix.apply([
        channel(pixel[0]),
        channel(pixel[1]),
        channel(pixel[2]),
        channel(pixel[3]),
      ]);

      for (slot, value) in pixel.iter_mut().zip(out) {
        *slot = (value * 255.0).round() as u8;
      }
    });
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

  has_alpha.then(|| Placement {
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
    dst[3] = fast_div_255(src[3] as u32 * src_alpha + dst[3] as u32 * inverse_alpha);
  }
}

pub(crate) fn apply_filters_to_pixmap<'f, F: Iterator<Item = &'f Filter>>(
  pixmap: &mut PixmapMut<'_>,
  sizing: &SizingContext,
  current_color: Color,
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
        match f {
          Filter::HueRotate(angle) => {
            let raw: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
            apply_hue_rotate_rgba_bytes(raw, *angle);
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
            )?;
          }
          Filter::DropShadow(drop_shadow) => {
            let size = Size {
              width: pixmap.width() as f32,
              height: pixmap.height() as f32,
            };
            let shadow = SizedShadow::from_text_shadow(*drop_shadow, sizing, current_color, size);
            apply_drop_shadow_filter(pixmap, &shadow)?;
          }
          // Delegates to the resvg pipeline; `apply_svg_filter` hands the
          // layer over without a base64 / data-URI roundtrip.
          #[cfg(feature = "svg")]
          Filter::Reference(reference) => {
            let (width, height) = (pixmap.width(), pixmap.height());
            if width > 0 && height > 0 {
              apply_svg_filter(
                pixmap.data_mut(),
                width,
                height,
                &reference.markup,
                FilterReference::ID,
              )
              .map_err(Error::ImageResolveError)?;
            }
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
  node_mask: Option<&TinyMask>,
) -> Result<()> {
  let filters = &context.style.backdrop_filter;

  if filters.iter().all(Filter::is_drop_shadow) {
    return Ok(());
  }

  let drop_shadow_filtered = filters.iter().filter(|f| !f.is_drop_shadow());

  let canvas_size = canvas.size();
  if canvas_size.width == 0 || canvas_size.height == 0 {
    return Ok(());
  }

  // Generate the mask for the element's shape (with border-radius)
  let mut paths = Vec::new();
  border.append_mask_commands(&mut paths, layout_size, Point::ZERO);

  // Render the mask for compositing.
  let (mut mask_data, mut placement) = render_mask(&paths, Some(transform), None);

  if placement.width == 0 || placement.height == 0 {
    return Ok(());
  }

  if let Some(node_mask) = node_mask {
    let node_placement = Placement {
      left: 0,
      top: 0,
      width: node_mask.width(),
      height: node_mask.height(),
    };
    let intersected =
      intersect_alpha_masks(&mask_data, placement, node_mask.data(), node_placement);
    let Some(intersected) = intersected else {
      return Ok(());
    };
    (mask_data, placement) = intersected;
  }

  let Some(mask_bounds) = mask_bounds(&mask_data, placement.width, placement.height) else {
    return Ok(());
  };

  let padding = backdrop_filter_padding(filters, &context.sizing);
  let Some(region) = backdrop_region(placement, mask_bounds, padding, canvas_size) else {
    return Ok(());
  };

  let region_width = region.width;
  let region_height = region.height;
  let region_row_bytes = region_width as usize * 4;
  let backdrop_len = region_row_bytes * region_height as usize;

  let mut backdrop_raw = vec![0; backdrop_len];

  canvas.with_pixmap_ref(|pixmap| {
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
    return Ok(());
  };

  apply_filters_to_pixmap(
    &mut backdrop_pixmap,
    &context.sizing,
    context.current_color,
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

  canvas.with_pixmap(|pixmap| {
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

  Ok(())
}

/// Applies a drop-shadow filter effect to an image.
fn apply_drop_shadow_filter(pixmap: &mut PixmapMut<'_>, shadow: &SizedShadow) -> Result<()> {
  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  if canvas_width == 0 || canvas_height == 0 {
    return Ok(());
  }

  let blur_radius = shadow.blur_radius;
  let padding = (blur_radius * BlurType::Shadow.extent_multiplier()).ceil() as u32;

  let offset_x = shadow.offset_x.round() as i32;
  let offset_y = shadow.offset_y.round() as i32;

  let [sr, sg, sb, sa] = shadow.color.0;
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

  let shadow_width = source_bounds
    .width
    .saturating_add(padding.saturating_mul(2));
  let shadow_height = source_bounds
    .height
    .saturating_add(padding.saturating_mul(2));

  let Some(area) = checked_area(shadow_width, shadow_height, 1) else {
    return Ok(());
  };
  let mut shadow_alpha = vec![0; area];

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

  Ok(())
}

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};
  use tiny_skia::PixmapMut;

  use super::*;
  use crate::viewport::Viewport;

  #[test]
  fn test_apply_filters_lut_batching() -> Result<()> {
    let mut image = RgbaImage::new(1, 1);
    image.put_pixel(0, 0, Rgba([100, 150, 200, 255]));

    let filters = [
      Filter::Brightness(PercentageNumber(1.2)),
      Filter::Invert(PercentageNumber(1.0)),
      Filter::Opacity(PercentageNumber(0.5)),
    ];

    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let width = image.width();
    let height = image.height();
    let Some(mut pixmap) = PixmapMut::from_bytes(image.as_mut(), width, height) else {
      return Ok(());
    };
    apply_filters_to_pixmap(&mut pixmap, &sizing, Color::black(), filters.iter())?;

    let pixel = image.get_pixel(0, 0);
    assert_eq!(pixel.0, [67, 37, 7, 127]);

    Ok(())
  }

  #[test]
  fn invert_reads_a_semi_transparent_pixel_as_straight_alpha() {
    let mut pixel = [128, 128, 128, 128];

    apply_batched_pixel_filters(&mut pixel, &[&Filter::Invert(PercentageNumber(1.0))]);

    assert_eq!(pixel, [0, 0, 0, 128]);
  }

  #[test]
  fn brightness_keeps_a_semi_transparent_pixel_premultiplied() {
    let mut pixel = [64, 64, 64, 128];

    apply_batched_pixel_filters(&mut pixel, &[&Filter::Brightness(PercentageNumber(4.0))]);

    assert_eq!(pixel, [128, 128, 128, 128]);
  }

  #[test]
  fn hue_rotate_keeps_a_semi_transparent_pixel_premultiplied() {
    let mut pixel = [128, 0, 0, 128];

    apply_hue_rotate_rgba_bytes(&mut pixel, Angle::new(120.0));

    assert!(pixel[..3].iter().all(|channel| *channel <= pixel[3]));
    assert_eq!(pixel[3], 128);
  }

  #[test]
  fn drop_shadow_on_transparent_content_does_not_panic() -> Result<()> {
    let mut image = RgbaImage::new(16, 16);
    let width = image.width();
    let height = image.height();
    let Some(mut pixmap) = PixmapMut::from_bytes(image.as_mut(), width, height) else {
      return Ok(());
    };

    let shadow = SizedShadow {
      offset_x: 1.0,
      offset_y: 1.0,
      blur_radius: 1.0,
      spread_radius: 0.0,
      color: Color::black(),
    };

    apply_drop_shadow_filter(&mut pixmap, &shadow)
  }
}

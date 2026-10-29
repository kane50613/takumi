use std::iter::successors;

use smallvec::SmallVec;
#[cfg(feature = "svg")]
use takumi_core::{Error, resources::image::apply_svg_filter, style::FilterReference};
use takumi_core::{
  filter::{ColorMatrix, ColorMatrixChain},
  geometry::{Point, Size},
  paint::compose_transfer_table,
};
use tiny_skia::{IntSize, Pixmap, PixmapMut, PixmapPaint, Transform as TinyTransform};

use crate::{
  BlurType, BorderProperties, Canvas, Placement, RenderContext, Result, SizedShadow,
  apply_blur_alpha_bytes, apply_blur_rgba_bytes,
  canvas::demultiply_rgba_in_place,
  checked_area, mul_div_255, render_mask,
  resources::image_buffer::premultiply_pixel,
  style::{Affine, Color, Filter, FilterCategory, SizingContext, TransferChannel, TransferTable},
};

/// Filter prepared for batch execution
enum PreparedFilter {
  Matrices(ColorMatrixChain),
  RgbLut(Box<TransferTable>),
  AlphaLut(Box<TransferTable>),
}

/// A plan fusing consecutive transfer tables into one lookup per channel.
fn prepare_pixel_filters(filters: &[&Filter]) -> SmallVec<[PreparedFilter; 4]> {
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
        if let Some(matrix) = ColorMatrix::from_filter(filter) {
          match prepared.last_mut() {
            Some(PreparedFilter::Matrices(chain)) => chain.push(matrix),
            _ => {
              let mut chain = ColorMatrixChain::default();

              chain.push(matrix);
              prepared.push(PreparedFilter::Matrices(chain));
            }
          }
        }
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

/// Runs `transform` on the pixel's straight-alpha channels, as Filter Effects defines them.
#[inline(always)]
fn on_straight_alpha(pixel: &mut [u8; 4], transform: impl FnOnce(&mut [u8; 4])) {
  let opaque = pixel[3] == u8::MAX;

  if !opaque {
    demultiply_rgba_in_place(pixel);
  }

  transform(pixel);

  if !opaque || pixel[3] != u8::MAX {
    *pixel = premultiply_pixel(*pixel);
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
          PreparedFilter::Matrices(chain) => *pixel = chain.apply_rgba8(*pixel),
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

/// Draws the premultiplied `dst` over a shadow pixel of colour `under_rgb` and coverage
/// `under_alpha`, as source-over does.
#[inline(always)]
fn composite_pixel_under(dst: &mut [u8], under_rgb: [u8; 3], under_alpha: u8) {
  if under_alpha == 0 || dst[3] == 255 {
    return;
  }

  let uncovered = 255 - dst[3];

  for (channel, color) in dst.iter_mut().zip(under_rgb) {
    let under = mul_div_255(color, under_alpha);

    *channel = channel.saturating_add(mul_div_255(under, uncovered));
  }
  dst[3] = dst[3].saturating_add(mul_div_255(under_alpha, uncovered));
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

  for (y, row) in pixels
    .chunks_exact(width as usize)
    .take(height as usize)
    .enumerate()
  {
    let Some(first) = row.iter().position(|pixel| alpha_of(pixel) != 0) else {
      continue;
    };

    let last = row
      .iter()
      .rposition(|pixel| alpha_of(pixel) != 0)
      .unwrap_or(first);
    min_x = min_x.min(first as u32);
    max_x = max_x.max(last as u32);
    min_y = min_y.min(y as u32);
    max_y = y as u32;
  }

  (min_x < width).then(|| Placement {
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
    .filter(|filter| !filter.is_drop_shadow())
    .map(|filter| filter.reach(sizing).ceil() as i32)
    .max()
    .unwrap_or(0)
}

/// A backdrop grown by `padding` on every side with its own pixels mirrored across its edges, so
/// a filter reads nothing from outside the element, as Chrome's mirror edge mode does.
struct MirroredRegion {
  raw: Vec<u8>,
  width: u32,
  height: u32,
  padding: u32,
}

impl MirroredRegion {
  /// Halves `padding` until the grown region fits the pixel budget.
  fn of(raw: &[u8], region: Placement, padding: u32) -> Option<Self> {
    let (width, height, capacity, padding) = successors(Some(padding), |&padding| {
      (padding > 0).then_some(padding / 2)
    })
    .find_map(|padding| {
      let width = region.width.checked_add(padding.checked_mul(2)?)?;
      let height = region.height.checked_add(padding.checked_mul(2)?)?;

      Some((width, height, checked_area(width, height, 4)?, padding))
    })?;
    let mirror = |index: u32, size: u32| {
      let period = size as i64 * 2;
      let offset = (index as i64 - padding as i64).rem_euclid(period);

      (if offset >= size as i64 {
        period - 1 - offset
      } else {
        offset
      }) as usize
    };
    let source_rows = raw.as_chunks::<4>().0;
    let columns: Vec<usize> = (0..width).map(|x| mirror(x, region.width)).collect();
    // The columns a row copies straight across, without mirroring.
    let straight = padding.min(width) as usize..(padding + region.width).min(width) as usize;
    let mut padded = vec![[0; 4]; capacity / 4];

    for (y, row) in padded.chunks_exact_mut(width as usize).enumerate() {
      let source = &source_rows[mirror(y as u32, region.height) * region.width as usize..]
        [..region.width as usize];

      let (left, rest) = row.split_at_mut(straight.start);
      let (middle, right) = rest.split_at_mut(straight.len());

      middle.copy_from_slice(&source[columns[straight.start]..][..straight.len()]);
      for (pixel, &column) in left
        .iter_mut()
        .zip(&columns)
        .chain(right.iter_mut().zip(&columns[straight.end..]))
      {
        *pixel = source[column];
      }
    }

    let padded = padded.into_flattened();

    Some(Self {
      raw: padded,
      width,
      height,
      padding,
    })
  }

  /// The bytes of the original region's row `y`.
  fn inner_row(&self, y: u32) -> &[u8] {
    let start = ((y + self.padding) * self.width + self.padding) as usize * 4;

    &self.raw[start..start + (self.width - self.padding * 2) as usize * 4]
  }
}

fn mask_backdrop_row(output_row: &mut [u8], backdrop_row: &[u8], mask_row: &[u8]) {
  let pixels = output_row
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .zip(backdrop_row.as_chunks::<4>().0)
    .zip(mask_row);

  for ((dst, src), &alpha) in pixels {
    *dst = (*src).map(|channel| mul_div_255(channel, alpha));
  }
}

pub(crate) fn apply_filters_to_pixmap<'f, F: Iterator<Item = &'f Filter>>(
  pixmap: &mut PixmapMut<'_>,
  sizing: &SizingContext,
  current_color: Color,
  filters: F,
) -> Result<()> {
  let mut pending_pixel_filters: SmallVec<[&Filter; 8]> = SmallVec::new();

  for filter in filters {
    match filter.categorize() {
      FilterCategory::Pixel(f) => pending_pixel_filters.push(f),
      FilterCategory::Complex(f) => {
        apply_batched_pixel_filters(
          bytemuck::cast_slice_mut(pixmap.pixels_mut()),
          &pending_pixel_filters,
        );
        pending_pixel_filters.clear();

        match f {
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
          // Runs the SVG filter pipeline; `apply_svg_filter` hands the
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

  apply_batched_pixel_filters(
    bytemuck::cast_slice_mut(pixmap.pixels_mut()),
    &pending_pixel_filters,
  );

  Ok(())
}

/// A filtered backdrop ready to paint inside its effect layer.
pub(crate) struct Backdrop {
  image: Pixmap,
  origin: Point<i32>,
}

impl Backdrop {
  /// Filters the parent canvas through the element's border box.
  pub(crate) fn new(
    canvas: &Canvas,
    border: BorderProperties,
    layout_size: Size<f32>,
    transform: Affine,
    context: &RenderContext,
  ) -> Result<Option<Self>> {
    let filters = &context.style.misc1_data.backdrop_filter;

    if filters.iter().all(Filter::is_drop_shadow) {
      return Ok(None);
    }

    let canvas_size = canvas.size();
    if canvas_size.width == 0 || canvas_size.height == 0 {
      return Ok(None);
    }

    let mut paths = Vec::new();
    border.append_mask_commands(&mut paths, layout_size, Point::ZERO);

    let (mask_data, placement) =
      render_mask(&paths, Some(transform), None, Some(canvas.viewport()));

    if placement.width == 0 || placement.height == 0 {
      return Ok(None);
    }

    let Some(mask_bounds) = mask_bounds(&mask_data, placement.width, placement.height) else {
      return Ok(None);
    };

    let Some(region) = mask_bounds
      .translate(placement.left, placement.top)
      .clamp_to(canvas_size)
    else {
      return Ok(None);
    };

    let region_row_bytes = region.width as usize * 4;
    // Padding past the canvas only mirrors the same pixels again, as reading past it used to stop.
    let padding = backdrop_filter_padding(filters, &context.sizing)
      .clamp(0, canvas_size.width.max(canvas_size.height) as i32) as u32;
    let Some(mut padded) = MirroredRegion::of(&canvas.read_region(region), region, padding) else {
      return Ok(None);
    };
    let Some(mut backdrop_pixmap) =
      PixmapMut::from_bytes(&mut padded.raw, padded.width, padded.height)
    else {
      return Ok(None);
    };

    apply_filters_to_pixmap(
      &mut backdrop_pixmap,
      &context.sizing,
      context.current_color,
      filters.iter().filter(|filter| !filter.is_drop_shadow()),
    )?;

    let mask_offset_x = region.left - placement.left;
    let mask_offset_y = region.top - placement.top;
    let x_start = (-mask_offset_x).max(0) as usize;
    let x_end = (placement.width as i32 - mask_offset_x)
      .min(region.width as i32)
      .max(0) as usize;
    let visible_width = x_end.saturating_sub(x_start);

    let Some(size) = IntSize::from_wh(region.width, region.height) else {
      return Ok(None);
    };
    let Some(len) = checked_area(region.width, region.height, 4) else {
      return Ok(None);
    };
    let mut raw = vec![0; len];

    for y in 0..region.height {
      let mask_y = mask_offset_y + y as i32;
      if mask_y < 0 {
        continue;
      }
      if mask_y >= placement.height as i32 {
        break;
      }

      let start = y as usize * region_row_bytes;
      let output_row = &mut raw[start..start + region_row_bytes];

      let backdrop_row = padded.inner_row(y);
      let mask_row_start = mask_y as usize * placement.width as usize;
      let mask_start = mask_row_start + (mask_offset_x + x_start as i32) as usize;

      mask_backdrop_row(
        &mut output_row[x_start * 4..(x_start + visible_width) * 4],
        &backdrop_row[x_start * 4..(x_start + visible_width) * 4],
        &mask_data[mask_start..mask_start + visible_width],
      );
    }
    let Some(image) = Pixmap::from_vec(raw, size) else {
      return Ok(None);
    };
    let viewport = canvas.viewport();

    Ok(Some(Self {
      image,
      origin: Point {
        x: viewport.origin.x + region.left,
        y: viewport.origin.y + region.top,
      },
    }))
  }

  /// Paints the filtered backdrop into its source layer.
  pub(crate) fn paint(self, canvas: &mut Canvas) {
    let viewport = canvas.viewport();

    canvas.with_pixmap(|pixmap| {
      pixmap.draw_pixmap(
        self.origin.x - viewport.origin.x,
        self.origin.y - viewport.origin.y,
        self.image.as_ref(),
        &PixmapPaint::default(),
        TinyTransform::identity(),
        None,
      );
    });
  }
}

/// Applies `drop-shadow()`, its blur a standard deviation and its offset snapped as Chrome does.
fn apply_drop_shadow_filter(pixmap: &mut PixmapMut<'_>, shadow: &SizedShadow) -> Result<()> {
  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  if canvas_width == 0 || canvas_height == 0 {
    return Ok(());
  }

  let padding = BlurType::Filter.extent(shadow.blur_radius).ceil() as u32;

  let offset_x = shadow.offset_x.floor() as i32;
  let offset_y = shadow.offset_y.floor() as i32;

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
      shadow_alpha[dst_row + x as usize] = mul_div_255(sa, alpha);
    }
  }

  apply_blur_alpha_bytes(
    &mut shadow_alpha,
    shadow_width,
    shadow_height,
    shadow.blur_radius,
    BlurType::Filter,
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
  use crate::Bitmap;
  use tiny_skia::PixmapMut;

  use super::*;
  use crate::{
    Fonts, RenderOptions,
    layout::node::Node,
    render,
    style::{Angle, PercentageNumber},
    viewport::Viewport,
  };

  #[test]
  fn a_shadow_under_a_clear_pixel_is_premultiplied() {
    let mut pixel = [0, 0, 0, 0];

    composite_pixel_under(&mut pixel, [255, 0, 0], 128);
    assert_eq!(pixel, [128, 0, 0, 128]);
  }

  #[test]
  fn a_shadow_fills_what_a_translucent_pixel_leaves_uncovered() {
    let mut full = [128, 128, 128, 128];
    let mut half = [128, 128, 128, 128];

    composite_pixel_under(&mut full, [255, 0, 0], 255);
    composite_pixel_under(&mut half, [255, 0, 0], 128);
    assert_eq!(full, [255, 128, 128, 255]);
    assert_eq!(half, [192, 128, 128, 192]);
  }

  #[test]
  fn a_shadow_leaves_an_opaque_pixel_alone() {
    let mut opaque = [10, 20, 30, 255];
    let mut clear = [10, 20, 30, 40];

    composite_pixel_under(&mut opaque, [255, 0, 0], 200);
    composite_pixel_under(&mut clear, [255, 0, 0], 0);
    assert_eq!(opaque, [10, 20, 30, 255]);
    assert_eq!(clear, [10, 20, 30, 40]);
  }

  #[test]
  fn a_huge_backdrop_blur_stays_within_the_canvas() {
    let node: Node = serde_json::from_str(
      r##"{"type": "container", "style": {"width": "100%", "height": "100%", "backgroundColor": "#ffffff"}, "children": [
        {"type": "container", "style": {"position": "absolute", "left": "10px", "top": "10px", "width": "40px", "height": "40px", "backdropFilter": "blur(20000px)"}, "children": []}
      ]}"##,
    )
    .unwrap();
    let fonts = Fonts::default();

    render(
      RenderOptions::builder()
        .viewport(Viewport::new((64, 64)))
        .node(node)
        .fonts(&fonts)
        .build(),
    )
    .unwrap();
  }

  #[test]
  fn mask_bounds_span_the_first_and_last_visible_pixels() {
    let bounds = |mask: &[u8]| mask_bounds(mask, 5, 3).map(|p| (p.left, p.top, p.width, p.height));
    assert_eq!(bounds(&[0; 15]), None);
    assert_eq!(bounds(&[255; 15]), Some((0, 0, 5, 3)));
    let mut single = [0u8; 15];
    single[7] = 1;
    assert_eq!(bounds(&single), Some((2, 1, 1, 1)));
    let mut corners = [0u8; 15];
    corners[0] = 9;
    corners[14] = 9;
    assert_eq!(bounds(&corners), Some((0, 0, 5, 3)));
    let mut middle_rows = [0u8; 15];
    middle_rows[6] = 3;
    middle_rows[8] = 3;
    assert_eq!(bounds(&middle_rows), Some((1, 1, 3, 1)));
  }

  #[test]
  fn test_apply_filters_lut_batching() -> Result<()> {
    let mut image = Bitmap::new(1, 1);
    image.put_pixel(0, 0, [100, 150, 200, 255]);

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
    let mut pixmap = PixmapMut::from_bytes(image.as_raw_mut(), width, height).unwrap();
    apply_filters_to_pixmap(&mut pixmap, &sizing, Color::black(), filters.iter())?;

    let pixel = image.get_pixel(0, 0);
    assert_eq!(*pixel, [68, 38, 8, 128]);

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

    apply_batched_pixel_filters(&mut pixel, &[&Filter::HueRotate(Angle::new(120.0))]);

    assert!(pixel[..3].iter().all(|channel| *channel <= pixel[3]));
    assert_eq!(pixel[3], 128);
  }

  #[test]
  fn drop_shadow_on_transparent_content_does_not_panic() -> Result<()> {
    let mut image = Bitmap::new(16, 16);
    let width = image.width();
    let height = image.height();
    let mut pixmap = PixmapMut::from_bytes(image.as_raw_mut(), width, height).unwrap();

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

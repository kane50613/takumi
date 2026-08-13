use takumi_core::geometry::Point;
use tiny_skia::PixmapMut;

use super::{
  MaskSamplingOptions, MaskView, PaintSource, SamplingFootprint,
  blit::{OverlayBounds, compute_overlay_bounds_for_canvas},
  paint_source::{MaskCompositeColor, apply_mask_color_mode, sample_paint_source},
};
use crate::{
  Placement,
  blend::{blend_premultiplied_pixel, composite_premultiplied_over, scale_premultiplied_pixel},
  style::{Affine, BlendMode},
};

#[derive(Clone, Copy)]
struct DestRegion {
  bounds: OverlayBounds,
  canvas_width: usize,
  mask_stride: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Options<'a> {
  pub placement: Placement,
  pub sampling: MaskSamplingOptions,
  pub color_mode: MaskCompositeColor,
  pub mode: BlendMode,
  pub combined_mask: Option<MaskView<'a>>,
}

pub(super) fn constant(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  color: [u8; 4],
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if color[3] == 0 {
    return;
  }

  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some(bounds) = compute_overlay_bounds_for_canvas(
    canvas_width,
    canvas_height,
    Point {
      x: placement.left as f32,
      y: placement.top as f32,
    },
    placement.width,
    placement.height,
  ) else {
    return;
  };

  let region = DestRegion {
    bounds,
    canvas_width: canvas_width as usize,
    mask_stride: placement.width as usize,
  };
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  if mode == BlendMode::Normal && combined_mask.is_none() {
    constant_normal(pixels, mask, color, region);
    return;
  }

  constant_general(pixels, mask, color, mode, combined_mask, region);
}

pub(super) fn source(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  source: PaintSource<'_>,
  options: Options<'_>,
) {
  if mask.is_empty() {
    return;
  }

  if let Some(color) = source.premultiplied_constant() {
    constant(
      pixmap,
      mask,
      options.placement,
      apply_mask_color_mode(color, options.color_mode),
      options.mode,
      options.combined_mask,
    );
    return;
  }

  let canvas_width = pixmap.width();
  let canvas_height = pixmap.height();
  let Some(bounds) = compute_overlay_bounds_for_canvas(
    canvas_width,
    canvas_height,
    Point {
      x: options.placement.left as f32,
      y: options.placement.top as f32,
    },
    options.placement.width,
    options.placement.height,
  ) else {
    return;
  };
  let region = DestRegion {
    bounds,
    canvas_width: canvas_width as usize,
    mask_stride: options.placement.width as usize,
  };

  if options.color_mode == MaskCompositeColor::SourceOnly
    && options.mode == BlendMode::Normal
    && try_translation_blit(pixmap, mask, source, &options, region)
  {
    return;
  }

  source_general(pixmap, mask, source, &options, region);
}

#[inline]
fn constant_normal(pixels: &mut [[u8; 4]], mask: &[u8], color: [u8; 4], region: DestRegion) {
  let DestRegion {
    bounds,
    canvas_width,
    mask_stride,
  } = region;
  let span = (bounds.x_max - bounds.x_min) as usize;

  for dest_y in bounds.y_min..bounds.y_max {
    let mask_y = (dest_y - bounds.offset_y) as usize;
    let mask_x = (bounds.x_min - bounds.offset_x) as usize;
    let dst_row_start = dest_y as usize * canvas_width + bounds.x_min as usize;
    let mask_row_start = mask_y * mask_stride + mask_x;

    let dst = &mut pixels[dst_row_start..dst_row_start + span];
    let mask_row = &mask[mask_row_start..mask_row_start + span];

    for (dst_px, &alpha) in dst.iter_mut().zip(mask_row) {
      if alpha == 0 {
        continue;
      }
      let src = scale_premultiplied_pixel(color, alpha);
      if src[3] != 0 {
        composite_premultiplied_over(dst_px, src);
      }
    }
  }
}

#[inline]
fn constant_general(
  pixels: &mut [[u8; 4]],
  mask: &[u8],
  color: [u8; 4],
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
  region: DestRegion,
) {
  let DestRegion {
    bounds,
    canvas_width,
    mask_stride,
  } = region;

  for dest_y in bounds.y_min..bounds.y_max {
    let combined_row = combined_mask.map(|view| view.row(dest_y, bounds.x_min));
    if combined_row.is_some_and(|row| row.is_empty()) {
      continue;
    }

    let mask_y = (dest_y - bounds.offset_y) as usize;
    let dst_row = dest_y as usize * canvas_width;
    let mask_row = mask_y * mask_stride;
    for (i, dest_x) in (bounds.x_min..bounds.x_max).enumerate() {
      let alpha = mask[mask_row + (dest_x - bounds.offset_x) as usize];
      if alpha == 0 {
        continue;
      }
      let mut src = scale_premultiplied_pixel(color, alpha);
      if src[3] == 0 {
        continue;
      }
      if let Some(row) = combined_row {
        let extra = row.alpha_at_offset(i);
        if extra == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, extra);
        if src[3] == 0 {
          continue;
        }
      }
      blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
    }
  }
}

fn try_translation_blit(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  source: PaintSource<'_>,
  options: &Options<'_>,
  region: DestRegion,
) -> bool {
  let transform = options.sampling.canvas_to_source;
  if !transform.only_translation() || transform.x.fract() != 0.0 || transform.y.fract() != 0.0 {
    return false;
  }
  if options.sampling.sample_bias != (Point { x: 0.5, y: 0.5 }) {
    return false;
  }
  let Some(source_pixmap) = source.as_pixmap_ref() else {
    return false;
  };

  let DestRegion {
    bounds,
    canvas_width,
    mask_stride,
  } = region;
  let source_width = source_pixmap.width() as i32;
  let source_height = source_pixmap.height() as i32;
  let source_pixels = source_pixmap.pixels();
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let sample_dx = transform.x as i32;
  let sample_dy = transform.y as i32;
  let span = (bounds.x_max - bounds.x_min) as usize;

  for dest_y in bounds.y_min..bounds.y_max {
    let src_y = dest_y + sample_dy;
    if src_y < 0 || src_y >= source_height {
      continue;
    }
    let mask_y = (dest_y - bounds.offset_y) as usize;
    let mask_x_start = (bounds.x_min - bounds.offset_x) as usize;
    let mask_row =
      &mask[mask_y * mask_stride + mask_x_start..mask_y * mask_stride + mask_x_start + span];

    let combined_row = options
      .combined_mask
      .as_ref()
      .map(|view| view.row(dest_y, bounds.x_min));

    let src_row_offset = src_y as usize * source_width as usize;
    let dst_row_offset = dest_y as usize * canvas_width;
    let dst_row = &mut pixels
      [dst_row_offset + bounds.x_min as usize..dst_row_offset + bounds.x_min as usize + span];

    let src_x_base = bounds.x_min + sample_dx;
    for (i, (dst, &mask_alpha)) in dst_row.iter_mut().zip(mask_row).enumerate() {
      if mask_alpha == 0 {
        continue;
      }
      let alpha = match combined_row.as_ref() {
        Some(row) => {
          let extra = row.alpha_at_offset(i);
          if extra == 0 {
            continue;
          }
          if extra == u8::MAX {
            mask_alpha
          } else if mask_alpha == u8::MAX {
            extra
          } else {
            crate::fast_div_255(mask_alpha as u32 * extra as u32)
          }
        }
        None => mask_alpha,
      };
      if alpha == 0 {
        continue;
      }

      let src_x = src_x_base + i as i32;
      if src_x < 0 || src_x >= source_width {
        continue;
      }
      let src_pixel = source_pixels[src_row_offset + src_x as usize];
      let src_a = src_pixel.alpha();
      if src_a == 0 {
        continue;
      }

      let src_rgba = [src_pixel.red(), src_pixel.green(), src_pixel.blue(), src_a];
      if alpha == u8::MAX && src_a == u8::MAX {
        *dst = src_rgba;
        continue;
      }

      let src = scale_premultiplied_pixel(src_rgba, alpha);
      if src[3] == 0 {
        continue;
      }
      composite_premultiplied_over(dst, src);
    }
  }

  true
}

fn source_general(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  source: PaintSource<'_>,
  options: &Options<'_>,
  region: DestRegion,
) {
  let DestRegion {
    bounds,
    canvas_width,
    mask_stride,
  } = region;
  let footprint = sampling_footprint(options.sampling.canvas_to_source);
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let resolved = source.resolve();

  for dest_y in bounds.y_min..bounds.y_max {
    let combined_row = options
      .combined_mask
      .map(|view| view.row(dest_y, bounds.x_min));
    if combined_row.is_some_and(|row| row.is_empty()) {
      continue;
    }

    let mask_y = (dest_y - bounds.offset_y) as usize;
    let dst_row = dest_y as usize * canvas_width;
    let mask_row = mask_y * mask_stride;
    let (mut sample_x, mut sample_y) = options.sampling.canvas_to_source.transform_point(
      bounds.x_min as f32 + options.sampling.sample_bias.x,
      dest_y as f32 + options.sampling.sample_bias.y,
    );
    for (i, dest_x) in (bounds.x_min..bounds.x_max).enumerate() {
      let mask_alpha = mask[mask_row + (dest_x - bounds.offset_x) as usize];
      let sampled = if mask_alpha == 0 {
        None
      } else {
        sample_paint_source(
          resolved,
          options.sampling.algorithm,
          sample_x,
          sample_y,
          footprint,
        )
      };
      sample_x += options.sampling.canvas_to_source.a;
      sample_y += options.sampling.canvas_to_source.b;

      let Some(mut src) = sampled else {
        continue;
      };

      src = apply_mask_color_mode(src, options.color_mode);
      src = scale_premultiplied_pixel(src, mask_alpha);
      if src[3] == 0 {
        continue;
      }

      if let Some(row) = combined_row {
        let extra = row.alpha_at_offset(i);
        if extra == 0 {
          continue;
        }
        src = scale_premultiplied_pixel(src, extra);
        if src[3] == 0 {
          continue;
        }
      }

      blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, options.mode);
    }
  }
}

#[inline(always)]
pub(super) fn sampling_footprint(transform: Affine) -> SamplingFootprint {
  SamplingFootprint::new(
    transform.a.hypot(transform.b),
    transform.c.hypot(transform.d),
  )
}

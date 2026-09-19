use takumi_core::geometry::Point;
use tiny_skia::PixmapMut;

use super::{
  MaskSamplingOptions, MaskView, PaintSource, SamplingFootprint,
  blit::{OverlayBounds, compute_overlay_bounds_for_canvas},
  mask::MaskRow,
  paint_source::{
    MaskCompositeColor, ResolvedSource, ScaledRows, apply_mask_color_mode, sample_paint_source,
  },
};
use crate::{
  Placement,
  blend::{blend_premultiplied_pixel, composite_premultiplied_over, scale_premultiplied_pixel},
  style::{Affine, BlendMode, ImageScalingAlgorithm},
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
  let transform = options.sampling.canvas_to_source;
  let bias = options.sampling.sample_bias;

  if let Some(rows) = ScaledRows::new(
    source,
    transform,
    options.sampling.algorithm,
    bounds.x_min as f32 + bias.x,
    (bounds.x_max - bounds.x_min) as usize,
  ) {
    source_scaled_rows(pixmap, mask, &rows, options, region);
    return;
  }

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  PixelSampler {
    resolved: source.resolve(),
    transform,
    algorithm: options.sampling.algorithm,
    color_mode: options.color_mode,
    mode: options.mode,
    combined_mask: options.combined_mask,
  }
  .sample_into(
    pixels,
    canvas_width,
    bounds,
    |dest_y| transform.transform_point(bounds.x_min as f32 + bias.x, dest_y as f32 + bias.y),
    |dest_y, dest_x| {
      mask[(dest_y - bounds.offset_y) as usize * mask_stride + (dest_x - bounds.offset_x) as usize]
    },
  );
}

/// The per-pixel sampler behind every transform `ScaledRows` does not cover:
/// each destination pixel inverse-maps and interpolates on its own.
pub(super) struct PixelSampler<'a> {
  pub resolved: ResolvedSource<'a>,
  pub transform: Affine,
  pub algorithm: ImageScalingAlgorithm,
  pub color_mode: MaskCompositeColor,
  pub mode: BlendMode,
  pub combined_mask: Option<MaskView<'a>>,
}

impl PixelSampler<'_> {
  /// `row_start` gives a destination row's first sample position and
  /// `mask_alpha` the rectangular mask at a pixel, 255 where there is none.
  pub(super) fn sample_into(
    &self,
    pixels: &mut [[u8; 4]],
    canvas_width: usize,
    bounds: OverlayBounds,
    row_start: impl Fn(i32) -> (f32, f32),
    mask_alpha: impl Fn(i32, i32) -> u8,
  ) {
    let footprint = SamplingFootprint::of(self.transform);
    for dest_y in bounds.y_min..bounds.y_max {
      let combined_row = self
        .combined_mask
        .map(|view| view.row(dest_y, bounds.x_min));
      if combined_row.is_some_and(|row| row.is_empty()) {
        continue;
      }

      let dst_row = dest_y as usize * canvas_width;
      let (mut sample_x, mut sample_y) = row_start(dest_y);
      for (i, dest_x) in (bounds.x_min..bounds.x_max).enumerate() {
        let mask_alpha = mask_alpha(dest_y, dest_x);
        let sampled = if mask_alpha == 0 {
          None
        } else {
          sample_paint_source(self.resolved, self.algorithm, sample_x, sample_y, footprint)
        };
        sample_x += self.transform.a;
        sample_y += self.transform.b;

        if let Some(src) = sampled {
          blend_sampled(
            &mut pixels[dst_row + dest_x as usize],
            src,
            mask_alpha,
            combined_row,
            i,
            self.color_mode,
            self.mode,
          );
        }
      }
    }
  }
}

/// An axis-aligned, non-minifying bitmap sampled a row at a time.
fn source_scaled_rows(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  rows: &ScaledRows<'_>,
  options: &Options<'_>,
  region: DestRegion,
) {
  let DestRegion {
    bounds,
    canvas_width,
    mask_stride,
  } = region;
  let bias = options.sampling.sample_bias;
  let span = (bounds.x_max - bounds.x_min) as usize;
  let mut row = vec![[0u8; 4]; span];
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

  for dest_y in bounds.y_min..bounds.y_max {
    let combined_row = options
      .combined_mask
      .map(|view| view.row(dest_y, bounds.x_min));
    if combined_row.is_some_and(|row| row.is_empty()) {
      continue;
    }

    rows.fill(dest_y as f32 + bias.y, &mut row);
    let mask_y = (dest_y - bounds.offset_y) as usize;
    let mask_x = (bounds.x_min - bounds.offset_x) as usize;
    let mask_row = &mask[mask_y * mask_stride + mask_x..mask_y * mask_stride + mask_x + span];
    let dst_start = dest_y as usize * canvas_width + bounds.x_min as usize;
    let dst_row = &mut pixels[dst_start..dst_start + span];

    for (i, ((dst, &mask_alpha), &src)) in dst_row.iter_mut().zip(mask_row).zip(&row).enumerate() {
      if mask_alpha == 0 {
        continue;
      }
      blend_sampled(
        dst,
        src,
        mask_alpha,
        combined_row,
        i,
        options.color_mode,
        options.mode,
      );
    }
  }
}

#[inline(always)]
fn blend_sampled(
  dst: &mut [u8; 4],
  src: [u8; 4],
  mask_alpha: u8,
  combined_row: Option<MaskRow<'_>>,
  offset: usize,
  color_mode: MaskCompositeColor,
  mode: BlendMode,
) {
  let mut src = apply_mask_color_mode(src, color_mode);
  src = scale_premultiplied_pixel(src, mask_alpha);
  if src[3] == 0 {
    return;
  }

  if let Some(row) = combined_row {
    let extra = row.alpha_at_offset(offset);
    if extra == 0 {
      return;
    }
    src = scale_premultiplied_pixel(src, extra);
    if src[3] == 0 {
      return;
    }
  }

  blend_premultiplied_pixel(dst, src, mode);
}

#[cfg(test)]
mod scaled_rows_tests {
  use tiny_skia::{Pixmap, PremultipliedColorU8};

  use super::*;
  use crate::style::ImageScalingAlgorithm;

  fn source() -> Pixmap {
    let mut source = Pixmap::new(7, 5).unwrap();
    for (i, pixel) in source.pixels_mut().iter_mut().enumerate() {
      let alpha = [255u8, 200, 90, 0, 255][i % 5];
      let channel = |k: usize| ((i * 37 + k * 11) % 256) as u8;
      *pixel = PremultipliedColorU8::from_rgba(
        channel(1).min(alpha),
        channel(2).min(alpha),
        channel(3).min(alpha),
        alpha,
      )
      .unwrap();
    }
    source
  }

  fn render(
    scaled: bool,
    transform: Affine,
    mode: BlendMode,
    color_mode: MaskCompositeColor,
  ) -> Vec<u8> {
    let source = source();
    let mut canvas = Pixmap::new(40, 30).unwrap();
    for (i, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
      *pixel = PremultipliedColorU8::from_rgba(0, (i % 100) as u8, 20, 120).unwrap();
    }
    let mask: Vec<u8> = (0..36 * 26)
      .map(|i| match i % 7 {
        0 => 0,
        1 => 128,
        2 => 17,
        _ => 255,
      })
      .collect();
    let placement = Placement {
      left: 2,
      top: 3,
      width: 36,
      height: 26,
    };
    let options = Options {
      placement,
      sampling: MaskSamplingOptions {
        canvas_to_source: transform,
        sample_bias: Point { x: 0.5, y: 0.5 },
        algorithm: ImageScalingAlgorithm::Auto,
      },
      color_mode,
      mode,
      combined_mask: None,
    };
    let bounds =
      compute_overlay_bounds_for_canvas(40, 30, Point { x: 2.0, y: 3.0 }, 36, 26).unwrap();
    let region = DestRegion {
      bounds,
      canvas_width: 40,
      mask_stride: 36,
    };
    let mut pixmap = canvas.as_mut();
    if scaled {
      let rows = ScaledRows::new(
        PaintSource::Pixmap(source.as_ref()),
        transform,
        ImageScalingAlgorithm::Auto,
        bounds.x_min as f32 + 0.5,
        36,
      )
      .unwrap();
      source_scaled_rows(&mut pixmap, &mask, &rows, &options, region);
    } else {
      let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
      PixelSampler {
        resolved: PaintSource::Pixmap(source.as_ref()).resolve(),
        transform,
        algorithm: ImageScalingAlgorithm::Auto,
        color_mode,
        mode,
        combined_mask: None,
      }
      .sample_into(
        pixels,
        40,
        bounds,
        |dest_y| transform.transform_point(bounds.x_min as f32 + 0.5, dest_y as f32 + 0.5),
        |dest_y, dest_x| mask[(dest_y - 3) as usize * 36 + (dest_x - 2) as usize],
      );
    }
    canvas.data().to_vec()
  }

  #[test]
  fn scaled_rows_match_per_pixel_sampling() {
    for (a, d, x, y) in [
      (0.19, 0.17, 0.0, 0.0),
      (0.5, 0.25, -0.3, 0.7),
      (1.0, 1.0, 0.5, 0.5),
      (0.999, 0.31, 3.2, -1.1),
      (0.0625, 0.9, 1.0, 1.0),
      (-0.5, 0.75, 6.0, 0.0),
      (0.4, -0.3, 0.0, 4.5),
    ] {
      let transform = Affine {
        a,
        b: 0.0,
        c: 0.0,
        d,
        x,
        y,
      };
      for mode in [BlendMode::Normal, BlendMode::Multiply] {
        for color_mode in [
          MaskCompositeColor::SourceOnly,
          MaskCompositeColor::SourceOverColor([255, 0, 0, 255]),
          MaskCompositeColor::ColorOverSource([0, 40, 0, 40]),
        ] {
          assert_eq!(
            render(true, transform, mode, color_mode),
            render(false, transform, mode, color_mode),
            "a={a} d={d} x={x} y={y} mode={mode:?} color_mode={color_mode:?}"
          );
        }
      }
    }
  }
}

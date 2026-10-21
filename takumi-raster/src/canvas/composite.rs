use std::ops::Range;

use takumi_core::geometry::{Point, Size};
use tiny_skia::PixmapMut;

use super::{
  MASK_RUN, MaskSamplingOptions, MaskView, PaintSource, SamplingFootprint,
  blit::{OverlayBounds, apply_mask_row, composite_run_over},
  mask::MaskRow,
  paint_source::{ResolvedSource, ScaledRows, sample_paint_source},
  whole_pixel_translation,
};
use crate::{
  Placement,
  blend::{blend_premultiplied_pixel, composite_premultiplied_over_row, scale_premultiplied_pixel},
  mul_div_255,
  style::{Affine, BlendMode, ImageScalingAlgorithm},
};

#[derive(Clone, Copy)]
struct DestRegion {
  bounds: OverlayBounds,
  canvas_width: usize,
  mask_stride: usize,
}

impl DestRegion {
  /// The part of `placement` inside a `canvas`-sized pixmap, or `None` when
  /// they do not overlap.
  fn new(canvas: Size<u32>, placement: Placement) -> Option<Self> {
    let bounds = OverlayBounds::new(
      canvas,
      Point {
        x: placement.left as f32,
        y: placement.top as f32,
      },
      Size::new(placement.width, placement.height),
    )?;

    Some(Self {
      bounds,
      canvas_width: canvas.width as usize,
      mask_stride: placement.width as usize,
    })
  }

  fn span(&self) -> usize {
    (self.bounds.x_max - self.bounds.x_min) as usize
  }

  fn dst_row(&self, dest_y: i32) -> Range<usize> {
    let start = dest_y as usize * self.canvas_width + self.bounds.x_min as usize;

    start..start + self.span()
  }

  fn mask_row(&self, dest_y: i32) -> Range<usize> {
    let start = (dest_y - self.bounds.offset_y) as usize * self.mask_stride
      + (self.bounds.x_min - self.bounds.offset_x) as usize;

    start..start + self.span()
  }
}

#[derive(Clone, Copy)]
pub(super) struct Options<'a> {
  pub placement: Placement,
  pub sampling: MaskSamplingOptions,
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

  let Some(region) = DestRegion::new(Size::new(pixmap.width(), pixmap.height()), placement) else {
    return;
  };
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  if mode == BlendMode::Normal {
    match combined_mask {
      None => constant_normal(pixels, mask, color, region),
      Some(clip) => constant_clipped(pixels, mask, color, clip, region),
    }
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
      color,
      options.mode,
      options.combined_mask,
    );
    return;
  }

  let Some(region) = DestRegion::new(
    Size::new(pixmap.width(), pixmap.height()),
    options.placement,
  ) else {
    return;
  };

  if options.mode == BlendMode::Normal
    && try_translation_blit(pixmap, mask, source, &options, region)
  {
    return;
  }

  source_general(pixmap, mask, source, &options, region);
}

#[inline]
fn constant_normal(pixels: &mut [[u8; 4]], mask: &[u8], color: [u8; 4], region: DestRegion) {
  for dest_y in region.bounds.y_min..region.bounds.y_max {
    let dst = &mut pixels[region.dst_row(dest_y)];
    let mask_row = &mask[region.mask_row(dest_y)];
    let (dst_runs, dst_tail) = dst.as_chunks_mut::<MASK_RUN>();
    let (mask_runs, mask_tail) = mask_row.as_chunks::<MASK_RUN>();

    for (dst, mask) in dst_runs.iter_mut().zip(mask_runs) {
      match u64::from_ne_bytes(*mask) {
        0 => {}
        u64::MAX if color[3] == u8::MAX => dst.fill(color),
        _ => constant_over(dst, mask, color),
      }
    }
    constant_over(dst_tail, mask_tail, color);
  }
}

/// `color` scaled by `mask` over `dst`, both at most a [`MASK_RUN`] long.
#[inline(always)]
fn constant_over(dst: &mut [[u8; 4]], mask: &[u8], color: [u8; 4]) {
  composite_run_over(
    dst,
    mask
      .iter()
      .map(|&alpha| scale_premultiplied_pixel(color, alpha)),
  );
}

/// [`constant_general`]'s normal-mode source-over under `clip`, a run of pixels at a time.
fn constant_clipped(
  pixels: &mut [[u8; 4]],
  mask: &[u8],
  color: [u8; 4],
  clip: MaskView<'_>,
  region: DestRegion,
) {
  for dest_y in region.bounds.y_min..region.bounds.y_max {
    let (first, clip_row) = clip.row(dest_y, region.bounds.x_min).covered(region.span());
    let dst = &mut pixels[region.dst_row(dest_y)][first..][..clip_row.len()];
    let mask_row = &mask[region.mask_row(dest_y)][first..][..clip_row.len()];
    let (dst_runs, dst_tail) = dst.as_chunks_mut::<MASK_RUN>();
    let (mask_runs, mask_tail) = mask_row.as_chunks::<MASK_RUN>();
    let (clip_runs, clip_tail) = clip_row.as_chunks::<MASK_RUN>();

    for ((dst, mask), clip) in dst_runs.iter_mut().zip(mask_runs).zip(clip_runs) {
      match (u64::from_ne_bytes(*mask), u64::from_ne_bytes(*clip)) {
        (0, _) | (_, 0) => {}
        (u64::MAX, u64::MAX) if color[3] == u8::MAX => dst.fill(color),
        _ => constant_clipped_over(dst, mask, clip, color),
      }
    }
    constant_clipped_over(dst_tail, mask_tail, clip_tail, color);
  }
}

/// `color` scaled by `mask`, then by `clip`, over `dst`, all at most a [`MASK_RUN`] long.
#[inline(always)]
fn constant_clipped_over(dst: &mut [[u8; 4]], mask: &[u8], clip: &[u8], color: [u8; 4]) {
  composite_run_over(
    dst,
    mask.iter().zip(clip).map(|(&alpha, &clip)| {
      scale_premultiplied_pixel(scale_premultiplied_pixel(color, alpha), clip)
    }),
  );
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
  let Some(sample_offset) = whole_pixel_translation(options.sampling.canvas_to_source) else {
    return false;
  };
  if options.sampling.sample_bias != (Point { x: 0.5, y: 0.5 }) {
    return false;
  }
  let Some(source_pixmap) = source.as_pixmap_ref() else {
    return false;
  };

  let bounds = region.bounds;
  let source_width = source_pixmap.width() as i32;
  let source_height = source_pixmap.height() as i32;
  let source_pixels: &[[u8; 4]] = bytemuck::cast_slice(source_pixmap.pixels());
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let sample_dx = sample_offset.x as i32;
  let sample_dy = sample_offset.y as i32;
  // The destination columns whose source column lies inside the source.
  let x_min = bounds.x_min.max(-sample_dx);
  let x_max = bounds.x_max.min(source_width - sample_dx);

  if x_min >= x_max {
    return true;
  }

  let skip = (x_min - bounds.x_min) as usize;
  let len = (x_max - x_min) as usize;
  let mut alphas = vec![0u8; len];
  let mut sources = vec![[0u8; 4]; len];

  for dest_y in bounds.y_min..bounds.y_max {
    let src_y = dest_y + sample_dy;

    if src_y < 0 || src_y >= source_height {
      continue;
    }

    let mask_row = &mask[region.mask_row(dest_y)][skip..skip + len];

    match options
      .combined_mask
      .map(|view| view.row(dest_y, bounds.x_min))
    {
      Some(clip_row) => {
        clip_row.write_alphas(skip, &mut alphas);
        for (alpha, &mask_alpha) in alphas.iter_mut().zip(mask_row) {
          *alpha = mul_div_255(mask_alpha, *alpha);
        }
      }
      None => alphas.copy_from_slice(mask_row),
    }

    let src_start = src_y as usize * source_width as usize + (x_min + sample_dx) as usize;

    for ((source, &pixel), &alpha) in sources
      .iter_mut()
      .zip(&source_pixels[src_start..src_start + len])
      .zip(&alphas)
    {
      *source = scale_premultiplied_pixel(pixel, alpha);
    }
    composite_premultiplied_over_row(
      &mut pixels[region.dst_row(dest_y)][skip..skip + len],
      &sources,
    );
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
  let bounds = region.bounds;
  let bias = options.sampling.sample_bias;
  let resolved = source.resolve();

  if let Some(rows) = ScaledRows::new(
    resolved,
    options.sampling.canvas_to_source,
    options.sampling.algorithm,
    bounds.x_min as f32 + bias.x,
    region.span(),
  ) {
    source_scaled_rows(pixmap, mask, &rows, options, region);

    return;
  }

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

  PixelSampler {
    resolved,
    transform: options.sampling.canvas_to_source,
    algorithm: options.sampling.algorithm,
    mode: options.mode,
    combined_mask: options.combined_mask,
  }
  .sample_into(
    pixels,
    region.canvas_width,
    bounds,
    |dest_y| {
      options
        .sampling
        .canvas_to_source
        .transform_point(bounds.x_min as f32 + bias.x, dest_y as f32 + bias.y)
    },
    |dest_y, dest_x| mask[region.mask_row(dest_y).start + (dest_x - bounds.x_min) as usize],
  );
}

/// Per-pixel sampling from a transformed row start and per-pixel increments.
pub(super) struct PixelSampler<'a> {
  pub resolved: ResolvedSource<'a>,
  pub transform: Affine,
  pub algorithm: ImageScalingAlgorithm,
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
            self.mode,
          );
        }
      }
    }
  }
}

fn source_scaled_rows(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  rows: &ScaledRows<'_>,
  options: &Options<'_>,
  region: DestRegion,
) {
  let bounds = region.bounds;
  let mut row = vec![[0u8; 4]; region.span()];
  let mut clip_alphas = vec![0u8; region.span()];
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

  for dest_y in bounds.y_min..bounds.y_max {
    let combined_row = options
      .combined_mask
      .map(|view| view.row(dest_y, bounds.x_min));

    if combined_row.is_some_and(|row| row.is_empty()) {
      continue;
    }

    rows.fill(dest_y as f32 + options.sampling.sample_bias.y, &mut row);

    for (pixel, &mask_alpha) in row.iter_mut().zip(&mask[region.mask_row(dest_y)]) {
      *pixel = scale_premultiplied_pixel(*pixel, mask_alpha);
    }
    if let Some(clip_row) = combined_row {
      clip_row.write_alphas(0, &mut clip_alphas);
      for (pixel, &alpha) in row.iter_mut().zip(&clip_alphas) {
        *pixel = scale_premultiplied_pixel(*pixel, alpha);
      }
    }

    let dst_row = &mut pixels[region.dst_row(dest_y)];

    if options.mode == BlendMode::Normal {
      composite_premultiplied_over_row(dst_row, &row);
      continue;
    }

    for (dst, &src) in dst_row.iter_mut().zip(&row) {
      blend_premultiplied_pixel(dst, src, options.mode);
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
  mode: BlendMode,
) {
  let src = scale_premultiplied_pixel(src, mask_alpha);

  if src[3] == 0 {
    return;
  }

  let Some(src) = apply_mask_row(src, combined_row, offset) else {
    return;
  };

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

  fn render(scaled: bool, transform: Affine, mode: BlendMode) -> Vec<u8> {
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
      mode,
      combined_mask: None,
    };

    let region = DestRegion::new(Size::new(canvas.width(), canvas.height()), placement).unwrap();
    let bounds = region.bounds;

    let mut pixmap = canvas.as_mut();

    if scaled {
      let rows = ScaledRows::new(
        PaintSource::Pixmap(source.as_ref()).resolve(),
        transform,
        options.sampling.algorithm,
        bounds.x_min as f32 + 0.5,
        region.span(),
      )
      .unwrap();
      source_scaled_rows(&mut pixmap, &mask, &rows, &options, region);
    } else {
      let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
      PixelSampler {
        resolved: PaintSource::Pixmap(source.as_ref()).resolve(),
        transform,
        algorithm: options.sampling.algorithm,
        mode,
        combined_mask: None,
      }
      .sample_into(
        pixels,
        region.canvas_width,
        bounds,
        |dest_y| transform.transform_point(bounds.x_min as f32 + 0.5, dest_y as f32 + 0.5),
        |dest_y, dest_x| mask[region.mask_row(dest_y).start + (dest_x - bounds.x_min) as usize],
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
        assert_eq!(
          render(true, transform, mode),
          render(false, transform, mode),
          "a={a} d={d} x={x} y={y} mode={mode:?}"
        );
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use takumi_core::geometry::{Point, Size};
  use tiny_skia::{IntSize, Mask};

  use super::{DestRegion, MaskView, constant_clipped, constant_general, constant_normal};
  use crate::{Placement, style::BlendMode};

  /// Bytes from a fixed hash of `index`, so every run sees the same noise.
  fn noise(index: usize, salt: u32) -> u8 {
    ((index as u32 ^ salt).wrapping_mul(2_654_435_761) >> 13) as u8
  }

  /// Coverage that runs empty or full, as glyph and clip masks do.
  fn coverage(index: usize, salt: u32) -> u8 {
    match noise(index / 16, salt) % 4 {
      0 => 0,
      1 => u8::MAX,
      _ => noise(index, salt + 1),
    }
  }

  #[test]
  fn constant_runs_match_the_pixel_path() {
    let canvas = Size::new(23, 17);
    let clip = Mask::from_vec(
      (0..20 * 15).map(|index| coverage(index, 7)).collect(),
      IntSize::from_wh(20, 15).unwrap(),
    )
    .unwrap();

    for (case, (left, top)) in [(0, 0), (-3, 2), (5, -4), (9, 6)].into_iter().enumerate() {
      let placement = Placement {
        left,
        top,
        width: 16,
        height: 12,
      };
      let region = || DestRegion::new(canvas, placement).unwrap();
      let mask: Vec<u8> = (0..16 * 12).map(|index| coverage(index, 3)).collect();
      let alpha = if case % 2 == 0 { u8::MAX } else { 200 };
      let color = [noise(case, 1), noise(case, 2), 40, alpha].map(|channel| channel.min(alpha));
      let view = MaskView {
        mask: &clip,
        origin: Point { x: 2, y: 1 },
        canvas_origin: Point { x: 0, y: 0 },
      };
      let canvas_pixels = (canvas.width * canvas.height) as usize;
      // Every fifth pixel is transparent but keeps colour, which only an exact path leaves alone.
      let dst: Vec<[u8; 4]> = (0..canvas_pixels)
        .map(|index| {
          let [red, green, blue, alpha] = [0, 1, 2, 3].map(|lane| noise(index * 4 + lane, 5));

          [
            red,
            green,
            blue,
            if index.is_multiple_of(5) { 0 } else { alpha },
          ]
        })
        .collect();
      let mut expected = dst.clone();
      let mut actual = dst.clone();

      constant_general(
        &mut expected,
        &mask,
        color,
        BlendMode::Normal,
        Some(view),
        region(),
      );
      constant_clipped(&mut actual, &mask, color, view, region());
      assert_eq!(actual, expected, "clipped case {case}");

      let mut expected = dst.clone();
      let mut actual = dst;

      constant_general(
        &mut expected,
        &mask,
        color,
        BlendMode::Normal,
        None,
        region(),
      );
      constant_normal(&mut actual, &mask, color, region());
      assert_eq!(actual, expected, "unclipped case {case}");
    }
  }
}

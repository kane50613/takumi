//! Free blitters and the image/mask overlay dispatchers that paint onto a
//! [`DrawTarget`].

use std::array;

use image::Rgba;
use takumi_core::geometry::{Point, Size};
use tiny_skia::{PixmapMut, PixmapRef, PremultipliedColorU8};

use super::{
  DrawTarget, MASK_RUN, MaskSamplingOptions, MaskView, OverlayOptions, PaintSource,
  SamplingOptions, composite,
  composite::PixelSampler,
  mask::{MaskRow, render_cached_mask},
  paint_source::{RowSource, ScaledRows},
  skia::{
    FillColorOptions, ImagePathFillOptions, try_draw_image_with_tiny_skia,
    try_fill_color_with_tiny_skia, try_fill_image_path_with_tiny_skia,
  },
};
use crate::{
  Placement,
  blend::*,
  resources::image_buffer::premultiply_pixel,
  style::{Affine, BlendMode, ImageScalingAlgorithm},
};

/// The clipped destination region of an overlay: `offset_*` is the overlay's
/// top-left in canvas space, `x/y` bounds are the canvas-space pixel range to
/// write.
#[derive(Clone, Copy)]
pub(crate) struct OverlayBounds {
  pub offset_x: i32,
  pub offset_y: i32,
  pub x_min: i32,
  pub x_max: i32,
  pub y_min: i32,
  pub y_max: i32,
}

#[derive(Clone, Copy)]
pub(crate) struct PlacementOverlap {
  pub(crate) placement: Placement,
  pub(crate) lhs_offset: Point<u32>,
  pub(crate) rhs_offset: Point<u32>,
}

pub(crate) fn placement_overlap(lhs: Placement, rhs: Placement) -> Option<PlacementOverlap> {
  let placement = Placement::from_bounds(
    lhs.left.max(rhs.left),
    lhs.top.max(rhs.top),
    lhs.right().min(rhs.right()),
    lhs.bottom().min(rhs.bottom()),
  )?;

  Some(PlacementOverlap {
    lhs_offset: Point::new(
      (placement.left - lhs.left) as u32,
      (placement.top - lhs.top) as u32,
    ),
    rhs_offset: Point::new(
      (placement.left - rhs.left) as u32,
      (placement.top - rhs.top) as u32,
    ),
    placement,
  })
}

impl OverlayBounds {
  /// Clips a `size` overlay at `offset` to a `canvas`-sized pixmap, or `None`
  /// when nothing of it lands inside.
  #[inline(always)]
  pub(crate) fn new(canvas: Size<u32>, offset: Point<f32>, size: Size<u32>) -> Option<Self> {
    Self::at_whole_pixel(
      canvas,
      Point {
        x: offset.x.trunc() as i32,
        y: offset.y.trunc() as i32,
      },
      size,
    )
  }

  /// [`Self::new`] for an `offset` already in whole pixels, which `f32` cannot hold past 2^24.
  #[inline(always)]
  pub(crate) fn at_whole_pixel(
    canvas: Size<u32>,
    offset: Point<i32>,
    size: Size<u32>,
  ) -> Option<Self> {
    if size.width == 0 || size.height == 0 {
      return None;
    }

    let clipped = Placement {
      left: offset.x,
      top: offset.y,
      width: size.width,
      height: size.height,
    }
    .clamp_to(canvas)?;

    Some(Self {
      offset_x: offset.x,
      offset_y: offset.y,
      x_min: clipped.left,
      x_max: clipped.right(),
      y_min: clipped.top,
      y_max: clipped.bottom(),
    })
  }
}

#[inline(always)]
pub(crate) fn apply_mask_row(
  src: [u8; 4],
  row: Option<MaskRow<'_>>,
  offset: usize,
) -> Option<[u8; 4]> {
  let Some(row) = row else {
    return Some(src);
  };
  let alpha = row.alpha_at_offset(offset);
  if alpha == 0 {
    return None;
  }
  let src = scale_premultiplied_pixel(src, alpha);
  if src[3] == 0 {
    return None;
  }
  Some(src)
}

fn blit_sampled_paint_source_translation(
  pixmap: &mut PixmapMut<'_>,
  source: PaintSource<'_>,
  size: Size<u32>,
  offset: Point<f32>,
  sampling: SamplingOptions,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if sampling.logical_to_source.is_identity()
    && size.width == source.width()
    && size.height == source.height()
  {
    blit_paint_source_translation(pixmap, source, offset, mode, combined_mask);
    return;
  }

  let canvas_width = pixmap.width();
  let Some(bounds) = OverlayBounds::new(Size::new(canvas_width, pixmap.height()), offset, size)
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let resolved = source.resolve();

  if let Some(rows) = ScaledRows::new(
    resolved,
    sampling.logical_to_source,
    sampling.algorithm,
    (bounds.x_min - bounds.offset_x) as f32 + 0.5,
    (bounds.x_max - bounds.x_min) as usize,
  ) {
    blit_rows(
      pixels,
      canvas_width,
      bounds,
      mode,
      combined_mask,
      |src_y, row| {
        rows.fill(src_y as f32 + 0.5, row);
      },
    );

    return;
  }

  PixelSampler {
    resolved,
    transform: sampling.logical_to_source,
    algorithm: sampling.algorithm,
    mode,
    combined_mask,
  }
  .sample_into(
    pixels,
    canvas_width as usize,
    bounds,
    |dest_y| {
      sampling.logical_to_source.transform_point(
        (bounds.x_min - bounds.offset_x) as f32 + 0.5,
        (dest_y - bounds.offset_y) as f32 + 0.5,
      )
    },
    |_, _| u8::MAX,
  );
}

/// Blends `bounds` row by row; `fill` produces each source-local row, premultiplied.
pub(super) fn blit_rows(
  pixels: &mut [[u8; 4]],
  canvas_width: u32,
  bounds: OverlayBounds,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
  mut fill: impl FnMut(u32, &mut [[u8; 4]]),
) {
  let mut row = vec![[0u8; 4]; (bounds.x_max - bounds.x_min).max(0) as usize];

  for dest_y in bounds.y_min..bounds.y_max {
    let mask_row = combined_mask.map(|view| view.row(dest_y, bounds.x_min));
    if mask_row.is_some_and(|row| row.is_empty()) {
      continue;
    }

    fill((dest_y - bounds.offset_y) as u32, &mut row);

    let dst_start = dest_y as usize * canvas_width as usize + bounds.x_min as usize;
    let dst = &mut pixels[dst_start..dst_start + row.len()];

    let Some(mask_row) = mask_row else {
      blend_row(dst, &row, mode);
      continue;
    };
    let (first, alphas) = mask_row.covered(row.len());
    let dst = &mut dst[first..first + alphas.len()];
    let row = &row[first..first + alphas.len()];
    let (dst_runs, dst_tail) = dst.as_chunks_mut::<MASK_RUN>();
    let (row_runs, row_tail) = row.as_chunks::<MASK_RUN>();
    let (alpha_runs, alpha_tail) = alphas.as_chunks::<MASK_RUN>();

    for ((dst, row), alphas) in dst_runs.iter_mut().zip(row_runs).zip(alpha_runs) {
      match u64::from_ne_bytes(*alphas) {
        0 => {}
        u64::MAX => blend_row(dst, row, mode),
        _ => blend_masked(dst, row, alphas, mode),
      }
    }
    blend_masked(dst_tail, row_tail, alpha_tail, mode);
  }
}

fn blend_masked(dst: &mut [[u8; 4]], row: &[[u8; 4]], alphas: &[u8], mode: BlendMode) {
  if mode == BlendMode::Normal {
    composite_run_over(
      dst,
      row
        .iter()
        .zip(alphas)
        .map(|(&src, &alpha)| scale_premultiplied_pixel(src, alpha)),
    );
    return;
  }

  for ((dst, &src), &alpha) in dst.iter_mut().zip(row).zip(alphas) {
    if src[3] != 0 && alpha != 0 {
      blend_premultiplied_pixel(dst, scale_premultiplied_pixel(src, alpha), mode);
    }
  }
}

/// `sources` over `dst`, at most a [`MASK_RUN`] long, gathered first so the compositing
/// vectorizes.
#[inline(always)]
pub(super) fn composite_run_over(dst: &mut [[u8; 4]], sources: impl Iterator<Item = [u8; 4]>) {
  let mut run = [[0; 4]; MASK_RUN];

  for (slot, source) in run.iter_mut().zip(sources) {
    *slot = source;
  }
  composite_premultiplied_over_row(dst, &run[..dst.len()]);
}

/// `row` over `dst` in `mode`, the row compositor's vectorized source-over in normal mode.
fn blend_row(dst: &mut [[u8; 4]], row: &[[u8; 4]], mode: BlendMode) {
  if mode == BlendMode::Normal {
    composite_premultiplied_over_row(dst, row);
    return;
  }

  for (dst, &src) in dst.iter_mut().zip(row) {
    blend_premultiplied_pixel(dst, src, mode);
  }
}

pub(super) fn blit_paint_source_translation(
  pixmap: &mut PixmapMut<'_>,
  source: PaintSource<'_>,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if let Some(color) = source.premultiplied_constant() {
    blit_solid_translation(
      pixmap,
      source.width(),
      source.height(),
      color,
      offset,
      mode,
      combined_mask,
    );
    return;
  }

  let canvas_width = pixmap.width();
  let Some(bounds) = OverlayBounds::new(
    Size::new(canvas_width, pixmap.height()),
    offset,
    Size::new(source.width(), source.height()),
  ) else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let rows = source.rows(
    (bounds.x_min - bounds.offset_x) as u32,
    (bounds.x_max - bounds.x_min) as u32,
  );
  match rows {
    RowSource::Copy { source, x_start } if mode == BlendMode::Normal && combined_mask.is_none() => {
      let source_pixels = source.pixels();
      let source_width = source.width() as usize;
      let copy_width = (bounds.x_max - bounds.x_min) as usize;
      for dest_y in bounds.y_min..bounds.y_max {
        let src_start = (dest_y - bounds.offset_y) as usize * source_width + x_start as usize;
        let dst_start = (dest_y as u32 * canvas_width + bounds.x_min as u32) as usize;
        let dst = bytemuck::cast_slice_mut(&mut pixels[dst_start..dst_start + copy_width]);
        composite_premultiplied_over_span(dst, &source_pixels[src_start..src_start + copy_width]);
      }
    }
    rows => blit_rows(
      pixels,
      canvas_width,
      bounds,
      mode,
      combined_mask,
      |src_y, row| rows.fill(src_y, row),
    ),
  }
}

/// Composites `source` whole-pixel at `offset` over `pixmap` at `opacity`, exactly as tiny-skia's
/// high-precision pipeline draws a source-over pattern there: each channel scaled to `[0, 1]`, the
/// source multiplied by `opacity`, `dst * (1 - source alpha) + source`, rounded half to even.
pub(super) fn blit_translucent_translation(
  pixmap: &mut PixmapMut<'_>,
  source: PixmapRef<'_>,
  offset: Point<i32>,
  opacity: f32,
) {
  let canvas_width = pixmap.width() as usize;
  let Some(bounds) = OverlayBounds::at_whole_pixel(
    Size::new(pixmap.width(), pixmap.height()),
    offset,
    Size::new(source.width(), source.height()),
  ) else {
    return;
  };
  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let source_pixels: &[[u8; 4]] = bytemuck::cast_slice(source.pixels());
  let source_width = source.width() as usize;
  let width = (bounds.x_max - bounds.x_min) as usize;

  for dest_y in bounds.y_min..bounds.y_max {
    let source_start = (dest_y - bounds.offset_y) as usize * source_width
      + (bounds.x_min - bounds.offset_x) as usize;
    let dest_start = dest_y as usize * canvas_width + bounds.x_min as usize;

    for (dest, &source) in pixels[dest_start..dest_start + width]
      .iter_mut()
      .zip(&source_pixels[source_start..source_start + width])
    {
      *dest = source_over_at_opacity(*dest, source, opacity);
    }
  }
}

/// One pixel of [`blit_translucent_translation`].
#[inline(always)]
fn source_over_at_opacity(dest: [u8; 4], source: [u8; 4], opacity: f32) -> [u8; 4] {
  const UNIT: f32 = 1.0 / 255.0;

  let source = source.map(|channel| f32::from(channel) * UNIT * opacity);
  let behind = 1.0 - source[3];

  array::from_fn(|channel| {
    let blended = f32::from(dest[channel]) * UNIT * behind + source[channel];

    (blended.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8
  })
}

fn blit_solid_translation(
  pixmap: &mut PixmapMut<'_>,
  source_width: u32,
  source_height: u32,
  color: [u8; 4],
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if color[3] == 0 {
    return;
  }

  let canvas_width = pixmap.width();
  let Some(bounds) = OverlayBounds::new(
    Size::new(canvas_width, pixmap.height()),
    offset,
    Size::new(source_width, source_height),
  ) else {
    return;
  };

  let data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let row_stride = canvas_width as usize * 4;
    let x_byte_start = bounds.x_min as usize * 4;
    let x_byte_end = bounds.x_max as usize * 4;
    for dest_y in bounds.y_min..bounds.y_max {
      let row_start = dest_y as usize * row_stride;
      let row = &mut data[row_start + x_byte_start..row_start + x_byte_end];
      if color[3] == u8::MAX {
        fill_repeated_premultiplied_pixel(row, color);
      } else {
        blend_repeated_premultiplied_pixel(
          row,
          PremultipliedColorU8::from_rgba(color[0], color[1], color[2], color[3])
            .unwrap_or(PremultipliedColorU8::TRANSPARENT),
        );
      }
    }
    return;
  }

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(data);
  blit_rows(
    pixels,
    canvas_width,
    bounds,
    mode,
    combined_mask,
    |_, row| row.fill(color),
  );
}

pub(crate) fn draw_mask(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  placement: Placement,
  color: Rgba<u8>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  if mask.is_empty() {
    return;
  }

  assert_eq!(
    mask.len(),
    placement.width as usize * placement.height as usize,
  );

  composite::constant(
    pixmap,
    mask,
    placement,
    premultiply_pixel(color.0),
    mode,
    combined_mask,
  );
}

pub(crate) fn overlay_image<'a, I: Into<PaintSource<'a>>>(
  target: &mut DrawTarget,
  image: I,
  options: OverlayOptions,
) {
  let image = image.into();
  let content_size = Size {
    width: image.width(),
    height: image.height(),
  };

  if let PaintSource::ColorTile(color) = image
    && try_fill_color_with_tiny_skia(
      target,
      FillColorOptions {
        color,
        content_size,
        border: options.border,
        transform: options.transform,
        mode: options.mode,
      },
    )
  {
    return;
  }

  if let Some(offset) = options.whole_pixel_translation() {
    blit_paint_source_translation(
      target.pixmap,
      image,
      offset,
      options.mode,
      target.combined_mask,
    );
    return;
  }

  if options.border.is_zero()
    && try_draw_image_with_tiny_skia(
      target,
      image,
      options.transform,
      options.algorithm,
      options.mode,
    )
  {
    return;
  }

  if !options.border.is_zero()
    && image.supports_rounded_fill_fast_path()
    && try_fill_image_path_with_tiny_skia(
      target,
      image,
      ImagePathFillOptions {
        content_size,
        border: options.border,
        transform: options.transform,
        source_to_canvas: Affine {
          x: 0.0,
          y: 0.0,
          ..options.transform
        },
        algorithm: options.algorithm,
        mode: options.mode,
      },
    )
  {
    return;
  }

  composite_bordered_source(
    target,
    image,
    content_size,
    Affine::IDENTITY,
    options.algorithm,
    options,
  );
}

/// Shared slow path for bordered/transformed overlays: renders the border mask
/// and composites `source` through it, mapping canvas coordinates back to the
/// source via `logical_to_source` (and the transform's inverse when the
/// transform is not identity).
fn composite_bordered_source(
  target: &mut DrawTarget,
  source: PaintSource<'_>,
  size: Size<u32>,
  logical_to_source: Affine,
  algorithm: ImageScalingAlgorithm,
  options: OverlayOptions,
) {
  let mut paths = Vec::new();
  options
    .border
    .append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);

  let (mask, placement) = render_cached_mask(
    &paths,
    size.map(|v| v as f32),
    options.transform,
    target.viewport(),
  );
  let canvas_to_source =
    if options.transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
      Some(logical_to_source)
    } else {
      options
        .transform
        .invert()
        .map(|inverse| logical_to_source * inverse)
    };

  if let Some(canvas_to_source) = canvas_to_source {
    composite::source(
      target.pixmap,
      &mask,
      source,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm,
        },
        mode: options.mode,
        combined_mask: target.combined_mask,
      },
    );
  }
}

pub(crate) fn overlay_sampled_paint_source(
  target: &mut DrawTarget,
  source: PaintSource<'_>,
  size: Size<u32>,
  options: OverlayOptions,
  sampling: SamplingOptions,
) {
  // A whole-pixel translation copies row spans directly. Tiny-skia would take
  // the same draw through its sampling pipeline, so this is tried first.
  if let Some(offset) = options.whole_pixel_translation() {
    blit_sampled_paint_source_translation(
      target.pixmap,
      source,
      size,
      offset,
      sampling,
      options.mode,
      target.combined_mask,
    );
    return;
  }

  let direct_identity_mapping = options.border.is_zero()
    && sampling.logical_to_source.is_identity()
    && size.width == source.width()
    && size.height == source.height();

  if direct_identity_mapping
    && try_draw_image_with_tiny_skia(
      target,
      source,
      options.transform,
      options.algorithm,
      options.mode,
    )
  {
    return;
  }

  composite_bordered_source(
    target,
    source,
    size,
    sampling.logical_to_source,
    sampling.algorithm,
    options,
  );
}

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};
  use takumi_core::geometry::Size;
  use tiny_skia::{
    BlendMode as TinyBlendMode, FilterQuality, Mask as TinyMask, Pixmap, PixmapPaint, PixmapRef,
    PremultipliedColorU8, Transform,
  };

  use super::*;
  use crate::{
    BorderProperties, Canvas, PaintSource, Result, pixmap_from_buffer,
    resources::image_buffer::ImageBuffer, style::ImageScalingAlgorithm,
  };

  /// Bytes from a fixed hash of `index`, so every run sees the same noise.
  fn noise(index: usize, salt: u32) -> u8 {
    ((index as u32 ^ salt).wrapping_mul(2_654_435_761) >> 13) as u8
  }

  #[test]
  fn normal_rows_match_the_pixel_path() {
    let canvas = Size::new(29, 13);
    let clip = TinyMask::from_vec(
      (0..24 * 11)
        .map(|index| match noise(index / 12, 1) % 3 {
          0 => 0,
          1 => u8::MAX,
          _ => noise(index, 2),
        })
        .collect(),
      tiny_skia::IntSize::from_wh(24, 11).unwrap(),
    )
    .unwrap();
    let view = MaskView {
      mask: &clip,
      origin: Point { x: 1, y: 0 },
      canvas_origin: Point { x: 0, y: 0 },
    };
    // Sources and destinations both include transparent pixels that keep colour.
    let pixel = |index: usize, salt: u32| {
      let [red, green, blue, alpha] = [0, 1, 2, 3].map(|lane| noise(index * 4 + lane, salt));

      [
        red,
        green,
        blue,
        if index.is_multiple_of(6) { 0 } else { alpha },
      ]
    };
    let bounds = OverlayBounds::new(canvas, Point { x: -2.0, y: 1.0 }, Size::new(27, 14)).unwrap();
    let start: Vec<[u8; 4]> = (0..29 * 13).map(|index| pixel(index, 3)).collect();
    let fill = |y: u32, row: &mut [[u8; 4]]| {
      for (x, src) in row.iter_mut().enumerate() {
        *src = pixel(y as usize * 31 + x, 4);
      }
    };

    for clip in [None, Some(view)] {
      let mut actual = start.clone();
      let mut expected = start.clone();

      blit_rows(
        &mut actual,
        canvas.width,
        bounds,
        BlendMode::Normal,
        clip,
        fill,
      );
      for dest_y in bounds.y_min..bounds.y_max {
        let mut row = vec![[0; 4]; (bounds.x_max - bounds.x_min) as usize];

        fill((dest_y - bounds.offset_y) as u32, &mut row);
        for (offset, &src) in row.iter().enumerate() {
          let dst = &mut expected[dest_y as usize * 29 + bounds.x_min as usize + offset];
          let alpha = clip.map_or(u8::MAX, |view| {
            view.row(dest_y, bounds.x_min).alpha_at_offset(offset)
          });

          if src[3] != 0 && alpha != 0 {
            composite_premultiplied_over(dst, scale_premultiplied_pixel(src, alpha));
          }
        }
      }
      assert_eq!(actual, expected, "clipped: {}", clip.is_some());
    }
  }

  /// A pixmap of valid premultiplied pixels, varied by `seed`.
  fn noisy_pixmap(width: u32, height: u32, seed: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    let mut state = seed;

    for pixel in pixmap.pixels_mut() {
      let mut next = || {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (state >> 16) as u8
      };
      let alpha = next();

      *pixel = PremultipliedColorU8::from_rgba(
        next() % (alpha.saturating_add(1)),
        next() % (alpha.saturating_add(1)),
        next() % (alpha.saturating_add(1)),
        alpha,
      )
      .unwrap();
    }
    pixmap
  }

  #[test]
  fn whole_pixel_bounds_keep_offsets_past_f32_precision() {
    let bounds = OverlayBounds::at_whole_pixel(
      Size::new(16_777_218, 1),
      Point {
        x: 16_777_217,
        y: 0,
      },
      Size::new(1, 1),
    )
    .unwrap();

    assert_eq!((bounds.x_min, bounds.x_max), (16_777_217, 16_777_218));
  }

  #[test]
  fn translucent_blit_matches_tiny_skia() {
    let source = noisy_pixmap(37, 23, 7);

    for (offset, opacity) in [
      (Point { x: 3, y: 4 }, 0.1),
      (Point { x: -5, y: 2 }, 1.0 / 3.0),
      (Point { x: 20, y: -6 }, 0.5),
      (Point { x: 30, y: 30 }, 0.731),
      (Point { x: 0, y: 0 }, 0.999),
    ] {
      let mut ours = noisy_pixmap(50, 40, 11);
      let mut theirs = ours.clone();

      blit_translucent_translation(&mut ours.as_mut(), source.as_ref(), offset, opacity);
      theirs.draw_pixmap(
        offset.x,
        offset.y,
        source.as_ref(),
        &PixmapPaint {
          opacity,
          blend_mode: TinyBlendMode::SourceOver,
          quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
      );
      assert!(ours.data() == theirs.data(), "differs at opacity {opacity}");
    }
  }

  #[test]
  fn placement_overlap_tracks_both_source_offsets() {
    let overlap = placement_overlap(
      Placement {
        left: 0,
        top: 0,
        width: 4,
        height: 3,
      },
      Placement {
        left: -1,
        top: 1,
        width: 4,
        height: 3,
      },
    )
    .unwrap();

    assert_eq!(
      overlap.placement,
      Placement {
        left: 0,
        top: 1,
        width: 3,
        height: 2,
      }
    );
    assert_eq!(overlap.lhs_offset, Point::new(0, 1));
    assert_eq!(overlap.rhs_offset, Point::new(1, 0));
    assert!(
      placement_overlap(
        Placement {
          left: 0,
          top: 0,
          width: 1,
          height: 1,
        },
        Placement {
          left: 1,
          top: 0,
          width: 1,
          height: 1,
        },
      )
      .is_none()
    );
  }

  #[test]
  fn test_subcanvas_overlay_sampled_image_matches_direct_render() -> Result<()> {
    let source = RgbaImage::from_fn(2, 1, |x, _| {
      if x == 0 {
        Rgba([255, 0, 0, 255])
      } else {
        Rgba([0, 0, 255, 255])
      }
    });
    let source_pixmap =
      ImageBuffer::from_rgba_bytes(source.as_raw().to_vec(), source.width(), source.height())
        .as_ref()
        .and_then(pixmap_from_buffer)
        .expect("fixture pixmap conversion");

    let mut direct = Canvas::new(Size {
      width: 8,
      height: 6,
    });
    direct.overlay_sampled_pixmap(
      source_pixmap.as_ref(),
      Size {
        width: 4,
        height: 2,
      },
      BorderProperties::default(),
      Affine::translation(2.0, 2.0),
      SamplingOptions {
        logical_to_source: Affine::scale(0.5, 0.5),
        algorithm: ImageScalingAlgorithm::Pixelated,
      },
      BlendMode::Normal,
    );

    let mut isolated = Canvas::new(Size {
      width: 8,
      height: 6,
    });
    let subcanvas = isolated.begin_subcanvas(Placement {
      left: 2,
      top: 2,
      width: 4,
      height: 2,
    })?;
    isolated.overlay_sampled_pixmap(
      source_pixmap.as_ref(),
      Size {
        width: 4,
        height: 2,
      },
      BorderProperties::default(),
      Affine::translation(2.0, 2.0),
      SamplingOptions {
        logical_to_source: Affine::scale(0.5, 0.5),
        algorithm: ImageScalingAlgorithm::Pixelated,
      },
      BlendMode::Normal,
    );
    isolated.composite_subcanvas(subcanvas, BlendMode::Normal, 1.0);

    assert_eq!(
      direct.into_inner()?.into_raw(),
      isolated.into_inner()?.into_raw()
    );
    Ok(())
  }

  #[test]
  fn test_overlay_image_with_parent_mask() {
    use takumi_core::style::{Sides, SpacePair};

    let mut canvas = Canvas::new(Size {
      width: 10,
      height: 10,
    });

    let mut parent_mask = TinyMask::new(10, 10).unwrap();
    parent_mask.data_mut()[0..50].fill(255);
    canvas.push_mask(parent_mask);

    let image_data = [0u8, 255, 0, 255].repeat(16);
    let image_pixmap = PixmapRef::from_bytes(&image_data, 4, 4).unwrap();
    let paint_source = PaintSource::Pixmap(image_pixmap);

    let border = BorderProperties {
      radius: Sides([SpacePair::from_single(1.0); 4]),
      ..Default::default()
    };

    canvas.overlay_image(
      paint_source,
      border,
      Affine::translation(1.0, 1.0),
      ImageScalingAlgorithm::Pixelated,
      BlendMode::Normal,
    );

    canvas.pop_mask();

    let output = canvas.into_inner().unwrap();
    let pixel = output.get_pixel(2, 2);
    assert_eq!(pixel.0[1], 255);
    assert_eq!(pixel.0[3], 255);
  }
}

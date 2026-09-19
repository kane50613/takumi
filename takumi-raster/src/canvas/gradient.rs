//! Gradient tile overlay fast paths and their reference-parity tests.

use takumi_core::{
  geometry::{Point, Size},
  paint::{
    GradientOverlayTile, LinearGradientFastPathKind, LinearGradientTile, RadialGradientTile,
  },
};
use tiny_skia::PixmapMut;

use super::{
  MaskView,
  blit::{OverlayBounds, blit_rows, compute_overlay_bounds_for_canvas},
};
use crate::{BackgroundTile, blend::*, simd, style::BlendMode};

/// Overlays a gradient-shaped [`BackgroundTile`] at a plain translation, reporting whether the tile
/// was one. Non-gradient tiles are left for the caller's generic overlay path.
pub(crate) fn try_overlay_gradient_tile(
  pixmap: &mut PixmapMut<'_>,
  tile: &BackgroundTile,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) -> bool {
  match tile {
    BackgroundTile::Linear(gradient) => {
      overlay_linear_gradient_tile(pixmap, gradient, offset, mode, combined_mask)
    }
    BackgroundTile::Radial(gradient) => {
      overlay_radial_gradient_tile(pixmap, gradient, offset, mode, combined_mask)
    }
    BackgroundTile::Conic(gradient) => {
      overlay_gradient_tile(pixmap, gradient, offset, mode, combined_mask)
    }
    _ => return false,
  }

  true
}

pub(crate) fn overlay_gradient_tile<T>(
  pixmap: &mut PixmapMut<'_>,
  gradient: &T,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) where
  T: GradientOverlayTile,
{
  let bottom_width = pixmap.width();
  let bottom_height = pixmap.height();
  let top_size = Size {
    width: gradient.width(),
    height: gradient.height(),
  };

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let bottom_data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
    gradient.overlay_unconstrained(bottom_data, bottom_width, bottom_height, offset);
    return;
  }

  let Some(bounds) = compute_overlay_bounds_for_canvas(
    bottom_width,
    bottom_height,
    offset,
    top_size.width,
    top_size.height,
  ) else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let x_start = (bounds.x_min - bounds.offset_x) as u32;
  blit_rows(
    pixels,
    bottom_width,
    bounds,
    mode,
    combined_mask,
    |y, row| {
      for (i, pixel) in row.iter_mut().enumerate() {
        *pixel = premultiplied_from_pixel(gradient.sample_pixel_dithered(x_start + i as u32, y));
      }
    },
  );
}

fn try_overlay_linear_gradient_tile_fast_normal_unconstrained(
  data: &mut [u8],
  bottom_width: u32,
  bottom_height: u32,
  gradient: &LinearGradientTile,
  offset: Point<f32>,
) -> bool {
  let Some(bounds) = compute_overlay_bounds_for_canvas(
    bottom_width,
    bottom_height,
    offset,
    gradient.width(),
    gradient.height(),
  ) else {
    return true;
  };

  let Some(fast_path) = gradient.fast_path() else {
    return overlay_linear_gradient_row_lanes(data, bottom_width, bounds, gradient);
  };

  let row_stride = bottom_width as usize * 4;
  let row_count = (bounds.y_max - bounds.y_min) as usize;
  let segment_pixel_count = (bounds.x_max - bounds.x_min) as usize;
  let dest_byte_start = bounds.x_min as usize * 4;
  let dest_byte_end = dest_byte_start + segment_pixel_count * 4;
  let rows = &mut data[bounds.y_min as usize * row_stride..bounds.y_max as usize * row_stride];

  match fast_path.kind {
    LinearGradientFastPathKind::Horizontal => {
      let src_x_start = (bounds.x_min - bounds.offset_x) as usize;
      let src_x_end = src_x_start + segment_pixel_count;
      let src_y_start = (bounds.y_min - bounds.offset_y) as usize;
      let width = gradient.width() as usize;
      let variant = |src_y: usize| {
        if fast_path.dithered {
          &fast_path.axis_samples[(src_y & 7) * width..][src_x_start..src_x_end]
        } else {
          &fast_path.axis_samples[src_x_start..src_x_end]
        }
      };

      if fast_path.fully_opaque {
        for (row_offset, row) in rows.chunks_mut(row_stride).enumerate() {
          let scanline: &[u8] = bytemuck::cast_slice(variant(src_y_start + row_offset));
          row[dest_byte_start..dest_byte_end].copy_from_slice(scanline);
        }
      } else {
        for (row_offset, row) in rows.chunks_mut(row_stride).enumerate() {
          composite_premultiplied_over_span(
            &mut row[dest_byte_start..dest_byte_end],
            variant(src_y_start + row_offset),
          );
        }
      }
    }
    LinearGradientFastPathKind::Vertical => {
      let src_y_start = (bounds.y_min - bounds.offset_y) as usize;
      let src_x_start = (bounds.x_min - bounds.offset_x) as usize;

      if fast_path.dithered {
        for (row_offset, row) in rows.chunks_mut(row_stride).enumerate() {
          let src_y = src_y_start + row_offset;
          let pattern = &fast_path.axis_samples[src_y * 8..src_y * 8 + 8];
          let row_segment = &mut row[dest_byte_start..dest_byte_end];
          for (i, dst) in row_segment.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let pixel = pattern[(src_x_start + i) & 7];
            if fast_path.fully_opaque {
              *dst = [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()];
            } else {
              blend_premultiplied_pixel_normal(dst, pixel);
            }
          }
        }
      } else {
        let src_pixels = &fast_path.axis_samples[src_y_start..src_y_start + row_count];

        for (row_offset, row) in rows.chunks_mut(row_stride).enumerate() {
          let row_segment = &mut row[dest_byte_start..dest_byte_end];
          let pixel = src_pixels[row_offset];
          if fast_path.fully_opaque {
            fill_repeated_premultiplied_pixel(
              row_segment,
              [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
            );
          } else {
            blend_repeated_premultiplied_pixel(row_segment, pixel);
          }
        }
      }
    }
  }

  true
}

const ROW_LANES: usize = 4;

/// Fills an opaque, non-repeating, undithered linear gradient four rows at a
/// time. Each row keeps the generic path's per-pixel projection recurrence, so
/// the lanes only remove the dependency between neighbouring pixels.
fn overlay_linear_gradient_row_lanes(
  data: &mut [u8],
  bottom_width: u32,
  bounds: OverlayBounds,
  gradient: &LinearGradientTile,
) -> bool {
  if gradient.repeating || !gradient.fully_opaque || gradient.lut.dither_active() {
    return false;
  }
  let lut = gradient.lut.colors();
  if lut.is_empty() {
    return true;
  }

  let max_index = (lut.len() - 1) as u32;
  let axis_length = gradient.axis_length;
  let scale = gradient.position_to_lut_scale;
  let step = gradient.dir_x;
  let src_x_start = (bounds.x_min - bounds.offset_x) as f32;
  let row_projection = |dest_y: i32| {
    let src_y = (dest_y - bounds.offset_y) as f32;
    src_x_start * gradient.dir_x + src_y * gradient.dir_y + gradient.projection_bias
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(data);
  let row_pixels = bottom_width as usize;
  let (x_min, x_max) = (bounds.x_min as usize, bounds.x_max as usize);
  let mut dest_y = bounds.y_min;

  while dest_y + ROW_LANES as i32 <= bounds.y_max {
    let band_start = dest_y as usize * row_pixels;
    let band = &mut pixels[band_start..band_start + ROW_LANES * row_pixels];
    let (r0, rest) = band.split_at_mut(row_pixels);
    let (r1, rest) = rest.split_at_mut(row_pixels);
    let (r2, r3) = rest.split_at_mut(row_pixels);
    let lanes = r0[x_min..x_max]
      .iter_mut()
      .zip(&mut r1[x_min..x_max])
      .zip(&mut r2[x_min..x_max])
      .zip(&mut r3[x_min..x_max]);
    let mut projections: [f32; ROW_LANES] =
      std::array::from_fn(|lane| row_projection(dest_y + lane as i32));

    for (((p0, p1), p2), p3) in lanes {
      let [i0, i1, i2, i3] = simd::linear_lut_indices(projections, axis_length, scale, max_index);
      *p0 = premultiplied_from_pixel(lut[i0 as usize]);
      *p1 = premultiplied_from_pixel(lut[i1 as usize]);
      *p2 = premultiplied_from_pixel(lut[i2 as usize]);
      *p3 = premultiplied_from_pixel(lut[i3 as usize]);
      for projection in &mut projections {
        *projection += step;
      }
    }
    dest_y += ROW_LANES as i32;
  }

  for dest_y in dest_y..bounds.y_max {
    let row_start = dest_y as usize * row_pixels;
    let row = &mut pixels[row_start + x_min..row_start + x_max];
    let mut projection = row_projection(dest_y);
    for pixel in row {
      let index = simd::scalar::lut_index(projection, axis_length, scale, max_index);
      *pixel = premultiplied_from_pixel(lut[index as usize]);
      projection += step;
    }
  }

  true
}

pub(crate) fn overlay_linear_gradient_tile(
  pixmap: &mut PixmapMut<'_>,
  gradient: &LinearGradientTile,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  overlay_gradient_tile_with_fast_path(
    pixmap,
    gradient,
    offset,
    mode,
    combined_mask,
    try_overlay_linear_gradient_tile_fast_normal_unconstrained,
  );
}

fn overlay_gradient_tile_with_fast_path<T>(
  pixmap: &mut PixmapMut<'_>,
  gradient: &T,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
  try_fast_path: impl FnOnce(&mut [u8], u32, u32, &T, Point<f32>) -> bool,
) where
  T: GradientOverlayTile,
{
  let bottom_width = pixmap.width();
  let bottom_height = pixmap.height();

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let bottom_data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
    if try_fast_path(bottom_data, bottom_width, bottom_height, gradient, offset) {
      return;
    }

    gradient.overlay_unconstrained(bottom_data, bottom_width, bottom_height, offset);
    return;
  }

  overlay_gradient_tile(pixmap, gradient, offset, mode, combined_mask);
}

fn try_overlay_radial_gradient_tile_fast_normal_unconstrained(
  data: &mut [u8],
  bottom_width: u32,
  bottom_height: u32,
  gradient: &RadialGradientTile,
  offset: Point<f32>,
) -> bool {
  let Some(bounds) = compute_overlay_bounds_for_canvas(
    bottom_width,
    bottom_height,
    offset,
    gradient.width(),
    gradient.height(),
  ) else {
    return true;
  };

  if gradient.repeating {
    return false;
  }

  let Some(outer_pixel) = gradient.outer_sample() else {
    return true;
  };

  let lut_len = gradient.lut_len();
  if lut_len == 0 {
    return true;
  }

  let row_stride = bottom_width as usize * 4;
  let dither = gradient.dither_active();
  // The exterior reuses the outermost LUT entry, so a dithered fill has to
  // quantize it per pixel like the generic path or the ellipse edge seams.
  let fill_outer = |span: &mut [u8], span_x_start: u32, src_y: u32| {
    if dither {
      for (i, pixel) in span.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let src = gradient.sample_dithered_at(lut_len - 1, span_x_start + i as u32, src_y);
        blend_premultiplied_pixel_normal(pixel, src);
      }
    } else {
      composite_repeated_premultiplied_pixel_normal(span, outer_pixel);
    }
  };

  for dest_y in bounds.y_min..bounds.y_max {
    let src_y = (dest_y - bounds.offset_y) as u32;
    let src_x_start = (bounds.x_min - bounds.offset_x) as u32;
    let src_x_end = (bounds.x_max - bounds.offset_x) as u32;
    let Some((active_x_start, active_x_end)) =
      gradient.non_repeating_active_span(src_x_start, src_x_end, src_y)
    else {
      return false;
    };

    let row_start = dest_y as usize * row_stride + bounds.x_min as usize * 4;
    let row_end = row_start + (bounds.x_max - bounds.x_min) as usize * 4;
    let row = &mut data[row_start..row_end];

    let left_pixels = (active_x_start - src_x_start) as usize;
    if left_pixels > 0 {
      fill_outer(&mut row[..left_pixels * 4], src_x_start, src_y);
    }

    let center_pixels = (active_x_end - active_x_start) as usize;
    if center_pixels > 0 {
      let center_byte_start = left_pixels * 4;
      let center_byte_end = center_byte_start + center_pixels * 4;
      let center_row = &mut row[center_byte_start..center_byte_end];
      let mut row_state = gradient.begin_row(active_x_start, src_y, lut_len);
      for (i, pixel) in center_row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let lut_idx = gradient.next_lut_index(&mut row_state);
        let src = if dither {
          gradient.sample_dithered_at(lut_idx, active_x_start + i as u32, src_y)
        } else {
          gradient.sample_at(lut_idx)
        };
        blend_premultiplied_pixel_normal(pixel, src);
      }
    }

    let right_pixels = (src_x_end - active_x_end) as usize;
    if right_pixels > 0 {
      let right_byte_start = row.len() - right_pixels * 4;
      fill_outer(&mut row[right_byte_start..], active_x_end, src_y);
    }
  }

  true
}

pub(crate) fn overlay_radial_gradient_tile(
  pixmap: &mut PixmapMut<'_>,
  gradient: &RadialGradientTile,
  offset: Point<f32>,
  mode: BlendMode,
  combined_mask: Option<MaskView<'_>>,
) {
  overlay_gradient_tile_with_fast_path(
    pixmap,
    gradient,
    offset,
    mode,
    combined_mask,
    try_overlay_radial_gradient_tile_fast_normal_unconstrained,
  );
}

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};
  use takumi_core::paint::{ConicGradientTile, LinearGradientTile, RadialGradientTile};
  use tiny_skia::PixmapMut;

  use super::*;
  use crate::{
    Canvas, Fonts, RenderContext, Result, blend_pixel,
    canvas::demultiply_rgba_in_place,
    style::{
      Angle, Color, ColorInterpolationMethod, ConicGradient, FromCssStr, GradientStop, Length,
      LinearGradient, PositionValue, RadialGradient, SizingContext, StopPosition,
    },
    viewport::Viewport,
  };

  fn overlay_area_reference(
    bottom: &mut RgbaImage,
    offset: Point<f32>,
    top_size: Size<u32>,
    f: impl Fn(u32, u32) -> Rgba<u8>,
  ) {
    let offset_x = offset.x as i32;
    let offset_y = offset.y as i32;
    let dest_x_min = offset_x.max(0);
    let dest_x_max = (offset_x + top_size.width as i32).min(bottom.width() as i32);
    let dest_y_min = offset_y.max(0);
    let dest_y_max = (offset_y + top_size.height as i32).min(bottom.height() as i32);

    for dest_y in dest_y_min..dest_y_max {
      let src_y = (dest_y - offset_y) as u32;
      for dest_x in dest_x_min..dest_x_max {
        let src_x = (dest_x - offset_x) as u32;
        let pixel = f(src_x, src_y);
        if pixel.0[3] == 0 {
          continue;
        }
        let current = bottom.get_pixel_mut(dest_x as u32, dest_y as u32);
        blend_pixel(current, pixel, BlendMode::Normal);
      }
    }
  }

  fn assert_gradient_overlay_matches_reference_with<T>(
    tile: &T,
    canvas_size: Size<u32>,
    offset: Point<f32>,
    overlay: impl FnOnce(&mut PixmapMut<'_>, &T, Point<f32>),
  ) -> Result<()>
  where
    T: GradientOverlayTile,
  {
    let mut canvas = Canvas::new(canvas_size);
    let mut reference =
      RgbaImage::from_pixel(canvas_size.width, canvas_size.height, Rgba([0, 0, 0, 0]));

    {
      let mut pixmap = canvas.image.as_mut();
      overlay(&mut pixmap, tile, offset);
    }

    overlay_area_reference(
      &mut reference,
      offset,
      Size {
        width: tile.width(),
        height: tile.height(),
      },
      |x, y| {
        // The canvas demultiplies on the way out, so the reference has to use
        // the same rounding or this compares two conversions, not two paths.
        let color = tile.sample_pixel_dithered(x, y);
        let mut pixel = [color.red(), color.green(), color.blue(), color.alpha()];
        demultiply_rgba_in_place(&mut pixel);
        Rgba(pixel)
      },
    );

    let fast = canvas.into_inner()?;
    assert_eq!(fast.as_raw(), reference.as_raw());
    Ok(())
  }

  fn assert_gradient_overlay_matches_reference<T>(
    tile: &T,
    canvas_size: Size<u32>,
    offset: Point<f32>,
  ) -> Result<()>
  where
    T: GradientOverlayTile,
  {
    assert_gradient_overlay_matches_reference_with(
      tile,
      canvas_size,
      offset,
      |pixmap, tile, offset| {
        overlay_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
      },
    )
  }

  #[test]
  fn test_overlay_linear_gradient_matches_reference() -> Result<()> {
    let gradient = LinearGradient::from_css_str("linear-gradient(to right, red, blue)")?;
    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((32, 16)))
          .build(),
      )
      .build();
    let tile = LinearGradientTile::new(
      &gradient,
      32,
      16,
      &render_context.sizing,
      render_context.current_color,
      true,
    );
    assert_gradient_overlay_matches_reference_with(
      &tile,
      Size {
        width: 40,
        height: 24,
      },
      Point { x: 3.0, y: 4.0 },
      |pixmap, tile, offset| {
        overlay_linear_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
      },
    )?;
    Ok(())
  }

  /// The four-row path against the generic per-row recurrence it replaces,
  /// across widths that leave a tail, clipped offsets, and every quadrant.
  #[test]
  fn oblique_linear_gradient_rows_match_the_generic_overlay() -> Result<()> {
    let fonts = Fonts::default();
    let canvas_size = Size {
      width: 4101,
      height: 12,
    };
    for angle in [33, 135, 225, 315] {
      let gradient =
        LinearGradient::from_css_str(&format!("linear-gradient({angle}deg, red, blue)"))?;
      for height in 1..=9 {
        let render_context = RenderContext::builder()
          .fonts(fonts.snapshot())
          .sizing(
            SizingContext::builder()
              .viewport(Viewport::new((4099, height)))
              .build(),
          )
          .build();
        let tile = LinearGradientTile::new(
          &gradient,
          4099,
          height,
          &render_context.sizing,
          render_context.current_color,
          false,
        );
        assert!(tile.fast_path().is_none() && tile.fully_opaque && !tile.repeating);
        for offset in [Point { x: 0.0, y: 0.0 }, Point { x: -3.0, y: -1.0 }] {
          let mut fast = Canvas::new(canvas_size);
          let mut generic = Canvas::new(canvas_size);
          {
            let mut pixmap = fast.image.as_mut();
            overlay_linear_gradient_tile(&mut pixmap, &tile, offset, BlendMode::Normal, None);
          }
          generic.with_pixmap(|pixmap| {
            let data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
            tile.overlay_unconstrained(data, canvas_size.width, canvas_size.height, offset);
          });
          assert_eq!(
            fast.into_inner()?.as_raw(),
            generic.into_inner()?.as_raw(),
            "angle {angle} height {height} offset {offset:?}"
          );
        }
      }
    }
    Ok(())
  }

  #[test]
  fn test_overlay_radial_gradient_fast_paths_match_reference() -> Result<()> {
    let cases = [
      (
        "radial-gradient(circle, red, blue)",
        Size {
          width: 32,
          height: 24,
        },
        Size {
          width: 40,
          height: 30,
        },
        Point { x: 4.0, y: 3.0 },
      ),
      (
        "radial-gradient(circle at 20% 30%, red, rgba(0,0,255,0.25))",
        Size {
          width: 40,
          height: 28,
        },
        Size {
          width: 52,
          height: 36,
        },
        Point { x: 5.0, y: 4.0 },
      ),
      // The outer stop premultiplies to a fractional channel, so the exterior
      // spans must dither per pixel like the generic path.
      (
        "radial-gradient(circle closest-side at 20% 30%, red, rgba(100,0,0,0.45))",
        Size {
          width: 40,
          height: 28,
        },
        Size {
          width: 52,
          height: 36,
        },
        Point { x: 5.0, y: 4.0 },
      ),
    ];

    let global_context = Fonts::default();
    for (gradient_css, tile_size, canvas_size, offset) in cases {
      let gradient = RadialGradient::from_css_str(gradient_css)?;
      let render_context = RenderContext::builder()
        .fonts(global_context.snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::new((tile_size.width, tile_size.height)))
            .build(),
        )
        .build();
      let tile = RadialGradientTile::new(
        &gradient,
        tile_size.width,
        tile_size.height,
        &render_context.sizing,
        render_context.current_color,
        true,
      );
      assert_gradient_overlay_matches_reference_with(
        &tile,
        canvas_size,
        offset,
        |pixmap, tile, offset| {
          overlay_radial_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
        },
      )?;
    }
    Ok(())
  }

  #[test]
  fn test_overlay_conic_gradient_matches_reference() -> Result<()> {
    let gradient = ConicGradient::from_css_str("conic-gradient(red, blue)")?;

    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((32, 24)))
          .build(),
      )
      .build();
    let tile = ConicGradientTile::new(
      &gradient,
      32,
      24,
      &render_context.sizing,
      render_context.current_color,
      true,
    );
    assert_gradient_overlay_matches_reference_with(
      &tile,
      Size {
        width: 40,
        height: 30,
      },
      Point { x: 4.0, y: 3.0 },
      |pixmap, tile, offset| {
        overlay_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
      },
    )?;
    Ok(())
  }

  #[test]
  fn test_overlay_linear_gradient_fast_paths_match_reference() -> Result<()> {
    let cases = [
      (
        "linear-gradient(to right, red 0px, lime 0.5px, blue 32px)",
        Size {
          width: 32,
          height: 16,
        },
        Size {
          width: 40,
          height: 24,
        },
        Point { x: 3.0, y: 4.0 },
      ),
      (
        "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
        Size {
          width: 48,
          height: 12,
        },
        Size {
          width: 56,
          height: 24,
        },
        Point { x: 4.0, y: 6.0 },
      ),
      (
        "linear-gradient(180deg, rgba(0,128,255,0.9), rgba(0,128,255,0))",
        Size {
          width: 24,
          height: 48,
        },
        Size {
          width: 36,
          height: 64,
        },
        Point { x: 6.0, y: 5.0 },
      ),
      (
        "linear-gradient(to right, grey 1px, transparent 1px)",
        Size {
          width: 40,
          height: 8,
        },
        Size {
          width: 48,
          height: 16,
        },
        Point { x: 4.0, y: 3.0 },
      ),
      (
        "repeating-linear-gradient(90deg, red 0px 5px, blue 5px 10px)",
        Size {
          width: 40,
          height: 8,
        },
        Size {
          width: 52,
          height: 16,
        },
        Point { x: 5.0, y: 4.0 },
      ),
    ];

    let global_context = Fonts::default();
    for (gradient_css, tile_size, canvas_size, offset) in cases {
      let gradient = LinearGradient::from_css_str(gradient_css)?;
      let render_context = RenderContext::builder()
        .fonts(global_context.snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::new((tile_size.width, tile_size.height)))
            .build(),
        )
        .build();
      let tile = LinearGradientTile::new(
        &gradient,
        tile_size.width,
        tile_size.height,
        &render_context.sizing,
        render_context.current_color,
        true,
      );
      assert_gradient_overlay_matches_reference_with(
        &tile,
        canvas_size,
        offset,
        |pixmap, tile, offset| {
          overlay_linear_gradient_tile(pixmap, tile, offset, BlendMode::Normal, None);
        },
      )?;
    }
    Ok(())
  }

  #[test]
  fn test_overlay_conic_gradient_hard_stops_matches_reference() -> Result<()> {
    let gradient = ConicGradient {
      repeating: false,
      from_angle: Angle::zero(),
      center: PositionValue::center(),
      interpolation: ColorInterpolationMethod::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(25.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(25.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((48, 48)))
          .build(),
      )
      .build();
    let tile = ConicGradientTile::new(
      &gradient,
      48,
      48,
      &render_context.sizing,
      render_context.current_color,
      true,
    );
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 56,
        height: 56,
      },
      Point { x: 4.0, y: 4.0 },
    )
  }

  #[test]
  fn test_overlay_radial_gradient_clustered_stops_matches_reference() -> Result<()> {
    let gradient =
      RadialGradient::from_css_str("radial-gradient(circle, red 0%, lime 1%, blue 100%)")?;
    let global_context = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(global_context.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((32, 24)))
          .build(),
      )
      .build();
    let tile = RadialGradientTile::new(
      &gradient,
      32,
      24,
      &render_context.sizing,
      render_context.current_color,
      true,
    );
    assert_gradient_overlay_matches_reference(
      &tile,
      Size {
        width: 40,
        height: 30,
      },
      Point { x: 4.0, y: 3.0 },
    )?;
    Ok(())
  }
}

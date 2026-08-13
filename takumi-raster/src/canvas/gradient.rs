//! Gradient tile overlay fast paths and their reference-parity tests.

use takumi_core::{
  geometry::{Point, Size},
  paint::{
    GradientOverlayTile, LinearGradientFastPathKind, LinearGradientTile, RadialGradientTile,
    overlay_gradient_tile_fast_normal_unconstrained,
  },
};
use tiny_skia::PixmapMut;

use super::{
  MaskView,
  blit::{blit_rows_from_sampler, compute_overlay_bounds_for_canvas},
};
use crate::{BackgroundTile, blend::*, style::BlendMode};

/// Overlays a gradient-shaped [`BackgroundTile`] at a plain translation,
/// reporting whether the tile was one. Non-gradient tiles are left for the
/// caller's generic overlay path.
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
    overlay_gradient_tile_fast_normal_unconstrained(
      bottom_data,
      bottom_width,
      bottom_height,
      gradient,
      offset,
    );
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
  blit_rows_from_sampler(pixels, bottom_width, bounds, mode, combined_mask, |x, y| {
    premultiplied_from_pixel(gradient.sample_pixel(x, y))
  });
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
    return false;
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
      let src_pixels = &fast_path.axis_samples[src_x_start..src_x_end];

      if fast_path.fully_opaque {
        let scanline: &[u8] = bytemuck::cast_slice(src_pixels);

        for row in rows.chunks_mut(row_stride) {
          row[dest_byte_start..dest_byte_end].copy_from_slice(scanline);
        }
      } else {
        for row in rows.chunks_mut(row_stride) {
          composite_premultiplied_over_span(&mut row[dest_byte_start..dest_byte_end], src_pixels);
        }
      }
    }
    LinearGradientFastPathKind::Vertical => {
      let src_y_start = (bounds.y_min - bounds.offset_y) as usize;
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

    overlay_gradient_tile_fast_normal_unconstrained(
      bottom_data,
      bottom_width,
      bottom_height,
      gradient,
      offset,
    );
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
      composite_repeated_premultiplied_pixel_normal(&mut row[..left_pixels * 4], outer_pixel);
    }

    let center_pixels = (active_x_end - active_x_start) as usize;
    if center_pixels > 0 {
      let center_byte_start = left_pixels * 4;
      let center_byte_end = center_byte_start + center_pixels * 4;
      let center_row = &mut row[center_byte_start..center_byte_end];
      let mut row_state = gradient.begin_row(active_x_start, src_y, lut_len);
      for pixel in center_row.chunks_exact_mut(4) {
        let lut_idx = gradient.next_lut_index(&mut row_state);
        let src = gradient.sample_at(lut_idx);
        blend_premultiplied_pixel_normal(pixel, src);
      }
    }

    let right_pixels = (src_x_end - active_x_end) as usize;
    if right_pixels > 0 {
      let right_byte_start = row.len() - right_pixels * 4;
      composite_repeated_premultiplied_pixel_normal(&mut row[right_byte_start..], outer_pixel);
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
        let color = tile.sample_pixel(x, y).demultiply();
        Rgba([color.red(), color.green(), color.blue(), color.alpha()])
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

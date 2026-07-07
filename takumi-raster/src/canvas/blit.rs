//! Free blitters and the image/mask overlay dispatchers that paint onto a
//! [`DrawTarget`].

use image::Rgba;
use takumi_core::geometry::{Point, Size};
use tiny_skia::{PixmapMut, PremultipliedColorU8};

use super::{
  DrawTarget, MaskSamplingOptions, MaskSourceToPixmapOptions, MaskView, OverlayOptions,
  PaintSource, SamplingFootprint, SamplingOptions, composite,
  composite::sampling_footprint,
  paint_source::{MaskCompositeColor, sample_paint_source},
  skia::{
    FillColorOptions, ImagePathFillOptions, try_draw_image_with_tiny_skia,
    try_fill_color_with_tiny_skia, try_fill_image_path_with_tiny_skia,
  },
};
use crate::{
  Placement,
  blend::*,
  render_mask,
  style::{Affine, BlendMode, ImageScalingAlgorithm},
};

#[inline(always)]
pub(crate) fn compute_overlay_bounds_for_canvas(
  canvas_width: u32,
  canvas_height: u32,
  offset: Point<f32>,
  width: u32,
  height: u32,
) -> Option<(i32, i32, i32, i32, i32, i32)> {
  if width == 0 || height == 0 {
    return None;
  }

  let offset_x = offset.x.trunc() as i32;
  let offset_y = offset.y.trunc() as i32;
  let bottom_width = canvas_width as i32;
  let bottom_height = canvas_height as i32;
  let dest_y_min = offset_y.max(0);
  let dest_y_max = (offset_y + height as i32).min(bottom_height);
  if dest_y_min >= dest_y_max {
    return None;
  }

  let dest_x_min = offset_x.max(0);
  let dest_x_max = (offset_x + width as i32).min(bottom_width);
  if dest_x_min >= dest_x_max {
    return None;
  }

  Some((
    offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max,
  ))
}

#[inline(always)]
pub(crate) fn apply_combined_mask(
  src: [u8; 4],
  combined_mask: Option<MaskView<'_>>,
  dest_x: u32,
  dest_y: u32,
) -> Option<[u8; 4]> {
  let Some(mask) = combined_mask else {
    return Some(src);
  };
  let alpha = mask.alpha_at(dest_x, dest_y);
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
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(canvas_width, canvas_height, offset, size.width, size.height)
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  let footprint = sampling_footprint(sampling.logical_to_source);
  for dest_y in dest_y_min..dest_y_max {
    let src_y = (dest_y - offset_y) as f32;
    let (mut sample_x, mut sample_y) = sampling
      .logical_to_source
      .transform_point((dest_x_min - offset_x) as f32 + 0.5, src_y + 0.5);
    for dest_x in dest_x_min..dest_x_max {
      let src = sample_paint_source(source, sampling.algorithm, sample_x, sample_y, footprint)
        .unwrap_or([0, 0, 0, 0]);
      sample_x += sampling.logical_to_source.a;
      sample_y += sampling.logical_to_source.b;
      if src[3] == 0 {
        continue;
      }

      let dest_x = dest_x as u32;
      let dest_y = dest_y as u32;
      let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y) else {
        continue;
      };

      let index = (dest_y * canvas_width + dest_x) as usize;
      blend_premultiplied_pixel(&mut pixels[index], src, mode);
    }
  }
}

fn blit_paint_source_translation(
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
  let canvas_height = pixmap.height();
  let Some((offset_x, offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      canvas_width,
      canvas_height,
      offset,
      source.width(),
      source.height(),
    )
  else {
    return;
  };

  let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(pixmap.pixels_mut());
  match source {
    PaintSource::Pixmap(source) => {
      let source_pixels = source.pixels();
      let source_width = source.width();
      if mode == BlendMode::Normal && combined_mask.is_none() {
        let copy_width = (dest_x_max - dest_x_min) as usize;
        let src_x_start = (dest_x_min - offset_x) as usize;
        for dest_y in dest_y_min..dest_y_max {
          let src_y = (dest_y - offset_y) as usize;
          let src_start = src_y * source_width as usize + src_x_start;
          let src_end = src_start + copy_width;
          let dst_start = (dest_y as u32 * canvas_width + dest_x_min as u32) as usize;
          let dst_end = dst_start + copy_width;
          let dst = bytemuck::cast_slice_mut(&mut pixels[dst_start..dst_end]);
          composite_premultiplied_over_span(dst, &source_pixels[src_start..src_end]);
        }
        return;
      }

      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as u32;
        let dst_row = dest_y as usize * canvas_width as usize;
        let src_row = src_y as usize * source_width as usize;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as u32;
          let src = premultiplied_from_pixel(source_pixels[src_row + src_x as usize]);
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y as u32) else {
            continue;
          };

          blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
        }
      }
    }
    _ => {
      for dest_y in dest_y_min..dest_y_max {
        let src_y = (dest_y - offset_y) as f32;
        let dst_row = dest_y as usize * canvas_width as usize;
        for dest_x in dest_x_min..dest_x_max {
          let src_x = (dest_x - offset_x) as f32;
          let src = sample_paint_source(
            source,
            ImageScalingAlgorithm::Pixelated,
            src_x,
            src_y,
            SamplingFootprint::PIXEL,
          )
          .unwrap_or([0; 4]);
          if src[3] == 0 {
            continue;
          }

          let dest_x = dest_x as u32;
          let Some(src) = apply_combined_mask(src, combined_mask, dest_x, dest_y as u32) else {
            continue;
          };

          blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
        }
      }
    }
  }
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
  let canvas_height = pixmap.height();
  let Some((_offset_x, _offset_y, dest_x_min, dest_x_max, dest_y_min, dest_y_max)) =
    compute_overlay_bounds_for_canvas(
      canvas_width,
      canvas_height,
      offset,
      source_width,
      source_height,
    )
  else {
    return;
  };

  let data: &mut [u8] = bytemuck::cast_slice_mut(pixmap.pixels_mut());

  if mode == BlendMode::Normal && combined_mask.is_none() {
    let row_stride = canvas_width as usize * 4;
    let x_byte_start = dest_x_min as usize * 4;
    let x_byte_end = dest_x_max as usize * 4;
    for dest_y in dest_y_min..dest_y_max {
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
  for dest_y in dest_y_min..dest_y_max {
    let dst_row = dest_y as usize * canvas_width as usize;
    for dest_x in dest_x_min..dest_x_max {
      let dest_x = dest_x as u32;
      let Some(src) = apply_combined_mask(color, combined_mask, dest_x, dest_y as u32) else {
        continue;
      };

      blend_premultiplied_pixel(&mut pixels[dst_row + dest_x as usize], src, mode);
    }
  }
}

pub(crate) fn composite_mask_source_to_pixmap(
  pixmap: &mut PixmapMut<'_>,
  mask: &[u8],
  source: PaintSource<'_>,
  options: MaskSourceToPixmapOptions<'_>,
) {
  composite::source(
    pixmap,
    mask,
    source,
    composite::Options {
      placement: options.placement,
      sampling: options.sampling,
      color_mode: MaskCompositeColor::SourceOnly,
      mode: options.mode,
      combined_mask: options.combined_mask,
    },
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
    premultiply_rgba(color),
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
  let width = image.width();
  let height = image.height();
  let content_size = Size { width, height };

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

  if options.border.is_zero() && options.transform.only_translation() {
    let offset = Point {
      x: options.transform.x,
      y: options.transform.y,
    };
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

  let mut paths = Vec::new();
  options
    .border
    .append_mask_commands(&mut paths, content_size.map(|v| v as f32), Point::ZERO);

  let (mask, placement) = render_mask(&paths, Some(options.transform), None, target.buffer_pool);
  let inverse = options.transform.invert();
  if options.transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
    composite::source(
      target.pixmap,
      &mask,
      image,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: Affine::IDENTITY,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: options.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: target.combined_mask,
      },
    );
  } else if let Some(inverse) = inverse {
    composite::source(
      target.pixmap,
      &mask,
      image,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: inverse,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: options.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: target.combined_mask,
      },
    );
  }

  target.buffer_pool.release(mask);
}

pub(crate) fn overlay_sampled_paint_source(
  target: &mut DrawTarget,
  source: PaintSource<'_>,
  size: Size<u32>,
  options: OverlayOptions,
  sampling: SamplingOptions,
) {
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

  if options.border.is_zero() && options.transform.only_translation() {
    blit_sampled_paint_source_translation(
      target.pixmap,
      source,
      size,
      Point {
        x: options.transform.x,
        y: options.transform.y,
      },
      sampling,
      options.mode,
      target.combined_mask,
    );
    return;
  }

  let mut paths = Vec::new();
  options
    .border
    .append_mask_commands(&mut paths, size.map(|v| v as f32), Point::ZERO);
  let (mask, placement) = render_mask(&paths, Some(options.transform), None, target.buffer_pool);

  let inverse = options.transform.invert();
  if options.transform.is_identity() && placement.left >= 0 && placement.top >= 0 {
    composite::source(
      target.pixmap,
      &mask,
      source,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: sampling.logical_to_source,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: sampling.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: target.combined_mask,
      },
    );
  } else if let Some(inverse) = inverse {
    let combined_inverse = sampling.logical_to_source * inverse;
    composite::source(
      target.pixmap,
      &mask,
      source,
      composite::Options {
        placement,
        sampling: MaskSamplingOptions {
          canvas_to_source: combined_inverse,
          sample_bias: Point { x: 0.5, y: 0.5 },
          algorithm: sampling.algorithm,
        },
        color_mode: MaskCompositeColor::SourceOnly,
        mode: options.mode,
        combined_mask: target.combined_mask,
      },
    );
  }

  target.buffer_pool.release(mask);
}

#[cfg(test)]
mod tests {
  use image::{Rgba, RgbaImage};
  use takumi_core::geometry::Size;
  use tiny_skia::{Mask as TinyMask, PixmapRef};

  use super::*;
  use crate::{
    BorderProperties, Canvas, PaintSource, pixmap_from_buffer,
    resources::image_buffer::ImageBuffer, style::ImageScalingAlgorithm,
  };

  #[test]
  fn test_subcanvas_overlay_sampled_image_matches_direct_render() {
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
    let Ok(subcanvas) = isolated.begin_subcanvas(Placement {
      left: 2,
      top: 2,
      width: 4,
      height: 2,
    }) else {
      return;
    };
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
      direct.into_inner().map(RgbaImage::into_raw).ok(),
      isolated.into_inner().map(RgbaImage::into_raw).ok()
    );
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

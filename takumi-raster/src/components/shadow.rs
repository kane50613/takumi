use takumi_core::geometry::{ComputedLayout as Layout, Point, Size};
use tiny_skia::PixmapRef;

pub(crate) use crate::shadow::SizedShadow;
use crate::{
  BlurFormat, BlurType, BorderProperties, BufferPool, Canvas, Command, Fill, Placement, Result,
  SamplingOptions, Style, apply_blur, attenuate_alpha_by_mask, fast_div_255, render_mask,
  style::{Affine, BlendMode, ImageScalingAlgorithm, Sides},
};

/// Draws the outset mask of the shadow.
pub(crate) fn draw_outset_shadow(
  shadow: &SizedShadow,
  canvas: &mut Canvas,
  paths: &[Command],
  transform: Affine,
  style: Style,
  cutout_paths: Option<&[Command]>,
) -> Result<()> {
  let (mask, mut placement) =
    render_mask(paths, Some(transform), Some(style), &mut canvas.buffer_pool);

  placement.left += shadow.offset_x as i32;
  placement.top += shadow.offset_y as i32;

  if shadow.blur_radius <= 0.0 && cutout_paths.is_none() {
    canvas.draw_mask(&mask, placement, shadow.color, BlendMode::Normal);
    canvas.buffer_pool.release(mask);
    return Ok(());
  }

  let blur_padding = if shadow.blur_radius > 0.0 {
    shadow.blur_radius * BlurType::Shadow.extent_multiplier()
  } else {
    0.0
  };

  let shadow_width = placement.width + (blur_padding * 2.0) as u32;
  let shadow_height = placement.height + (blur_padding * 2.0) as u32;
  let mut shadow_alpha = canvas
    .buffer_pool
    .acquire((shadow_width * shadow_height) as usize);

  let padding = blur_padding as u32;
  for y in 0..placement.height {
    let src_row = y as usize * placement.width as usize;
    let dst_row = (y + padding) as usize * shadow_width as usize + padding as usize;
    shadow_alpha[dst_row..dst_row + placement.width as usize]
      .copy_from_slice(&mask[src_row..src_row + placement.width as usize]);
  }
  canvas.buffer_pool.release(mask);

  apply_blur(
    BlurFormat::Alpha {
      data: &mut shadow_alpha,
      width: shadow_width,
      height: shadow_height,
    },
    shadow.blur_radius,
    BlurType::Shadow,
    &mut canvas.buffer_pool,
  )?;

  let img_origin_x = placement.left as f32 - blur_padding;
  let img_origin_y = placement.top as f32 - blur_padding;

  if let Some(cutout_paths) = cutout_paths {
    let (erase_mask, erase_placement) = render_mask(
      cutout_paths,
      Some(transform),
      Some(Fill::NonZero.into()),
      &mut canvas.buffer_pool,
    );

    if !erase_mask.is_empty() {
      let shadow_placement = Placement {
        left: img_origin_x as i32,
        top: img_origin_y as i32,
        width: shadow_width,
        height: shadow_height,
      };
      attenuate_alpha_by_mask(
        &mut shadow_alpha,
        shadow_placement,
        &erase_mask,
        erase_placement,
      );
    }
    canvas.buffer_pool.release(erase_mask);
  }

  canvas.draw_mask(
    &shadow_alpha,
    Placement {
      left: img_origin_x as i32,
      top: img_origin_y as i32,
      width: shadow_width,
      height: shadow_height,
    },
    shadow.color,
    BlendMode::Normal,
  );
  canvas.buffer_pool.release(shadow_alpha);
  Ok(())
}

pub(crate) fn draw_inset_shadow_to_canvas(
  shadow: &SizedShadow,
  transform: Affine,
  border_radius: BorderProperties,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let (data, width, height) =
    draw_inset_shadow(shadow, border_radius, layout.size, &mut canvas.buffer_pool)?;

  if let Some(source) = PixmapRef::from_bytes(&data, width, height) {
    canvas.overlay_sampled_pixmap(
      source,
      Size { width, height },
      border_radius,
      transform,
      SamplingOptions {
        logical_to_source: Affine::IDENTITY,
        algorithm: ImageScalingAlgorithm::Auto,
      },
      BlendMode::Normal,
    );
  }
  canvas.buffer_pool.release(data);

  Ok(())
}

pub(crate) fn draw_inset_shadow(
  shadow: &SizedShadow,
  mut border: BorderProperties,
  border_box: Size<f32>,
  buffer_pool: &mut BufferPool,
) -> Result<(Vec<u8>, u32, u32)> {
  let width = border_box.width as u32;
  let height = border_box.height as u32;
  let [red, green, blue, alpha] = shadow.color.0;
  let mut shadow_alpha = buffer_pool.acquire_dirty((width * height) as usize);
  shadow_alpha.fill(alpha);

  let offset = Point {
    x: shadow.offset_x,
    y: shadow.offset_y,
  };

  let mut paths = Vec::new();

  border.expand_by(Sides([-shadow.spread_radius; 4]).into());
  border.append_mask_commands(
    &mut paths,
    border_box
      - Size {
        width: shadow.spread_radius * 2.0,
        height: shadow.spread_radius * 2.0,
      },
    offset
      + Point {
        x: shadow.spread_radius,
        y: shadow.spread_radius,
      },
  );

  let (mask, placement) = render_mask(&paths, None, Some(Fill::NonZero.into()), buffer_pool);

  if !mask.is_empty() {
    let shadow_placement = Placement {
      left: 0,
      top: 0,
      width,
      height,
    };
    attenuate_alpha_by_mask(&mut shadow_alpha, shadow_placement, &mask, placement);
  }
  buffer_pool.release(mask);

  apply_blur(
    BlurFormat::Alpha {
      data: &mut shadow_alpha,
      width,
      height,
    },
    shadow.blur_radius,
    BlurType::Shadow,
    buffer_pool,
  )?;

  let mut data = buffer_pool.acquire_dirty((width * height * 4) as usize);
  for (pixel, &alpha) in bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut data)
    .iter_mut()
    .zip(&shadow_alpha)
  {
    if alpha == u8::MAX {
      *pixel = [red, green, blue, alpha];
      continue;
    }
    if alpha == 0 {
      *pixel = [0, 0, 0, 0];
      continue;
    }

    let alpha_u32 = alpha as u32;
    *pixel = [
      fast_div_255(red as u32 * alpha_u32),
      fast_div_255(green as u32 * alpha_u32),
      fast_div_255(blue as u32 * alpha_u32),
      alpha,
    ];
  }
  buffer_pool.release(shadow_alpha);

  Ok((data, width, height))
}

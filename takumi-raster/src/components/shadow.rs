pub(crate) use crate::shadow::SizedShadow;
use crate::{
  BlurType, Canvas, Command, Fill, Placement, Result, Style, apply_blur_alpha_bytes,
  attenuate_alpha_by_mask, checked_area, render_mask,
  style::{Affine, BlendMode},
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
  let blur_padding = if shadow.blur_radius > 0.0 {
    shadow.blur_radius * BlurType::Shadow.extent_multiplier()
  } else {
    0.0
  };

  // The mask shifts by the shadow offset and bleeds by the blur extent before
  // it lands on the canvas, so the cull rect grows by both.
  let cull = canvas.viewport().inflate(
    blur_padding + shadow.offset_x.abs(),
    blur_padding + shadow.offset_y.abs(),
  );
  let (mask, mut placement) = render_mask(paths, Some(transform), Some(style), Some(cull));

  placement.left += shadow.offset_x as i32;
  placement.top += shadow.offset_y as i32;

  if shadow.blur_radius <= 0.0 && cutout_paths.is_none() {
    canvas.draw_mask(&mask, placement, shadow.color, BlendMode::Normal);
    return Ok(());
  }

  let total_padding = (blur_padding * 2.0) as u32;
  let shadow_width = placement.width.saturating_add(total_padding);
  let shadow_height = placement.height.saturating_add(total_padding);
  let Some(area) = checked_area(shadow_width, shadow_height, 1) else {
    return Ok(());
  };
  let mut shadow_alpha = vec![0; area];

  let padding = blur_padding as u32;
  for y in 0..placement.height {
    let src_row = y as usize * placement.width as usize;
    let dst_row = (y + padding) as usize * shadow_width as usize + padding as usize;
    shadow_alpha[dst_row..dst_row + placement.width as usize]
      .copy_from_slice(&mask[src_row..src_row + placement.width as usize]);
  }

  apply_blur_alpha_bytes(
    &mut shadow_alpha,
    shadow_width,
    shadow_height,
    shadow.blur_radius,
    BlurType::Shadow,
  )?;

  let shadow_placement = Placement {
    left: (placement.left as f32 - blur_padding) as i32,
    top: (placement.top as f32 - blur_padding) as i32,
    width: shadow_width,
    height: shadow_height,
  };

  if let Some(cutout_paths) = cutout_paths {
    let (erase_mask, erase_placement) = render_mask(
      cutout_paths,
      Some(transform),
      Some(Fill::NonZero.into()),
      Some(cull),
    );

    if !erase_mask.is_empty() {
      attenuate_alpha_by_mask(
        &mut shadow_alpha,
        shadow_placement,
        &erase_mask,
        erase_placement,
      );
    }
  }

  canvas.draw_mask(
    &shadow_alpha,
    shadow_placement,
    shadow.color,
    BlendMode::Normal,
  );
  Ok(())
}

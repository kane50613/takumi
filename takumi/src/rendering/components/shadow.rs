use taffy::{Layout, Point, Size};
use tiny_skia::{IntSize, Pixmap};

use crate::{
  Result,
  layout::style::{Affine, BlendMode, BoxShadow, Color, ImageScalingAlgorithm, Sides, TextShadow},
  rendering::{
    BlurFormat, BlurType, BorderProperties, BufferPool, Canvas, Command, Fill, Placement, Sizing,
    Style, apply_blur, fast_div_255, render_mask,
  },
};

/// Represents a resolved box shadow with all its properties.
#[derive(Clone, Copy)]
pub(crate) struct SizedShadow {
  /// Horizontal offset of the shadow.
  pub offset_x: f32,
  /// Vertical offset of the shadow.
  pub offset_y: f32,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  pub blur_radius: f32,
  /// Spread radius of the shadow. Positive values expand the shadow, negative values shrink it.
  pub spread_radius: f32,
  /// Color of the shadow.
  pub color: Color,
}

impl SizedShadow {
  /// Creates a new [`SizedShadow`] from a [`BoxShadow`].
  pub fn from_box_shadow(
    shadow: BoxShadow,
    sizing: &Sizing,
    current_color: Color,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, size.width),
      spread_radius: shadow.spread_radius.to_px(sizing, size.width),
      color: shadow.color.resolve(current_color),
    }
  }

  /// Creates a new `SizedShadow` from a `TextShadow`.
  pub fn from_text_shadow(
    shadow: TextShadow,
    sizing: &Sizing,
    current_color: Color,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, size.width),
      // Text shadows do not support spread radius; set to 0.
      spread_radius: 0.0,
      color: shadow.color.resolve(current_color),
    }
  }

  /// Draws the outset mask of the shadow.
  #[allow(clippy::too_many_arguments)]
  pub fn draw_outset(
    &self,
    canvas: &mut Canvas,
    paths: &[Command],
    transform: Affine,
    style: Style,
    cutout_paths: Option<&[Command]>,
  ) -> Result<()> {
    let (mask, mut placement) =
      render_mask(paths, Some(transform), Some(style), &mut canvas.buffer_pool);

    placement.left += self.offset_x as i32;
    placement.top += self.offset_y as i32;

    if self.blur_radius <= 0.0 && cutout_paths.is_none() {
      canvas.draw_mask(&mask, placement, self.color, BlendMode::Normal);
      canvas.buffer_pool.release(mask);
      return Ok(());
    }

    let blur_padding = if self.blur_radius > 0.0 {
      self.blur_radius * BlurType::Shadow.extent_multiplier()
    } else {
      0.0
    };

    let shadow_width = placement.width + (blur_padding * 2.0) as u32;
    let shadow_height = placement.height + (blur_padding * 2.0) as u32;
    let mut shadow_alpha = canvas
      .buffer_pool
      .acquire_dirty((shadow_width * shadow_height) as usize);
    shadow_alpha.fill(0);

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
      self.blur_radius,
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
        let shadow_width_usize = shadow_width as usize;
        for my in 0..erase_placement.height as i32 {
          for mx in 0..erase_placement.width as i32 {
            let canvas_x = erase_placement.left + mx;
            let canvas_y = erase_placement.top + my;
            let ix = canvas_x - img_origin_x as i32;
            let iy = canvas_y - img_origin_y as i32;

            if ix >= 0 && iy >= 0 && ix < shadow_width as i32 && iy < shadow_height as i32 {
              let mask_alpha =
                erase_mask[(my as u32 * erase_placement.width + mx as u32) as usize] as u32;
              if mask_alpha > 0 {
                let idx = iy as usize * shadow_width_usize + ix as usize;
                let factor = 255 - mask_alpha;
                shadow_alpha[idx] = fast_div_255(shadow_alpha[idx] as u32 * factor);
              }
            }
          }
        }
        canvas.buffer_pool.release(erase_mask);
      }
    }

    canvas.draw_mask(
      &shadow_alpha,
      Placement {
        left: img_origin_x as i32,
        top: img_origin_y as i32,
        width: shadow_width,
        height: shadow_height,
      },
      self.color,
      BlendMode::Normal,
    );
    canvas.buffer_pool.release(shadow_alpha);
    Ok(())
  }

  pub fn draw_inset(
    &self,
    transform: Affine,
    border_radius: BorderProperties,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let image = draw_inset_shadow(self, border_radius, layout.size, &mut canvas.buffer_pool)?;

    canvas.overlay_sampled_pixmap(
      &image,
      image.width(),
      image.height(),
      border_radius,
      transform,
      Affine::IDENTITY,
      ImageScalingAlgorithm::Auto,
      BlendMode::Normal,
    );

    Ok(())
  }
}

pub(crate) fn draw_inset_shadow(
  shadow: &SizedShadow,
  mut border: BorderProperties,
  border_box: Size<f32>,
  buffer_pool: &mut BufferPool,
) -> Result<Pixmap> {
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
    let img_w = width as i32;
    let img_h = height as i32;
    let img_w_usize = img_w as usize;

    for my in 0..placement.height as i32 {
      for mx in 0..placement.width as i32 {
        let ix = placement.left + mx;
        let iy = placement.top + my;

        if ix >= 0 && iy >= 0 && ix < img_w && iy < img_h {
          let mask_alpha = mask[(my as u32 * placement.width + mx as u32) as usize] as u32;
          if mask_alpha > 0 {
            let idx = iy as usize * img_w_usize + ix as usize;
            let factor = 255 - mask_alpha;
            shadow_alpha[idx] = fast_div_255(shadow_alpha[idx] as u32 * factor);
          }
        }
      }
    }
    buffer_pool.release(mask);
  }

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

  let mut data = vec![0u8; (width * height * 4) as usize];
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

  let size = IntSize::from_wh(width, height).unwrap_or_else(|| unreachable!());
  let pixmap = Pixmap::from_vec(data, size).unwrap_or_else(|| unreachable!());
  Ok(pixmap)
}

use image::RgbaImage;
use taffy::{Layout, Point, Size};

use crate::{
  Result,
  layout::style::{Affine, BlendMode, BoxShadow, Color, ImageScalingAlgorithm, Sides, TextShadow},
  rendering::{
    BlurFormat, BlurType, BorderProperties, BufferPool, Canvas, Command, Fill, Placement, Sizing,
    Style, apply_blur, draw_mask, overlay_image, render_mask,
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
      draw_mask(
        &mut canvas.image,
        &mask,
        placement,
        self.color,
        BlendMode::Normal,
        &canvas.constrains,
      );
      canvas.buffer_pool.release(mask);
      return Ok(());
    }

    let blur_padding = if self.blur_radius > 0.0 {
      self.blur_radius * BlurType::Shadow.extent_multiplier()
    } else {
      0.0
    };

    let mut image = canvas.buffer_pool.acquire_image(
      placement.width + (blur_padding * 2.0) as u32,
      placement.height + (blur_padding * 2.0) as u32,
    )?;

    draw_mask(
      &mut image,
      &mask,
      Placement {
        left: blur_padding as i32,
        top: blur_padding as i32,
        width: placement.width,
        height: placement.height,
      },
      self.color,
      BlendMode::Normal,
      &[],
    );
    canvas.buffer_pool.release(mask);

    apply_blur(
      BlurFormat::Rgba(&mut image),
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

      let img_w = image.width() as i32;
      let img_h = image.height() as i32;
      let img_w_usize = img_w as usize;

      if !erase_mask.is_empty() {
        let data = image.as_mut();
        for my in 0..erase_placement.height as i32 {
          for mx in 0..erase_placement.width as i32 {
            let canvas_x = erase_placement.left + mx;
            let canvas_y = erase_placement.top + my;
            let ix = canvas_x - img_origin_x as i32;
            let iy = canvas_y - img_origin_y as i32;

            if ix >= 0 && iy >= 0 && ix < img_w && iy < img_h {
              let mask_alpha =
                erase_mask[(my as u32 * erase_placement.width + mx as u32) as usize] as u32;
              if mask_alpha > 0 {
                let idx = (iy as usize * img_w_usize + ix as usize) * 4;
                let alpha = &mut data[idx + 3];
                *alpha = ((*alpha as u32 * (255 - mask_alpha)) / 255) as u8;
              }
            }
          }
        }
        canvas.buffer_pool.release(erase_mask);
      }
    }

    overlay_image(
      &mut canvas.image,
      &image,
      BorderProperties::zero(),
      Affine::translation(img_origin_x, img_origin_y),
      ImageScalingAlgorithm::Auto,
      BlendMode::Normal,
      &canvas.constrains,
      &mut canvas.buffer_pool,
    );

    canvas.buffer_pool.release_image(image);
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

    canvas.overlay_image(
      &image,
      border_radius,
      transform,
      ImageScalingAlgorithm::Auto,
      BlendMode::Normal,
    );

    canvas.buffer_pool.release_image(image);
    Ok(())
  }
}

pub(crate) fn draw_inset_shadow(
  shadow: &SizedShadow,
  mut border: BorderProperties,
  border_box: Size<f32>,
  buffer_pool: &mut BufferPool,
) -> Result<RgbaImage> {
  let mut shadow_image =
    buffer_pool.acquire_image(border_box.width as u32, border_box.height as u32)?;

  // Fill with shadow color (BufferPool returns zeroed/dirty buffers)
  let shadow_raw = shadow_image.as_mut();
  let color_rgba: [u8; 4] = shadow.color.0;
  for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(shadow_raw) {
    *pixel = color_rgba;
  }

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
    let img_w = shadow_image.width() as i32;
    let img_h = shadow_image.height() as i32;
    let img_w_usize = img_w as usize;
    let data = shadow_image.as_mut();

    for my in 0..placement.height as i32 {
      for mx in 0..placement.width as i32 {
        let ix = placement.left + mx;
        let iy = placement.top + my;

        if ix >= 0 && iy >= 0 && ix < img_w && iy < img_h {
          let mask_alpha = mask[(my as u32 * placement.width + mx as u32) as usize] as u32;
          if mask_alpha > 0 {
            let idx = (iy as usize * img_w_usize + ix as usize) * 4;
            let alpha = &mut data[idx + 3];
            *alpha = ((*alpha as u32 * (255 - mask_alpha)) / 255) as u8;
          }
        }
      }
    }
    buffer_pool.release(mask);
  }

  apply_blur(
    BlurFormat::Rgba(&mut shadow_image),
    shadow.blur_radius,
    BlurType::Shadow,
    buffer_pool,
  )?;

  Ok(shadow_image)
}

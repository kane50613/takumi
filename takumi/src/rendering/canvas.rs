use std::sync::Mutex;

use image::{
  Pixel, Rgba, RgbaImage,
  imageops::{interpolate_bilinear, interpolate_nearest},
};
use taffy::{Point, Size};
use zeno::{Mask, Placement};

use crate::{
  layout::style::{Affine, Angle, Color, ImageScalingAlgorithm},
  rendering::BorderProperties,
};

/// A canvas handle for sending drawing commands asynchronously.
///
/// This struct wraps a channel sender that can be cloned and used to send
/// drawing commands to a canvas rendering loop without blocking the main thread.
pub struct Canvas(Mutex<RgbaImage>);

impl Canvas {
  /// Creates a new canvas handle from a draw command sender.
  pub(crate) fn new(size: Size<u32>) -> Self {
    Self(Mutex::new(RgbaImage::new(size.width, size.height)))
  }

  pub(crate) fn into_inner(self) -> RgbaImage {
    self.0.into_inner().unwrap()
  }

  /// Overlays an image onto the canvas with optional border radius.
  pub(crate) fn overlay_image(
    &self,
    image: &RgbaImage,
    offset: Point<i32>,
    border: BorderProperties,
    transform: Affine,
    algorithm: ImageScalingAlgorithm,
  ) {
    if image.is_empty() {
      return;
    }

    let mut lock = self.0.lock().unwrap();
    overlay_image(&mut lock, image, offset, border, transform, algorithm);
  }

  /// Draws a mask with the specified color onto the canvas.
  pub(crate) fn draw_mask(
    &self,
    mask: &[u8],
    placement: Placement,
    color: Color,
    image: Option<RgbaImage>,
  ) {
    if mask.is_empty() {
      return;
    }

    let mut lock = self.0.lock().unwrap();
    draw_mask(&mut lock, mask, placement, color, image.as_ref());
  }

  /// Fills a rectangular area with the specified color and optional border radius.
  pub(crate) fn fill_color(
    &self,
    offset: Point<i32>,
    size: Size<u32>,
    color: Color,
    border: BorderProperties,
    transform: Affine,
  ) {
    if color.0[3] == 0 {
      return;
    }

    let mut lock = self.0.lock().unwrap();
    fill_color(&mut lock, size, offset, color, border, transform);
  }
}

/// Draws a single pixel on the canvas with alpha blending.
///
/// If the color is fully transparent (alpha = 0), no operation is performed.
/// Otherwise, the pixel is blended with the existing canvas pixel using alpha blending.
pub(crate) fn draw_pixel(canvas: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
  if color.0[3] == 0 {
    return;
  }

  // image-rs blend will skip the operation if the source color is fully transparent
  if let Some(pixel) = canvas.get_pixel_mut_checked(x, y) {
    if pixel.0[3] == 0 {
      // If the destination pixel is fully transparent, we directly assign the new color.
      // This is a performance optimization: blending with a fully transparent pixel is
      // equivalent to assignment, so we skip the blend operation. This deviates from the
      // standard alpha blending approach for efficiency.
      *pixel = color;
    } else {
      pixel.blend(&color);
    }
  }
}

pub(crate) fn apply_mask_alpha_to_pixel(mut pixel: Rgba<u8>, alpha: u8) -> Rgba<u8> {
  if alpha == u8::MAX {
    pixel
  } else {
    pixel.0[3] = ((pixel.0[3] as f32) * (alpha as f32 / 255.0)).round() as u8;

    pixel
  }
}

/// Draws a filled rectangle with a solid color.
pub(crate) fn fill_color<C: Into<Rgba<u8>>>(
  image: &mut RgbaImage,
  size: Size<u32>,
  offset: Point<i32>,
  color: C,
  radius: BorderProperties,
  transform: Affine,
) {
  let color: Rgba<u8> = color.into();

  // Fast path: if drawing on the entire canvas, we can just replace the entire canvas with the color
  if transform.is_identity()
    && radius.is_zero()
    && color.0[3] == 255
    && offset.x == 0
    && offset.y == 0
    && size.width == image.width()
    && size.height == image.height()
  {
    let image_mut = image.as_mut();
    let image_len = image_mut.len();

    for i in (0..image_len).step_by(4) {
      image_mut[i..i + 4].copy_from_slice(&color.0);
    }

    return;
  }

  let transform_part = transform.decompose();
  let can_direct_draw = transform_part.rotation == Angle::zero() && radius.is_zero();

  // Fast path: if no sub-pixel interpolation is needed, we can just draw the color directly
  if can_direct_draw {
    let transformed_size = Size {
      width: (size.width as f32 * transform_part.scale.width).round() as u32,
      height: (size.height as f32 * transform_part.scale.height).round() as u32,
    };

    let transformed_offset = Point {
      x: (offset.x as f32 + transform_part.translation.width).round() as i32,
      y: (offset.y as f32 + transform_part.translation.height).round() as i32,
    };

    for y in 0..transformed_size.height {
      for x in 0..transformed_size.width {
        let dest_x = x as i32 + transformed_offset.x;
        let dest_y = y as i32 + transformed_offset.y;

        if dest_x < 0 || dest_y < 0 {
          continue;
        }

        draw_pixel(image, dest_x as u32, dest_y as u32, color);
      }
    }

    return;
  }

  let mut paths = Vec::new();

  radius.append_mask_commands(&mut paths);
  transform.apply_on_paths(&mut paths);

  let (mask, mut placement) = Mask::new(&paths).render();

  placement.left += offset.x;
  placement.top += offset.y;

  draw_mask(image, &mask, placement, color, None);
}

pub(crate) fn draw_mask<C: Into<Rgba<u8>>>(
  canvas: &mut RgbaImage,
  mask: &[u8],
  placement: Placement,
  color: C,
  image: Option<&RgbaImage>,
) {
  let color: Rgba<u8> = color.into();
  let mut i = 0;

  for y in 0..placement.height {
    for x in 0..placement.width {
      let alpha = mask[i];
      i += 1;

      if alpha == 0 {
        continue;
      }

      let dest_x = x as i32 + placement.left;
      let dest_y = y as i32 + placement.top;

      if dest_x < 0 || dest_y < 0 {
        continue;
      }

      let pixel = image
        .map(|image| {
          let pixel = *image.get_pixel(x, y);
          apply_mask_alpha_to_pixel(pixel, alpha)
        })
        .unwrap_or_else(|| apply_mask_alpha_to_pixel(color, alpha));

      draw_pixel(canvas, dest_x as u32, dest_y as u32, pixel);
    }
  }
}

pub(crate) fn overlay_image(
  canvas: &mut RgbaImage,
  image: &RgbaImage,
  offset: Point<i32>,
  border: BorderProperties,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
) {
  let transform_part = transform.decompose();
  let can_direct_draw =
    !transform_part.is_rotated() && !transform_part.is_scaled() && border.is_zero();

  if can_direct_draw {
    let transformed_offset = Point {
      x: (offset.x as f32 + transform_part.translation.width).round() as i32,
      y: (offset.y as f32 + transform_part.translation.height).round() as i32,
    };

    for y in 0..image.height() {
      for x in 0..image.width() {
        let dest_x = x as i32 + transformed_offset.x;
        let dest_y = y as i32 + transformed_offset.y;

        if dest_x < 0 || dest_y < 0 {
          continue;
        }

        draw_pixel(canvas, dest_x as u32, dest_y as u32, *image.get_pixel(x, y));
      }
    }

    return;
  }

  let Some(inverse) = transform.invert() else {
    return;
  };

  let mut paths = Vec::new();

  border.append_mask_commands(&mut paths);
  transform.apply_on_paths(&mut paths);

  let (mask, placement) = Mask::new(&paths).render();

  let mut i = 0;

  for y in 0..placement.height {
    for x in 0..placement.width {
      let alpha = mask[i];
      i += 1;

      if alpha == 0 {
        continue;
      }

      let canvas_x = x as i32 + offset.x + placement.left;
      let canvas_y = y as i32 + offset.y + placement.top;

      if canvas_x < 0 || canvas_y < 0 {
        continue;
      }

      let point = Point {
        x: x as f32 + placement.left as f32,
        y: y as f32 + placement.top as f32,
      } * inverse;

      let sampled_pixel = match algorithm {
        ImageScalingAlgorithm::Pixelated => interpolate_nearest(image, point.x, point.y),
        _ => interpolate_bilinear(image, point.x, point.y),
      };

      if let Some(mut pixel) = sampled_pixel {
        if alpha != u8::MAX {
          pixel = apply_mask_alpha_to_pixel(pixel, alpha);
        }

        draw_pixel(canvas, canvas_x as u32, canvas_y as u32, pixel);
      }
    }
  }
}

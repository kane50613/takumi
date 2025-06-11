use image::{GenericImage, GenericImageView, Rgba, RgbaImage};
use imageproc::drawing::Blend;
use imageproc::{drawing::draw_filled_rect_mut, rect::Rect};
use taffy::Layout;

use crate::border_radius::apply_border_radius_antialiased;
use crate::node::draw::draw_image_overlay_fast;
use crate::node::style::Style;

/// Draws borders around the node with optional border radius.
///
/// This function draws borders with specified size and color. If border_radius is specified,
/// it creates a rounded border using a custom drawing approach.
///
/// # Arguments
/// * `style` - The style containing border properties
/// * `canvas` - The canvas to draw on
/// * `layout` - The layout information for positioning
pub fn draw_border(style: &Style, canvas: &mut Blend<RgbaImage>, layout: &Layout) {
  if layout.border.top == 0.0
    && layout.border.right == 0.0
    && layout.border.bottom == 0.0
    && layout.border.left == 0.0
  {
    return;
  }

  let border_color = style.inheritable_style.border_color.unwrap_or_default();
  let border_radius = style.inheritable_style.border_radius;

  if let Some(radius) = border_radius {
    draw_rounded_border(canvas, layout, border_color.into(), radius);
  } else {
    draw_rectangular_border(canvas, layout, border_color.into());
  }
}

/// Draws a rectangular border without rounded corners.
fn draw_rectangular_border(canvas: &mut Blend<RgbaImage>, layout: &Layout, color: Rgba<u8>) {
  // Top border
  if layout.border.top > 0.0 {
    draw_filled_rect_mut(
      canvas,
      Rect::at(layout.location.x as i32, layout.location.y as i32)
        .of_size(layout.size.width as u32, layout.border.top as u32),
      color,
    );
  }

  // Bottom border
  if layout.border.bottom > 0.0 {
    draw_filled_rect_mut(
      canvas,
      Rect::at(
        layout.location.x as i32,
        layout.location.y as i32 + layout.size.height as i32 - layout.border.bottom as i32,
      )
      .of_size(layout.size.width as u32, layout.border.bottom as u32),
      color,
    );
  }

  // Left border (excluding corners already drawn by top/bottom)
  if layout.border.left > 0.0 {
    draw_filled_rect_mut(
      canvas,
      Rect::at(
        layout.location.x as i32,
        layout.location.y as i32 + layout.border.top as i32,
      )
      .of_size(
        layout.border.left as u32,
        (layout.size.height - layout.border.top - layout.border.bottom) as u32,
      ),
      color,
    );
  }

  // Right border (excluding corners already drawn by top/bottom)
  if layout.border.right > 0.0 {
    draw_filled_rect_mut(
      canvas,
      Rect::at(
        layout.location.x as i32 + layout.size.width as i32 - layout.border.right as i32,
        layout.location.y as i32 + layout.border.top as i32,
      )
      .of_size(
        layout.border.right as u32,
        (layout.size.height - layout.border.top - layout.border.bottom) as u32,
      ),
      color,
    );
  }
}

/// Draws a rounded border with border radius.
fn draw_rounded_border(
  canvas: &mut Blend<RgbaImage>,
  layout: &Layout,
  color: Rgba<u8>,
  radius: f32,
) {
  if layout.border.left == 0.0
    && layout.border.right == 0.0
    && layout.border.top == 0.0
    && layout.border.bottom == 0.0
  {
    return;
  }

  // Create a temporary image filled with border color
  let mut border_image =
    RgbaImage::from_pixel(layout.size.width as u32, layout.size.height as u32, color);

  // Apply antialiased border radius to the outer edge
  apply_border_radius_antialiased(&mut border_image, radius);

  // Calculate inner bounds (content area)
  let inner_left = layout.border.left as u32;
  let inner_right = layout.size.width as u32 - layout.border.right as u32;
  let inner_top = layout.border.top as u32;
  let inner_bottom = layout.size.height as u32 - layout.border.bottom as u32;

  // Calculate inner radius (outer radius minus average border width, clamped to 0)
  let avg_border_width =
    (layout.border.left + layout.border.right + layout.border.top + layout.border.bottom) / 4.0;
  let inner_radius = (radius - avg_border_width).max(0.0);

  // Cut out the inner area if there's space for content
  if inner_left < inner_right && inner_top < inner_bottom {
    let inner_width = inner_right - inner_left;
    let inner_height = inner_bottom - inner_top;

    // Create inner cutout with antialiased border radius
    let mut inner_image =
      RgbaImage::from_pixel(inner_width, inner_height, Rgba([255, 255, 255, 255]));
    apply_border_radius_antialiased(&mut inner_image, inner_radius);

    // Cut out the inner area while preserving antialiasing from inner border
    for py in 0..inner_height {
      for px in 0..inner_width {
        let inner_pixel = unsafe { inner_image.unsafe_get_pixel(px, py) };
        // Use inverted alpha for cutting out - where inner has alpha, we remove border
        let cutout_alpha = 255 - inner_pixel[3];
        if cutout_alpha < 255 {
          let border_x = px + inner_left;
          let border_y = py + inner_top;
          let current_pixel = unsafe { border_image.unsafe_get_pixel(border_x, border_y) };
          // Blend the cutout with existing border color, preserving border's antialiasing
          let new_alpha = ((current_pixel[3] as u32 * cutout_alpha as u32) / 255) as u8;
          unsafe {
            border_image.unsafe_put_pixel(
              border_x,
              border_y,
              Rgba([
                current_pixel[0],
                current_pixel[1],
                current_pixel[2],
                new_alpha,
              ]),
            );
          }
        }
      }
    }
  }

  // Overlay the border image onto the canvas
  draw_image_overlay_fast(
    canvas,
    &border_image,
    layout.location.x as u32,
    layout.location.y as u32,
  );
}

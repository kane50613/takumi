use image::{Rgba, RgbaImage, imageops::fast_blur};
use imageproc::drawing::Canvas;
use taffy::Layout;

use crate::{
  border_radius::{BorderRadius, apply_border_radius_antialiased},
  color::ColorInput,
  node::style::{BoxShadow, BoxShadowInput, Style},
  render::RenderContext,
};

use crate::node::draw::{FastBlendImage, create_image_from_color_input};

/// Applies a fast blur to an image using image-rs's optimized implementation.
fn apply_fast_blur(image: &mut RgbaImage, radius: f32) {
  if radius <= 0.0 {
    return;
  }

  // Convert CSS blur radius to sigma for fast_blur
  // CSS blur radius is roughly 3x the standard deviation (sigma)
  let sigma = radius / 3.0;

  *image = fast_blur(image, sigma);
}

/// Draws box shadows for an element.
pub fn draw_box_shadow(
  box_shadow_input: &BoxShadowInput,
  style: &Style,
  context: &RenderContext,
  canvas: &mut FastBlendImage,
  layout: Layout,
) {
  let shadows = match box_shadow_input {
    BoxShadowInput::Single(shadow) => vec![shadow],
    BoxShadowInput::Multiple(shadows) => shadows.iter().collect(),
  };

  // Draw shadows from back to front (reverse order)
  for shadow in shadows.iter().rev() {
    draw_single_box_shadow(shadow, style, context, canvas, layout);
  }
}

/// Draws a single box shadow.
fn draw_single_box_shadow(
  shadow: &BoxShadow,
  style: &Style,
  context: &RenderContext,
  canvas: &mut FastBlendImage,
  layout: Layout,
) {
  if shadow.inset {
    draw_inset_shadow(shadow, style, context, canvas, layout);
  } else {
    draw_outset_shadow(shadow, style, context, canvas, layout);
  }
}

/// Draws an outset (external) box shadow.
fn draw_outset_shadow(
  shadow: &BoxShadow,
  style: &Style,
  context: &RenderContext,
  canvas: &mut FastBlendImage,
  layout: Layout,
) {
  let shadow = shadow.clone().resolve(context);
  let blur_extent = shadow.blur_radius * 2.0; // Blur extends in all directions
  let spread = shadow.spread_radius;

  // Calculate the full shadow bounds including blur
  let shadow_x = layout.location.x + shadow.offset_x - spread - blur_extent;
  let shadow_y = layout.location.y + shadow.offset_y - spread - blur_extent;
  let shadow_width = layout.size.width + (spread + blur_extent) * 2.0;
  let shadow_height = layout.size.height + (spread + blur_extent) * 2.0;

  // Skip if completely outside canvas
  let canvas_dims = canvas.dimensions();
  if shadow_x + shadow_width < 0.0
    || shadow_y + shadow_height < 0.0
    || shadow_x >= canvas_dims.0 as f32
    || shadow_y >= canvas_dims.1 as f32
  {
    return;
  }

  // Create the base shadow shape (element + spread)
  let base_width = (layout.size.width + spread * 2.0) as u32;
  let base_height = (layout.size.height + spread * 2.0) as u32;
  let mut shadow_image = create_image_from_color_input(&shadow.color, base_width, base_height);

  // Apply border radius to shadow shape
  if let Some(border_radius) = style.inheritable_style.border_radius {
    let mut adjusted_radius = BorderRadius::from_layout(context, &layout, border_radius.into());

    adjusted_radius.top_left += spread;
    adjusted_radius.top_right += spread;
    adjusted_radius.bottom_right += spread;
    adjusted_radius.bottom_left += spread;

    apply_border_radius_antialiased(&mut shadow_image, adjusted_radius);
  }

  // Apply blur if needed
  if shadow.blur_radius > 0.0 {
    // Expand canvas for blur to prevent edge artifacts
    let blur_padding = (shadow.blur_radius * 2.0) as u32;
    let padded_width = base_width + blur_padding * 2;
    let padded_height = base_height + blur_padding * 2;

    let mut padded_image = RgbaImage::new(padded_width, padded_height);

    // Center the shadow in the padded image
    for y in 0..shadow_image.height() {
      for x in 0..shadow_image.width() {
        let pixel = *shadow_image.get_pixel(x, y);
        padded_image.put_pixel(x + blur_padding, y + blur_padding, pixel);
      }
    }

    apply_fast_blur(&mut padded_image, shadow.blur_radius);
    shadow_image = padded_image;
  }

  // Calculate final position accounting for blur expansion
  let final_x = if shadow.blur_radius > 0.0 {
    shadow_x
  } else {
    layout.location.x + shadow.offset_x - spread
  };
  let final_y = if shadow.blur_radius > 0.0 {
    shadow_y
  } else {
    layout.location.y + shadow.offset_y - spread
  };

  // Draw the shadow with clipping
  draw_image_with_clipping(canvas, &shadow_image, final_x as i32, final_y as i32);
}

/// Draws an inset (internal) box shadow.
fn draw_inset_shadow(
  shadow: &BoxShadow,
  style: &Style,
  context: &RenderContext,
  canvas: &mut FastBlendImage,
  layout: Layout,
) {
  let shadow = shadow.clone().resolve(context);
  let content_box = layout.content_box_size();
  let element_x = layout.content_box_x();
  let element_y = layout.content_box_y();

  // Create element mask
  let mut element_mask = RgbaImage::from_pixel(
    content_box.width as u32,
    content_box.height as u32,
    Rgba([255, 255, 255, 255]),
  );

  // Apply border radius to mask
  if let Some(border_radius) = style.inheritable_style.border_radius {
    apply_border_radius_antialiased(
      &mut element_mask,
      BorderRadius::from_layout(context, &layout, border_radius.into()),
    );
  }

  // Create inverted shadow
  let blur_extent = shadow.blur_radius * 2.0;
  let shadow_width = (content_box.width + blur_extent * 2.0) as u32;
  let shadow_height = (content_box.height + blur_extent * 2.0) as u32;

  let mut shadow_image = RgbaImage::new(shadow_width, shadow_height);

  // Fill with shadow color, leaving hole for element
  for y in 0..shadow_height {
    for x in 0..shadow_width {
      let rel_x = x as f32 - blur_extent;
      let rel_y = y as f32 - blur_extent;

      // Check if we're inside the element bounds
      if rel_x >= -shadow.offset_x - shadow.spread_radius
        && rel_y >= -shadow.offset_y - shadow.spread_radius
        && rel_x < content_box.width - shadow.offset_x + shadow.spread_radius
        && rel_y < content_box.height - shadow.offset_y + shadow.spread_radius
      {
        let mask_x = (rel_x + shadow.offset_x + shadow.spread_radius) as u32;
        let mask_y = (rel_y + shadow.offset_y + shadow.spread_radius) as u32;

        if mask_x < element_mask.width() && mask_y < element_mask.height() {
          let mask_pixel = element_mask.get_pixel(mask_x, mask_y);
          if mask_pixel[3] == 0 {
            // Outside element shape, draw shadow
            let shadow_color: Rgba<u8> = match &shadow.color {
              ColorInput::Color(color) => (*color).into(),
              ColorInput::Gradient(_) => Rgba([0, 0, 0, 128]), // Fallback for gradients
            };
            shadow_image.put_pixel(x, y, shadow_color);
          }
        }
      }
    }
  }

  // Apply blur
  if shadow.blur_radius > 0.0 {
    apply_fast_blur(&mut shadow_image, shadow.blur_radius);
  }

  // Composite with element mask
  for y in 0..element_mask.height() {
    for x in 0..element_mask.width() {
      let mask_pixel = element_mask.get_pixel(x, y);
      if mask_pixel[3] > 0 {
        let shadow_x = x + blur_extent as u32;
        let shadow_y = y + blur_extent as u32;

        if shadow_x < shadow_image.width() && shadow_y < shadow_image.height() {
          let shadow_pixel = shadow_image.get_pixel(shadow_x, shadow_y);
          if shadow_pixel[3] > 0 {
            canvas.draw_pixel(element_x as u32 + x, element_y as u32 + y, *shadow_pixel);
          }
        }
      }
    }
  }
}

/// Draws an image with proper clipping to canvas bounds.
fn draw_image_with_clipping(canvas: &mut FastBlendImage, image: &RgbaImage, x: i32, y: i32) {
  let canvas_dims = canvas.dimensions();
  let (canvas_width, canvas_height) = (canvas_dims.0 as i32, canvas_dims.1 as i32);

  // Calculate clipping bounds
  let start_x = x.max(0);
  let start_y = y.max(0);
  let end_x = (x + image.width() as i32).min(canvas_width);
  let end_y = (y + image.height() as i32).min(canvas_height);

  // Draw only the visible portion
  for canvas_y in start_y..end_y {
    for canvas_x in start_x..end_x {
      let img_x = (canvas_x - x) as u32;
      let img_y = (canvas_y - y) as u32;

      if img_x < image.width() && img_y < image.height() {
        let pixel = *image.get_pixel(img_x, img_y);
        if pixel[3] > 0 {
          canvas.draw_pixel(canvas_x as u32, canvas_y as u32, pixel);
        }
      }
    }
  }
}

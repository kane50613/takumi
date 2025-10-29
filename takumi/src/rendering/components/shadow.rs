use std::borrow::Cow;

use image::{RgbaImage, imageops::fast_blur};
use taffy::{Layout, Point, Size};
use zeno::{Fill, Mask, Placement};

use crate::{
  layout::style::{Affine, BoxShadow, Color, ImageScalingAlgorithm, TextShadow},
  rendering::{BorderProperties, Canvas, RenderContext, apply_mask_alpha_to_pixel, draw_mask},
};

/// Applies a fast blur to an image using image-rs's optimized implementation.
pub(crate) fn apply_fast_blur(image: &mut RgbaImage, radius: f32) {
  if radius <= 0.0 {
    return;
  }

  // Convert CSS blur radius to sigma for fast_blur
  // CSS blur radius is roughly 3x the standard deviation (sigma)
  let sigma = radius / 3.0;

  *image = fast_blur(image, sigma);
}

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
  pub fn from_box_shadow(shadow: BoxShadow, context: &RenderContext, size: Size<f32>) -> Self {
    Self {
      offset_x: shadow.offset_x.resolve_to_px(context, size.width),
      offset_y: shadow.offset_y.resolve_to_px(context, size.height),
      blur_radius: shadow.blur_radius.resolve_to_px(context, size.width),
      spread_radius: shadow
        .spread_radius
        .resolve_to_px(context, size.width)
        .max(0.0),
      color: shadow.color.resolve(context.current_color, context.opacity),
    }
  }

  /// Creates a new `SizedShadow` from a `TextShadow`.
  pub fn from_text_shadow(shadow: TextShadow, context: &RenderContext, size: Size<f32>) -> Self {
    Self {
      offset_x: shadow.offset_x.resolve_to_px(context, size.width),
      offset_y: shadow.offset_y.resolve_to_px(context, size.height),
      blur_radius: shadow.blur_radius.resolve_to_px(context, size.width),
      // Text shadows do not support spread radius; set to 0.
      spread_radius: 0.0,
      color: shadow.color.resolve(context.current_color, context.opacity),
    }
  }

  pub fn draw_outset(
    &self,
    canvas: &Canvas,
    spread_mask: Cow<[u8]>,
    spread_placement: Placement,
    offset: Point<f32>,
  ) {
    let offset_with_radius = Point {
      x: (spread_placement.left as f32 + offset.x + self.offset_x
        - self.blur_radius
        - self.spread_radius) as i32,
      y: (spread_placement.top as f32 + offset.y + self.offset_y
        - self.blur_radius
        - self.spread_radius) as i32,
    };

    // Fast path: if the blur radius is 0, we can just draw the spread mask
    if self.blur_radius <= 0.0 {
      let placement = Placement {
        left: offset_with_radius.x,
        top: offset_with_radius.y,
        width: spread_placement.width,
        height: spread_placement.height,
      };

      return canvas.draw_mask(&spread_mask, placement, self.color, None);
    }

    // Create a new image with the spread mask on, blurred by the blur radius
    let mut image = RgbaImage::new(
      spread_placement.width + (self.blur_radius * 2.0) as u32,
      spread_placement.height + (self.blur_radius * 2.0) as u32,
    );

    draw_mask(
      &mut image,
      &spread_mask,
      Placement {
        left: self.blur_radius as i32,
        top: self.blur_radius as i32,
        width: spread_placement.width,
        height: spread_placement.height,
      },
      self.color,
      None,
    );

    apply_fast_blur(&mut image, self.blur_radius);

    canvas.overlay_image(
      &image,
      offset_with_radius,
      BorderProperties::zero(),
      Affine::identity(),
      ImageScalingAlgorithm::Auto,
    );
  }

  pub fn draw_inset(
    &self,
    transform: Affine,
    border_radius: BorderProperties,
    canvas: &Canvas,
    layout: Layout,
  ) {
    let image = draw_inset_shadow(self, border_radius, layout);

    canvas.overlay_image(
      &image,
      Point {
        x: layout.location.x as i32,
        y: layout.location.y as i32,
      },
      border_radius,
      transform,
      ImageScalingAlgorithm::Auto,
    );
  }
}

fn draw_inset_shadow(shadow: &SizedShadow, border: BorderProperties, layout: Layout) -> RgbaImage {
  let mut shadow_image = RgbaImage::from_pixel(
    layout.size.width as u32,
    layout.size.height as u32,
    shadow.color.into(),
  );

  let mut paths = Vec::new();

  let border = BorderProperties {
    offset: Point {
      x: shadow.offset_x,
      y: shadow.offset_y,
    },
    ..border
  };

  border.append_mask_commands(&mut paths);

  border
    .expand_by(-shadow.spread_radius)
    .append_mask_commands(&mut paths);

  let (mask, placement) = Mask::new(&paths).style(Fill::EvenOdd).render();

  let mut i = 0;

  for y in 0..placement.height {
    for x in 0..placement.width {
      let alpha = mask[i];

      i += 1;

      if alpha == u8::MAX {
        continue;
      }

      let x = x as i32 + placement.left;
      let y = y as i32 + placement.top;

      let color = apply_mask_alpha_to_pixel(shadow.color.0.into(), alpha);

      if let Some(pixel) = shadow_image.get_pixel_mut_checked(x as u32, y as u32) {
        *pixel = color;
      }
    }
  }

  apply_fast_blur(&mut shadow_image, shadow.blur_radius);

  shadow_image
}

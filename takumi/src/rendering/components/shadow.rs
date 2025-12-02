use image::{RgbaImage, imageops::fast_blur};
use taffy::{Layout, Point, Size};
use zeno::{Fill, PathData, Placement};

use crate::{
  layout::style::{Affine, BoxShadow, Color, ImageScalingAlgorithm, Sides, TextShadow},
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, MaskMemory, RenderContext, draw_mask, overlay_image,
  },
};

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

  /// Draws the outset mask of the shadow.
  pub fn draw_outset<D: PathData>(
    &self,
    canvas: &mut RgbaImage,
    mask_memory: &mut MaskMemory,
    constrain: Option<&CanvasConstrain>,
    paths: D,
    transform: Affine,
    style: zeno::Style,
  ) {
    let (mask, mut placement) = mask_memory.render(&paths, Some(transform), Some(style));

    placement.left += self.offset_x as i32;
    placement.top += self.offset_y as i32;

    // Fast path: if the blur radius is 0, we can just draw the spread mask
    if self.blur_radius <= 0.0 {
      return draw_mask(canvas, mask, placement, self.color, None, constrain);
    }

    // Create a new image with the spread mask on, blurred by the blur radius
    let mut image = RgbaImage::new(
      placement.width + (self.blur_radius * 2.0) as u32,
      placement.height + (self.blur_radius * 2.0) as u32,
    );

    draw_mask(
      &mut image,
      mask,
      Placement {
        left: self.blur_radius as i32,
        top: self.blur_radius as i32,
        width: placement.width,
        height: placement.height,
      },
      self.color,
      None,
      None,
    );

    apply_fast_blur(&mut image, self.blur_radius);

    overlay_image(
      canvas,
      &image,
      BorderProperties::zero(),
      Affine::translation(
        placement.left as f32 - self.blur_radius,
        placement.top as f32 - self.blur_radius,
      ),
      ImageScalingAlgorithm::Auto,
      None,
      constrain,
      mask_memory,
    );
  }

  pub fn draw_inset(
    &self,
    transform: Affine,
    border_radius: BorderProperties,
    canvas: &mut Canvas,
    layout: Layout,
  ) {
    let image = draw_inset_shadow(self, border_radius, layout.size, &mut canvas.mask_memory);

    canvas.overlay_image(
      &image,
      border_radius,
      transform,
      ImageScalingAlgorithm::Auto,
      None,
    );
  }
}

fn draw_inset_shadow(
  shadow: &SizedShadow,
  mut border: BorderProperties,
  border_box: Size<f32>,
  mask_memory: &mut MaskMemory,
) -> RgbaImage {
  let mut shadow_image = RgbaImage::from_pixel(
    border_box.width as u32,
    border_box.height as u32,
    shadow.color.into(),
  );

  let mut paths = Vec::new();

  let offset = Point {
    x: shadow.offset_x,
    y: shadow.offset_y,
  };

  border.append_mask_commands(&mut paths, border_box, offset);

  border.expand_by(Sides([shadow.spread_radius; 4]).into());
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

  let (mask, placement) = mask_memory.render(&paths, None, Some(Fill::EvenOdd.into()));

  draw_mask(&mut shadow_image, mask, placement, shadow.color, None, None);

  apply_fast_blur(&mut shadow_image, shadow.blur_radius);

  shadow_image
}

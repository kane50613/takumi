use image::RgbaImage;
use taffy::{Layout, Point, Size};
use zeno::{Fill, PathData, Placement};

use crate::{
  layout::style::{Affine, BoxShadow, Color, ImageScalingAlgorithm, Sides, TextShadow},
  rendering::{
    BlurType, BorderProperties, Canvas, CanvasConstrain, MaskMemory, Sizing, apply_blur, draw_mask,
    overlay_image,
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
    opacity: u8,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, size.width),
      spread_radius: shadow.spread_radius.to_px(sizing, size.width).max(0.0),
      color: shadow.color.resolve(current_color, opacity),
    }
  }

  /// Creates a new `SizedShadow` from a `TextShadow`.
  pub fn from_text_shadow(
    shadow: TextShadow,
    sizing: &Sizing,
    current_color: Color,
    opacity: u8,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, size.width),
      // Text shadows do not support spread radius; set to 0.
      spread_radius: 0.0,
      color: shadow.color.resolve(current_color, opacity),
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
      return draw_mask(canvas, mask, placement, self.color, constrain);
    }

    // Create a new image with the spread mask on, blurred by the blur radius
    let blur_padding = self.blur_radius * BlurType::Shadow.extent_multiplier();
    let mut image = RgbaImage::new(
      placement.width + (blur_padding * 2.0) as u32,
      placement.height + (blur_padding * 2.0) as u32,
    );

    draw_mask(
      &mut image,
      mask,
      Placement {
        left: blur_padding as i32,
        top: blur_padding as i32,
        width: placement.width,
        height: placement.height,
      },
      self.color,
      None,
    );

    apply_blur(&mut image, self.blur_radius, BlurType::Shadow);

    overlay_image(
      canvas,
      &image,
      BorderProperties::zero(),
      Affine::translation(
        placement.left as f32 - blur_padding,
        placement.top as f32 - blur_padding,
      ),
      ImageScalingAlgorithm::Auto,
      255,
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
      255,
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

  draw_mask(&mut shadow_image, mask, placement, shadow.color, None);

  apply_blur(&mut shadow_image, shadow.blur_radius, BlurType::Shadow);

  shadow_image
}

use crate::{
  geometry::Size,
  style::{BoxShadow, Color, SizingContext, TextShadow},
};

/// Represents a resolved box shadow with all its properties.
#[derive(Clone, Copy)]
pub struct SizedShadow {
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
  pub(crate) fn from_box_shadow(
    shadow: BoxShadow,
    sizing: &SizingContext,
    current_color: Color,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, 1.0),
      spread_radius: shadow.spread_radius.to_px(sizing, 1.0),
      color: shadow.color.resolve(current_color),
    }
  }

  /// Creates a new `SizedShadow` from a `TextShadow`.
  pub fn from_text_shadow(
    shadow: TextShadow,
    sizing: &SizingContext,
    current_color: Color,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, 1.0),
      // Text shadows do not support spread radius; set to 0.
      spread_radius: 0.0,
      color: shadow.color.resolve(current_color),
    }
  }
}

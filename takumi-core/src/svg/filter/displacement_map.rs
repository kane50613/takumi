use super::{BYTE_TO_FLOAT, ColorChannel, DisplacementMap};
use crate::geometry::Rect;

impl DisplacementMap {
  /// Blink's `FEDisplacementMap` as Skia's `sk_displacement` shader runs it, over the `window` of
  /// `dest`: each pixel takes the `src` pixel nearest its center moved by the straight `map`
  /// channels. `scale` is the layer's pixels per user unit on each axis.
  ///
  /// `src` and `map` are premultiplied, and all three are `width` pixels wide and the same length.
  pub(super) fn apply(
    &self,
    scale: [f32; 2],
    width: usize,
    window: Rect<usize>,
    src: &[[u8; 4]],
    map: &[[u8; 4]],
    dest: &mut [[u8; 4]],
  ) {
    let height = src.len() / width;
    let index = |channel| match channel {
      ColorChannel::R => 0,
      ColorChannel::G => 1,
      ColorChannel::B => 2,
      ColorChannel::A => 3,
    };
    let (x_channel, y_channel) = (index(self.x_channel), index(self.y_channel));
    let (scale_x, scale_y) = (scale[0] * self.scale, scale[1] * self.scale);

    for y in window.top..window.bottom {
      for x in window.left..window.right {
        let pixel = map[y * width + x];
        let alpha = f32::from(pixel[3]) * BYTE_TO_FLOAT;
        // `saturate(unpremul(displMap.eval(coord)))`.
        let straight = |channel: usize| {
          let value = f32::from(pixel[channel]) * BYTE_TO_FLOAT;

          if channel == 3 {
            value
          } else {
            (value / alpha.max(0.0001)).clamp(0.0, 1.0)
          }
        };
        let source_x = (x as f32 + 0.5 + scale_x * (straight(x_channel) - 0.5)).floor();
        let source_y = (y as f32 + 0.5 + scale_y * (straight(y_channel) - 0.5)).floor();

        if (0.0..width as f32).contains(&source_x) && (0.0..height as f32).contains(&source_y) {
          dest[y * width + x] = src[source_y as usize * width + source_x as usize];
        }
      }
    }
  }
}

use std::borrow::Cow;

use image::RgbaImage;

#[cfg(target_arch = "wasm32")]
mod image_webp;
#[cfg(not(target_arch = "wasm32"))]
mod libwebp;

#[cfg(target_arch = "wasm32")]
pub use image_webp::encode_animated_webp;
#[cfg(target_arch = "wasm32")]
pub(crate) use image_webp::write_webp;
#[cfg(not(target_arch = "wasm32"))]
pub use libwebp::encode_animated_webp;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use libwebp::write_webp;

pub(super) const U24_MAX: u32 = 0xffffff;

pub(super) fn strip_alpha_channel(image: Cow<'_, RgbaImage>) -> Vec<u8> {
  match image {
    Cow::Owned(image) => {
      let mut rgba = image.into_raw();
      let pixels = rgba.len() / 4;

      for pixel_index in 0..pixels {
        let src_offset = pixel_index * 4;
        let dst_offset = pixel_index * 3;
        rgba[dst_offset] = rgba[src_offset];
        rgba[dst_offset + 1] = rgba[src_offset + 1];
        rgba[dst_offset + 2] = rgba[src_offset + 2];
      }

      rgba.truncate(pixels * 3);
      rgba
    }
    Cow::Borrowed(image) => {
      let pixels = bytemuck::cast_slice::<u8, [u8; 4]>(image.as_raw());
      let mut rgb = Vec::with_capacity(pixels.len() * 3);

      for [r, g, b, _] in pixels {
        rgb.extend_from_slice(&[*r, *g, *b]);
      }

      rgb
    }
  }
}

pub(super) fn has_any_alpha_pixel(image: &RgbaImage) -> bool {
  bytemuck::cast_slice::<u8, [u8; 4]>(image.as_raw())
    .iter()
    .any(|[_, _, _, a]| *a != u8::MAX)
}

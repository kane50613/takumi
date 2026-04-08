use image::Rgba;
use tiny_skia::PremultipliedColorU8;

use crate::layout::style::BlendMode;
use crate::rendering::blend_pixel;

use super::{fast_div_255, premultiplied_to_rgba};

#[inline(always)]
pub(super) fn premultiply_rgba_pixel(red: u8, green: u8, blue: u8, alpha: u8) -> [u8; 4] {
  [
    fast_div_255(red as u32 * alpha as u32),
    fast_div_255(green as u32 * alpha as u32),
    fast_div_255(blue as u32 * alpha as u32),
    alpha,
  ]
}

#[inline(always)]
pub(super) fn premultiply_rgba(color: Rgba<u8>) -> [u8; 4] {
  let [red, green, blue, alpha] = color.0;
  premultiply_rgba_pixel(red, green, blue, alpha)
}

#[inline(always)]
pub(super) fn premultiplied_from_pixel(pixel: PremultipliedColorU8) -> [u8; 4] {
  [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
}

#[inline(always)]
pub(super) fn scale_premultiplied_pixel(pixel: [u8; 4], alpha: u8) -> [u8; 4] {
  if alpha == u8::MAX {
    return pixel;
  }

  [
    fast_div_255(pixel[0] as u32 * alpha as u32),
    fast_div_255(pixel[1] as u32 * alpha as u32),
    fast_div_255(pixel[2] as u32 * alpha as u32),
    fast_div_255(pixel[3] as u32 * alpha as u32),
  ]
}

#[inline(always)]
pub(super) fn composite_premultiplied_over(dst: &mut [u8; 4], src: [u8; 4]) {
  let src_alpha = src[3];
  if src_alpha == 0 {
    return;
  }

  let dst_alpha = dst[3];
  if src_alpha == u8::MAX || dst_alpha == 0 {
    *dst = src;
    return;
  }

  let inverse_alpha = u8::MAX - src_alpha;
  dst[0] = src[0].saturating_add(fast_div_255(dst[0] as u32 * inverse_alpha as u32));
  dst[1] = src[1].saturating_add(fast_div_255(dst[1] as u32 * inverse_alpha as u32));
  dst[2] = src[2].saturating_add(fast_div_255(dst[2] as u32 * inverse_alpha as u32));
  dst[3] = src_alpha.saturating_add(fast_div_255(dst_alpha as u32 * inverse_alpha as u32));
}

#[inline(always)]
pub(super) fn blend_premultiplied_pixel(dst: &mut [u8; 4], src: [u8; 4], mode: BlendMode) {
  if src[3] == 0 {
    return;
  }

  if mode == BlendMode::Normal {
    composite_premultiplied_over(dst, src);
    return;
  }

  if src[3] == u8::MAX && dst[3] == u8::MAX {
    let mut current = Rgba(*dst);
    let color = Rgba(src);
    blend_pixel(&mut current, color, mode);
    *dst = current.0;
    return;
  }

  let mut current = premultiplied_to_rgba(
    PremultipliedColorU8::from_rgba(
      dst[0].min(dst[3]),
      dst[1].min(dst[3]),
      dst[2].min(dst[3]),
      dst[3],
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT),
  );
  let color = premultiplied_to_rgba(
    PremultipliedColorU8::from_rgba(src[0], src[1], src[2], src[3])
      .unwrap_or(PremultipliedColorU8::TRANSPARENT),
  );
  blend_pixel(&mut current, color, mode);
  *dst = premultiply_rgba(current);
}

#[inline(always)]
pub(super) fn blend_premultiplied_pixel_normal(dst: &mut [u8], src: PremultipliedColorU8) {
  let src_a = src.alpha();
  if src_a == 0 {
    return;
  }

  if src_a == u8::MAX {
    dst[0] = src.red();
    dst[1] = src.green();
    dst[2] = src.blue();
    dst[3] = src_a;
    return;
  }

  let inv_src_a = u8::MAX - src_a;
  dst[0] = src
    .red()
    .saturating_add(fast_div_255(dst[0] as u32 * inv_src_a as u32));
  dst[1] = src
    .green()
    .saturating_add(fast_div_255(dst[1] as u32 * inv_src_a as u32));
  dst[2] = src
    .blue()
    .saturating_add(fast_div_255(dst[2] as u32 * inv_src_a as u32));
  dst[3] = src_a.saturating_add(fast_div_255(dst[3] as u32 * inv_src_a as u32));
}

pub(super) fn composite_premultiplied_over_span(dst: &mut [u8], pixels: &[PremultipliedColorU8]) {
  for (dst_pixel, src_pixel) in dst.chunks_exact_mut(4).zip(pixels) {
    blend_premultiplied_pixel_normal(dst_pixel, *src_pixel);
  }
}

pub(super) fn fill_repeated_premultiplied_pixel(dst: &mut [u8], pixel: [u8; 4]) {
  if dst.is_empty() {
    return;
  }

  dst[..4].copy_from_slice(&pixel);
  let mut written = 4;
  while written < dst.len() {
    let copy_len = written.min(dst.len() - written);
    let (filled, remaining) = dst.split_at_mut(written);
    remaining[..copy_len].copy_from_slice(&filled[..copy_len]);
    written += copy_len;
  }
}

pub(super) fn blend_repeated_premultiplied_pixel(dst: &mut [u8], pixel: PremultipliedColorU8) {
  for dst_pixel in dst.chunks_exact_mut(4) {
    blend_premultiplied_pixel_normal(dst_pixel, pixel);
  }
}

pub(super) fn composite_repeated_premultiplied_pixel_normal(
  dst: &mut [u8],
  pixel: PremultipliedColorU8,
) {
  let alpha = pixel.alpha();
  if alpha == 0 {
    return;
  }

  if alpha == u8::MAX {
    fill_repeated_premultiplied_pixel(
      dst,
      [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
    );
  } else {
    blend_repeated_premultiplied_pixel(dst, pixel);
  }
}

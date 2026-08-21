use image::Rgba;
use tiny_skia::PremultipliedColorU8;

use crate::{fast_div_255, fast_div_255_u32, style::BlendMode};

#[inline(always)]
fn composited_alpha(bottom_alpha: u32, top_alpha: u32) -> u32 {
  top_alpha + bottom_alpha - fast_div_255_u32(bottom_alpha * top_alpha)
}

#[inline(always)]
fn blend_plus_lighter(bottom: &mut Rgba<u8>, top: Rgba<u8>) {
  let result_alpha = top.0[3].saturating_add(bottom.0[3]);
  if result_alpha == 0 {
    return;
  }

  let top_alpha = top.0[3] as u32;
  let bottom_alpha = bottom.0[3] as u32;

  for i in 0..3 {
    let top_premul = fast_div_255(top.0[i] as u32 * top_alpha);
    let bottom_premul = fast_div_255(bottom.0[i] as u32 * bottom_alpha);
    bottom.0[i] = top_premul.saturating_add(bottom_premul);
  }

  bottom.0[3] = result_alpha;
}

pub(crate) fn blend_pixel(bottom: &mut Rgba<u8>, top: Rgba<u8>, mode: BlendMode) {
  if top.0[3] == 0 {
    return;
  }

  let top_alpha = top.0[3];
  let bottom_alpha = bottom.0[3];

  if bottom_alpha == 0 {
    *bottom = top;
    return;
  }

  match mode {
    BlendMode::Normal => {
      if top_alpha == 255 {
        *bottom = top;
        return;
      }

      if bottom_alpha == 255 {
        let alpha = top_alpha as u32;
        let inverse_alpha = 255 - alpha;

        for i in 0..3 {
          bottom.0[i] = fast_div_255(top[i] as u32 * alpha + bottom[i] as u32 * inverse_alpha);
        }
      } else {
        blend_normal_partial_transparency(bottom, top);
      }
    }
    BlendMode::PlusLighter => {
      blend_plus_lighter(bottom, top);
    }
    BlendMode::PlusDarker => {
      blend_with_float(bottom, top, mode);
    }
    BlendMode::Multiply
    | BlendMode::Screen
    | BlendMode::Darken
    | BlendMode::Lighten
    | BlendMode::ColorDodge
    | BlendMode::ColorBurn
    | BlendMode::Difference
    | BlendMode::Exclusion => {
      blend_with_integer(bottom, top, mode);
    }
    _ => {
      blend_with_float(bottom, top, mode);
    }
  }
}

#[inline(always)]
fn blend_normal_partial_transparency(bottom: &mut Rgba<u8>, top: Rgba<u8>) {
  let top_alpha = top.0[3] as u32;
  let bottom_alpha = bottom.0[3] as u32;

  let result_alpha = composited_alpha(bottom_alpha, top_alpha);

  if result_alpha == 0 {
    return;
  }

  let inverse_top_alpha = 255 - top_alpha;

  for i in 0..3 {
    let top_premul = top.0[i] as u32 * top_alpha;
    let bottom_premul = bottom.0[i] as u32 * bottom_alpha;
    let result_premul = top_premul + ((bottom_premul * inverse_top_alpha + 127) / 255);

    bottom.0[i] = ((result_premul + result_alpha / 2) / result_alpha).min(255) as u8;
  }

  bottom.0[3] = result_alpha.min(255) as u8;
}

#[inline(always)]
fn blend_with_integer(bottom: &mut Rgba<u8>, top: Rgba<u8>, mode: BlendMode) {
  let bottom_alpha = bottom.0[3] as u32;
  let top_alpha = top.0[3] as u32;
  let result_alpha = composited_alpha(bottom_alpha, top_alpha);

  if result_alpha == 0 {
    return;
  }
  const ROUNDING_OFFSET: u32 = 32512;
  const ALPHA_DIVISOR: u32 = 65025;
  const MAX_ALPHA: u32 = u8::MAX as u32;

  for i in 0..3 {
    let b = bottom.0[i];
    let t = top.0[i];
    let blended = blend_channel_integer(mode, b, t) as u32;
    let composited = ((MAX_ALPHA - top_alpha) * bottom_alpha * b as u32
      + (MAX_ALPHA - bottom_alpha) * top_alpha * t as u32
      + top_alpha * bottom_alpha * blended
      + ROUNDING_OFFSET)
      / ALPHA_DIVISOR;
    bottom.0[i] = ((composited * 255 + result_alpha / 2) / result_alpha).min(255) as u8;
  }

  bottom.0[3] = result_alpha.min(255) as u8;
}

#[inline(always)]
fn blend_channel_integer(mode: BlendMode, bottom: u8, top: u8) -> u8 {
  match mode {
    BlendMode::Multiply => fast_div_255(top as u32 * bottom as u32),
    BlendMode::Screen => 255 - fast_div_255((255 - top as u32) * (255 - bottom as u32)),
    BlendMode::Darken => top.min(bottom),
    BlendMode::Lighten => top.max(bottom),
    BlendMode::ColorDodge => color_dodge_integer(bottom, top),
    BlendMode::ColorBurn => color_burn_integer(bottom, top),
    BlendMode::Difference => top.abs_diff(bottom),
    BlendMode::Exclusion => (bottom as u32 + top as u32
      - (2 * fast_div_255_u32(bottom as u32 * top as u32)))
    .min(255) as u8,
    _ => top,
  }
}

#[inline(always)]
fn color_dodge_integer(bottom: u8, top: u8) -> u8 {
  if bottom == 0 {
    return 0;
  }

  if top == u8::MAX {
    return u8::MAX;
  }

  let bottom = u32::from(bottom);
  let top = u32::from(top);
  let max = u32::from(u8::MAX);

  if bottom + top >= max {
    return u8::MAX;
  }

  let denominator = max - top;
  ((bottom * max + denominator / 2) / denominator) as u8
}

#[inline(always)]
fn color_burn_integer(bottom: u8, top: u8) -> u8 {
  if bottom == u8::MAX {
    return u8::MAX;
  }

  if top == 0 {
    return 0;
  }

  let bottom = u32::from(bottom);
  let top = u32::from(top);
  let max = u32::from(u8::MAX);

  if bottom + top <= max {
    return 0;
  }

  let numerator = (bottom + top - max) * max;
  ((numerator + top / 2) / top) as u8
}

#[inline(always)]
fn blend_with_float(bottom: &mut Rgba<u8>, top: Rgba<u8>, mode: BlendMode) {
  let top_normalized = normalize_rgba(top);
  let bottom_normalized = normalize_rgba(*bottom);

  let result_alpha = top_normalized.alpha + bottom_normalized.alpha * (1.0 - top_normalized.alpha);

  if result_alpha <= 0.0 {
    bottom.0 = [0; 4];
    return;
  }

  let blended = compute_blend_float(mode, &bottom_normalized, &top_normalized);
  let composited = composite_float(&bottom_normalized, &top_normalized, &blended);

  for (pixel, composited_pixel) in bottom.0.iter_mut().zip(composited.iter()) {
    *pixel = (composited_pixel / result_alpha * 255.0)
      .round()
      .clamp(0.0, 255.0) as u8;
  }

  bottom.0[3] = (result_alpha * 255.0).round() as u8;
}

#[derive(Copy, Clone)]
struct NormalizedColor {
  channels: [f32; 3],
  alpha: f32,
}

impl NormalizedColor {
  #[inline(always)]
  fn red(&self) -> f32 {
    self.channels[0]
  }

  #[inline(always)]
  fn green(&self) -> f32 {
    self.channels[1]
  }

  #[inline(always)]
  fn blue(&self) -> f32 {
    self.channels[2]
  }
}

#[inline(always)]
fn normalize_rgba(color: Rgba<u8>) -> NormalizedColor {
  const INV_255: f32 = 1.0 / 255.0;
  let [r, g, b, a] = color.0;
  NormalizedColor {
    channels: [r as f32 * INV_255, g as f32 * INV_255, b as f32 * INV_255],
    alpha: a as f32 * INV_255,
  }
}

#[inline(always)]
fn compute_blend_float(
  mode: BlendMode,
  bottom: &NormalizedColor,
  top: &NormalizedColor,
) -> [f32; 3] {
  match mode {
    BlendMode::Overlay => [
      overlay(bottom.red(), top.red()),
      overlay(bottom.green(), top.green()),
      overlay(bottom.blue(), top.blue()),
    ],
    BlendMode::ColorDodge => [
      color_dodge(bottom.red(), top.red()),
      color_dodge(bottom.green(), top.green()),
      color_dodge(bottom.blue(), top.blue()),
    ],
    BlendMode::ColorBurn => [
      color_burn(bottom.red(), top.red()),
      color_burn(bottom.green(), top.green()),
      color_burn(bottom.blue(), top.blue()),
    ],
    BlendMode::HardLight => [
      overlay(top.red(), bottom.red()),
      overlay(top.green(), bottom.green()),
      overlay(top.blue(), bottom.blue()),
    ],
    BlendMode::SoftLight => [
      soft_light(bottom.red(), top.red()),
      soft_light(bottom.green(), top.green()),
      soft_light(bottom.blue(), top.blue()),
    ],
    BlendMode::Hue => {
      let color = set_sat(top.channels, sat(bottom.channels));
      set_lum(color, lum(bottom.channels))
    }
    BlendMode::Saturation => {
      let color = set_sat(bottom.channels, sat(top.channels));
      set_lum(color, lum(bottom.channels))
    }
    BlendMode::Color => set_lum(top.channels, lum(bottom.channels)),
    BlendMode::Luminosity => set_lum(bottom.channels, lum(top.channels)),
    BlendMode::PlusDarker => [
      (bottom.red() + top.red() - 1.0).max(0.0),
      (bottom.green() + top.green() - 1.0).max(0.0),
      (bottom.blue() + top.blue() - 1.0).max(0.0),
    ],
    _ => top.channels,
  }
}

#[inline(always)]
fn composite_float(
  bottom: &NormalizedColor,
  top: &NormalizedColor,
  blended: &[f32; 3],
) -> [f32; 3] {
  let inv_top_alpha = 1.0 - top.alpha;
  let inv_bottom_alpha = 1.0 - bottom.alpha;
  let alpha_product = top.alpha * bottom.alpha;

  let mut result = [0.0; 3];
  for i in 0..3 {
    result[i] = inv_top_alpha * bottom.alpha * bottom.channels[i]
      + inv_bottom_alpha * top.alpha * top.channels[i]
      + alpha_product * blended[i];
  }
  result
}

fn overlay(bottom: f32, top: f32) -> f32 {
  if bottom <= 0.5 {
    2.0 * bottom * top
  } else {
    1.0 - 2.0 * (1.0 - bottom) * (1.0 - top)
  }
}

fn color_dodge(bottom: f32, top: f32) -> f32 {
  if bottom == 0.0 {
    0.0
  } else if top >= 1.0 {
    1.0
  } else {
    (bottom / (1.0 - top)).min(1.0)
  }
}

fn color_burn(bottom: f32, top: f32) -> f32 {
  if bottom >= 1.0 {
    1.0
  } else if top <= 0.0 {
    0.0
  } else {
    1.0 - ((1.0 - bottom) / top).min(1.0)
  }
}

fn soft_light(bottom: f32, top: f32) -> f32 {
  if top <= 0.5 {
    bottom - (1.0 - 2.0 * top) * bottom * (1.0 - bottom)
  } else {
    let delta = if bottom <= 0.25 {
      ((16.0 * bottom - 12.0) * bottom + 4.0) * bottom
    } else {
      bottom.sqrt()
    };
    bottom + (2.0 * top - 1.0) * (delta - bottom)
  }
}

fn lum(color: [f32; 3]) -> f32 {
  0.3 * color[0] + 0.59 * color[1] + 0.11 * color[2]
}

fn set_lum(mut color: [f32; 3], luminosity: f32) -> [f32; 3] {
  let delta = luminosity - lum(color);
  color[0] += delta;
  color[1] += delta;
  color[2] += delta;
  clip_color(color)
}

fn clip_color(mut color: [f32; 3]) -> [f32; 3] {
  let luminosity = lum(color);
  let min_channel = color[0].min(color[1]).min(color[2]);
  let max_channel = color[0].max(color[1]).max(color[2]);

  if min_channel < 0.0 && (luminosity - min_channel).abs() > f32::EPSILON {
    for channel in color.iter_mut() {
      *channel = luminosity + (((*channel - luminosity) * luminosity) / (luminosity - min_channel));
    }
  }

  if max_channel > 1.0 && (max_channel - luminosity).abs() > f32::EPSILON {
    for channel in color.iter_mut() {
      *channel =
        luminosity + (((*channel - luminosity) * (1.0 - luminosity)) / (max_channel - luminosity));
    }
  }

  color
}

fn sat(color: [f32; 3]) -> f32 {
  color[0].max(color[1]).max(color[2]) - color[0].min(color[1]).min(color[2])
}

fn set_sat(mut color: [f32; 3], saturation: f32) -> [f32; 3] {
  let mut indices = [0, 1, 2];
  indices.sort_by(|&i, &j| color[i].total_cmp(&color[j]));

  let min_idx = indices[0];
  let mid_idx = indices[1];
  let max_idx = indices[2];

  if color[max_idx] > color[min_idx] {
    color[mid_idx] =
      ((color[mid_idx] - color[min_idx]) * saturation) / (color[max_idx] - color[min_idx]);
    color[max_idx] = saturation;
  } else {
    color[mid_idx] = 0.0;
    color[max_idx] = 0.0;
  }
  color[min_idx] = 0.0;
  color
}

const DEMUL: [f32; 256] = {
  let mut t = [0.0f32; 256];
  let mut i = 1usize;
  while i < 256 {
    t[i] = 255.0 / i as f32;
    i += 1;
  }
  t
};

#[inline(always)]
pub(crate) fn premultiply_rgba_pixel(red: u8, green: u8, blue: u8, alpha: u8) -> [u8; 4] {
  [
    fast_div_255(red as u32 * alpha as u32),
    fast_div_255(green as u32 * alpha as u32),
    fast_div_255(blue as u32 * alpha as u32),
    alpha,
  ]
}

#[inline(always)]
pub(crate) fn premultiply_rgba(color: Rgba<u8>) -> [u8; 4] {
  let [red, green, blue, alpha] = color.0;
  premultiply_rgba_pixel(red, green, blue, alpha)
}

/// Premultiplies a straight-alpha pixel into a `tiny_skia`
/// [`PremultipliedColorU8`].
pub(crate) fn color_to_premultiplied(color: Rgba<u8>) -> PremultipliedColorU8 {
  let [red, green, blue, alpha] = premultiply_rgba(color);
  PremultipliedColorU8::from_rgba(red, green, blue, alpha)
    .unwrap_or(PremultipliedColorU8::TRANSPARENT)
}

#[inline(always)]
pub(crate) fn premultiplied_from_pixel(pixel: PremultipliedColorU8) -> [u8; 4] {
  [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
}

#[inline(always)]
pub(crate) fn scale_premultiplied_pixel(pixel: [u8; 4], alpha: u8) -> [u8; 4] {
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
pub(crate) fn composite_premultiplied_over(dst: &mut [u8; 4], src: [u8; 4]) {
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
pub(crate) fn blend_premultiplied_pixel(dst: &mut [u8; 4], src: [u8; 4], mode: BlendMode) {
  if src[3] == 0 {
    return;
  }

  if mode == BlendMode::Normal {
    composite_premultiplied_over(dst, src);
    return;
  }

  blend_premultiplied_pixel_slow(dst, src, mode);
}

// Outlined so per-pixel loops only inline the normal-mode fast path; keeps one
// copy of the non-normal blend machinery in the binary (wasm size).
#[inline(never)]
fn blend_premultiplied_pixel_slow(dst: &mut [u8; 4], src: [u8; 4], mode: BlendMode) {
  if src[3] == u8::MAX && dst[3] == u8::MAX {
    let mut current = Rgba(*dst);
    let color = Rgba(src);
    blend_pixel(&mut current, color, mode);
    *dst = current.0;
    return;
  }

  let dst_a = dst[3];
  let mut current = if dst_a == 0 {
    Rgba([0, 0, 0, 0])
  } else if dst_a == u8::MAX {
    Rgba([dst[0], dst[1], dst[2], dst_a])
  } else {
    let inv = DEMUL[dst_a as usize];
    Rgba([
      (dst[0] as f32 * inv + 0.5) as u8,
      (dst[1] as f32 * inv + 0.5) as u8,
      (dst[2] as f32 * inv + 0.5) as u8,
      dst_a,
    ])
  };

  let src_a = src[3];
  let color = if src_a == 0 {
    Rgba([0, 0, 0, 0])
  } else if src_a == u8::MAX {
    Rgba([src[0], src[1], src[2], src_a])
  } else {
    let inv = DEMUL[src_a as usize];
    Rgba([
      (src[0] as f32 * inv + 0.5) as u8,
      (src[1] as f32 * inv + 0.5) as u8,
      (src[2] as f32 * inv + 0.5) as u8,
      src_a,
    ])
  };

  blend_pixel(&mut current, color, mode);
  *dst = premultiply_rgba(current);
}

#[inline(always)]
pub(crate) fn blend_premultiplied_pixel_normal(dst: &mut [u8], src: PremultipliedColorU8) {
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

pub(crate) fn composite_premultiplied_over_span(dst: &mut [u8], pixels: &[PremultipliedColorU8]) {
  for (dst_pixel, src_pixel) in dst.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
    let src_a = src_pixel.alpha();
    let inv_src_a = (u8::MAX - src_a) as u32;
    dst_pixel[0] = src_pixel
      .red()
      .saturating_add(fast_div_255(dst_pixel[0] as u32 * inv_src_a));
    dst_pixel[1] = src_pixel
      .green()
      .saturating_add(fast_div_255(dst_pixel[1] as u32 * inv_src_a));
    dst_pixel[2] = src_pixel
      .blue()
      .saturating_add(fast_div_255(dst_pixel[2] as u32 * inv_src_a));
    dst_pixel[3] = src_a.saturating_add(fast_div_255(dst_pixel[3] as u32 * inv_src_a));
  }
}

pub(crate) fn fill_repeated_premultiplied_pixel(dst: &mut [u8], pixel: [u8; 4]) {
  bytemuck::cast_slice_mut::<u8, [u8; 4]>(dst).fill(pixel);
}

pub(crate) fn blend_repeated_premultiplied_pixel(dst: &mut [u8], pixel: PremultipliedColorU8) {
  for dst_pixel in dst.as_chunks_mut::<4>().0 {
    blend_premultiplied_pixel_normal(dst_pixel, pixel);
  }
}

pub(crate) fn composite_repeated_premultiplied_pixel_normal(
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

#[cfg(test)]
mod tests {
  use image::Rgba;

  use crate::{blend::blend_pixel, style::BlendMode};

  // https://github.com/kane50613/takumi/issues/447
  #[test]
  fn plus_lighter_issue_447() {
    let mut bottom = Rgba([0xF5, 0xB1, 0x2D, 0xFF]);
    let top = Rgba([0xFF, 0xFF, 0xFF, 0x7F]);

    blend_pixel(&mut bottom, top, BlendMode::PlusLighter);

    assert_eq!(bottom, Rgba([0xFF, 0xFF, 0xAC, 0xFF]));
  }

  // https://github.com/kane50613/takumi/issues/501
  #[test]
  fn plus_darker_issue_501() {
    let mut bottom = Rgba([0x96, 0x77, 0x00, 0xFF]);
    let top = Rgba([0xFF, 0xFF, 0xFF, 0xFF]);

    blend_pixel(&mut bottom, top, BlendMode::PlusDarker);

    assert_eq!(bottom, Rgba([0x96, 0x77, 0x00, 0xFF]));
  }

  // https://github.com/kane50613/takumi/issues/643
  #[test]
  fn plus_darker_preserves_opaque_backdrop_under_partial_white() {
    let mut bottom = Rgba([0x96, 0x77, 0x00, 0xFF]);
    let top = Rgba([0xFF, 0xFF, 0xFF, 0x7F]);

    blend_pixel(&mut bottom, top, BlendMode::PlusDarker);

    assert_eq!(bottom, Rgba([0x96, 0x77, 0x00, 0xFF]));
  }

  // https://github.com/kane50613/takumi/issues/643
  #[test]
  fn plus_darker_with_low_opacity_black_stays_faint() {
    let mut bottom = Rgba([0x96, 0x77, 0x00, 0xFF]);
    let top = Rgba([0x00, 0x00, 0x00, 0x1A]);

    blend_pixel(&mut bottom, top, BlendMode::PlusDarker);

    assert_eq!(bottom, Rgba([0x87, 0x6B, 0x00, 0xFF]));
  }
}

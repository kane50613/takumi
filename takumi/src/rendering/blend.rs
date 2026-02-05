use image::Rgba;

use crate::{layout::style::BlendMode, rendering::fast_div_255};

#[inline(always)]
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
        let alpha = top_alpha as u16;
        let inverse_alpha = 255 - alpha;

        for i in 0..3 {
          bottom.0[i] = fast_div_255(top[i] as u16 * alpha + bottom[i] as u16 * inverse_alpha);
        }
      } else {
        blend_normal_partial_transparency(bottom, top);
      }
    }
    BlendMode::Multiply
    | BlendMode::Screen
    | BlendMode::Darken
    | BlendMode::Lighten
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

  let result_alpha =
    top_alpha + bottom_alpha - fast_div_255(bottom.0[3] as u16 * top.0[3] as u16) as u32;

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
  let bottom_alpha = bottom.0[3] as u16;
  let top_alpha = top.0[3] as u16;

  let result_alpha =
    top_alpha as u32 + bottom_alpha as u32 - fast_div_255(bottom_alpha * top_alpha) as u32;

  if result_alpha == 0 {
    return;
  }

  let blended = compute_blend_integer(mode, *bottom, top);
  let composited = composite_integer(*bottom, top, &blended);

  for (channel, composited_channel) in bottom.0.iter_mut().zip(composited.iter()) {
    *channel = ((composited_channel * 255 + result_alpha / 2) / result_alpha).min(255) as u8;
  }

  bottom.0[3] = result_alpha.min(255) as u8;
}

#[inline(always)]
fn compute_blend_integer(mode: BlendMode, bottom: Rgba<u8>, top: Rgba<u8>) -> [u8; 3] {
  let [bottom_r, bottom_g, bottom_b, _] = bottom.0;
  let [top_r, top_g, top_b, _] = top.0;

  match mode {
    BlendMode::Multiply => [
      fast_div_255(top_r as u16 * bottom_r as u16),
      fast_div_255(top_g as u16 * bottom_g as u16),
      fast_div_255(top_b as u16 * bottom_b as u16),
    ],
    BlendMode::Screen => [
      255 - fast_div_255((255 - top_r as u16) * (255 - bottom_r as u16)),
      255 - fast_div_255((255 - top_g as u16) * (255 - bottom_g as u16)),
      255 - fast_div_255((255 - top_b as u16) * (255 - bottom_b as u16)),
    ],
    BlendMode::Darken => [
      top_r.min(bottom_r),
      top_g.min(bottom_g),
      top_b.min(bottom_b),
    ],
    BlendMode::Lighten => [
      top_r.max(bottom_r),
      top_g.max(bottom_g),
      top_b.max(bottom_b),
    ],
    BlendMode::Difference => [
      top_r.abs_diff(bottom_r),
      top_g.abs_diff(bottom_g),
      top_b.abs_diff(bottom_b),
    ],
    BlendMode::Exclusion => [
      (bottom_r as u16 + top_r as u16 - (2 * fast_div_255(bottom_r as u16 * top_r as u16) as u16))
        .min(255) as u8,
      (bottom_g as u16 + top_g as u16 - (2 * fast_div_255(bottom_g as u16 * top_g as u16) as u16))
        .min(255) as u8,
      (bottom_b as u16 + top_b as u16 - (2 * fast_div_255(bottom_b as u16 * top_b as u16) as u16))
        .min(255) as u8,
    ],
    _ => unreachable!(),
  }
}

#[inline(always)]
fn composite_integer(bottom: Rgba<u8>, top: Rgba<u8>, blended: &[u8; 3]) -> [u32; 3] {
  const ROUNDING_OFFSET: u32 = 32512;
  const ALPHA_DIVISOR: u32 = 65025;
  const MAX_ALPHA: u32 = u8::MAX as u32;

  let top_alpha = top.0[3] as u32;
  let bottom_alpha = bottom.0[3] as u32;

  let mut result = [0u32; 3];
  for i in 0..3 {
    result[i] = ((MAX_ALPHA - top_alpha) * bottom_alpha * bottom.0[i] as u32
      + (MAX_ALPHA - bottom_alpha) * top_alpha * top.0[i] as u32
      + top_alpha * bottom_alpha * blended[i] as u32
      + ROUNDING_OFFSET)
      / ALPHA_DIVISOR;
  }

  result
}

#[inline(always)]
fn blend_with_float(bottom: &mut Rgba<u8>, top: Rgba<u8>, mode: BlendMode) {
  let top_normalized = normalize_rgba(top);
  let bottom_normalized = normalize_rgba(*bottom);

  let result_alpha = top_normalized.alpha + bottom_normalized.alpha * (1.0 - top_normalized.alpha);

  if result_alpha <= 0.0 {
    bottom.0 = [0, 0, 0, 0];
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
  red: f32,
  green: f32,
  blue: f32,
  alpha: f32,
}

#[inline(always)]
fn normalize_rgba(color: Rgba<u8>) -> NormalizedColor {
  let [r, g, b, a] = color.0;
  NormalizedColor {
    red: r as f32 / 255.0,
    green: g as f32 / 255.0,
    blue: b as f32 / 255.0,
    alpha: a as f32 / 255.0,
  }
}

#[inline(always)]
fn compute_blend_float(
  mode: BlendMode,
  bottom: &NormalizedColor,
  top: &NormalizedColor,
) -> [f32; 3] {
  match mode {
    BlendMode::Normal => [top.red, top.green, top.blue],
    BlendMode::Multiply => [
      top.red * bottom.red,
      top.green * bottom.green,
      top.blue * bottom.blue,
    ],
    BlendMode::Screen => [
      1.0 - (1.0 - top.red) * (1.0 - bottom.red),
      1.0 - (1.0 - top.green) * (1.0 - bottom.green),
      1.0 - (1.0 - top.blue) * (1.0 - bottom.blue),
    ],
    BlendMode::Overlay => [
      overlay(bottom.red, top.red),
      overlay(bottom.green, top.green),
      overlay(bottom.blue, top.blue),
    ],
    BlendMode::Darken => [
      top.red.min(bottom.red),
      top.green.min(bottom.green),
      top.blue.min(bottom.blue),
    ],
    BlendMode::Lighten => [
      top.red.max(bottom.red),
      top.green.max(bottom.green),
      top.blue.max(bottom.blue),
    ],
    BlendMode::ColorDodge => [
      color_dodge(bottom.red, top.red),
      color_dodge(bottom.green, top.green),
      color_dodge(bottom.blue, top.blue),
    ],
    BlendMode::ColorBurn => [
      color_burn(bottom.red, top.red),
      color_burn(bottom.green, top.green),
      color_burn(bottom.blue, top.blue),
    ],
    BlendMode::HardLight => [
      overlay(top.red, bottom.red),
      overlay(top.green, bottom.green),
      overlay(top.blue, bottom.blue),
    ],
    BlendMode::SoftLight => [
      soft_light(bottom.red, top.red),
      soft_light(bottom.green, top.green),
      soft_light(bottom.blue, top.blue),
    ],
    BlendMode::Difference => [
      (top.red - bottom.red).abs(),
      (top.green - bottom.green).abs(),
      (top.blue - bottom.blue).abs(),
    ],
    BlendMode::Exclusion => [
      bottom.red + top.red - 2.0 * bottom.red * top.red,
      bottom.green + top.green - 2.0 * bottom.green * top.green,
      bottom.blue + top.blue - 2.0 * bottom.blue * top.blue,
    ],
    BlendMode::Hue => {
      let color = set_sat(
        [top.red, top.green, top.blue],
        sat([bottom.red, bottom.green, bottom.blue]),
      );

      set_lum(color, lum([bottom.red, bottom.green, bottom.blue]))
    }
    BlendMode::Saturation => {
      let color = set_sat(
        [bottom.red, bottom.green, bottom.blue],
        sat([top.red, top.green, top.blue]),
      );

      set_lum(color, lum([bottom.red, bottom.green, bottom.blue]))
    }
    BlendMode::Color => set_lum(
      [top.red, top.green, top.blue],
      lum([bottom.red, bottom.green, bottom.blue]),
    ),
    BlendMode::Luminosity => set_lum(
      [bottom.red, bottom.green, bottom.blue],
      lum([top.red, top.green, top.blue]),
    ),
  }
}

#[inline(always)]
fn composite_float(
  bottom: &NormalizedColor,
  top: &NormalizedColor,
  blended: &[f32; 3],
) -> [f32; 3] {
  [
    (1.0 - top.alpha) * bottom.alpha * bottom.red
      + (1.0 - bottom.alpha) * top.alpha * top.red
      + top.alpha * bottom.alpha * blended[0],
    (1.0 - top.alpha) * bottom.alpha * bottom.green
      + (1.0 - bottom.alpha) * top.alpha * top.green
      + top.alpha * bottom.alpha * blended[1],
    (1.0 - top.alpha) * bottom.alpha * bottom.blue
      + (1.0 - bottom.alpha) * top.alpha * top.blue
      + top.alpha * bottom.alpha * blended[2],
  ]
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

  if min_channel < 0.0 {
    for channel in color.iter_mut() {
      *channel = luminosity + (((*channel - luminosity) * luminosity) / (luminosity - min_channel));
    }
  }
  if max_channel > 1.0 {
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

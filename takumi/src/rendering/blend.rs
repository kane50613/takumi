use image::Rgba;

use crate::{layout::style::BlendMode, rendering::fast_div_255};

/// Blends two pixels using the specified blend mode.
#[inline(always)]
pub(crate) fn blend_pixel(bottom: &mut Rgba<u8>, top: Rgba<u8>, mode: BlendMode) {
  let [src_r, src_g, src_b, src_a] = top.0;
  if src_a == 0 {
    return;
  }

  if mode == BlendMode::Normal {
    let [dst_r, dst_g, dst_b, dst_a] = bottom.0;
    match (dst_a, src_a) {
      // Source is fully transparent, handled above but for completeness
      (_, 0) => {}
      // Destination is fully transparent, or source is fully opaque: direct assignment
      (0, _) | (_, 255) => *bottom = top,
      // Destination is fully opaque: use integer math to preserve alpha = 255
      (255, src_a) => {
        let src_a = src_a as u16;
        let inv_a = 255 - src_a;

        bottom.0[0] = fast_div_255(src_r as u16 * src_a + dst_r as u16 * inv_a);
        bottom.0[1] = fast_div_255(src_g as u16 * src_a + dst_g as u16 * inv_a);
        bottom.0[2] = fast_div_255(src_b as u16 * src_a + dst_b as u16 * inv_a);
      }
      // Both have partial transparency: use integer-only blend
      (dst_a, src_a) => {
        // Alpha compositing: out_a = src_a + dst_a * (1 - src_a)
        // Using integer math: out_a = src_a + dst_a - (dst_a * src_a) / 255
        let src_a_u16 = src_a as u16;
        let dst_a_u16 = dst_a as u16;
        let out_a = src_a_u16 + dst_a_u16 - fast_div_255(dst_a_u16 * src_a_u16) as u16;

        if out_a == 0 {
          return;
        }

        // Premultiply RGB channels: premul = color * alpha
        let src_r_pm = src_r as u32 * src_a as u32;
        let src_g_pm = src_g as u32 * src_a as u32;
        let src_b_pm = src_b as u32 * src_a as u32;

        let dst_r_pm = dst_r as u32 * dst_a as u32;
        let dst_g_pm = dst_g as u32 * dst_a as u32;
        let dst_b_pm = dst_b as u32 * dst_a as u32;

        // Alpha compositing on premultiplied: out_pm = src_pm + dst_pm * (255 - src_a) / 255
        let inv_src_a = 255 - src_a as u32;
        let out_r_pm = src_r_pm + ((dst_r_pm * inv_src_a + 127) / 255);
        let out_g_pm = src_g_pm + ((dst_g_pm * inv_src_a + 127) / 255);
        let out_b_pm = src_b_pm + ((dst_b_pm * inv_src_a + 127) / 255);

        // Unpremultiply: out = out_pm / out_a
        let out_a_u32 = out_a as u32;

        bottom.0[0] = ((out_r_pm + out_a_u32 / 2) / out_a_u32).min(255) as u8;
        bottom.0[1] = ((out_g_pm + out_a_u32 / 2) / out_a_u32).min(255) as u8;
        bottom.0[2] = ((out_b_pm + out_a_u32 / 2) / out_a_u32).min(255) as u8;
        bottom.0[3] = out_a.min(255) as u8;
      }
    }
    return;
  }

  let [dst_r, dst_g, dst_b, dst_a] = bottom.0;

  match (dst_a, src_a) {
    // Source is fully transparent, handled above but for completeness
    (_, 0) => {}
    // Destination is fully transparent: direct assignment (all blend modes behave this way)
    (0, _) => *bottom = top,
    // Both have some opacity: use the blend mode
    (dst_a, src_a) => {
      let src_r_f = src_r as f32 / 255.0;
      let src_g_f = src_g as f32 / 255.0;
      let src_b_f = src_b as f32 / 255.0;
      let src_a_f = src_a as f32 / 255.0;

      let dst_r_f = dst_r as f32 / 255.0;
      let dst_g_f = dst_g as f32 / 255.0;
      let dst_b_f = dst_b as f32 / 255.0;
      let dst_a_f = dst_a as f32 / 255.0;

      let out_a_f = src_a_f + dst_a_f * (1.0 - src_a_f);

      if out_a_f <= 0.0 {
        bottom.0 = [0, 0, 0, 0];
        return;
      }

      let (res_r, res_g, res_b) = match mode {
        BlendMode::Normal => (src_r_f, src_g_f, src_b_f),
        BlendMode::Multiply => (src_r_f * dst_r_f, src_g_f * dst_g_f, src_b_f * dst_b_f),
        BlendMode::Screen => (
          1.0 - (1.0 - src_r_f) * (1.0 - dst_r_f),
          1.0 - (1.0 - src_g_f) * (1.0 - dst_g_f),
          1.0 - (1.0 - src_b_f) * (1.0 - dst_b_f),
        ),
        BlendMode::Overlay => (
          overlay(dst_r_f, src_r_f),
          overlay(dst_g_f, src_g_f),
          overlay(dst_b_f, src_b_f),
        ),
        BlendMode::Darken => (
          src_r_f.min(dst_r_f),
          src_g_f.min(dst_g_f),
          src_b_f.min(dst_b_f),
        ),
        BlendMode::Lighten => (
          src_r_f.max(dst_r_f),
          src_g_f.max(dst_g_f),
          src_b_f.max(dst_b_f),
        ),
        BlendMode::ColorDodge => (
          color_dodge(dst_r_f, src_r_f),
          color_dodge(dst_g_f, src_g_f),
          color_dodge(dst_b_f, src_b_f),
        ),
        BlendMode::ColorBurn => (
          color_burn(dst_r_f, src_r_f),
          color_burn(dst_g_f, src_g_f),
          color_burn(dst_b_f, src_b_f),
        ),
        BlendMode::HardLight => (
          overlay(src_r_f, dst_r_f),
          overlay(src_g_f, dst_g_f),
          overlay(src_b_f, dst_b_f),
        ),
        BlendMode::SoftLight => (
          soft_light(dst_r_f, src_r_f),
          soft_light(dst_g_f, src_g_f),
          soft_light(dst_b_f, src_b_f),
        ),
        BlendMode::Difference => (
          (src_r_f - dst_r_f).abs(),
          (src_g_f - dst_g_f).abs(),
          (src_b_f - dst_b_f).abs(),
        ),
        BlendMode::Exclusion => (
          dst_r_f + src_r_f - 2.0 * dst_r_f * src_r_f,
          dst_g_f + src_g_f - 2.0 * dst_g_f * src_g_f,
          dst_b_f + src_b_f - 2.0 * dst_b_f * src_b_f,
        ),
        BlendMode::Hue => {
          let c = set_sat(
            [src_r_f, src_g_f, src_b_f],
            sat([dst_r_f, dst_g_f, dst_b_f]),
          );
          let c = set_lum(c, lum([dst_r_f, dst_g_f, dst_b_f]));
          (c[0], c[1], c[2])
        }
        BlendMode::Saturation => {
          let c = set_sat(
            [dst_r_f, dst_g_f, dst_b_f],
            sat([src_r_f, src_g_f, src_b_f]),
          );
          let c = set_lum(c, lum([dst_r_f, dst_g_f, dst_b_f]));
          (c[0], c[1], c[2])
        }
        BlendMode::Color => {
          let c = set_lum(
            [src_r_f, src_g_f, src_b_f],
            lum([dst_r_f, dst_g_f, dst_b_f]),
          );
          (c[0], c[1], c[2])
        }
        BlendMode::Luminosity => {
          let c = set_lum(
            [dst_r_f, dst_g_f, dst_b_f],
            lum([src_r_f, src_g_f, src_b_f]),
          );
          (c[0], c[1], c[2])
        }
      };

      // Alpha compositing formula for colors:
      // Co = (1 - αs/αo) * Cb + (αs/αo) * ((1 - αb) * Cs + αb * B(Cb, Cs))
      // But wait, the standard formula from W3C is:
      // αo * Co = (1 - αs) * αb * Cb + (1 - αb) * αs * Cs + αs * αb * B(Cb, Cs)

      let out_r = (1.0 - src_a_f) * dst_a_f * dst_r_f
        + (1.0 - dst_a_f) * src_a_f * src_r_f
        + src_a_f * dst_a_f * res_r;
      let out_g = (1.0 - src_a_f) * dst_a_f * dst_g_f
        + (1.0 - dst_a_f) * src_a_f * src_g_f
        + src_a_f * dst_a_f * res_g;
      let out_b = (1.0 - src_a_f) * dst_a_f * dst_b_f
        + (1.0 - dst_a_f) * src_a_f * src_b_f
        + src_a_f * dst_a_f * res_b;

      // Since we want Co, we divide by out_a_f
      bottom.0[0] = (out_r / out_a_f * 255.0).round().clamp(0.0, 255.0) as u8;
      bottom.0[1] = (out_g / out_a_f * 255.0).round().clamp(0.0, 255.0) as u8;
      bottom.0[2] = (out_b / out_a_f * 255.0).round().clamp(0.0, 255.0) as u8;
      bottom.0[3] = (out_a_f * 255.0).round() as u8;
    }
  }
}

fn overlay(b: f32, s: f32) -> f32 {
  if b <= 0.5 {
    2.0 * b * s
  } else {
    1.0 - 2.0 * (1.0 - b) * (1.0 - s)
  }
}

fn color_dodge(b: f32, s: f32) -> f32 {
  if b == 0.0 {
    0.0
  } else if s >= 1.0 {
    1.0
  } else {
    (b / (1.0 - s)).min(1.0)
  }
}

fn color_burn(b: f32, s: f32) -> f32 {
  if b >= 1.0 {
    1.0
  } else if s <= 0.0 {
    0.0
  } else {
    1.0 - ((1.0 - b) / s).min(1.0)
  }
}

fn soft_light(b: f32, s: f32) -> f32 {
  if s <= 0.5 {
    b - (1.0 - 2.0 * s) * b * (1.0 - b)
  } else {
    let d = if b <= 0.25 {
      ((16.0 * b - 12.0) * b + 4.0) * b
    } else {
      b.sqrt()
    };
    b + (2.0 * s - 1.0) * (d - b)
  }
}

// W3C spec functions for non-separable blend modes (Hue, Saturation, Color, Luminosity)
// https://www.w3.org/TR/compositing-1/#blendingnonseparable

fn lum(c: [f32; 3]) -> f32 {
  0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn set_lum(mut c: [f32; 3], l: f32) -> [f32; 3] {
  let d = l - lum(c);
  c[0] += d;
  c[1] += d;
  c[2] += d;
  clip_color(c)
}

fn clip_color(mut c: [f32; 3]) -> [f32; 3] {
  let l = lum(c);
  let n = c[0].min(c[1]).min(c[2]);
  let x = c[0].max(c[1]).max(c[2]);

  if n < 0.0 {
    c[0] = l + (((c[0] - l) * l) / (l - n));
    c[1] = l + (((c[1] - l) * l) / (l - n));
    c[2] = l + (((c[2] - l) * l) / (l - n));
  }
  if x > 1.0 {
    c[0] = l + (((c[0] - l) * (1.0 - l)) / (x - l));
    c[1] = l + (((c[1] - l) * (1.0 - l)) / (x - l));
    c[2] = l + (((c[2] - l) * (1.0 - l)) / (x - l));
  }
  c
}

fn sat(c: [f32; 3]) -> f32 {
  c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(mut c: [f32; 3], s: f32) -> [f32; 3] {
  let mut indices = [0, 1, 2];
  indices.sort_by(|&i, &j| c[i].total_cmp(&c[j]));

  let min = indices[0];
  let mid = indices[1];
  let max = indices[2];

  if c[max] > c[min] {
    c[mid] = ((c[mid] - c[min]) * s) / (c[max] - c[min]);
    c[max] = s;
  } else {
    c[mid] = 0.0;
    c[max] = 0.0;
  }
  c[min] = 0.0;
  c
}

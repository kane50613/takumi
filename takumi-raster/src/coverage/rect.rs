//! Coverage of an axis-aligned rect, a port of `SkScan::AntiFillRect` (`antifillrect`,
//! `antifilldot8` and `do_scanline` in `src/core/SkScan_Antihair.cpp`), which `SkDraw::drawRect`
//! uses instead of a path fill whenever the matrix keeps rects rects.

use super::blitter::Coverage;

/// Fixed point with 8 fractional bits, `FDot8`.
type FDot8 = i32;

/// `SkScalarToFixed` then `SkFixedToFDot8`.
fn to_fdot8(value: f32) -> FDot8 {
  (((value * 65536.0) as i32).wrapping_add(0x80)) >> 8
}

/// `SkAlphaMul`.
const fn alpha_mul(value: i32, alpha256: i32) -> u8 {
  ((value * alpha256) >> 8) as u8
}

/// `do_scanline`.
fn scanline(left: FDot8, top: i32, right: FDot8, alpha: i32, coverage: &mut Coverage) {
  if left >> 8 == (right - 1) >> 8 {
    coverage.blit_v(left >> 8, top, 1, alpha_mul(alpha, right - left));
    return;
  }

  let mut x = left >> 8;

  if left & 0xFF != 0 {
    coverage.blit_v(x, top, 1, alpha_mul(alpha, 256 - (left & 0xFF)));
    x += 1;
  }

  let rite = right >> 8;

  if rite > x {
    coverage.blit_anti_h(x, top, &vec![alpha as u8; (rite - x) as usize]);
  }
  if right & 0xFF != 0 {
    coverage.blit_v(rite, top, 1, alpha_mul(alpha, right & 0xFF));
  }
}

/// `antifilldot8` with `fillInner`.
fn fill_dot8(left: FDot8, top: FDot8, right: FDot8, bottom: FDot8, coverage: &mut Coverage) {
  if left >= right || top >= bottom {
    return;
  }

  let mut y = top >> 8;

  if y == (bottom - 1) >> 8 {
    scanline(left, y, right, bottom - top - 1, coverage);
    return;
  }

  if top & 0xFF != 0 {
    scanline(left, y, right, 256 - (top & 0xFF), coverage);
    y += 1;
  }

  let bot = bottom >> 8;
  let height = bot - y;

  if height > 0 {
    let mut x = left >> 8;

    if x == (right - 1) >> 8 {
      coverage.blit_v(x, y, height, (right - left - 1) as u8);
    } else {
      if left & 0xFF != 0 {
        coverage.blit_v(x, y, height, (256 - (left & 0xFF)) as u8);
        x += 1;
      }

      let rite = right >> 8;

      if rite > x {
        coverage.blit_rect(x, y, rite - x, height);
      }
      if right & 0xFF != 0 {
        coverage.blit_v(rite, y, height, (right & 0xFF) as u8);
      }
    }
  }

  if bottom & 0xFF != 0 {
    scanline(left, bot, right, bottom & 0xFF, coverage);
  }
}

/// `SkScan::AntiFillRect` of `[left, top, right, bottom]`, clipped to the mask as to the device.
pub(super) fn fill_rect([left, top, right, bottom]: [f32; 4], coverage: &mut Coverage) {
  let (width, height) = coverage.size();
  let left = left.max(0.0);
  let top = top.max(0.0);
  let right = right.min(width as f32);
  let bottom = bottom.min(height as f32);

  if left >= right || top >= bottom {
    return;
  }

  fill_dot8(
    to_fdot8(left),
    to_fdot8(top),
    to_fdot8(right),
    to_fdot8(bottom),
    coverage,
  );
}

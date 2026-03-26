use image::RgbaImage;

use crate::Result;
use crate::rendering::{BufferPool, premultiply_alpha, unpremultiply_alpha};

/// Specifies the type of blur operation, which affects how the CSS radius is interpreted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlurType {
  /// CSS `filter: blur()` - radius equals σ (standard deviation).
  Filter,
  /// CSS `box-shadow` / `text-shadow` blur - radius equals 2σ.
  Shadow,
}

impl BlurType {
  #[inline]
  pub fn to_sigma(self, css_radius: f32) -> f32 {
    match self {
      BlurType::Filter => css_radius,
      BlurType::Shadow => css_radius * 0.5,
    }
  }

  #[inline]
  pub fn extent_multiplier(self) -> f32 {
    match self {
      BlurType::Filter => 3.0,
      BlurType::Shadow => 1.5,
    }
  }
}

#[derive(Clone, Copy)]
struct BlurPassParams {
  width: u32,
  height: u32,
  radius: u32,
  stride: usize,
  mul_val: u32,
  shg: i32,
}

pub(crate) enum BlurFormat<'a> {
  Rgba(&'a mut RgbaImage),
  Alpha {
    data: &'a mut [u8],
    width: u32,
    height: u32,
  },
}

impl<'a> BlurFormat<'a> {
  pub fn width(&self) -> u32 {
    match self {
      Self::Rgba(img) => img.width(),
      Self::Alpha { width, .. } => *width,
    }
  }

  pub fn height(&self) -> u32 {
    match self {
      Self::Rgba(img) => img.height(),
      Self::Alpha { height, .. } => *height,
    }
  }
}

/// Applies a Gaussian approximation using 3-pass Box Blur.
pub(crate) fn apply_blur(
  format: BlurFormat<'_>,
  radius: f32,
  blur_type: BlurType,
  pool: &mut BufferPool,
) -> Result<()> {
  let sigma = blur_type.to_sigma(radius);
  if sigma <= 0.5 {
    return Ok(());
  }

  let width = format.width();
  let height = format.height();
  if width == 0 || height == 0 {
    return Ok(());
  }

  let box_radius = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) * 0.5)
    .round()
    .max(1.0) as u32;

  let div = 2 * box_radius + 1;
  let (mul_val, shg) = compute_mul_shg(div);

  let stride = match format {
    BlurFormat::Rgba(_) => width as usize * 4,
    BlurFormat::Alpha { .. } => width as usize,
  };

  let pass_params = BlurPassParams {
    width,
    height,
    radius: box_radius,
    stride,
    mul_val,
    shg,
  };

  let mut col_sums = vec![0u32; stride];

  match format {
    BlurFormat::Rgba(image) => {
      for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(image.as_mut()) {
        premultiply_alpha(pixel);
      }

      let mut temp_image = pool.acquire_image_dirty(width, height)?;
      let temp_data = &mut *temp_image;
      let img_data = image.as_mut();

      for _ in 0..3 {
        box_blur_h::<4>(img_data, temp_data, pass_params);
        box_blur_v(temp_data, img_data, pass_params, &mut col_sums);
      }

      pool.release_image(temp_image);

      for pixel in bytemuck::cast_slice_mut::<u8, [u8; 4]>(image.as_mut()) {
        unpremultiply_alpha(pixel);
      }
    }
    BlurFormat::Alpha { data, .. } => {
      let mut temp_image = pool.acquire_dirty((width * height) as usize);
      let temp_data = &mut *temp_image;

      for _ in 0..3 {
        box_blur_h::<1>(data, temp_data, pass_params);
        box_blur_v(temp_data, data, pass_params, &mut col_sums);
      }

      pool.release(temp_image);
    }
  }

  Ok(())
}

macro_rules! update_h_pixel {
  ($src:expr, $dst:expr, $sum:expr, $out:expr, $entering:expr, $leaving:expr, $mul:expr, $shift:expr) => {
    if $sum[STRIDE - 1] == 0 && unsafe { *$src.get_unchecked($entering + STRIDE - 1) } == 0 {
      for c in 0..STRIDE {
        unsafe {
          *$dst.get_unchecked_mut($out + c) = 0;
        }
      }
    } else {
      for c in 0..STRIDE {
        unsafe {
          *$dst.get_unchecked_mut($out + c) = (($sum[c] * $mul) >> $shift) as u8;
          $sum[c] += *$src.get_unchecked($entering + c) as u32;
          $sum[c] -= *$src.get_unchecked($leaving + c) as u32;
        }
      }
    }
  };
}

/// Horizontal Box Blur Pass
// Kept as a range loop for forced unrolling and to avoid iterator overhead
#[allow(clippy::needless_range_loop)]
fn box_blur_h<const STRIDE: usize>(src: &[u8], dst: &mut [u8], params: BlurPassParams) {
  let radius = params.radius as usize;
  let width = params.width as usize;
  let multiplier = params.mul_val;
  let shift = params.shg;
  let stride = params.stride;

  assert!(src.len() >= params.height as usize * stride);
  assert!(dst.len() >= params.height as usize * stride);

  for y in 0..params.height as usize {
    let line_offset = y * stride;
    let mut sum = [0u32; STRIDE];

    let first_px = line_offset;
    for c in 0..STRIDE {
      sum[c] = unsafe { *src.get_unchecked(first_px + c) } as u32 * (radius as u32 + 1);
    }

    for dx in 1..=radius {
      let px = dx.min(width - 1);
      let src_offset = line_offset + px * STRIDE;
      for c in 0..STRIDE {
        sum[c] += unsafe { *src.get_unchecked(src_offset + c) } as u32;
      }
    }

    let left_end = (radius + 1).min(width);
    for x in 0..left_end {
      let out_offset = line_offset + x * STRIDE;
      let entering_x = (x + radius + 1).min(width - 1);
      let entering_offset = line_offset + entering_x * STRIDE;
      update_h_pixel!(
        src,
        dst,
        sum,
        out_offset,
        entering_offset,
        first_px,
        multiplier,
        shift
      );
    }

    let middle_end = width.saturating_sub(radius + 1).max(left_end);
    for x in left_end..middle_end {
      let out_offset = line_offset + x * STRIDE;
      let leaving_offset = line_offset + (x - radius) * STRIDE;
      let entering_offset = line_offset + (x + radius + 1) * STRIDE;
      update_h_pixel!(
        src,
        dst,
        sum,
        out_offset,
        entering_offset,
        leaving_offset,
        multiplier,
        shift
      );
    }

    let last_px = line_offset + (width - 1) * STRIDE;
    for x in middle_end..width {
      let out_offset = line_offset + x * STRIDE;
      let leaving_offset = line_offset + (x - radius) * STRIDE;
      update_h_pixel!(
        src,
        dst,
        sum,
        out_offset,
        last_px,
        leaving_offset,
        multiplier,
        shift
      );
    }
  }
}

macro_rules! update_v_pixel {
  ($src:expr, $dst:expr, $sums:expr, $x:expr, $out:expr, $entering:expr, $leaving:expr, $mul:expr, $shift:expr) => {
    let sum = $sums[$x];
    let entering = unsafe { *$src.get_unchecked($entering + $x) } as u32;
    if sum == 0 && entering == 0 {
      unsafe {
        *$dst.get_unchecked_mut($out + $x) = 0;
      }
    } else {
      unsafe {
        *$dst.get_unchecked_mut($out + $x) = ((sum * $mul) >> $shift) as u8;
        $sums[$x] = sum + entering - *$src.get_unchecked($leaving + $x) as u32;
      }
    }
  };
}

/// Vertical Box Blur Pass
// Kept as a range loop for forced unrolling and to avoid iterator overhead in WASM
#[allow(clippy::needless_range_loop)]
fn box_blur_v(src: &[u8], dst: &mut [u8], params: BlurPassParams, sums: &mut [u32]) {
  let radius = params.radius as usize;
  let height = params.height as usize;
  let multiplier = params.mul_val;
  let shift = params.shg;
  let stride = params.stride;

  assert!(src.len() >= params.height as usize * stride);
  assert!(dst.len() >= params.height as usize * stride);

  // Initialize sums with the first row repeated
  for x in 0..stride {
    sums[x] = unsafe { *src.get_unchecked(x) } as u32 * (radius as u32 + 1);
  }

  // Add trailing edge
  for dy in 1..=radius {
    let py = dy.min(height - 1);
    let row_offset = py * stride;
    for x in 0..stride {
      sums[x] += unsafe { *src.get_unchecked(row_offset + x) } as u32;
    }
  }

  let left_end = (radius + 1).min(height);
  for y in 0..left_end {
    let out_offset = y * stride;
    let entering_y = (y + radius + 1).min(height - 1);
    let entering_row = entering_y * stride;

    for x in 0..stride {
      update_v_pixel!(
        src,
        dst,
        sums,
        x,
        out_offset,
        entering_row,
        0,
        multiplier,
        shift
      );
    }
  }

  let middle_end = height.saturating_sub(radius + 1).max(left_end);
  for y in left_end..middle_end {
    let out_offset = y * stride;
    let leaving_row = (y - radius) * stride;
    let entering_row = (y + radius + 1) * stride;

    for x in 0..stride {
      update_v_pixel!(
        src,
        dst,
        sums,
        x,
        out_offset,
        entering_row,
        leaving_row,
        multiplier,
        shift
      );
    }
  }

  let last_row = (height - 1) * stride;
  for y in middle_end..height {
    let out_offset = y * stride;
    let leaving_row = (y - radius) * stride;

    for x in 0..stride {
      update_v_pixel!(
        src,
        dst,
        sums,
        x,
        out_offset,
        last_row,
        leaving_row,
        multiplier,
        shift
      );
    }
  }
}

#[inline(always)]
fn compute_mul_shg(d: u32) -> (u32, i32) {
  let shg = 23;
  let mul = ((1u64 << shg) as f64 / d as f64).round() as u32;
  (mul, shg)
}

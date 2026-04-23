use crate::rendering::BufferPool;
use crate::{Error, Result};

const BLUR_DOWNSAMPLE_TARGET_SIGMA: f32 = 6.0;
const BLUR_DOWNSAMPLE_MIN_DIMENSION: u32 = 128;
const BLUR_DOWNSAMPLE_MAX_SCALE: u32 = 8;

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
  Alpha {
    data: &'a mut [u8],
    width: u32,
    height: u32,
  },
}

impl<'a> BlurFormat<'a> {
  pub fn width(&self) -> u32 {
    match self {
      Self::Alpha { width, .. } => *width,
    }
  }

  pub fn height(&self) -> u32 {
    match self {
      Self::Alpha { height, .. } => *height,
    }
  }
}

fn blur_pass_params(
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
  stride: usize,
) -> Option<BlurPassParams> {
  let sigma = blur_type.to_sigma(radius);
  if sigma <= 0.5 || width == 0 || height == 0 {
    return None;
  }

  let box_radius = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) * 0.5)
    .round()
    .max(1.0) as u32;
  let div = 2 * box_radius + 1;
  let (mul_val, shg) = compute_mul_shg(div);

  Some(BlurPassParams {
    width,
    height,
    radius: box_radius,
    stride,
    mul_val,
    shg,
  })
}

/// Applies a Gaussian approximation using 3-pass Box Blur.
pub(crate) fn apply_blur(
  format: BlurFormat<'_>,
  radius: f32,
  blur_type: BlurType,
  pool: &mut BufferPool,
) -> Result<()> {
  let width = format.width();
  let height = format.height();
  let Some(pass_params) = blur_pass_params(width, height, radius, blur_type, width as usize) else {
    return Ok(());
  };

  let mut col_sums = pool.acquire_u32(pass_params.stride);

  match format {
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

  pool.release_u32(col_sums);

  Ok(())
}

pub(crate) fn apply_blur_rgba_bytes(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
  pool: &mut BufferPool,
) -> Result<()> {
  apply_blur_rgba_bytes_internal(data, width, height, radius, blur_type, pool, true)
}

fn apply_blur_rgba_bytes_internal(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
  pool: &mut BufferPool,
  allow_downsample: bool,
) -> Result<()> {
  if width == 0 || height == 0 {
    return Ok(());
  }

  let expected = (width as usize)
    .saturating_mul(height as usize)
    .saturating_mul(4);
  if data.len() != expected {
    return Err(Error::InvalidRgbaBufferLength {
      actual: data.len(),
      expected,
    });
  }

  if allow_downsample {
    let scale = blur_downsample_scale(width, height, radius, blur_type);
    if scale > 1 {
      return apply_blur_rgba_downsampled(data, width, height, radius, blur_type, pool, scale);
    }
  }

  let Some(pass_params) = blur_pass_params(width, height, radius, blur_type, width as usize * 4)
  else {
    return Ok(());
  };
  let mut col_sums = pool.acquire_u32(pass_params.stride);

  let mut temp = pool.acquire_dirty(expected);
  for _ in 0..3 {
    box_blur_h::<4>(data, &mut temp, pass_params);
    box_blur_v(&temp, data, pass_params, &mut col_sums);
  }
  pool.release(temp);
  pool.release_u32(col_sums);

  Ok(())
}

fn blur_downsample_scale(width: u32, height: u32, radius: f32, blur_type: BlurType) -> u32 {
  let sigma = blur_type.to_sigma(radius);
  if sigma <= BLUR_DOWNSAMPLE_TARGET_SIGMA {
    return 1;
  }

  let min_dim = width.min(height);
  if min_dim < BLUR_DOWNSAMPLE_MIN_DIMENSION {
    return 1;
  }

  let mut scale = 1u32;
  while scale < BLUR_DOWNSAMPLE_MAX_SCALE {
    let next_scale = scale * 2;
    if sigma / (next_scale as f32) < BLUR_DOWNSAMPLE_TARGET_SIGMA {
      break;
    }
    if min_dim / next_scale < BLUR_DOWNSAMPLE_MIN_DIMENSION {
      break;
    }
    scale = next_scale;
  }

  scale
}

fn apply_blur_rgba_downsampled(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
  pool: &mut BufferPool,
  scale: u32,
) -> Result<()> {
  let ds_width = width.div_ceil(scale);
  let ds_height = height.div_ceil(scale);
  let ds_len = (ds_width as usize)
    .saturating_mul(ds_height as usize)
    .saturating_mul(4);

  let mut downsampled = pool.acquire_dirty(ds_len);
  downsample_rgba_box(
    data,
    width,
    height,
    &mut downsampled,
    ds_width,
    ds_height,
    scale,
  );

  let scaled_radius = radius / scale as f32;
  apply_blur_rgba_bytes_internal(
    &mut downsampled,
    ds_width,
    ds_height,
    scaled_radius,
    blur_type,
    pool,
    false,
  )?;

  upsample_rgba_bilinear(
    &downsampled,
    ds_width,
    ds_height,
    data,
    width,
    height,
    scale,
  );
  pool.release(downsampled);
  Ok(())
}

fn downsample_rgba_box(
  src: &[u8],
  src_width: u32,
  src_height: u32,
  dst: &mut [u8],
  dst_width: u32,
  dst_height: u32,
  scale: u32,
) {
  for dy in 0..dst_height {
    let src_y0 = dy * scale;
    let src_y1 = (src_y0 + scale).min(src_height);
    for dx in 0..dst_width {
      let src_x0 = dx * scale;
      let src_x1 = (src_x0 + scale).min(src_width);
      let mut sum = [0u32; 4];
      let mut count = 0u32;

      for sy in src_y0..src_y1 {
        let row_offset = sy as usize * src_width as usize * 4;
        for sx in src_x0..src_x1 {
          let index = row_offset + sx as usize * 4;
          sum[0] += src[index] as u32;
          sum[1] += src[index + 1] as u32;
          sum[2] += src[index + 2] as u32;
          sum[3] += src[index + 3] as u32;
          count += 1;
        }
      }

      let dst_index = (dy as usize * dst_width as usize + dx as usize) * 4;
      if count == 0 {
        dst[dst_index] = 0;
        dst[dst_index + 1] = 0;
        dst[dst_index + 2] = 0;
        dst[dst_index + 3] = 0;
        continue;
      }

      dst[dst_index] = (sum[0] / count) as u8;
      dst[dst_index + 1] = (sum[1] / count) as u8;
      dst[dst_index + 2] = (sum[2] / count) as u8;
      dst[dst_index + 3] = (sum[3] / count) as u8;
    }
  }
}

fn upsample_rgba_bilinear(
  src: &[u8],
  src_width: u32,
  src_height: u32,
  dst: &mut [u8],
  dst_width: u32,
  dst_height: u32,
  scale: u32,
) {
  if src_width == 0 || src_height == 0 {
    dst.fill(0);
    return;
  }

  let x_positions = bilinear_axis_positions(dst_width, src_width, scale);
  let y_positions = bilinear_axis_positions(dst_height, src_height, scale);

  for (y, y_position) in y_positions.iter().enumerate() {
    let top_row = y_position.start * src_width as usize * 4;
    let bottom_row = y_position.end * src_width as usize * 4;
    let dst_row = y * dst_width as usize * 4;

    for (x, x_position) in x_positions.iter().enumerate() {
      let top_left = top_row + x_position.start * 4;
      let top_right = top_row + x_position.end * 4;
      let bottom_left = bottom_row + x_position.start * 4;
      let bottom_right = bottom_row + x_position.end * 4;
      let dst_index = dst_row + x * 4;

      for channel in 0..4 {
        let top = src[top_left + channel] as f32 * (1.0 - x_position.weight)
          + src[top_right + channel] as f32 * x_position.weight;
        let bottom = src[bottom_left + channel] as f32 * (1.0 - x_position.weight)
          + src[bottom_right + channel] as f32 * x_position.weight;
        dst[dst_index + channel] = (top * (1.0 - y_position.weight) + bottom * y_position.weight)
          .round()
          .clamp(0.0, 255.0) as u8;
      }
    }
  }
}

#[derive(Clone, Copy)]
struct BilinearAxisPosition {
  start: usize,
  end: usize,
  weight: f32,
}

fn bilinear_axis_positions(dst_len: u32, src_len: u32, scale: u32) -> Vec<BilinearAxisPosition> {
  let max_source = src_len.saturating_sub(1) as f32;
  let scale = scale as f32;

  (0..dst_len)
    .map(|index| {
      let source = ((index as f32 + 0.5) / scale - 0.5).clamp(0.0, max_source);
      let start = source.floor() as u32;
      let end = (start + 1).min(src_len - 1);
      BilinearAxisPosition {
        start: start as usize,
        end: end as usize,
        weight: source - start as f32,
      }
    })
    .collect()
}

macro_rules! update_h_pixel {
  ($src:expr, $dst:expr, $sum:expr, $out:expr, $entering:expr, $leaving:expr, $mul:expr, $shift:expr) => {
    if $sum[STRIDE - 1] == 0 && $src[$entering + STRIDE - 1] == 0 {
      for c in 0..STRIDE {
        $dst[$out + c] = 0;
        let entering = $src[$entering + c] as u32;
        let leaving = $src[$leaving + c] as u32;
        $sum[c] = $sum[c].saturating_add(entering).saturating_sub(leaving);
      }
    } else {
      for c in 0..STRIDE {
        $dst[$out + c] = (($sum[c] * $mul) >> $shift) as u8;
        let entering = $src[$entering + c] as u32;
        let leaving = $src[$leaving + c] as u32;
        $sum[c] = $sum[c].saturating_add(entering).saturating_sub(leaving);
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
      sum[c] = src[first_px + c] as u32 * (radius as u32 + 1);
    }

    for dx in 1..=radius {
      let px = dx.min(width - 1);
      let src_offset = line_offset + px * STRIDE;
      for c in 0..STRIDE {
        sum[c] += src[src_offset + c] as u32;
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
    let entering = $src[$entering + $x] as u32;
    let leaving = $src[$leaving + $x] as u32;
    if sum == 0 && entering == 0 {
      $dst[$out + $x] = 0;
      $sums[$x] = sum.saturating_add(entering).saturating_sub(leaving);
    } else {
      $dst[$out + $x] = ((sum * $mul) >> $shift) as u8;
      $sums[$x] = sum.saturating_add(entering).saturating_sub(leaving);
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
    sums[x] = src[x] as u32 * (radius as u32 + 1);
  }

  // Add trailing edge
  for dy in 1..=radius {
    let py = dy.min(height - 1);
    let row_offset = py * stride;
    for x in 0..stride {
      sums[x] += src[row_offset + x] as u32;
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

#[cfg(test)]
mod tests {
  use super::{BlurType, apply_blur_rgba_bytes, blur_downsample_scale, upsample_rgba_bilinear};
  use crate::{Error, rendering::BufferPool};

  #[test]
  fn apply_blur_rgba_bytes_returns_error_for_invalid_buffer_length() {
    let mut data = vec![0u8; 3];
    let mut pool = BufferPool::default();

    assert!(matches!(
      apply_blur_rgba_bytes(&mut data, 1, 1, 4.0, BlurType::Filter, &mut pool),
      Err(Error::InvalidRgbaBufferLength {
        actual: 3,
        expected: 4
      })
    ));
  }

  #[test]
  fn blur_downsample_scale_uses_sigma_and_dimensions() {
    assert_eq!(blur_downsample_scale(80, 80, 24.0, BlurType::Filter), 1);
    assert_eq!(blur_downsample_scale(256, 256, 6.0, BlurType::Filter), 1);
    assert_eq!(blur_downsample_scale(256, 256, 16.0, BlurType::Filter), 2);
    assert_eq!(blur_downsample_scale(1024, 1024, 64.0, BlurType::Filter), 8);
  }

  #[test]
  fn upsample_rgba_bilinear_matches_reference() {
    let src_width = 3;
    let src_height = 2;
    let dst_width = 7;
    let dst_height = 5;
    let scale = 3;
    let src = (0..src_width * src_height * 4)
      .map(|value| ((value * 17 + 11) % 251) as u8)
      .collect::<Vec<_>>();
    let mut actual = vec![0; (dst_width * dst_height * 4) as usize];
    let mut expected = actual.clone();

    upsample_rgba_bilinear(
      &src,
      src_width,
      src_height,
      &mut actual,
      dst_width,
      dst_height,
      scale,
    );
    upsample_rgba_bilinear_reference(
      &src,
      src_width,
      src_height,
      &mut expected,
      dst_width,
      dst_height,
      scale,
    );

    assert_eq!(actual, expected);
  }

  fn upsample_rgba_bilinear_reference(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    scale: u32,
  ) {
    let max_x = src_width.saturating_sub(1) as f32;
    let max_y = src_height.saturating_sub(1) as f32;
    let scale_f = scale as f32;

    for y in 0..dst_height {
      let src_y = ((y as f32 + 0.5) / scale_f - 0.5).clamp(0.0, max_y);
      let y0 = src_y.floor() as u32;
      let y1 = (y0 + 1).min(src_height - 1);
      let wy = src_y - y0 as f32;
      for x in 0..dst_width {
        let src_x = ((x as f32 + 0.5) / scale_f - 0.5).clamp(0.0, max_x);
        let x0 = src_x.floor() as u32;
        let x1 = (x0 + 1).min(src_width - 1);
        let wx = src_x - x0 as f32;

        let top_left = ((y0 as usize * src_width as usize) + x0 as usize) * 4;
        let top_right = ((y0 as usize * src_width as usize) + x1 as usize) * 4;
        let bottom_left = ((y1 as usize * src_width as usize) + x0 as usize) * 4;
        let bottom_right = ((y1 as usize * src_width as usize) + x1 as usize) * 4;
        let dst_index = ((y as usize * dst_width as usize) + x as usize) * 4;

        for channel in 0..4 {
          let top =
            src[top_left + channel] as f32 * (1.0 - wx) + src[top_right + channel] as f32 * wx;
          let bottom = src[bottom_left + channel] as f32 * (1.0 - wx)
            + src[bottom_right + channel] as f32 * wx;
          dst[dst_index + channel] =
            (top * (1.0 - wy) + bottom * wy).round().clamp(0.0, 255.0) as u8;
        }
      }
    }
  }
}

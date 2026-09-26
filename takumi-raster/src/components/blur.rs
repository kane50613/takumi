use crate::{Error, Result, checked_area, simd};

const BLUR_DOWNSAMPLE_TARGET_SIGMA: f32 = 6.0;
const BLUR_DOWNSAMPLE_MIN_DIMENSION: u32 = 128;
const BLUR_DOWNSAMPLE_MAX_SCALE: u32 = 8;

pub(crate) use takumi_core::style::BlurType;

#[derive(Clone, Copy)]
struct BlurPassParams {
  width: u32,
  height: u32,
  radius: u32,
  mul_val: u32,
  shg: i32,
}

#[derive(Clone, Copy)]
struct Dims {
  width: u32,
  height: u32,
}

fn blur_pass_params(
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
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
    mul_val,
    shg,
  })
}

/// Blurs a `width` by `height` alpha mask with a Gaussian approximated by
/// three box blur passes.
pub(crate) fn apply_blur_alpha_bytes(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
) -> Result<()> {
  let Some(pass_params) = blur_pass_params(width, height, radius, blur_type) else {
    return Ok(());
  };
  let Some(expected) = checked_area(width, height, 1) else {
    return Ok(());
  };

  if data.len() != expected {
    return Err(Error::InvalidAlphaBufferLength {
      actual: data.len(),
      expected,
    });
  }

  let mut col_sums = vec![0u32; width as usize];
  let mut temp = vec![0; expected];

  for _ in 0..3 {
    box_blur_h_alpha(data, &mut temp, pass_params);
    box_blur_v_alpha(&temp, data, pass_params, &mut col_sums);
  }

  Ok(())
}

pub(crate) fn apply_blur_rgba_bytes(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
) -> Result<()> {
  apply_blur_rgba_bytes_internal(data, Dims { width, height }, radius, blur_type, true)
}

fn apply_blur_rgba_bytes_internal(
  data: &mut [u8],
  dims: Dims,
  radius: f32,
  blur_type: BlurType,
  allow_downsample: bool,
) -> Result<()> {
  let Dims { width, height } = dims;
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
      return apply_blur_rgba_downsampled(data, dims, radius, blur_type, scale);
    }
  }

  let Some(pass_params) = blur_pass_params(width, height, radius, blur_type) else {
    return Ok(());
  };

  let mut col_sums = vec![0u32; width as usize * 4];

  let mut temp = vec![0; expected];
  let src_pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(data);
  let temp_pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(&mut temp);
  for _ in 0..3 {
    box_blur_h_rgba(src_pixels, temp_pixels, pass_params);
    box_blur_v_rgba(temp_pixels, src_pixels, pass_params, &mut col_sums);
  }

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
  dims: Dims,
  radius: f32,
  blur_type: BlurType,
  scale: u32,
) -> Result<()> {
  let ds_dims = Dims {
    width: dims.width.div_ceil(scale),
    height: dims.height.div_ceil(scale),
  };
  let ds_len = (ds_dims.width as usize)
    .saturating_mul(ds_dims.height as usize)
    .saturating_mul(4);

  let mut downsampled = vec![0; ds_len];
  downsample_rgba_box(data, dims, &mut downsampled, ds_dims, scale);

  let scaled_radius = radius / scale as f32;
  apply_blur_rgba_bytes_internal(&mut downsampled, ds_dims, scaled_radius, blur_type, false)?;

  upsample_rgba_bilinear(&downsampled, ds_dims, data, dims, scale);
  Ok(())
}

fn downsample_rgba_box(src: &[u8], src_dims: Dims, dst: &mut [u8], dst_dims: Dims, scale: u32) {
  let Dims {
    width: src_width,
    height: src_height,
  } = src_dims;
  let Dims {
    width: dst_width,
    height: dst_height,
  } = dst_dims;
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

fn upsample_rgba_bilinear(src: &[u8], src_dims: Dims, dst: &mut [u8], dst_dims: Dims, scale: u32) {
  let Dims {
    width: src_width,
    height: src_height,
  } = src_dims;
  let Dims {
    width: dst_width,
    height: dst_height,
  } = dst_dims;
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

#[inline(always)]
fn pack_pixel(sum: [u32; 4], mul: u32, shift: i32) -> [u8; 4] {
  sum.map(|channel| pack_alpha(channel, mul, shift))
}

#[inline(always)]
fn slide_pixel(sum: [u32; 4], entering: [u8; 4], leaving: [u8; 4]) -> [u32; 4] {
  [
    sum[0] + entering[0] as u32 - leaving[0] as u32,
    sum[1] + entering[1] as u32 - leaving[1] as u32,
    sum[2] + entering[2] as u32 - leaving[2] as u32,
    sum[3] + entering[3] as u32 - leaving[3] as u32,
  ]
}

#[inline(always)]
fn scale_pixel(p: [u8; 4], k: u32) -> [u32; 4] {
  [
    p[0] as u32 * k,
    p[1] as u32 * k,
    p[2] as u32 * k,
    p[3] as u32 * k,
  ]
}

#[inline(always)]
fn widen_pixel(p: [u8; 4]) -> [u32; 4] {
  [p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32]
}

#[inline(always)]
fn add_pixel(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
  [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn box_blur_h_rgba(src: &[[u8; 4]], dst: &mut [[u8; 4]], params: BlurPassParams) {
  let radius = params.radius as usize;
  let width = params.width as usize;
  let height = params.height as usize;
  let mul = params.mul_val;
  let shift = params.shg;
  let k = radius as u32 + 1;

  for y in 0..height {
    let row_start = y * width;
    let src_row = &src[row_start..row_start + width];
    let dst_row = &mut dst[row_start..row_start + width];

    let first = src_row[0];
    let mut sum = scale_pixel(first, k);
    for dx in 1..=radius {
      let px = dx.min(width - 1);
      sum = add_pixel(sum, widen_pixel(src_row[px]));
    }

    let left_end = (radius + 1).min(width);
    for x in 0..left_end {
      let entering = src_row[(x + radius + 1).min(width - 1)];
      dst_row[x] = pack_pixel(sum, mul, shift);
      sum = slide_pixel(sum, entering, first);
    }

    let middle_end = width.saturating_sub(radius + 1).max(left_end);
    for x in left_end..middle_end {
      let entering = src_row[x + radius + 1];
      let leaving = src_row[x - radius];
      dst_row[x] = pack_pixel(sum, mul, shift);
      sum = slide_pixel(sum, entering, leaving);
    }

    let last = src_row[width - 1];
    for x in middle_end..width {
      let leaving = src_row[x - radius];
      dst_row[x] = pack_pixel(sum, mul, shift);
      sum = slide_pixel(sum, last, leaving);
    }
  }
}

/// Each channel is an independent byte column in the vertical pass.
fn box_blur_v_rgba(src: &[[u8; 4]], dst: &mut [[u8; 4]], params: BlurPassParams, sums: &mut [u32]) {
  let Some(width) = params.width.checked_mul(4) else {
    return;
  };

  box_blur_v_alpha(
    bytemuck::cast_slice(src),
    bytemuck::cast_slice_mut(dst),
    BlurPassParams { width, ..params },
    sums,
  );
}

#[inline(always)]
fn pack_alpha(sum: u32, mul: u32, shift: i32) -> u8 {
  ((sum * mul) >> shift) as u8
}

fn box_blur_h_alpha(src: &[u8], dst: &mut [u8], params: BlurPassParams) {
  let radius = params.radius as usize;
  let width = params.width as usize;
  let height = params.height as usize;
  let k = radius as u32 + 1;

  for y in 0..height {
    let row_start = y * width;
    let src_row = &src[row_start..row_start + width];
    let dst_row = &mut dst[row_start..row_start + width];

    let first = src_row[0] as u32;
    let mut sum = first * k;
    for dx in 1..=radius {
      sum += src_row[dx.min(width - 1)] as u32;
    }

    let left_end = (radius + 1).min(width);
    for x in 0..left_end {
      let entering = src_row[(x + radius + 1).min(width - 1)] as u32;
      dst_row[x] = pack_alpha(sum, params.mul_val, params.shg);
      sum = sum + entering - first;
    }

    let middle_end = width.saturating_sub(radius + 1).max(left_end);

    if left_end < middle_end {
      let (entering, entering_tail) =
        src_row[left_end + radius + 1..middle_end + radius + 1].as_chunks::<4>();
      let (leaving, leaving_tail) =
        src_row[left_end - radius..middle_end - radius].as_chunks::<4>();
      let (out, out_tail) = dst_row[left_end..middle_end].as_chunks_mut::<4>();

      for ((out, entering), leaving) in out.iter_mut().zip(entering).zip(leaving) {
        (*out, sum) =
          simd::slide_box_alpha4(sum, *entering, *leaving, params.mul_val, params.shg as u32);
      }

      for ((out, &entering), &leaving) in out_tail.iter_mut().zip(entering_tail).zip(leaving_tail) {
        *out = pack_alpha(sum, params.mul_val, params.shg);
        sum = sum + entering as u32 - leaving as u32;
      }
    }

    let last = src_row[width - 1] as u32;
    for x in middle_end..width {
      let leaving = src_row[x - radius] as u32;
      dst_row[x] = pack_alpha(sum, params.mul_val, params.shg);
      sum = sum + last - leaving;
    }
  }
}

fn box_blur_v_alpha(src: &[u8], dst: &mut [u8], params: BlurPassParams, sums: &mut [u32]) {
  let radius = params.radius as usize;
  let width = params.width as usize;
  let height = params.height as usize;
  let mul = params.mul_val;
  let shift = params.shg;
  let k = radius as u32 + 1;

  for x in 0..width {
    sums[x] = src[x] as u32 * k;
  }
  for dy in 1..=radius {
    let py = dy.min(height - 1);
    let row_start = py * width;
    for x in 0..width {
      sums[x] += src[row_start + x] as u32;
    }
  }

  let left_end = (radius + 1).min(height);
  for y in 0..left_end {
    let entering_y = (y + radius + 1).min(height - 1);
    let entering_row = entering_y * width;
    let out_row = y * width;
    for x in 0..width {
      dst[out_row + x] = pack_alpha(sums[x], mul, shift);
      sums[x] = sums[x] + src[entering_row + x] as u32 - src[x] as u32;
    }
  }

  let middle_end = height.saturating_sub(radius + 1).max(left_end);
  for y in left_end..middle_end {
    let entering_row = (y + radius + 1) * width;
    let leaving_row = (y - radius) * width;
    let out_row = y * width;
    for x in 0..width {
      dst[out_row + x] = pack_alpha(sums[x], mul, shift);
      sums[x] = sums[x] + src[entering_row + x] as u32 - src[leaving_row + x] as u32;
    }
  }

  let last_row = (height - 1) * width;
  for y in middle_end..height {
    let leaving_row = (y - radius) * width;
    let out_row = y * width;
    for x in 0..width {
      dst[out_row + x] = pack_alpha(sums[x], mul, shift);
      sums[x] = sums[x] + src[last_row + x] as u32 - src[leaving_row + x] as u32;
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
  use std::assert_matches;

  use super::{
    BlurPassParams, BlurType, Dims, add_pixel, apply_blur_rgba_bytes, blur_downsample_scale,
    box_blur_h_alpha, box_blur_v_rgba, compute_mul_shg, pack_alpha, pack_pixel, scale_pixel,
    slide_pixel, upsample_rgba_bilinear, widen_pixel,
  };
  use crate::Error;

  /// Scalar horizontal alpha blur with clamped edges, the oracle for the four-pixel slide.
  fn box_blur_h_alpha_reference(src: &[u8], dst: &mut [u8], params: BlurPassParams) {
    let radius = params.radius as usize;
    let width = params.width as usize;
    let k = radius as u32 + 1;

    for y in 0..params.height as usize {
      let src_row = &src[y * width..(y + 1) * width];
      let dst_row = &mut dst[y * width..(y + 1) * width];
      let first = src_row[0] as u32;
      let mut sum = first * k;

      for dx in 1..=radius {
        sum += src_row[dx.min(width - 1)] as u32;
      }

      let left_end = (radius + 1).min(width);

      for x in 0..left_end {
        let entering = src_row[(x + radius + 1).min(width - 1)] as u32;
        dst_row[x] = pack_alpha(sum, params.mul_val, params.shg);
        sum = sum + entering - first;
      }

      let middle_end = width.saturating_sub(radius + 1).max(left_end);

      for x in left_end..middle_end {
        dst_row[x] = pack_alpha(sum, params.mul_val, params.shg);
        sum = sum + src_row[x + radius + 1] as u32 - src_row[x - radius] as u32;
      }

      let last = src_row[width - 1] as u32;

      for x in middle_end..width {
        dst_row[x] = pack_alpha(sum, params.mul_val, params.shg);
        sum = sum + last - src_row[x - radius] as u32;
      }
    }
  }

  /// Channel-wise vertical blur, independent of the flat-byte traversal.
  fn box_blur_v_rgba_reference(src: &[[u8; 4]], dst: &mut [[u8; 4]], params: BlurPassParams) {
    let radius = params.radius as usize;
    let width = params.width as usize;
    let height = params.height as usize;
    let k = radius as u32 + 1;
    let pack = |sum| pack_pixel(sum, params.mul_val, params.shg);
    let mut sums: Vec<[u32; 4]> = src[..width].iter().map(|&p| scale_pixel(p, k)).collect();

    for dy in 1..=radius {
      let row = &src[dy.min(height - 1) * width..][..width];

      for x in 0..width {
        sums[x] = add_pixel(sums[x], widen_pixel(row[x]));
      }
    }

    let left_end = (radius + 1).min(height);

    for y in 0..left_end {
      let entering = &src[(y + radius + 1).min(height - 1) * width..][..width];

      for x in 0..width {
        dst[y * width + x] = pack(sums[x]);
        sums[x] = slide_pixel(sums[x], entering[x], src[x]);
      }
    }

    let middle_end = height.saturating_sub(radius + 1).max(left_end);

    for y in left_end..middle_end {
      let entering = &src[(y + radius + 1) * width..][..width];
      let leaving = &src[(y - radius) * width..][..width];

      for x in 0..width {
        dst[y * width + x] = pack(sums[x]);
        sums[x] = slide_pixel(sums[x], entering[x], leaving[x]);
      }
    }

    let last = &src[(height - 1) * width..][..width];

    for y in middle_end..height {
      let leaving = &src[(y - radius) * width..][..width];

      for x in 0..width {
        dst[y * width + x] = pack(sums[x]);
        sums[x] = slide_pixel(sums[x], last[x], leaving[x]);
      }
    }
  }

  #[test]
  fn vertical_rgba_pass_matches_the_reference() {
    for radius in [1u32, 4, 25] {
      let (mul_val, shg) = compute_mul_shg(2 * radius + 1);

      for width in [1u32, 2, 3, 17] {
        for height in [1u32, 2, 3, 7, 10] {
          let params = BlurPassParams {
            width,
            height,
            radius,
            mul_val,
            shg,
          };

          let src: Vec<[u8; 4]> = (0..width * height)
            .map(|i| core::array::from_fn(|c| ((i * 37 + c as u32 * 53) % 256) as u8))
            .collect();
          let mut expected = vec![[0u8; 4]; src.len()];
          let mut actual = vec![[0u8; 4]; src.len()];
          let mut sums = vec![0u32; width as usize * 4];
          box_blur_v_rgba_reference(&src, &mut expected, params);
          box_blur_v_rgba(&src, &mut actual, params, &mut sums);
          assert_eq!(
            actual, expected,
            "radius {radius} width {width} height {height}"
          );
        }
      }
    }
  }

  #[test]
  fn horizontal_alpha_pass_matches_the_reference_at_every_width() {
    for radius in [1u32, 2, 4, 25] {
      let (mul_val, shg) = compute_mul_shg(2 * radius + 1);
      let widths = [
        1,
        radius,
        radius + 1,
        2 * radius + 1,
        2 * radius + 2,
        2 * radius + 3,
        2 * radius + 5,
        2 * radius + 6,
        2 * radius + 7,
      ];

      for width in widths {
        for height in [1u32, 3] {
          let params = BlurPassParams {
            width,
            height,
            radius,
            mul_val,
            shg,
          };

          let src: Vec<u8> = (0..width * height)
            .map(|i| ((31 * i + 17) % 256) as u8)
            .collect();
          let mut expected = vec![0u8; src.len()];
          let mut actual = vec![0u8; src.len()];
          box_blur_h_alpha_reference(&src, &mut expected, params);
          box_blur_h_alpha(&src, &mut actual, params);
          assert_eq!(
            actual, expected,
            "radius {radius} width {width} height {height}"
          );
        }
      }
    }
  }

  #[test]
  fn apply_blur_rgba_bytes_returns_error_for_invalid_buffer_length() {
    let mut data = vec![0u8; 3];

    assert_matches!(
      apply_blur_rgba_bytes(&mut data, 1, 1, 4.0, BlurType::Filter),
      Err(Error::InvalidRgbaBufferLength {
        actual: 3,
        expected: 4
      })
    );
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
      Dims {
        width: src_width,
        height: src_height,
      },
      &mut actual,
      Dims {
        width: dst_width,
        height: dst_height,
      },
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

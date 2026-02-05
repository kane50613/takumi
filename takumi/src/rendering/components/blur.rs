use image::RgbaImage;
use wide::{u32x4, u32x8, u32x16};

use crate::rendering::fast_div_255;

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

/// Trait for SIMD vectors that can process pixel data.
/// This allows the same blur algorithm to work with different SIMD widths.
trait PixelVector:
  Copy
  + Default
  + std::ops::Add<Output = Self>
  + std::ops::Sub<Output = Self>
  + std::ops::Mul<Output = Self>
  + std::ops::Shr<i32, Output = Self>
{
  /// Number of u32 lanes (4, 8, or 16)
  const LANES: usize;

  /// Number of complete RGBA pixels per vector (1, 2, or 4)
  const PIXELS_PER_BATCH: usize = Self::LANES / 4;

  /// Create with all lanes set to the same value
  fn splat(value: u32) -> Self;

  /// Zero vector
  fn zero() -> Self;

  /// Load channels from buffer (loads LANES u32 values as u8->u32)
  fn load_channels(buffer: &[u8], offset: usize) -> Self;

  /// Store channels to buffer (stores LANES u32 values as u32->u8 clamped)
  fn store_channels(self, buffer: &mut [u8], offset: usize);
}

impl PixelVector for u32x4 {
  const LANES: usize = 4;

  #[inline(always)]
  fn splat(value: u32) -> Self {
    u32x4::new([value; 4])
  }

  #[inline(always)]
  fn zero() -> Self {
    u32x4::ZERO
  }

  #[inline(always)]
  fn load_channels(buffer: &[u8], offset: usize) -> Self {
    u32x4::new([
      buffer[offset] as u32,
      buffer[offset + 1] as u32,
      buffer[offset + 2] as u32,
      buffer[offset + 3] as u32,
    ])
  }

  #[inline(always)]
  fn store_channels(self, buffer: &mut [u8], offset: usize) {
    let arr: [u32; 4] = self.into();
    buffer[offset] = arr[0].min(255) as u8;
    buffer[offset + 1] = arr[1].min(255) as u8;
    buffer[offset + 2] = arr[2].min(255) as u8;
    buffer[offset + 3] = arr[3].min(255) as u8;
  }
}

impl PixelVector for u32x8 {
  const LANES: usize = 8;

  #[inline(always)]
  fn splat(value: u32) -> Self {
    u32x8::new([value; 8])
  }

  #[inline(always)]
  fn zero() -> Self {
    u32x8::ZERO
  }

  #[inline(always)]
  fn load_channels(buffer: &[u8], offset: usize) -> Self {
    u32x8::new([
      buffer[offset] as u32,
      buffer[offset + 1] as u32,
      buffer[offset + 2] as u32,
      buffer[offset + 3] as u32,
      buffer[offset + 4] as u32,
      buffer[offset + 5] as u32,
      buffer[offset + 6] as u32,
      buffer[offset + 7] as u32,
    ])
  }

  #[inline(always)]
  fn store_channels(self, buffer: &mut [u8], offset: usize) {
    let arr: [u32; 8] = self.into();
    for i in 0..8 {
      buffer[offset + i] = arr[i].min(255) as u8;
    }
  }
}

impl PixelVector for u32x16 {
  const LANES: usize = 16;

  #[inline(always)]
  fn splat(value: u32) -> Self {
    u32x16::new([value; 16])
  }

  #[inline(always)]
  fn zero() -> Self {
    u32x16::ZERO
  }

  #[inline(always)]
  fn load_channels(buffer: &[u8], offset: usize) -> Self {
    u32x16::new([
      buffer[offset] as u32,
      buffer[offset + 1] as u32,
      buffer[offset + 2] as u32,
      buffer[offset + 3] as u32,
      buffer[offset + 4] as u32,
      buffer[offset + 5] as u32,
      buffer[offset + 6] as u32,
      buffer[offset + 7] as u32,
      buffer[offset + 8] as u32,
      buffer[offset + 9] as u32,
      buffer[offset + 10] as u32,
      buffer[offset + 11] as u32,
      buffer[offset + 12] as u32,
      buffer[offset + 13] as u32,
      buffer[offset + 14] as u32,
      buffer[offset + 15] as u32,
    ])
  }

  #[inline(always)]
  fn store_channels(self, buffer: &mut [u8], offset: usize) {
    let arr: [u32; 16] = self.into();
    for i in 0..16 {
      buffer[offset + i] = arr[i].min(255) as u8;
    }
  }
}

#[derive(Clone, Copy)]
enum SimdBestFit {
  U32x4, // Baseline SIMD - 128-bit
  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  U32x8, // AVX2 - 256-bit
  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  U32x16, // AVX-512 - 512-bit
}

// Only for x86/x86_64 to extend above 128-bit SIMD
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_best_simd() -> SimdBestFit {
  static BEST_SIMD: std::sync::OnceLock<SimdBestFit> = std::sync::OnceLock::new();

  let best_fit = BEST_SIMD.get_or_init(|| {
    if is_x86_feature_detected!("avx512f") {
      return SimdBestFit::U32x16;
    }
    if is_x86_feature_detected!("avx2") {
      return SimdBestFit::U32x8;
    }

    SimdBestFit::U32x4
  });

  *best_fit
}

// For non-x86/x86_64 there is no runtime dispatch needed
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
const fn detect_best_simd() -> SimdBestFit {
  SimdBestFit::U32x4
}

/// Applies a Gaussian approximation using 3-pass Box Blur with SIMD.
/// Automatically selects the best SIMD implementation available at runtime.
pub(crate) fn apply_blur(image: &mut RgbaImage, radius: f32, blur_type: BlurType) {
  let sigma = blur_type.to_sigma(radius);
  if sigma <= 0.5 {
    return;
  }

  let (width, height) = image.dimensions();
  if width == 0 || height == 0 {
    return;
  }

  // Detect best SIMD implementation once and cache it
  let simd_impl = detect_best_simd();

  match simd_impl {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    SimdBestFit::U32x16 => unsafe {
      apply_blur_avx512(image, radius, blur_type);
    },
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    SimdBestFit::U32x8 => unsafe {
      apply_blur_avx2(image, radius, blur_type);
    },
    SimdBestFit::U32x4 => apply_blur_impl::<u32x4>(image, radius, blur_type),
  }
}

/// AVX-512 implementation wrapper with target feature enabled
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512f")]
unsafe fn apply_blur_avx512(image: &mut RgbaImage, radius: f32, blur_type: BlurType) {
  apply_blur_impl::<u32x16>(image, radius, blur_type);
}

/// AVX2 implementation wrapper with target feature enabled
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn apply_blur_avx2(image: &mut RgbaImage, radius: f32, blur_type: BlurType) {
  apply_blur_impl::<u32x8>(image, radius, blur_type);
}

#[inline(always)]
fn apply_blur_impl<V: PixelVector>(image: &mut RgbaImage, radius: f32, blur_type: BlurType) {
  let sigma = blur_type.to_sigma(radius);

  let box_radius = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) * 0.5)
    .round()
    .max(1.0) as u32;

  let (width, height) = image.dimensions();

  premultiply_alpha(image);

  let mut temp_data = vec![0u8; (width * height * 4) as usize];
  let img_data = &mut **image;
  let stride = width as usize * 4;

  // 3-pass Box Blur to approximate Gaussian
  for _ in 0..3 {
    box_blur_h(img_data, &mut temp_data, width, height, box_radius, stride);
    box_blur_v::<V>(&temp_data, img_data, width, height, box_radius, stride);
  }

  unpremultiply_alpha(image);
}

/// Horizontal Box Blur Pass - Use u32x4 to process RGBA of one pixel with correct sliding window
fn box_blur_h(src: &[u8], dst: &mut [u8], width: u32, height: u32, radius: u32, stride: usize) {
  let r = radius as i32;
  let w = width as i32;
  let div = (2 * r + 1) as u32;
  let (mul_val, shg) = compute_mul_shg(div);
  let mul = u32x4::new([mul_val; 4]);

  for y in 0..height {
    let line_offset = y as usize * stride;
    let mut sum = u32x4::ZERO;

    // Initialize sum with first pixel repeated (r+1) times
    let p_first = load_pixel(src, line_offset);
    sum += p_first * u32x4::new([r as u32 + 1; 4]);

    // Add trailing edge
    for dx in 1..=r {
      let px = dx.min(w - 1);
      sum += load_pixel(src, line_offset + (px as usize * 4));
    }

    // Slide window across the whole row
    for x in 0..w {
      let out_offset = line_offset + (x as usize * 4);
      store_pixel(dst, out_offset, (sum * mul) >> shg);

      let p_leaving_x = (x - r).max(0);
      let p_entering_x = (x + r + 1).min(w - 1);

      let p_leaving = load_pixel(src, line_offset + (p_leaving_x as usize * 4));
      let p_entering = load_pixel(src, line_offset + (p_entering_x as usize * 4));

      sum = sum + p_entering - p_leaving;
    }
  }
}

/// Vertical Box Blur Pass - Correctly leverages wide SIMD by processing multiple columns in parallel
fn box_blur_v<V: PixelVector>(
  src: &[u8],
  dst: &mut [u8],
  width: u32,
  height: u32,
  radius: u32,
  stride: usize,
) {
  let r = radius as i32;
  let h = height as i32;
  let div = (2 * r + 1) as u32;
  let (mul_val, shg) = compute_mul_shg(div);
  let mul = V::splat(mul_val);

  let batch_size = V::PIXELS_PER_BATCH as u32;

  let mut x = 0;
  while x + batch_size <= width {
    let col_offset = x as usize * 4;
    let mut sum = V::zero();

    // Initialize sum with first pixel of each column repeated (r+1) times
    let p_first = V::load_channels(src, col_offset);
    sum = sum + p_first * V::splat(r as u32 + 1);

    // Add trailing edge
    for dy in 1..=r {
      let py = dy.min(h - 1);
      sum = sum + V::load_channels(src, col_offset + (py as usize * stride));
    }

    // Slide window down the columns
    for y in 0..h {
      let out_offset = col_offset + (y as usize * stride);
      V::store_channels((sum * mul) >> shg, dst, out_offset);

      let p_leaving_y = (y - r).max(0);
      let p_entering_y = (y + r + 1).min(h - 1);

      let p_leaving = V::load_channels(src, col_offset + (p_leaving_y as usize * stride));
      let p_entering = V::load_channels(src, col_offset + (p_entering_y as usize * stride));

      sum = sum + p_entering - p_leaving;
    }

    x += batch_size;
  }

  // Handle remaining columns with u32x4
  for x in x..width {
    let col_offset = x as usize * 4;
    let mut sum = u32x4::ZERO;

    let p_first = load_pixel(src, col_offset);
    sum += p_first * u32x4::new([r as u32 + 1; 4]);

    for dy in 1..=r {
      let py = dy.min(h - 1);
      sum += load_pixel(src, col_offset + (py as usize * stride));
    }

    let mul_scalar = u32x4::new([mul_val; 4]);
    for y in 0..h {
      let out_offset = col_offset + (y as usize * stride);
      store_pixel(dst, out_offset, (sum * mul_scalar) >> shg);

      let p_leaving_y = (y - r).max(0);
      let p_entering_y = (y + r + 1).min(h - 1);

      let p_leaving = load_pixel(src, col_offset + (p_leaving_y as usize * stride));
      let p_entering = load_pixel(src, col_offset + (p_entering_y as usize * stride));

      sum = sum + p_entering - p_leaving;
    }
  }
}

#[inline(always)]
fn load_pixel(buffer: &[u8], offset: usize) -> u32x4 {
  u32x4::new([
    buffer[offset] as u32,
    buffer[offset + 1] as u32,
    buffer[offset + 2] as u32,
    buffer[offset + 3] as u32,
  ])
}

#[inline(always)]
fn store_pixel(buffer: &mut [u8], offset: usize, pixel: u32x4) {
  let arr: [u32; 4] = pixel.into();
  buffer[offset] = arr[0].min(255) as u8;
  buffer[offset + 1] = arr[1].min(255) as u8;
  buffer[offset + 2] = arr[2].min(255) as u8;
  buffer[offset + 3] = arr[3].min(255) as u8;
}

#[inline(always)]
fn compute_mul_shg(d: u32) -> (u32, i32) {
  let shg = 23;
  let mul = ((1u64 << shg) as f64 / d as f64).round() as u32;
  (mul, shg)
}

fn premultiply_alpha(image: &mut RgbaImage) {
  for pixel in image.pixels_mut() {
    let alpha = pixel.0[3] as u32;

    if alpha == 0 {
      pixel.0 = [0; 4];
    } else if alpha < 255 {
      for channel in pixel.0.iter_mut().take(3) {
        *channel = fast_div_255(*channel as u32 * alpha);
      }
    }
  }
}

fn unpremultiply_alpha(image: &mut RgbaImage) {
  for pixel in image.pixels_mut() {
    let a = pixel.0[3] as u16;

    if a != 0 && a < 255 {
      pixel.0[0] = ((pixel.0[0] as u16 * 255 + a / 2) / a).min(255) as u8;
      pixel.0[1] = ((pixel.0[1] as u16 * 255 + a / 2) / a).min(255) as u8;
      pixel.0[2] = ((pixel.0[2] as u16 * 255 + a / 2) / a).min(255) as u8;
    }
  }
}

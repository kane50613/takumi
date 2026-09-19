//! Per-target SIMD primitives behind one scalar-equivalent entry point.
//!
//! Every backend implements the same function with the same contract, `scalar`
//! is always compiled and is the reference the other backends are tested
//! against, and the target dispatch happens only in the `pub(crate)` entry
//! point so callers never see an instruction set.

/// A 16-pixel RGBA8 run, the unit every backend inspects at once.
pub(crate) type PixelChunk = [u8; 64];

/// Calls `visit` on every 16-pixel chunk whose alpha is neither all opaque
/// nor all transparent, then on the sub-chunk tail.
pub(crate) fn for_each_mixed_alpha_chunk(data: &mut [u8], visit: impl FnMut(&mut [[u8; 4]])) {
  #[cfg(target_arch = "x86_64")]
  if data.len() >= size_of::<PixelChunk>() && std::arch::is_x86_feature_detected!("avx2") {
    // SAFETY: guarded by the AVX2 runtime check above.
    unsafe { avx2::for_each_mixed_alpha_chunk(data, visit) };
    return;
  }

  visit_chunks(data, visit, best::alpha_is_uniform);
}

#[inline(always)]
fn visit_chunks(
  data: &mut [u8],
  mut visit: impl FnMut(&mut [[u8; 4]]),
  alpha_is_uniform: impl Fn(&PixelChunk) -> bool,
) {
  let (chunks, rest) = data.as_chunks_mut::<64>();
  for chunk in chunks {
    if alpha_is_uniform(chunk) {
      continue;
    }
    visit(chunk.as_chunks_mut::<4>().0);
  }
  visit(rest.as_chunks_mut::<4>().0);
}

#[cfg(target_arch = "aarch64")]
use neon as best;
#[cfg(not(any(
  target_arch = "aarch64",
  target_arch = "x86_64",
  all(target_arch = "wasm32", target_feature = "simd128")
)))]
use scalar as best;
#[cfg(target_arch = "x86_64")]
use sse2 as best;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use wasm128 as best;

#[allow(
  dead_code,
  reason = "the reference every SIMD backend is tested against"
)]
pub(crate) mod scalar {
  use super::PixelChunk;

  /// True when every pixel's alpha is 255, or every pixel's alpha is 0.
  #[inline(always)]
  pub(crate) fn alpha_is_uniform(chunk: &PixelChunk) -> bool {
    let mut and_acc = u32::MAX;
    let mut or_acc = 0u32;
    for word in chunk.as_chunks::<4>().0 {
      let word = u32::from_ne_bytes(*word);
      and_acc &= word;
      or_acc |= word;
    }
    and_acc >> 24 == 0xFF || or_acc >> 24 == 0
  }
}

#[cfg(target_arch = "aarch64")]
pub(crate) mod neon {
  use core::arch::aarch64::{
    vandq_u8, vld1q_u8, vmaxvq_u32, vminvq_u32, vorrq_u8, vreinterpretq_u32_u8, vshrq_n_u32,
  };

  use super::PixelChunk;

  #[inline(always)]
  pub(crate) fn alpha_is_uniform(chunk: &PixelChunk) -> bool {
    // SAFETY: NEON is baseline on aarch64 and the four loads stay inside `chunk`.
    unsafe {
      let p = chunk.as_ptr();
      let (v0, v1, v2, v3) = (
        vld1q_u8(p),
        vld1q_u8(p.add(16)),
        vld1q_u8(p.add(32)),
        vld1q_u8(p.add(48)),
      );
      let and = vandq_u8(vandq_u8(v0, v1), vandq_u8(v2, v3));
      let or = vorrq_u8(vorrq_u8(v0, v1), vorrq_u8(v2, v3));
      vminvq_u32(vshrq_n_u32::<24>(vreinterpretq_u32_u8(and))) == 0xFF
        || vmaxvq_u32(vshrq_n_u32::<24>(vreinterpretq_u32_u8(or))) == 0
    }
  }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod wasm128 {
  use core::arch::wasm32::{
    u32x4_all_true, u32x4_eq, u32x4_shr, u32x4_splat, v128, v128_and, v128_any_true, v128_load,
    v128_or,
  };

  use super::PixelChunk;

  #[inline(always)]
  pub(crate) fn alpha_is_uniform(chunk: &PixelChunk) -> bool {
    // SAFETY: the four unaligned loads stay inside `chunk`.
    unsafe {
      let p = chunk.as_ptr() as *const v128;
      let (v0, v1, v2, v3) = (
        v128_load(p),
        v128_load(p.add(1)),
        v128_load(p.add(2)),
        v128_load(p.add(3)),
      );
      let and = v128_and(v128_and(v0, v1), v128_and(v2, v3));
      let or = v128_or(v128_or(v0, v1), v128_or(v2, v3));
      u32x4_all_true(u32x4_eq(u32x4_shr(and, 24), u32x4_splat(0xFF)))
        || !v128_any_true(u32x4_shr(or, 24))
    }
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod sse2 {
  use core::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128,
    _mm_set1_epi8, _mm_setzero_si128,
  };

  use super::PixelChunk;

  // Alpha is byte 3 of each pixel, so bits 3, 7, 11, 15 of a 16-byte movemask.
  const ALPHA_LANES: i32 = 0x8888;

  #[inline(always)]
  pub(crate) fn alpha_is_uniform(chunk: &PixelChunk) -> bool {
    // SAFETY: SSE2 is baseline on x86_64 and the four unaligned loads stay inside `chunk`.
    unsafe {
      let p = chunk.as_ptr() as *const __m128i;
      let (v0, v1, v2, v3) = (
        _mm_loadu_si128(p),
        _mm_loadu_si128(p.add(1)),
        _mm_loadu_si128(p.add(2)),
        _mm_loadu_si128(p.add(3)),
      );
      let and = _mm_and_si128(_mm_and_si128(v0, v1), _mm_and_si128(v2, v3));
      let or = _mm_or_si128(_mm_or_si128(v0, v1), _mm_or_si128(v2, v3));
      let all_opaque = _mm_movemask_epi8(_mm_cmpeq_epi8(and, _mm_set1_epi8(-1))) & ALPHA_LANES;
      let all_clear = _mm_movemask_epi8(_mm_cmpeq_epi8(or, _mm_setzero_si128())) & ALPHA_LANES;
      all_opaque == ALPHA_LANES || all_clear == ALPHA_LANES
    }
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod avx2 {
  use core::arch::x86_64::{
    __m256i, _mm256_and_si256, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
    _mm256_or_si256, _mm256_set1_epi8, _mm256_setzero_si256,
  };

  use super::{PixelChunk, visit_chunks};

  const ALPHA_LANES: u32 = 0x8888_8888;

  /// Caller must have checked `is_x86_feature_detected!("avx2")`.
  #[target_feature(enable = "avx2")]
  pub(crate) fn for_each_mixed_alpha_chunk(data: &mut [u8], visit: impl FnMut(&mut [[u8; 4]])) {
    visit_chunks(data, visit, |chunk| alpha_is_uniform(chunk));
  }

  #[target_feature(enable = "avx2")]
  #[inline]
  pub(crate) fn alpha_is_uniform(chunk: &PixelChunk) -> bool {
    // SAFETY: the two unaligned loads stay inside `chunk`.
    unsafe {
      let p = chunk.as_ptr() as *const __m256i;
      let (v0, v1) = (_mm256_loadu_si256(p), _mm256_loadu_si256(p.add(1)));
      let and = _mm256_and_si256(v0, v1);
      let or = _mm256_or_si256(v0, v1);
      let all_opaque = _mm256_movemask_epi8(_mm256_cmpeq_epi8(and, _mm256_set1_epi8(-1))) as u32;
      let all_clear = _mm256_movemask_epi8(_mm256_cmpeq_epi8(or, _mm256_setzero_si256())) as u32;
      all_opaque & ALPHA_LANES == ALPHA_LANES || all_clear & ALPHA_LANES == ALPHA_LANES
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn chunks() -> Vec<PixelChunk> {
    let mut chunks = vec![[0u8; 64], [0xFF; 64]];
    let mut opaque_rgb_noise = [0xFF; 64];
    for (i, byte) in opaque_rgb_noise.iter_mut().enumerate() {
      if i % 4 != 3 {
        *byte = (i * 37) as u8;
      }
    }
    chunks.push(opaque_rgb_noise);
    let mut clear_rgb_noise = [0u8; 64];
    for (i, byte) in clear_rgb_noise.iter_mut().enumerate() {
      if i % 4 != 3 {
        *byte = (i * 53) as u8;
      }
    }
    chunks.push(clear_rgb_noise);
    for pixel in 0..16 {
      for alpha in [0u8, 1, 0x80, 0xFE, 0xFF] {
        let mut opaque = [0xFF; 64];
        opaque[pixel * 4 + 3] = alpha;
        chunks.push(opaque);
        let mut clear = [0u8; 64];
        clear[pixel * 4 + 3] = alpha;
        chunks.push(clear);
      }
    }
    chunks
  }

  fn assert_matches_scalar(name: &str, backend: impl Fn(&PixelChunk) -> bool) {
    for chunk in chunks() {
      assert_eq!(
        backend(&chunk),
        scalar::alpha_is_uniform(&chunk),
        "{name} disagrees with scalar on {chunk:?}"
      );
    }
  }

  #[test]
  fn scalar_reference() {
    assert!(scalar::alpha_is_uniform(&[0xFF; 64]));
    assert!(scalar::alpha_is_uniform(&[0; 64]));
    let mut mixed = [0xFF; 64];
    mixed[3] = 0x80;
    assert!(!scalar::alpha_is_uniform(&mixed));
    let mut one_visible = [0; 64];
    one_visible[63] = 1;
    assert!(!scalar::alpha_is_uniform(&one_visible));
  }

  #[test]
  fn best_backend_matches_scalar() {
    assert_matches_scalar("best", best::alpha_is_uniform);
  }

  #[cfg(target_arch = "x86_64")]
  #[test]
  fn avx2_matches_scalar() {
    if !std::arch::is_x86_feature_detected!("avx2") {
      return;
    }
    // SAFETY: guarded by the AVX2 runtime check above.
    assert_matches_scalar("avx2", |chunk| unsafe { avx2::alpha_is_uniform(chunk) });
  }

  #[test]
  fn visits_mixed_chunks_and_tail() {
    let mut data = vec![0xFF; 64 * 3 + 8];
    data[64 + 3] = 0x80;
    let mut visited = Vec::new();
    for_each_mixed_alpha_chunk(&mut data, |pixels| visited.push(pixels.len()));
    assert_eq!(visited, [16, 2]);
  }
}

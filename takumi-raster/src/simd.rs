//! SIMD primitives behind one instruction-set-agnostic entry point. Every
//! backend module implements the same functions over a [`PixelRun`], `scalar`
//! is always compiled as the reference the others are tested against, and
//! [`Simd`] is the only place that picks a backend.

#[cfg(target_arch = "x86_64")]
use avx2::Avx2;

/// Sixteen RGBA8 pixels, the run every backend classifies at once.
pub(crate) type PixelRun = [u8; 64];

/// The widest instruction set the running CPU supports.
#[derive(Clone, Copy)]
pub(crate) enum Simd {
  /// The compile-time baseline: NEON, SSE2, wasm simd128, or scalar.
  Baseline,
  #[cfg(target_arch = "x86_64")]
  Avx2(Avx2),
}

impl Simd {
  pub(crate) fn detect() -> Self {
    #[cfg(target_arch = "x86_64")]
    if let Some(avx2) = Avx2::detect() {
      return Self::Avx2(avx2);
    }

    Self::Baseline
  }

  /// Hands `edit` every 16-pixel run whose alpha is neither all 255 nor all 0,
  /// then the tail shorter than a run.
  pub(crate) fn edit_mixed_alpha_runs(self, pixels: &mut [u8], edit: impl FnMut(&mut [[u8; 4]])) {
    match self {
      Self::Baseline => edit_runs_unless(pixels, edit, baseline::alpha_is_uniform),
      #[cfg(target_arch = "x86_64")]
      Self::Avx2(avx2) => avx2.edit_mixed_alpha_runs(pixels, edit),
    }
  }
}

#[inline(always)]
fn edit_runs_unless(
  pixels: &mut [u8],
  mut edit: impl FnMut(&mut [[u8; 4]]),
  skip: impl Fn(&PixelRun) -> bool,
) {
  let (runs, tail) = pixels.as_chunks_mut::<64>();
  for run in runs {
    if skip(run) {
      continue;
    }
    edit(run.as_chunks_mut::<4>().0);
  }
  edit(tail.as_chunks_mut::<4>().0);
}

#[cfg(all(target_arch = "aarch64", target_endian = "little"))]
use neon as baseline;
#[cfg(not(any(
  all(target_arch = "aarch64", target_endian = "little"),
  target_arch = "x86_64",
  all(target_arch = "wasm32", target_feature = "simd128")
)))]
use scalar as baseline;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use simd128 as baseline;
#[cfg(target_arch = "x86_64")]
use sse2 as baseline;

#[allow(
  dead_code,
  reason = "the reference every SIMD backend is tested against"
)]
pub(crate) mod scalar {
  use super::PixelRun;

  /// True when every pixel's alpha is 255, or every pixel's alpha is 0.
  #[inline(always)]
  pub(crate) fn alpha_is_uniform(run: &PixelRun) -> bool {
    let mut all = u32::MAX;
    let mut any = 0u32;
    for pixel in run.as_chunks::<4>().0 {
      let pixel = u32::from_le_bytes(*pixel);
      all &= pixel;
      any |= pixel;
    }
    all >> 24 == 0xFF || any >> 24 == 0
  }
}

#[cfg(all(target_arch = "aarch64", target_endian = "little"))]
pub(crate) mod neon {
  use std::arch::aarch64::{
    vandq_u8, vld1q_u8, vmaxvq_u32, vminvq_u32, vorrq_u8, vreinterpretq_u32_u8, vshrq_n_u32,
  };

  use super::PixelRun;

  #[inline(always)]
  pub(crate) fn alpha_is_uniform(run: &PixelRun) -> bool {
    // SAFETY: NEON is baseline on aarch64 and the loads stay inside `run`.
    unsafe {
      let p = run.as_ptr();
      let (v0, v1, v2, v3) = (
        vld1q_u8(p),
        vld1q_u8(p.add(16)),
        vld1q_u8(p.add(32)),
        vld1q_u8(p.add(48)),
      );
      let all = vandq_u8(vandq_u8(v0, v1), vandq_u8(v2, v3));
      let any = vorrq_u8(vorrq_u8(v0, v1), vorrq_u8(v2, v3));
      vminvq_u32(vshrq_n_u32::<24>(vreinterpretq_u32_u8(all))) == 0xFF
        || vmaxvq_u32(vshrq_n_u32::<24>(vreinterpretq_u32_u8(any))) == 0
    }
  }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod simd128 {
  use std::arch::wasm32::{
    u32x4_all_true, u32x4_eq, u32x4_shr, u32x4_splat, v128, v128_and, v128_any_true, v128_load,
    v128_or,
  };

  use super::PixelRun;

  #[inline(always)]
  pub(crate) fn alpha_is_uniform(run: &PixelRun) -> bool {
    // SAFETY: the unaligned loads stay inside `run`.
    unsafe {
      let p = run.as_ptr() as *const v128;
      let (v0, v1, v2, v3) = (
        v128_load(p),
        v128_load(p.add(1)),
        v128_load(p.add(2)),
        v128_load(p.add(3)),
      );
      let all = v128_and(v128_and(v0, v1), v128_and(v2, v3));
      let any = v128_or(v128_or(v0, v1), v128_or(v2, v3));
      u32x4_all_true(u32x4_eq(u32x4_shr(all, 24), u32x4_splat(0xFF)))
        || !v128_any_true(u32x4_shr(any, 24))
    }
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod sse2 {
  use std::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128,
    _mm_set1_epi8, _mm_setzero_si128,
  };

  use super::PixelRun;

  /// Movemask bits of the alpha byte in each of 4 pixels.
  const ALPHA_LANES: i32 = 0x8888;

  #[inline(always)]
  pub(crate) fn alpha_is_uniform(run: &PixelRun) -> bool {
    // SAFETY: SSE2 is baseline on x86_64 and the unaligned loads stay inside `run`.
    unsafe {
      let p = run.as_ptr() as *const __m128i;
      let (v0, v1, v2, v3) = (
        _mm_loadu_si128(p),
        _mm_loadu_si128(p.add(1)),
        _mm_loadu_si128(p.add(2)),
        _mm_loadu_si128(p.add(3)),
      );
      let all = _mm_and_si128(_mm_and_si128(v0, v1), _mm_and_si128(v2, v3));
      let any = _mm_or_si128(_mm_or_si128(v0, v1), _mm_or_si128(v2, v3));
      let all_opaque = _mm_movemask_epi8(_mm_cmpeq_epi8(all, _mm_set1_epi8(-1))) & ALPHA_LANES;
      let all_clear = _mm_movemask_epi8(_mm_cmpeq_epi8(any, _mm_setzero_si128())) & ALPHA_LANES;
      all_opaque == ALPHA_LANES || all_clear == ALPHA_LANES
    }
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod avx2 {
  use std::arch::{
    is_x86_feature_detected,
    x86_64::{
      __m256i, _mm256_and_si256, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
      _mm256_or_si256, _mm256_set1_epi8, _mm256_setzero_si256,
    },
  };

  use super::{PixelRun, edit_runs_unless};

  /// Movemask bits of the alpha byte in each of 8 pixels.
  const ALPHA_LANES: u32 = 0x8888_8888;

  /// Proof that this CPU runs AVX2; only [`Avx2::detect`] constructs it.
  #[derive(Clone, Copy)]
  pub(crate) struct Avx2(());

  impl Avx2 {
    pub(crate) fn detect() -> Option<Self> {
      is_x86_feature_detected!("avx2").then_some(Self(()))
    }

    pub(crate) fn edit_mixed_alpha_runs(self, pixels: &mut [u8], edit: impl FnMut(&mut [[u8; 4]])) {
      // SAFETY: `self` exists only after AVX2 was detected.
      unsafe { edit_mixed_alpha_runs(pixels, edit) }
    }

    #[cfg(test)]
    pub(crate) fn alpha_is_uniform(self, run: &PixelRun) -> bool {
      // SAFETY: `self` exists only after AVX2 was detected.
      unsafe { alpha_is_uniform(run) }
    }
  }

  #[target_feature(enable = "avx2")]
  fn edit_mixed_alpha_runs(pixels: &mut [u8], edit: impl FnMut(&mut [[u8; 4]])) {
    edit_runs_unless(pixels, edit, |run| alpha_is_uniform(run));
  }

  #[target_feature(enable = "avx2")]
  #[inline]
  fn alpha_is_uniform(run: &PixelRun) -> bool {
    // SAFETY: the unaligned loads stay inside `run`.
    unsafe {
      let p = run.as_ptr() as *const __m256i;
      let (v0, v1) = (_mm256_loadu_si256(p), _mm256_loadu_si256(p.add(1)));
      let all = _mm256_and_si256(v0, v1);
      let any = _mm256_or_si256(v0, v1);
      let all_opaque = _mm256_movemask_epi8(_mm256_cmpeq_epi8(all, _mm256_set1_epi8(-1))) as u32;
      let all_clear = _mm256_movemask_epi8(_mm256_cmpeq_epi8(any, _mm256_setzero_si256())) as u32;
      all_opaque & ALPHA_LANES == ALPHA_LANES || all_clear & ALPHA_LANES == ALPHA_LANES
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn runs() -> Vec<PixelRun> {
    let mut runs = vec![[0u8; 64], [0xFF; 64]];
    for (fill, salt) in [(0xFF, 37), (0, 53)] {
      let mut rgb_noise = [fill; 64];
      for (i, byte) in rgb_noise.iter_mut().enumerate() {
        if i % 4 != 3 {
          *byte = (i * salt) as u8;
        }
      }
      runs.push(rgb_noise);
    }
    for pixel in 0..16 {
      for alpha in [0u8, 1, 0x80, 0xFE, 0xFF] {
        for fill in [0xFF, 0] {
          let mut run = [fill; 64];
          run[pixel * 4 + 3] = alpha;
          runs.push(run);
        }
      }
    }
    runs
  }

  fn assert_matches_scalar(name: &str, backend: impl Fn(&PixelRun) -> bool) {
    for run in runs() {
      assert_eq!(
        backend(&run),
        scalar::alpha_is_uniform(&run),
        "{name} disagrees with scalar on {run:?}"
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
  fn baseline_matches_scalar() {
    assert_matches_scalar("baseline", baseline::alpha_is_uniform);
  }

  #[cfg(target_arch = "x86_64")]
  #[test]
  fn avx2_matches_scalar() {
    if let Some(avx2) = Avx2::detect() {
      assert_matches_scalar("avx2", |run| avx2.alpha_is_uniform(run));
    }
  }

  #[test]
  fn edits_mixed_runs_and_tail() {
    let mut pixels = vec![0xFF; 64 * 3 + 8];
    pixels[64 + 3] = 0x80;
    let mut edited = Vec::new();
    Simd::detect().edit_mixed_alpha_runs(&mut pixels, |run| edited.push(run.len()));
    assert_eq!(edited, [16, 2]);
  }
}

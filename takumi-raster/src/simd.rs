//! Per-target SIMD primitives; `scalar` is the reference every backend is tested against.

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

  /// Hands `edit` every run of mixed alpha, then the tail shorter than a run.
  pub(crate) fn edit_mixed_alpha_runs(self, pixels: &mut [u8], edit: impl FnMut(&mut [[u8; 4]])) {
    match self {
      Self::Baseline => edit_runs_unless(pixels, edit, baseline::alpha_is_uniform),
      #[cfg(target_arch = "x86_64")]
      Self::Avx2(avx2) => avx2.edit_mixed_alpha_runs(pixels, edit),
    }
  }
}

/// Four gradient projections to LUT indices; `axis_length` and `scale` are finite and
/// non-negative and the scaled index stays below 2^31, which every gradient LUT satisfies.
pub(crate) use baseline::linear_lut_indices;

/// Slides a box-blur window across four alpha samples: returns the four packed
/// outputs and the window sum after the fourth step, exactly as four scalar
/// `out = (sum * mul) >> shift; sum += entering - leaving` steps would.
#[inline(always)]
pub(crate) fn slide_box_alpha4(
  sum: u32,
  entering: [u8; 4],
  leaving: [u8; 4],
  mul: u32,
  shift: u32,
) -> ([u8; 4], u32) {
  baseline::slide_box_alpha4(sum, entering, leaving, mul, shift)
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

  /// Clamp to the axis, scale, round half away from zero, clamp to `max_index`.
  #[inline(always)]
  pub(crate) fn lut_index(projection: f32, axis_length: f32, scale: f32, max_index: u32) -> u32 {
    let position = projection.clamp(0.0, axis_length);
    ((position * scale).round() as u32).min(max_index)
  }

  #[inline(always)]
  pub(crate) fn linear_lut_indices(
    projections: [f32; 4],
    axis_length: f32,
    scale: f32,
    max_index: u32,
  ) -> [u32; 4] {
    projections.map(|projection| lut_index(projection, axis_length, scale, max_index))
  }

  #[inline(always)]
  pub(crate) fn slide_box_alpha4(
    mut sum: u32,
    entering: [u8; 4],
    leaving: [u8; 4],
    mul: u32,
    shift: u32,
  ) -> ([u8; 4], u32) {
    let mut out = [0u8; 4];
    for lane in 0..4 {
      out[lane] = ((sum * mul) >> shift) as u8;
      sum = sum + entering[lane] as u32 - leaving[lane] as u32;
    }
    (out, sum)
  }
}

#[cfg(all(target_arch = "aarch64", target_endian = "little"))]
pub(crate) mod neon {
  use std::arch::aarch64::*;

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

  #[inline(always)]
  pub(crate) fn linear_lut_indices(
    projections: [f32; 4],
    axis_length: f32,
    scale: f32,
    max_index: u32,
  ) -> [u32; 4] {
    // SAFETY: NEON is baseline on aarch64; loads and stores use owned arrays.
    unsafe {
      let projections = vld1q_f32(projections.as_ptr());
      let position = vminq_f32(
        vmaxq_f32(projections, vdupq_n_f32(0.0)),
        vdupq_n_f32(axis_length),
      );
      let rounded = vrndaq_f32(vmulq_n_f32(position, scale));
      let indices = vminq_u32(vcvtq_u32_f32(rounded), vdupq_n_u32(max_index));
      let mut out = [0u32; 4];
      vst1q_u32(out.as_mut_ptr(), indices);
      out
    }
  }

  #[inline(always)]
  pub(crate) fn slide_box_alpha4(
    sum: u32,
    entering: [u8; 4],
    leaving: [u8; 4],
    mul: u32,
    shift: u32,
  ) -> ([u8; 4], u32) {
    // SAFETY: NEON is baseline on aarch64; every load and store uses an owned array.
    unsafe {
      let widen = |bytes: [u8; 4]| {
        vmovl_u16(vget_low_u16(vmovl_u8(vcreate_u8(
          u32::from_le_bytes(bytes) as u64,
        ))))
      };
      let zero = vdupq_n_u32(0);
      let delta = vsubq_u32(widen(entering), widen(leaving));
      let scan = vaddq_u32(delta, vextq_u32::<3>(zero, delta));
      let scan = vaddq_u32(scan, vextq_u32::<2>(zero, scan));
      let sums = vaddq_u32(vdupq_n_u32(sum), vextq_u32::<3>(zero, scan));
      let packed = vshlq_u32(vmulq_n_u32(sums, mul), vdupq_n_s32(-(shift as i32)));
      let mut lanes = [0u32; 4];
      vst1q_u32(lanes.as_mut_ptr(), packed);
      (
        lanes.map(|lane| lane as u8),
        sum.wrapping_add(vgetq_lane_u32::<3>(scan)),
      )
    }
  }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod simd128 {
  use std::arch::wasm32::*;

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

  /// `f32x4_nearest` rounds half to even; for non-negative inputs
  /// `trunc + (frac >= 0.5)` equals `f32::round`.
  #[inline(always)]
  pub(crate) fn linear_lut_indices(
    projections: [f32; 4],
    axis_length: f32,
    scale: f32,
    max_index: u32,
  ) -> [u32; 4] {
    // SAFETY: the load and store use owned arrays.
    unsafe {
      let projections = v128_load(projections.as_ptr() as *const v128);
      let position = f32x4_min(
        f32x4_max(projections, f32x4_splat(0.0)),
        f32x4_splat(axis_length),
      );
      let scaled = f32x4_mul(position, f32x4_splat(scale));
      let truncated = f32x4_trunc(scaled);
      let round_up = v128_and(
        f32x4_ge(f32x4_sub(scaled, truncated), f32x4_splat(0.5)),
        f32x4_splat(1.0),
      );
      let indices = u32x4_min(
        u32x4_trunc_sat_f32x4(f32x4_add(truncated, round_up)),
        u32x4_splat(max_index),
      );
      let mut out = [0u32; 4];
      v128_store(out.as_mut_ptr() as *mut v128, indices);
      out
    }
  }

  #[inline(always)]
  pub(crate) fn slide_box_alpha4(
    sum: u32,
    entering: [u8; 4],
    leaving: [u8; 4],
    mul: u32,
    shift: u32,
  ) -> ([u8; 4], u32) {
    let widen = |bytes: [u8; 4]| {
      u32x4(
        bytes[0] as u32,
        bytes[1] as u32,
        bytes[2] as u32,
        bytes[3] as u32,
      )
    };
    let zero = u32x4_splat(0);
    let delta = u32x4_sub(widen(entering), widen(leaving));
    let scan = u32x4_add(delta, u32x4_shuffle::<4, 0, 1, 2>(delta, zero));
    let scan = u32x4_add(scan, u32x4_shuffle::<4, 5, 0, 1>(scan, zero));
    let sums = u32x4_add(u32x4_splat(sum), u32x4_shuffle::<4, 0, 1, 2>(scan, zero));
    let packed = u32x4_shr(u32x4_mul(sums, u32x4_splat(mul)), shift);
    let lanes = [
      u32x4_extract_lane::<0>(packed),
      u32x4_extract_lane::<1>(packed),
      u32x4_extract_lane::<2>(packed),
      u32x4_extract_lane::<3>(packed),
    ];
    (
      lanes.map(|lane| lane as u8),
      sum.wrapping_add(u32x4_extract_lane::<3>(scan)),
    )
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod sse2 {
  use std::arch::x86_64::*;

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

  /// SSE2 has no round instruction; for non-negative inputs `trunc + (frac >= 0.5)`
  /// equals `f32::round`.
  #[inline(always)]
  pub(crate) fn linear_lut_indices(
    projections: [f32; 4],
    axis_length: f32,
    scale: f32,
    max_index: u32,
  ) -> [u32; 4] {
    // SAFETY: SSE2 is baseline on x86_64; loads and stores use owned arrays.
    unsafe {
      let projections = _mm_loadu_ps(projections.as_ptr());
      let position = _mm_min_ps(
        _mm_max_ps(projections, _mm_set1_ps(0.0)),
        _mm_set1_ps(axis_length),
      );
      let scaled = _mm_mul_ps(position, _mm_set1_ps(scale));
      let truncated = _mm_cvtepi32_ps(_mm_cvttps_epi32(scaled));
      let round_up = _mm_and_ps(
        _mm_cmpge_ps(_mm_sub_ps(scaled, truncated), _mm_set1_ps(0.5)),
        _mm_set1_ps(1.0),
      );
      let mut rounded = [0f32; 4];
      _mm_storeu_ps(rounded.as_mut_ptr(), _mm_add_ps(truncated, round_up));
      rounded.map(|value| (value as u32).min(max_index))
    }
  }

  #[inline(always)]
  pub(crate) fn slide_box_alpha4(
    sum: u32,
    entering: [u8; 4],
    leaving: [u8; 4],
    mul: u32,
    shift: u32,
  ) -> ([u8; 4], u32) {
    // SAFETY: SSE2 is baseline on x86_64; every load and store uses an owned array.
    unsafe {
      let widen = |bytes: [u8; 4]| {
        let zero = _mm_setzero_si128();
        _mm_unpacklo_epi16(
          _mm_unpacklo_epi8(_mm_cvtsi32_si128(u32::from_le_bytes(bytes) as i32), zero),
          zero,
        )
      };
      let delta = _mm_sub_epi32(widen(entering), widen(leaving));
      let scan = _mm_add_epi32(delta, _mm_slli_si128::<4>(delta));
      let scan = _mm_add_epi32(scan, _mm_slli_si128::<8>(scan));
      let sums = _mm_add_epi32(_mm_set1_epi32(sum as i32), _mm_slli_si128::<4>(scan));
      let factor = _mm_set1_epi32(mul as i32);
      let even = _mm_mul_epu32(sums, factor);
      let odd = _mm_mul_epu32(_mm_srli_epi64::<32>(sums), factor);
      let products = _mm_unpacklo_epi32(
        _mm_shuffle_epi32::<0b10_00_10_00>(even),
        _mm_shuffle_epi32::<0b10_00_10_00>(odd),
      );
      let packed = _mm_srl_epi32(products, _mm_cvtsi32_si128(shift as i32));
      let mut lanes = [0u32; 4];
      _mm_storeu_si128(lanes.as_mut_ptr() as *mut __m128i, packed);
      let mut scanned = [0u32; 4];
      _mm_storeu_si128(scanned.as_mut_ptr() as *mut __m128i, scan);
      (lanes.map(|lane| lane as u8), sum.wrapping_add(scanned[3]))
    }
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod avx2 {
  use std::arch::{is_x86_feature_detected, x86_64::*};

  use super::{PixelRun, edit_runs_unless};

  /// Movemask bits of the alpha byte in each of 8 pixels.
  const ALPHA_LANES: u32 = 0x8888_8888;

  /// Proof of AVX2 support; only [`Avx2::detect`] constructs it.
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
  fn baseline_lut_indices_match_scalar() {
    let cases = [
      [0.0f32, -3.5, 12.49999, 12.5],
      [12.500001, 4299.9, 4300.0, 9999.0],
      [0.49999997, 0.5, 1.5, 2.5],
      [1e-3, 2149.5, 3.4999998, 1234.5678],
    ];

    for projections in cases {
      assert_eq!(
        baseline::linear_lut_indices(projections, 4300.0, 4095.0 / 4300.0, 4095),
        scalar::linear_lut_indices(projections, 4300.0, 4095.0 / 4300.0, 4095),
        "{projections:?}"
      );
    }

    assert_eq!(
      baseline::linear_lut_indices([0.499_999_97, 0.5, 1.5, 2.5], 8.0, 1.0, 8),
      [0, 1, 2, 3]
    );
    let scale = 1023.0 / 2000.0;

    for step in 0..200_000u32 {
      let base = step as f32 * 0.0100003 - 50.0;
      let projections = [base, base + 0.25, base + 0.5, base + 0.75];
      assert_eq!(
        baseline::linear_lut_indices(projections, 2000.0, scale, 1023),
        scalar::linear_lut_indices(projections, 2000.0, scale, 1023),
        "{projections:?}"
      );
    }
  }

  #[test]
  fn baseline_slide_box_alpha4_matches_scalar() {
    let mut state = 0x2545_F491u32;
    let mut next = || {
      state ^= state << 13;
      state ^= state >> 17;
      state ^= state << 5;
      state
    };
    for radius in [1u32, 4, 25, 200] {
      let div = 2 * radius + 1;
      let mul = ((1u64 << 23) as f64 / div as f64).round() as u32;
      for _ in 0..20_000 {
        let entering = next().to_le_bytes();
        let leaving = next().to_le_bytes();
        let window_floor = leaving.iter().map(|&b| b as u32).sum::<u32>();
        let headroom = (255 * div).saturating_sub(window_floor).max(1);
        let sum = window_floor + next() % headroom;
        assert_eq!(
          baseline::slide_box_alpha4(sum, entering, leaving, mul, 23),
          scalar::slide_box_alpha4(sum, entering, leaving, mul, 23),
          "sum={sum} entering={entering:?} leaving={leaving:?} radius={radius}"
        );
      }
    }
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

//! The blur of a shadow, approximated for PDF, which has no blur operator.
//!
//! Approximate: a blurred shadow is a stack of bands, each the shape spread a little further and
//! a little fainter, so the edge steps through the Gaussian's coverage instead of fading
//! smoothly. Blink's PDF output rasterizes the blurred mask instead.

/// Bands used to fake one blurred edge. Eight is enough that the steps read as
/// a gradient at the blur radii interfaces actually use.
const BLUR_BANDS: usize = 8;

/// One drawn copy of a shadow's shape: how much further it spreads, and how opaque it is.
pub(crate) struct Band {
  pub(crate) spread: f32,
  pub(crate) alpha: f32,
}

impl Band {
  /// A sharp shadow is one band at full alpha. A blurred one is a stack from the
  /// outermost, faintest band inward, with each band's alpha chosen so the fills
  /// composite to the coverage the blur would have had at that distance.
  pub(crate) fn of(blur_radius: f32) -> Vec<Self> {
    let core = Band {
      spread: 0.0,
      alpha: 1.0,
    };

    if blur_radius <= 0.0 {
      return vec![core];
    }
    // The unblurred shape is fully opaque; the blur only fades outward from its
    // edge, so that core is the last band drawn.
    let mut bands = Vec::with_capacity(BLUR_BANDS + 1);
    let mut covered = 0.0;

    for index in 0..BLUR_BANDS {
      // Walk inward: the outermost band sits a full blur radius out and is the
      // faintest, the innermost sits at the sharp edge and is opaque.
      let t = (index as f32 + 1.0) / BLUR_BANDS as f32;
      let target = coverage(1.0 - t);
      let alpha = ((target - covered) / (1.0 - covered)).clamp(0.0, 1.0);

      covered = target;
      bands.push(Band {
        spread: blur_radius * (1.0 - t),
        alpha,
      });
    }
    bands.push(core);
    bands
  }
}

/// Coverage a Gaussian blur leaves `distance` blur radii outside the shape.
///
/// CSS blurs with a standard deviation of half the blur radius, so the edge
/// profile is `0.5 * erfc(distance * blur / (sigma * sqrt(2)))`, which reduces
/// to `0.5 * erfc(sqrt(2) * distance)`.
fn coverage(distance: f32) -> f32 {
  0.5 * erfc(core::f32::consts::SQRT_2 * distance)
}

/// Abramowitz and Stegun 7.1.26, accurate to about 1e-7 over the range a
/// shadow band asks about.
fn erfc(x: f32) -> f32 {
  const A: [f32; 5] = [
    0.254_829_6,
    -0.284_496_74,
    1.421_413_7,
    -1.453_152,
    1.061_405_4,
  ];
  let sign = if x < 0.0 { -1.0 } else { 1.0 };
  let x = x.abs();
  let t = 1.0 / (1.0 + 0.327_591_1 * x);
  let poly = A.iter().rev().fold(0.0, |accumulated, coefficient| {
    (accumulated + coefficient) * t
  });
  let erf = 1.0 - poly * (-x * x).exp();

  1.0 - sign * erf
}

#[cfg(test)]
mod tests {
  use super::{Band, coverage};

  #[test]
  fn a_blurred_shadow_has_an_opaque_core() {
    let bands = Band::of(12.0);
    let core = bands.last().expect("a band");

    assert_eq!(core.alpha, 1.0);
    assert_eq!(core.spread, 0.0);
  }

  #[test]
  fn coverage_follows_the_gaussian_edge() {
    // A CSS blur has a standard deviation of half the blur radius, so half a
    // blur radius out is one sigma, where a normal tail leaves 0.1587.
    assert!((coverage(0.0) - 0.5).abs() < 1e-3);
    assert!((coverage(0.5) - 0.158_7).abs() < 1e-3);
    assert!(coverage(1.0) < 0.024);
  }
}

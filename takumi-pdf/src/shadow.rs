//! `box-shadow` as vector fills.
//!
//! Everything but the blur is exact: the shadow shape is the border box
//! offset and spread, and the box itself is cut out with an even-odd fill so
//! the shadow never paints under an opaque element. The blur is approximated
//! by stacking bands, since PDF has no blur operator.

use takumi_core::{
  geometry::{PathCommand, Point as CorePoint, Size},
  layout::{border::BorderProperties, decoration::ClipBox},
  shadow::SizedShadow,
  style::Color,
};

use crate::{
  krilla::{
    paint::{Fill, FillRule},
    surface::Surface,
  },
  paint::{fill_from_rgba, krilla_path},
};

/// Bands used to fake one blurred edge. Eight is enough that the steps read as
/// a gradient at the blur radii interfaces actually use.
// ponytail: a real Gaussian needs a raster pass or a soft mask; raise this or
// rasterize the shadow layer if someone lands a design with a huge blur.
const BLUR_BANDS: usize = 8;

/// Paints the outer shadows of a box, furthest layer first.
pub(crate) fn emit_outer_shadows(
  shadows: &[SizedShadow],
  border: &BorderProperties,
  size: Size<f32>,
  at: (f32, f32),
  surface: &mut Surface,
) {
  let mut element = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

  border.append_mask_commands(&mut element, size, CorePoint::ZERO);

  for shadow in shadows.iter().rev() {
    for band in Band::of(shadow) {
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
      let (shape, spread_size) = border.outset_shadow_box(size, band.spread);

      shape.append_mask_commands(
        &mut commands,
        spread_size,
        CorePoint {
          x: shadow.offset_x - band.spread,
          y: shadow.offset_y - band.spread,
        },
      );
      // The element's own shape, unshifted, so the even-odd fill leaves a ring.
      commands.extend_from_slice(&element);
      fill(&commands, shadow.color, band.alpha, at, surface);
    }
  }
}

/// Paints the inset shadows of a box, on top of its background. CSS draws
/// these inside the padding box, so a border neither carries shadow paint nor
/// widens it.
pub(crate) fn emit_inset_shadows(
  shadows: &[SizedShadow],
  clip: &ClipBox,
  at: (f32, f32),
  surface: &mut Surface,
) {
  let origin = CorePoint {
    x: at.0 + clip.offset.x,
    y: at.1 + clip.offset.y,
  };

  for shadow in shadows.iter().rev() {
    for band in Band::of(shadow) {
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

      // The filled region is the padding box minus the hole the shadow casts
      // into it, so a positive spread shrinks the hole.
      clip
        .border
        .append_mask_commands(&mut commands, clip.size, CorePoint::ZERO);

      let hole = ClipBox::inset_shadow_hole(
        clip.border,
        clip.size,
        band.spread,
        CorePoint {
          x: shadow.offset_x,
          y: shadow.offset_y,
        },
      );

      hole
        .border
        .append_mask_commands(&mut commands, hole.size, hole.offset);
      fill(
        &commands,
        shadow.color,
        band.alpha,
        (origin.x, origin.y),
        surface,
      );
    }
  }
}

/// One drawn ring of a shadow: how far it spreads, and how opaque it is.
struct Band {
  spread: f32,
  alpha: f32,
}

impl Band {
  /// A sharp shadow is one band at full alpha. A blurred one is a stack from the
  /// outermost, faintest band inward, with each band's alpha chosen so the fills
  /// composite to the coverage the blur would have had at that distance.
  fn of(shadow: &SizedShadow) -> Vec<Self> {
    if shadow.blur_radius <= 0.0 {
      return vec![Band {
        spread: shadow.spread_radius,
        alpha: 1.0,
      }];
    }
    // The shifted, unblurred shape is fully opaque; the blur only fades outward
    // from its edge, so that core is the last band drawn.
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
        spread: shadow.spread_radius + shadow.blur_radius * (1.0 - t),
        alpha,
      });
    }
    bands.push(Band {
      spread: shadow.spread_radius,
      alpha: 1.0,
    });
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

fn fill(commands: &[PathCommand], color: Color, alpha: f32, at: (f32, f32), surface: &mut Surface) {
  let Some(path) = krilla_path(commands, at.0, at.1) else {
    return;
  };

  // The band's own opacity multiplies the color's alpha, so a translucent
  // shadow stays translucent and a fully transparent one paints nothing.
  surface.set_fill(Some(Fill {
    rule: FillRule::EvenOdd,
    ..fill_from_rgba(color.0, alpha)
  }));
  surface.draw_path(&path);
}

#[cfg(test)]
mod tests {
  use takumi_core::{shadow::SizedShadow, style::Color};

  use super::{Band, coverage};

  #[test]
  fn a_blurred_shadow_has_an_opaque_core() {
    let shadow = SizedShadow {
      offset_x: 0.0,
      offset_y: 0.0,
      blur_radius: 12.0,
      spread_radius: 2.0,
      color: Color([0, 0, 0, 255]),
    };
    let bands = Band::of(&shadow);
    let core = bands.last().expect("a band");

    assert_eq!(core.alpha, 1.0);
    assert_eq!(core.spread, shadow.spread_radius);
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

//! `box-shadow` as vector fills.
//!
//! Everything but the blur is exact: the shadow shape is the border box
//! offset and spread, and the box itself is cut out with an even-odd fill so
//! the shadow never paints under an opaque element. The blur is approximated
//! by stacking bands, since PDF has no blur operator.

use takumi_core::{
  geometry::{PathCommand, Point as CorePoint, Size},
  layout::border::BorderProperties,
  shadow::SizedShadow,
  style::Color,
};

use crate::krilla::{
  num::NormalizedF32,
  paint::{Fill, FillRule},
  surface::Surface,
};
use crate::paint::{fill_from_rgba, krilla_path};

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
    for band in bands(shadow) {
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

/// Paints the inset shadows of a box, on top of its background.
pub(crate) fn emit_inset_shadows(
  shadows: &[SizedShadow],
  border: &BorderProperties,
  size: Size<f32>,
  at: (f32, f32),
  surface: &mut Surface,
) {
  for shadow in shadows.iter().rev() {
    for band in bands(shadow) {
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

      // The filled region is the box minus the hole the shadow casts into, so
      // a positive spread shrinks the hole.
      border.append_mask_commands(&mut commands, size, CorePoint::ZERO);

      let (hole, hole_size) = border.outset_shadow_box(size, -band.spread);

      hole.append_mask_commands(
        &mut commands,
        hole_size,
        CorePoint {
          x: shadow.offset_x + band.spread,
          y: shadow.offset_y + band.spread,
        },
      );
      fill(&commands, shadow.color, band.alpha, at, surface);
    }
  }
}

/// One drawn ring of a shadow: how far it spreads, and how opaque it is.
struct Band {
  spread: f32,
  alpha: f32,
}

/// A sharp shadow is one band at full alpha. A blurred one is a stack from the
/// outermost, faintest band inward, with each band's alpha chosen so the fills
/// composite to the coverage the blur would have had at that distance.
fn bands(shadow: &SizedShadow) -> Vec<Band> {
  if shadow.blur_radius <= 0.0 {
    return vec![Band {
      spread: shadow.spread_radius,
      alpha: 1.0,
    }];
  }
  let mut bands = Vec::with_capacity(BLUR_BANDS);
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
  bands
}

/// Coverage a Gaussian blur leaves at `distance` blur radii outside the shape,
/// approximated by a smoothstep, which stays within a few percent of the error
/// function over the range a shadow spans.
fn coverage(distance: f32) -> f32 {
  let t = (1.0 - distance).clamp(0.0, 1.0);

  t * t * (3.0 - 2.0 * t)
}

fn fill(commands: &[PathCommand], color: Color, alpha: f32, at: (f32, f32), surface: &mut Surface) {
  let Some(path) = krilla_path(commands, at.0, at.1) else {
    return;
  };
  let Some(opacity) = NormalizedF32::new(alpha) else {
    return;
  };

  surface.set_fill(Some(Fill {
    rule: FillRule::EvenOdd,
    opacity,
    ..fill_from_rgba(color.0, 1.0)
  }));
  surface.draw_path(&path);
}

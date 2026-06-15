//! Box-model geometry shared by the node walk: border-radius resolution,
//! rounded-rect path data, the element's affine transform, and overflow clipping.

use taffy::Size;
use takumi_core::context::RenderContext;
use takumi_core::layout::style::{
  Affine, ComputedStyle, LengthDefaultsToZero, Overflow, SizingContext, SpacePair,
};

/// Per-corner `[rx, ry]` radii in `[top-left, top-right, bottom-right, bottom-left]`
/// order.
pub(crate) type Radii = [[f32; 2]; 4];

/// Resolves the four border-radius corners against the box, then clamps them so
/// adjacent radii never overlap (CSS Backgrounds §5.5).
pub(crate) fn resolved_radii(
  style: &ComputedStyle,
  sizing: &SizingContext,
  w: f32,
  h: f32,
) -> Radii {
  let corner =
    |pair: SpacePair<LengthDefaultsToZero>| [pair.x.to_px(sizing, w), pair.y.to_px(sizing, h)];
  let mut radii = [
    corner(style.border_top_left_radius),
    corner(style.border_top_right_radius),
    corner(style.border_bottom_right_radius),
    corner(style.border_bottom_left_radius),
  ];

  let [tl, tr, br, bl] = radii;
  let factor = [
    (w, tl[0] + tr[0]),
    (w, bl[0] + br[0]),
    (h, tl[1] + bl[1]),
    (h, tr[1] + br[1]),
  ]
  .into_iter()
  .map(|(extent, sum)| {
    if sum > 0.0 {
      extent / sum
    } else {
      f32::INFINITY
    }
  })
  .fold(1.0_f32, f32::min);

  if factor < 1.0 {
    for corner in &mut radii {
      corner[0] *= factor;
      corner[1] *= factor;
    }
  }
  radii
}

pub(crate) fn has_radius(radii: &Radii) -> bool {
  radii.iter().any(|c| c[0] > 0.0 || c[1] > 0.0)
}

/// Builds SVG path data for a rounded rectangle. Zero-radius corners degrade to
/// straight lines (an `A` with `rx`/`ry` of 0 is a line per the SVG spec).
pub(crate) fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: Radii) -> String {
  let [tl, tr, br, bl] = r;
  format!(
    "M{} {} H{} A{} {} 0 0 1 {} {} V{} A{} {} 0 0 1 {} {} H{} A{} {} 0 0 1 {} {} V{} A{} {} 0 0 1 {} {} Z",
    x + tl[0],
    y,
    x + w - tr[0],
    tr[0],
    tr[1],
    x + w,
    y + tr[1],
    y + h - br[1],
    br[0],
    br[1],
    x + w - br[0],
    y + h,
    x + bl[0],
    bl[0],
    bl[1],
    x,
    y + h - bl[1],
    y + tl[1],
    tl[0],
    tl[1],
    x + tl[0],
    y,
  )
}

/// Whether the element clips overflowing content on either axis.
pub(crate) fn clips_overflow(style: &ComputedStyle) -> bool {
  style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible
}

/// Computes the element's paint transform as an absolute-space matrix (the walk
/// emits children in absolute coordinates, so the transform is conjugated by the
/// box origin). Returns `None` when the element has no transform.
pub(crate) fn element_transform(
  context: &RenderContext,
  border_box: Size<f32>,
  x: f32,
  y: f32,
) -> Option<Affine> {
  let local = context.style.local_transform(border_box, &context.sizing);
  if local.is_identity() {
    return None;
  }
  // Children are emitted in absolute coordinates; move the local transform into
  // that space: M_abs = T(x, y) * local * T(-x, -y).
  Some(Affine::translation(x, y) * local * Affine::translation(-x, -y))
}

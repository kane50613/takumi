//! Box-model geometry helpers for the node walk: the element's affine transform,
//! overflow clipping predicate, and serialization of takumi-core path commands to
//! SVG path `d` data. The rounded-rect / border-radius geometry itself is reused
//! from takumi-core's [`BorderProperties`] (the same geometry the raster backend
//! rasterizes), not reimplemented here.

use taffy::Size;
use takumi_core::context::RenderContext;
use takumi_core::layout::style::{Affine, ComputedStyle, Overflow};
use tiny_skia::{PathSegment, Point};

/// Whether the element clips overflowing content on either axis.
pub(crate) fn clips_overflow(style: &ComputedStyle) -> bool {
  style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible
}

/// Serializes takumi-core path commands ([`tiny_skia::PathSegment`], the shared
/// `Command` type) to SVG path `d` data, applying `transform` (`[a, b, c, d, e, f]`,
/// SVG `matrix` order) to every point. Shared by glyph, border, background, and
/// clip emission.
pub(crate) fn path_data(commands: &[PathSegment], [a, b, c, d, e, f]: [f32; 6]) -> String {
  use std::fmt::Write as _;

  let mut out = String::new();
  let map = |p: Point| (a * p.x + c * p.y + e, b * p.x + d * p.y + f);
  for command in commands {
    match command {
      PathSegment::MoveTo(p) => {
        let (x, y) = map(*p);
        let _ = write!(out, "M{x} {y}");
      }
      PathSegment::LineTo(p) => {
        let (x, y) = map(*p);
        let _ = write!(out, "L{x} {y}");
      }
      PathSegment::QuadTo(c0, p) => {
        let (x0, y0) = map(*c0);
        let (x, y) = map(*p);
        let _ = write!(out, "Q{x0} {y0} {x} {y}");
      }
      PathSegment::CubicTo(c0, c1, p) => {
        let (x0, y0) = map(*c0);
        let (x1, y1) = map(*c1);
        let (x, y) = map(*p);
        let _ = write!(out, "C{x0} {y0} {x1} {y1} {x} {y}");
      }
      PathSegment::Close => out.push('Z'),
    }
  }
  out
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

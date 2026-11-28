//! A8 coverage masks for filled paths, rasterized by a port of the analytic anti-aliasing scan
//! converter Chromium's Skia uses for every anti-aliased path fill (`SkScan::AntiFillPath`, which
//! always calls `SkScan::AAAFillPath`), of rects by its `SkScan::AntiFillRect`, and of aliased
//! paths by its `SkScan::FillPath`.

mod aliased;
mod blitter;
mod edge;
mod edge_builder;
mod edge_clipper;
mod rect;
mod scan;

use crate::path::Fill;
use takumi_core::path::{IntSize, Path};

use blitter::{Coverage, IntBounds, round_out};

/// The coverage of `path` over a `size` mask at the origin, filled by `fill_rule`.
pub(crate) fn fill_path(path: &Path, fill_rule: Fill, size: IntSize) -> Option<Vec<u8>> {
  let mask_bounds = IntBounds {
    left: 0,
    top: 0,
    right: size.width() as i32,
    bottom: size.height() as i32,
  };
  let ir = round_out(path.bounds());
  let fits = |bounds: IntBounds| mask_bounds.contains(bounds);
  // A path inside the mask would lie inside a device-sized clip even where its control points
  // leave the mask, so only a path the mask cuts is clipped.
  let clip = if fits(ir)
    || path
      .compute_tight_bounds()
      .is_some_and(|tight| fits(round_out(tight)))
  {
    mask_bounds.union(ir)
  } else {
    mask_bounds
  };
  let clipped_ir = ir.intersect(clip);

  // `rect_overflows_short_shift` with `SK_SUPERSAMPLE_SHIFT` 2, past which Skia fills aliased.
  // Approximate: Skia tests the bounds cut to the device clip in device space, and this tests them
  // cut to `clip` in mask space, so a fill near 8192 px can switch to aliased at another place.
  let overflows = [
    clipped_ir.left,
    clipped_ir.top,
    clipped_ir.right,
    clipped_ir.bottom,
  ]
  .into_iter()
  .any(|value| ((value << 18) >> 18) != value);

  if overflows {
    return fill_path_aliased(path, fill_rule, size);
  }

  let mut coverage = Coverage::new(size.width(), size.height());

  if clipped_ir.width() > 0 && clipped_ir.height() > 0 {
    scan::fill_path(path, fill_rule, ir, clip, &mut coverage);
  }

  Some(coverage.data)
}

/// The coverage of `path` over a `size` mask at the origin without anti-aliasing, as
/// `SkScan::FillPath` fills it.
pub(crate) fn fill_path_aliased(path: &Path, fill_rule: Fill, size: IntSize) -> Option<Vec<u8>> {
  let mut mask = vec![0; size.width() as usize * size.height() as usize];

  aliased::fill_path(
    path,
    fill_rule,
    IntBounds {
      left: 0,
      top: 0,
      right: size.width() as i32,
      bottom: size.height() as i32,
    },
    &mut mask,
  );

  Some(mask)
}

/// The coverage of the axis-aligned rect `[left, top, right, bottom]` over a `size` mask at the
/// origin, as `SkDraw::drawRect` fills a rect the matrix keeps a rect.
pub(crate) fn fill_rect(rect: [f32; 4], size: IntSize) -> Option<Vec<u8>> {
  // The scan takes the mask's edges to 16.16 fixed point, which holds 32767.
  if size.width() > 32767 || size.height() > 32767 {
    return None;
  }

  let mut coverage = Coverage::new(size.width(), size.height());

  rect::fill_rect(rect, &mut coverage);

  Some(coverage.data)
}

#[cfg(test)]
mod tests;

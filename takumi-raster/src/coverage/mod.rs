//! A8 coverage masks for filled paths, rasterized by a port of the analytic anti-aliasing scan
//! converter Chromium's Skia uses for every anti-aliased path fill (`SkScan::AntiFillPath`, which
//! always calls `SkScan::AAAFillPath`), and of rects by its `SkScan::AntiFillRect`.

mod blitter;
mod edge;
mod edge_builder;
mod edge_clipper;
mod rect;
mod scan;

use std::sync::atomic::{AtomicU8, Ordering};

use tiny_skia::{FillRule, IntSize, Mask, Path, PathBuilder, Rect, Transform};

use blitter::{Coverage, IntBounds, round_out};

/// Which rasterizer [`fill_path`] uses, switchable to compare them.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MaskRasterizer {
  /// tiny-skia's port of Skia's 4×4 supersampler, `SkScan_AntiPath.cpp` before 2024.
  TinySkia,
  /// The port of `SkScan_AAAPath.cpp` in this module.
  Analytic,
}

static MASK_RASTERIZER: AtomicU8 = AtomicU8::new(MaskRasterizer::Analytic as u8);

/// Selects the rasterizer for every later mask.
#[doc(hidden)]
pub fn set_mask_rasterizer(rasterizer: MaskRasterizer) {
  MASK_RASTERIZER.store(rasterizer as u8, Ordering::Relaxed);
}

/// The coverage of `path` over a `size` mask at the origin, filled by `fill_rule`.
pub(crate) fn fill_path(path: &Path, fill_rule: FillRule, size: IntSize) -> Option<Vec<u8>> {
  let mask_bounds = IntBounds {
    left: 0,
    top: 0,
    right: size.width() as i32,
    bottom: size.height() as i32,
  };
  let ir = round_out(path.bounds());
  let fits = |bounds: IntBounds| mask_bounds.contains(bounds);
  let analytic = MASK_RASTERIZER.load(Ordering::Relaxed) == MaskRasterizer::Analytic as u8;
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
  let overflows = [
    clipped_ir.left,
    clipped_ir.top,
    clipped_ir.right,
    clipped_ir.bottom,
  ]
  .into_iter()
  .any(|value| ((value << 18) >> 18) != value);

  if !analytic || overflows {
    let mut mask = Mask::from_vec(
      vec![0; size.width() as usize * size.height() as usize],
      size,
    )?;

    mask.fill_path(path, fill_rule, !overflows, Transform::identity());

    return Some(mask.take());
  }

  let mut coverage = Coverage::new(size.width(), size.height());

  if clipped_ir.width() > 0 && clipped_ir.height() > 0 {
    scan::fill_path(path, fill_rule, ir, clip, &mut coverage);
  }

  Some(coverage.data)
}

/// The coverage of the axis-aligned rect `[left, top, right, bottom]` over a `size` mask at the
/// origin, as `SkDraw::drawRect` fills a rect the matrix keeps a rect.
pub(crate) fn fill_rect(rect: [f32; 4], size: IntSize) -> Option<Vec<u8>> {
  // The scan takes the mask's edges to 16.16 fixed point, which holds 32767.
  if size.width() > 32767 || size.height() > 32767 {
    return None;
  }

  if MASK_RASTERIZER.load(Ordering::Relaxed) != MaskRasterizer::Analytic as u8 {
    let [left, top, right, bottom] = rect;
    let path = PathBuilder::from_rect(Rect::from_ltrb(left, top, right, bottom)?);

    return fill_path(&path, FillRule::Winding, size);
  }

  let mut coverage = Coverage::new(size.width(), size.height());

  rect::fill_rect(rect, &mut coverage);

  Some(coverage.data)
}

#[cfg(test)]
mod tests;

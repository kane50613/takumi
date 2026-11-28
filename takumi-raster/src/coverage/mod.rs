//! A8 coverage masks for filled paths, rasterized by a port of the analytic anti-aliasing scan
//! converter Chromium's Skia uses for every anti-aliased path fill (`SkScan::AntiFillPath`, which
//! always calls `SkScan::AAAFillPath`), of rects by its `SkScan::AntiFillRect`, and of aliased
//! paths by its `SkScan::FillPath`.

mod aliased;
mod blitter;
mod cubics;
mod edge;
mod edge_builder;
mod edge_clipper;
mod rect;
mod scan;

use crate::style::FillRule;
use takumi_core::geometry::Size;
use takumi_core::path::Path;
use takumi_core::style::Affine;

use blitter::{Coverage, IntBounds, round_out};

/// `SkDrawTiler`'s `kMaxDim`: the widest and tallest a fill scans at once, whose supersampled
/// edges still fit 16.16 fixed point.
const MAX_TILE: u32 = 8192 - 1;

/// The coverage of `path` over a `size` mask at the origin, filled by `fill_rule`.
pub(crate) fn fill_path(path: &Path, fill_rule: FillRule, size: Size<u32>) -> Vec<u8> {
  tiled(path, size, |path, size| {
    let mask_bounds = IntBounds::of_size(size);
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
    let mut coverage = Coverage::new(size.width, size.height);

    // A tile holds the bounds within `rect_overflows_short_shift`, so the scan never falls back
    // to an aliased fill.
    if clipped_ir.width() > 0 && clipped_ir.height() > 0 {
      scan::fill_path(path, fill_rule, ir, clip, &mut coverage);
    }

    coverage.data
  })
}

/// The coverage of `path` over a `size` mask at the origin without anti-aliasing, as
/// `SkScan::FillPath` fills it.
pub(crate) fn fill_path_aliased(path: &Path, fill_rule: FillRule, size: Size<u32>) -> Vec<u8> {
  tiled(path, size, |path, size| {
    let mut mask = vec![0; size.width as usize * size.height as usize];

    aliased::fill_path(path, fill_rule, IntBounds::of_size(size), &mut mask);

    mask
  })
}

/// The coverage `fill` gives `path` over a `size` mask, filled tile by tile, each at most
/// [`MAX_TILE`] wide and tall with `path` translated to its origin, as `SkDrawTiler` draws into a
/// large device. Approximate: Skia tiles the device from the draw's bounds, and this tiles the mask.
fn tiled(path: &Path, size: Size<u32>, fill: impl Fn(&Path, Size<u32>) -> Vec<u8>) -> Vec<u8> {
  if size.width <= MAX_TILE && size.height <= MAX_TILE {
    return fill(path, size);
  }

  let width = size.width as usize;
  let mut mask = vec![0; width * size.height as usize];

  for top in (0..size.height).step_by(MAX_TILE as usize) {
    for left in (0..size.width).step_by(MAX_TILE as usize) {
      let tile = Size {
        width: (size.width - left).min(MAX_TILE),
        height: (size.height - top).min(MAX_TILE),
      };
      let Some(local) = path
        .clone()
        .transform(Affine::translation(-(left as f32), -(top as f32)))
      else {
        continue;
      };

      for (row, line) in fill(&local, tile)
        .chunks_exact(tile.width as usize)
        .enumerate()
      {
        let start = (top as usize + row) * width + left as usize;

        mask[start..start + line.len()].copy_from_slice(line);
      }
    }
  }

  mask
}

/// The coverage of the axis-aligned rect `[left, top, right, bottom]` over a `size` mask at the
/// origin, as `SkDraw::drawRect` fills a rect the matrix keeps a rect.
pub(crate) fn fill_rect(rect: [f32; 4], size: Size<u32>) -> Option<Vec<u8>> {
  // The scan takes the mask's edges to 16.16 fixed point, which holds 32767.
  if size.width > 32767 || size.height > 32767 {
    return None;
  }

  let mut coverage = Coverage::new(size.width, size.height);

  rect::fill_rect(rect, &mut coverage);

  Some(coverage.data)
}

#[cfg(test)]
mod tests;

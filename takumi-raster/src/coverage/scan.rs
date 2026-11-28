//! Skia's analytic anti-aliasing scan converter, `SkScan::AAAFillPath` and its walks in
//! `src/core/SkScan_AAAPath.cpp`, writing A8 coverage. Each horizontal strip between consecutive
//! scan lines is a set of trapezoids whose exact pixel coverage is computed from the edges'
//! slopes, so no supersampling is involved.

use crate::style::FillRule;
use smallvec::SmallVec;
use takumi_core::geometry::Rect;
use takumi_core::path::Path;

use super::{
  blitter::{
    Additive, AlphaSum, Coverage, IntBounds, MaskBlitter, RunBlitter, round_out, safely_add_alpha,
  },
  edge::{
    ACCURACY, Curve, Edge, FIXED_ONE, Fixed, MAX_S32, MIN_S32, ceil_to_fixed, ceil_to_int,
    fixed_mul, floor_to_fixed, floor_to_int, int_to_fixed, left_shift, round_to_int,
  },
  edge_builder::{build_edges, is_convex, trivial_rect},
  edge_clipper::EdgeClipper,
};

/// The `nullptr` of the index-linked edge list.
const NONE: u32 = u32::MAX;

/// `Sk32_sat_add`.
fn sat_add(a: i32, b: i32) -> i32 {
  (i64::from(a) + i64::from(b)).clamp(i64::from(MIN_S32), i64::from(MAX_S32)) as i32
}

/// `Sk32_sat_sub`.
fn sat_sub(a: i32, b: i32) -> i32 {
  (i64::from(a) - i64::from(b)).clamp(i64::from(MIN_S32), i64::from(MAX_S32)) as i32
}

/// `trapezoid_to_alpha`: the alpha of a trapezoid of height 1.
const fn trapezoid_to_alpha(l1: Fixed, l2: Fixed) -> u8 {
  (((l1 + l2) / 2) >> 8) as u8
}

/// `partial_triangle_to_alpha`: the alpha of the right triangle `(a, a * b)`.
const fn partial_triangle_to_alpha(a: Fixed, b: Fixed) -> u8 {
  let area = (a >> 11) * (a >> 11) * (b >> 11);

  ((area >> 8) & 0xFF) as u8
}

/// `get_partial_alpha(SkAlpha, SkFixed)`.
const fn partial_alpha_fixed(alpha: u8, partial_height: Fixed) -> u8 {
  round_to_int(alpha as i32 * partial_height) as u8
}

/// `get_partial_alpha(SkAlpha, SkAlpha)`.
const fn partial_alpha(alpha: u8, full_alpha: u8) -> u8 {
  ((alpha as i32 * full_alpha as i32) >> 8) as u8
}

/// `fixed_to_alpha`.
const fn fixed_to_alpha(value: Fixed) -> u8 {
  partial_alpha_fixed(0xFF, value)
}

/// `approximate_intersection`: a coarse x where `(l1, y)-(r1, y + 1)` crosses `(l2, y)-(r2, y + 1)`.
fn approximate_intersection(l1: Fixed, r1: Fixed, l2: Fixed, r2: Fixed) -> Fixed {
  let (l1, r1) = if l1 > r1 { (r1, l1) } else { (l1, r1) };
  let (l2, r2) = if l2 > r2 { (r2, l2) } else { (l2, r2) };

  (l1.max(l2) + r1.min(r2)) / 2
}

/// `compute_alpha_above_line`, for `l < SK_Fixed1`.
fn compute_alpha_above_line(alphas: &mut [u8], l: Fixed, r: Fixed, dy: Fixed, full_alpha: u8) {
  let count = ceil_to_int(r);

  if count == 0 {
    return;
  }
  if count == 1 {
    alphas[0] = partial_alpha((((count << 17) - l - r) >> 9) as u8, full_alpha);
    return;
  }

  let count = count as usize;
  let first = FIXED_ONE - l;
  let last = r - (((count - 1) as i32) << 16);
  let first_height = fixed_mul(first, dy);

  alphas[0] = (fixed_mul(first, first_height) >> 9) as u8;

  let mut alpha16 = sat_add(first_height, dy >> 1);

  for alpha in &mut alphas[1..count - 1] {
    *alpha = (alpha16 >> 8) as u8;
    alpha16 = sat_add(alpha16, dy);
  }
  alphas[count - 1] = full_alpha.wrapping_sub(partial_triangle_to_alpha(last, dy));
}

/// `compute_alpha_below_line`, for `l < SK_Fixed1`.
fn compute_alpha_below_line(alphas: &mut [u8], l: Fixed, r: Fixed, dy: Fixed, full_alpha: u8) {
  let count = ceil_to_int(r);

  if count == 0 {
    return;
  }
  if count == 1 {
    alphas[0] = partial_alpha(trapezoid_to_alpha(l, r), full_alpha);
    return;
  }

  let count = count as usize;
  let first = FIXED_ONE - l;
  let last = r - (((count - 1) as i32) << 16);
  let last_height = fixed_mul(last, dy);

  alphas[count - 1] = (fixed_mul(last, last_height) >> 9) as u8;

  let mut alpha16 = sat_add(last_height, dy >> 1);

  for alpha in alphas[1..count - 1].iter_mut().rev() {
    *alpha = ((alpha16 >> 8) & 0xFF) as u8;
    alpha16 = sat_add(alpha16, dy);
  }
  alphas[0] = full_alpha.wrapping_sub(partial_triangle_to_alpha(first, dy));
}

/// The blitters a scan writes through: the additive one and the mask behind it.
struct Target<'a> {
  additive: Additive,
  coverage: &'a mut Coverage,
}

impl Target<'_> {
  /// `MaskAdditiveBlitter::getRow` when the additive blitter is a mask.
  fn mask_row(&self, y: i32) -> Option<isize> {
    match &self.additive {
      Additive::Mask(mask) => Some(mask.row(y)),
      Additive::Runs(_) => None,
    }
  }

  /// Applies `update` to pixel `x` of a mask row, which only a mask blitter hands out.
  fn mask_update(&mut self, row: isize, x: i32, update: impl FnOnce(&mut u8)) {
    if let Additive::Mask(mask) = &mut self.additive {
      update(mask.at(row, x));
    }
  }

  fn blit_anti_h(&mut self, x: i32, y: i32, alpha: u8) {
    self.additive.blit_anti_h(self.coverage, x, y, alpha);
  }

  fn blit_anti_h_len(&mut self, x: i32, y: i32, width: i32, alpha: u8) {
    self
      .additive
      .blit_anti_h_len(self.coverage, x, y, width, alpha);
  }

  fn flush_if_y_changed(&mut self, y: Fixed, next_y: Fixed) {
    self.additive.flush_if_y_changed(self.coverage, y, next_y);
  }

  /// `blit_single_alpha`.
  fn single_alpha(
    &mut self,
    y: i32,
    x: i32,
    alpha: u8,
    full_alpha: u8,
    mask_row: Option<isize>,
    no_real: bool,
  ) {
    let direct = full_alpha == 0xFF && !no_real;

    match mask_row {
      Some(row) if direct => self.mask_update(row, x, |pixel| *pixel = alpha),
      Some(row) => self.mask_update(row, x, |pixel| {
        safely_add_alpha(pixel, partial_alpha(alpha, full_alpha))
      }),
      None if direct => self.coverage.blit_v(x, y, 1, alpha),
      None => self.blit_anti_h(x, y, partial_alpha(alpha, full_alpha)),
    }
  }

  /// `blit_two_alphas`.
  #[allow(clippy::too_many_arguments)]
  fn two_alphas(
    &mut self,
    y: i32,
    x: i32,
    a1: u8,
    a2: u8,
    full_alpha: u8,
    mask_row: Option<isize>,
    no_real: bool,
  ) {
    match mask_row {
      Some(row) => {
        self.mask_update(row, x, |pixel| safely_add_alpha(pixel, a1));
        self.mask_update(row, x + 1, |pixel| safely_add_alpha(pixel, a2));
      }
      None if full_alpha == 0xFF && !no_real => self.coverage.blit_anti_h(x, y, &[a1, a2]),
      None => {
        self.blit_anti_h(x, y, a1);
        self.blit_anti_h(x + 1, y, a2);
      }
    }
  }

  /// `blit_full_alpha`.
  fn full_alpha(
    &mut self,
    y: i32,
    x: i32,
    len: i32,
    full_alpha: u8,
    mask_row: Option<isize>,
    no_real: bool,
  ) {
    match mask_row {
      Some(row) => {
        for offset in 0..len {
          self.mask_update(row, x + offset, |pixel| safely_add_alpha(pixel, full_alpha));
        }
      }
      None if full_alpha == 0xFF && !no_real => self.coverage.blit_h(x, y, len),
      None => self.blit_anti_h_len(x, y, len, full_alpha),
    }
  }

  /// `blit_aaa_trapezoid_row`.
  #[allow(clippy::too_many_arguments)]
  fn aaa_trapezoid_row(
    &mut self,
    y: i32,
    ul: Fixed,
    ur: Fixed,
    ll: Fixed,
    lr: Fixed,
    l_dy: Fixed,
    r_dy: Fixed,
    full_alpha: u8,
    mask_row: Option<isize>,
    no_real: bool,
  ) {
    let left = floor_to_int(ul);
    let len = ceil_to_int(lr) - left;

    if len == 1 {
      let alpha = trapezoid_to_alpha(ur - ul, lr - ll);

      self.single_alpha(y, left, alpha, full_alpha, mask_row, no_real);
      return;
    }

    let len = len.max(0) as usize;
    let mut alphas: SmallVec<[u8; 32]> = SmallVec::from_elem(full_alpha, len);
    let mut temp: SmallVec<[u8; 32]> = SmallVec::from_elem(0, len + 1);
    let subtract = |alphas: &mut [u8], index: usize, value: u8| {
      alphas[index] = alphas[index].saturating_sub(value);
    };

    let upper_left = floor_to_int(ul);
    let lower_left = ceil_to_int(ll);

    if upper_left + 2 == lower_left {
      let first = int_to_fixed(upper_left) + FIXED_ONE - ul;
      let second = ll - ul - first;

      subtract(
        &mut alphas,
        0,
        full_alpha.wrapping_sub(partial_triangle_to_alpha(first, l_dy)),
      );
      subtract(&mut alphas, 1, partial_triangle_to_alpha(second, l_dy));
    } else {
      let offset = (upper_left - left) as usize;

      compute_alpha_below_line(
        &mut temp[offset..],
        ul - int_to_fixed(upper_left),
        ll - int_to_fixed(upper_left),
        l_dy,
        full_alpha,
      );
      for index in upper_left..lower_left {
        let index = (index - left) as usize;

        subtract(&mut alphas, index, temp[index]);
      }
    }

    let upper_right = floor_to_int(ur);
    let lower_right = ceil_to_int(lr);

    if upper_right + 2 == lower_right {
      let first = int_to_fixed(upper_right) + FIXED_ONE - ur;
      let second = lr - ur - first;

      subtract(&mut alphas, len - 2, partial_triangle_to_alpha(first, r_dy));
      subtract(
        &mut alphas,
        len - 1,
        full_alpha.wrapping_sub(partial_triangle_to_alpha(second, r_dy)),
      );
    } else {
      let offset = (upper_right - left) as usize;

      compute_alpha_above_line(
        &mut temp[offset..],
        ur - int_to_fixed(upper_right),
        lr - int_to_fixed(upper_right),
        r_dy,
        full_alpha,
      );
      for index in upper_right..lower_right {
        let index = (index - left) as usize;

        subtract(&mut alphas, index, temp[index]);
      }
    }

    match mask_row {
      Some(row) => {
        for (offset, alpha) in alphas.iter().enumerate() {
          self.mask_update(row, left + offset as i32, |pixel| {
            safely_add_alpha(pixel, *alpha)
          });
        }
      }
      None if full_alpha == 0xFF && !no_real => self.coverage.blit_anti_h(left, y, &alphas),
      None => self
        .additive
        .blit_anti_h_alphas(self.coverage, left, y, &alphas),
    }
  }

  /// `blit_trapezoid_row`.
  #[allow(clippy::too_many_arguments)]
  fn trapezoid_row(
    &mut self,
    y: i32,
    mut ul: Fixed,
    mut ur: Fixed,
    mut ll: Fixed,
    mut lr: Fixed,
    l_dy: Fixed,
    r_dy: Fixed,
    full_alpha: u8,
    mask_row: Option<isize>,
    no_real: bool,
  ) {
    if ul > ur {
      return;
    }

    // Edges only cross here through precision loss, so a coarse intersection is enough.
    if ll > lr {
      ll = approximate_intersection(ul, ll, ur, lr);
      lr = ll;
    }

    if ul == ur && ll == lr {
      return;
    }

    if ul > ll {
      (ul, ll) = (ll, ul);
    }
    if ur > lr {
      (ur, lr) = (lr, ur);
    }

    let join_left = ceil_to_fixed(ll);
    let join_right = floor_to_fixed(ur);

    if join_left > join_right {
      self.aaa_trapezoid_row(y, ul, ur, ll, lr, l_dy, r_dy, full_alpha, mask_row, no_real);
      return;
    }

    if ul < join_left {
      match ceil_to_int(join_left - ul) {
        1 => {
          let alpha = trapezoid_to_alpha(join_left - ul, join_left - ll);

          self.single_alpha(y, ul >> 16, alpha, full_alpha, mask_row, no_real);
        }
        2 => {
          let first = join_left - FIXED_ONE - ul;
          let second = ll - ul - first;
          let a1 = partial_triangle_to_alpha(first, l_dy);
          let a2 = full_alpha.wrapping_sub(partial_triangle_to_alpha(second, l_dy));

          self.two_alphas(y, ul >> 16, a1, a2, full_alpha, mask_row, no_real);
        }
        _ => self.aaa_trapezoid_row(
          y, ul, join_left, ll, join_left, l_dy, MAX_S32, full_alpha, mask_row, no_real,
        ),
      }
    }

    // `SkAAClip` needs left-to-right blits, so `[ul, joinLeft]` goes before `[joinLeft, joinRite]`.
    if join_left < join_right {
      self.full_alpha(
        y,
        floor_to_int(join_left),
        floor_to_int(join_right - join_left),
        full_alpha,
        mask_row,
        no_real,
      );
    }

    if lr > join_right {
      match ceil_to_int(lr - join_right) {
        1 => {
          let alpha = trapezoid_to_alpha(ur - join_right, lr - join_right);

          self.single_alpha(y, join_right >> 16, alpha, full_alpha, mask_row, no_real);
        }
        2 => {
          let first = join_right + FIXED_ONE - ur;
          let second = lr - ur - first;
          let a1 = full_alpha.wrapping_sub(partial_triangle_to_alpha(first, r_dy));
          let a2 = partial_triangle_to_alpha(second, r_dy);

          self.two_alphas(y, join_right >> 16, a1, a2, full_alpha, mask_row, no_real);
        }
        _ => self.aaa_trapezoid_row(
          y, join_right, ur, join_right, lr, MAX_S32, r_dy, full_alpha, mask_row, no_real,
        ),
      }
    }
  }
}

/// `remove_edge` in `SkScanPriv.h`.
fn remove_edge(edges: &mut [Edge], edge: usize) {
  let (prev, next) = (edges[edge].prev, edges[edge].next);

  edges[prev as usize].next = next;
  edges[next as usize].prev = prev;
}

/// `insert_edge_after` in `SkScanPriv.h`.
fn insert_edge_after(edges: &mut [Edge], edge: usize, after: usize) {
  let next = edges[after].next;

  edges[edge].prev = after as u32;
  edges[edge].next = next;
  edges[next as usize].prev = edge as u32;
  edges[after].next = edge as u32;
}

/// `backward_insert_edge_based_on_x` in `SkScanPriv.h`.
fn backward_insert_edge_based_on_x(edges: &mut [Edge], edge: usize) {
  let x = edges[edge].x;
  let mut prev = edges[edge].prev as usize;

  while edges[prev].prev != NONE && edges[prev].x > x {
    prev = edges[prev].prev as usize;
  }
  if edges[prev].next as usize != edge {
    remove_edge(edges, edge);
    insert_edge_after(edges, edge, prev);
  }
}

/// `backward_insert_start` in `SkScanPriv.h`.
fn backward_insert_start(edges: &[Edge], mut prev: usize, x: Fixed) -> usize {
  while edges[prev].prev != NONE && edges[prev].x > x {
    prev = edges[prev].prev as usize;
  }

  prev
}

/// `is_smooth_enough(thisEdge, nextEdge, stop_y)`.
fn is_edge_smooth(this: &Edge, next: &Edge) -> bool {
  match (this.curve_count.signum(), this.curve) {
    (-1, Curve::Cubic(cubic)) => {
      let dd_shift = this.curve_shift as u32;

      cubic.cdx.abs() >> 1 >= cubic.cddx.abs() >> dd_shift
        && cubic.cdy.abs() >> 1 >= cubic.cddy.abs() >> dd_shift
        && (cubic.cdy - (cubic.cddy >> dd_shift)) >> cubic.to_fixed_shift >= FIXED_ONE
    }
    (1, Curve::Quad(quad)) => {
      quad.qdx.abs() >> 1 >= quad.qddx.abs()
        && quad.qdy.abs() >> 1 >= quad.qddy.abs()
        && (quad.qdy - quad.qddy) >> this.curve_shift >= FIXED_ONE
    }
    _ => sat_sub(next.dx, this.dx).abs() <= FIXED_ONE && next.lower_y - next.upper_y >= FIXED_ONE,
  }
}

/// `is_smooth_enough(leftE, riteE, currE, stop_y)`: whether both edges change slowly enough that
/// fractional scan lines can be skipped.
fn are_edges_smooth(
  edges: &[Edge],
  left: usize,
  right: usize,
  current: usize,
  stop_y: i32,
) -> bool {
  let (left, right) = (&edges[left], &edges[right]);
  let mut current = current;

  if edges[current].upper_y >= left_shift(stop_y, 16) {
    return false;
  }
  if left.lower_y.wrapping_add(FIXED_ONE) < right.lower_y {
    return is_edge_smooth(left, &edges[current]);
  }
  if left.lower_y > right.lower_y.wrapping_add(FIXED_ONE) {
    return is_edge_smooth(right, &edges[current]);
  }

  let mut next = edges[current].next as usize;

  if edges[next].upper_y >= left_shift(stop_y, 16) {
    return false;
  }
  if edges[next].upper_x < edges[current].upper_x {
    (current, next) = (next, current);
  }

  is_edge_smooth(left, &edges[current]) && is_edge_smooth(right, &edges[next])
}

/// `aaa_walk_convex_edges`.
#[allow(clippy::too_many_arguments)]
fn walk_convex_edges(
  edges: &mut [Edge],
  head: usize,
  target: &mut Target,
  stop_y: i32,
  left_bound: Fixed,
  right_bound: Fixed,
  is_using_mask: bool,
) {
  let mut left_edge = edges[head].next as usize;
  let mut right_edge = edges[left_edge].next as usize;
  let mut current = edges[right_edge].next as usize;
  let mut y = edges[left_edge].upper_y.max(edges[right_edge].upper_y);

  loop {
    // Edges may be left alone in a scan line through precision loss, so `fLowerY` is checked
    // first, and a smooth jump may pass several short edges.
    for side in [&mut left_edge, &mut right_edge] {
      while edges[*side].lower_y <= y {
        if !edges[*side].update() {
          if floor_to_int(edges[current].upper_y) >= stop_y {
            return;
          }
          *side = current;
          current = edges[current].next as usize;
        }
      }
    }

    if floor_to_int(y) >= stop_y {
      return;
    }

    edges[left_edge].go_y(y);
    edges[right_edge].go_y(y);

    let (left_x, right_x) = (edges[left_edge].x, edges[right_edge].x);

    if left_x > right_x || (left_x == right_x && edges[left_edge].dx > edges[right_edge].dx) {
      (left_edge, right_edge) = (right_edge, left_edge);
    }

    let mut local_bottom = edges[left_edge].lower_y.min(edges[right_edge].lower_y);

    if are_edges_smooth(edges, left_edge, right_edge, current, stop_y) {
      local_bottom = ceil_to_fixed(local_bottom);
    }
    local_bottom = local_bottom.min(int_to_fixed(stop_y));

    let mut left = left_bound.max(edges[left_edge].x);
    let d_left = edges[left_edge].dx;
    let mut right = right_bound.min(edges[right_edge].x);
    let d_right = edges[right_edge].dx;

    if d_left | d_right == 0 {
      let full_left = ceil_to_int(left);
      let full_right = floor_to_int(right);
      let partial_left = int_to_fixed(full_left) - left;
      let partial_right = right - int_to_fixed(full_right);
      let full_top = ceil_to_int(y);
      let full_bottom = floor_to_int(local_bottom);
      let mut partial_top = int_to_fixed(full_top) - y;
      let mut partial_bottom = local_bottom - int_to_fixed(full_bottom);

      if full_top > full_bottom {
        partial_top -= FIXED_ONE - partial_bottom;
        partial_bottom = 0;
      }

      if full_right >= full_left {
        if partial_top > 0 {
          if partial_left > 0 {
            target.blit_anti_h(
              full_left - 1,
              full_top - 1,
              fixed_to_alpha(fixed_mul(partial_top, partial_left)),
            );
          }
          target.blit_anti_h_len(
            full_left,
            full_top - 1,
            full_right - full_left,
            fixed_to_alpha(partial_top),
          );
          if partial_right > 0 {
            target.blit_anti_h(
              full_right,
              full_top - 1,
              fixed_to_alpha(fixed_mul(partial_top, partial_right)),
            );
          }
          target.flush_if_y_changed(y, y + partial_top);
        }

        // An empty rect is skipped for `SkAAClip` (chromium:662800).
        if full_bottom > full_top
          && (full_right > full_left
            || fixed_to_alpha(partial_left) > 0
            || fixed_to_alpha(partial_right) > 0)
        {
          target.additive.real_blit_anti_rect(
            target.coverage,
            full_left - 1,
            full_top,
            full_right - full_left,
            full_bottom - full_top,
            fixed_to_alpha(partial_left),
            fixed_to_alpha(partial_right),
          );
        }

        if partial_bottom > 0 {
          if partial_left > 0 {
            target.blit_anti_h(
              full_left - 1,
              full_bottom,
              fixed_to_alpha(fixed_mul(partial_bottom, partial_left)),
            );
          }
          target.blit_anti_h_len(
            full_left,
            full_bottom,
            full_right - full_left,
            fixed_to_alpha(partial_bottom),
          );
          if partial_right > 0 {
            target.blit_anti_h(
              full_right,
              full_bottom,
              fixed_to_alpha(fixed_mul(partial_bottom, partial_right)),
            );
          }
        }
      } else {
        // Both edges lie in one pixel column, unless both were clipped to the same bound.
        let width = right - left;

        if width > 0 {
          if partial_top > 0 {
            target.blit_anti_h_len(
              full_left - 1,
              full_top - 1,
              1,
              fixed_to_alpha(fixed_mul(partial_top, width)),
            );
            target.flush_if_y_changed(y, y + partial_top);
          }
          if full_bottom > full_top {
            target.additive.real_blit_v(
              target.coverage,
              full_left - 1,
              full_top,
              full_bottom - full_top,
              fixed_to_alpha(width),
            );
          }
          if partial_bottom > 0 {
            target.blit_anti_h_len(
              full_left - 1,
              full_bottom,
              1,
              fixed_to_alpha(fixed_mul(partial_bottom, width)),
            );
          }
        }
      }

      y = local_bottom;
    } else {
      // x snaps to 1/16 pixel, which avoids tiny triangles and precision edge cases.
      const SNAP_DIGIT: Fixed = FIXED_ONE >> 4;
      const SNAP_HALF: Fixed = SNAP_DIGIT >> 1;
      const SNAP_MASK: Fixed = -1 ^ (SNAP_DIGIT - 1);

      left += SNAP_HALF;
      right += SNAP_HALF;

      let mut count = ceil_to_int(local_bottom) - floor_to_int(y);
      let mut mask_row = is_using_mask.then(|| target.mask_row(y >> 16)).flatten();
      let (left_dy, right_dy) = (edges[left_edge].dy, edges[right_edge].dy);

      if count > 1 {
        if y & !(FIXED_ONE - 1) != y {
          count -= 1;

          let next_y = ceil_to_fixed(y + 1);
          let dy = next_y - y;
          let next_left = left + fixed_mul(d_left, dy);
          let next_right = right + fixed_mul(d_right, dy);

          target.trapezoid_row(
            y >> 16,
            left & SNAP_MASK,
            right & SNAP_MASK,
            next_left & SNAP_MASK,
            next_right & SNAP_MASK,
            left_dy,
            right_dy,
            partial_alpha_fixed(0xFF, dy),
            mask_row,
            false,
          );
          target.flush_if_y_changed(y, next_y);
          left = next_left;
          right = next_right;
          y = next_y;
        }

        while count > 1 {
          count -= 1;
          if is_using_mask {
            mask_row = target.mask_row(y >> 16);
          }

          let next_y = y + FIXED_ONE;
          let next_left = left + d_left;
          let next_right = right + d_right;

          target.trapezoid_row(
            y >> 16,
            left & SNAP_MASK,
            right & SNAP_MASK,
            next_left & SNAP_MASK,
            next_right & SNAP_MASK,
            left_dy,
            right_dy,
            0xFF,
            mask_row,
            false,
          );
          target.flush_if_y_changed(y, next_y);
          left = next_left;
          right = next_right;
          y = next_y;
        }
      }

      if is_using_mask {
        mask_row = target.mask_row(y >> 16);
      }

      let dy = local_bottom - y;
      // Jumping smoothly to an integer y can take the last x out of bounds, so it is pulled
      // back, allowing for the half snap subtracted below.
      let next_left = (left + fixed_mul(d_left, dy)).max(left_bound + SNAP_HALF);
      let next_right = (right + fixed_mul(d_right, dy)).min(right_bound + SNAP_HALF);

      target.trapezoid_row(
        y >> 16,
        left & SNAP_MASK,
        right & SNAP_MASK,
        next_left & SNAP_MASK,
        next_right & SNAP_MASK,
        left_dy,
        right_dy,
        partial_alpha_fixed(0xFF, dy),
        mask_row,
        false,
      );
      target.flush_if_y_changed(y, local_bottom);
      left = next_left - SNAP_HALF;
      right = next_right - SNAP_HALF;
      y = local_bottom;
    }

    edges[left_edge].x = left;
    edges[right_edge].x = right;
    edges[left_edge].y = y;
    edges[right_edge].y = y;
  }
}

/// `update_next_next_y`.
fn update_next_next_y(y: Fixed, next_y: Fixed, next_next_y: &mut Fixed) {
  if y > next_y && y < *next_next_y {
    *next_next_y = y;
  }
}

/// The next scan line after `next_y` when two edges cross within a quarter pixel.
const QUARTER: Fixed = FIXED_ONE >> ACCURACY;

/// `check_intersection`.
fn check_intersection(edges: &[Edge], edge: usize, next_y: Fixed, next_next_y: &mut Fixed) {
  let prev = &edges[edges[edge].prev as usize];

  if prev.prev != NONE && prev.x.wrapping_add(prev.dx) > edges[edge].x.wrapping_add(edges[edge].dx)
  {
    *next_next_y = next_y + QUARTER;
  }
}

/// `check_intersection_fwd`.
fn check_intersection_forward(edges: &[Edge], edge: usize, next_y: Fixed, next_next_y: &mut Fixed) {
  let next = &edges[edges[edge].next as usize];

  if next.next != NONE && edges[edge].x.wrapping_add(edges[edge].dx) > next.x.wrapping_add(next.dx)
  {
    *next_next_y = next_y + QUARTER;
  }
}

/// `insert_new_edges`: moves edges starting at `y` into x order.
fn insert_new_edges(edges: &mut [Edge], mut new_edge: usize, y: Fixed, next_next_y: &mut Fixed) {
  if edges[new_edge].upper_y > y {
    update_next_next_y(edges[new_edge].upper_y, y, next_next_y);
    return;
  }

  let prev = edges[new_edge].prev as usize;

  if edges[prev].x <= edges[new_edge].x {
    while edges[new_edge].upper_y <= y {
      check_intersection(edges, new_edge, y, next_next_y);
      update_next_next_y(edges[new_edge].lower_y, y, next_next_y);
      new_edge = edges[new_edge].next as usize;
    }
    update_next_next_y(edges[new_edge].upper_y, y, next_next_y);
    return;
  }

  let mut start = backward_insert_start(edges, prev, edges[new_edge].x);

  loop {
    let next = edges[new_edge].next as usize;
    let mut placed = false;

    loop {
      if edges[start].next as usize == new_edge {
        placed = true;
        break;
      }

      let after = edges[start].next as usize;

      if edges[after].x >= edges[new_edge].x {
        break;
      }
      start = after;
    }
    if !placed {
      remove_edge(edges, new_edge);
      insert_edge_after(edges, new_edge, start);
    }
    check_intersection(edges, new_edge, y, next_next_y);
    check_intersection_forward(edges, new_edge, y, next_next_y);
    update_next_next_y(edges[new_edge].lower_y, y, next_next_y);
    start = new_edge;
    new_edge = next;

    if edges[new_edge].upper_y > y {
      break;
    }
  }
  update_next_next_y(edges[new_edge].upper_y, y, next_next_y);
}

/// `edges_too_close(prev, next, lowerY)`: whether two edges are within a pixel in this row.
fn edges_too_close(edges: &[Edge], prev: u32, next: u32, lower_y: Fixed) -> bool {
  if prev == NONE || next == NONE {
    return false;
  }

  let (prev, next) = (&edges[prev as usize], &edges[next as usize]);

  next.upper_y < lower_y && prev.x.wrapping_add(FIXED_ONE) >= next.x.wrapping_sub(next.dx.abs())
}

/// `edges_too_close(prevRite, ul, ll)`, for when the previous right edge has ended.
const fn right_edge_too_close(prev_right: i32, ul: Fixed, ll: Fixed) -> bool {
  prev_right > floor_to_int(ul) || prev_right > floor_to_int(ll)
}

/// `aaa_walk_edges` for non-inverse fills, without `SkAAClip`'s forced run blitting.
#[allow(clippy::too_many_arguments)]
fn walk_edges(
  edges: &mut [Edge],
  head: usize,
  tail: usize,
  fill_rule: FillRule,
  target: &mut Target,
  start_y: i32,
  stop_y: i32,
  left_clip: Fixed,
  right_clip: Fixed,
  is_using_mask: bool,
  skip_intersect: bool,
) {
  edges[head].x = left_clip;
  edges[head].upper_x = left_clip;
  edges[tail].x = right_clip;
  edges[tail].upper_x = right_clip;

  let mut y = edges[edges[head].next as usize]
    .upper_y
    .max(int_to_fixed(start_y));
  let mut next_next_y = MAX_S32;

  let mut edge = edges[head].next as usize;

  while edges[edge].upper_y <= y {
    edges[edge].go_y(y);
    update_next_next_y(edges[edge].lower_y, y, &mut next_next_y);
    edge = edges[edge].next as usize;
  }
  update_next_next_y(edges[edge].upper_y, y, &mut next_next_y);

  let winding_mask = if fill_rule == FillRule::EvenOdd {
    1
  } else {
    -1
  };

  loop {
    let mut winding = 0;
    let mut in_interval = false;
    let mut prev_x = edges[head].x;
    let mut next_y = next_next_y.min(ceil_to_fixed(y + 1));
    let mut current = edges[head].next as usize;
    let mut left_edge = head;
    let mut left = left_clip;
    let mut left_dy = 0;
    let mut prev_right = floor_to_int(left_clip);

    next_next_y = MAX_S32;

    let mut y_shift = 0;

    if (next_y - y) & (FIXED_ONE >> 2) != 0 {
      y_shift = 2;
      next_y = y + (FIXED_ONE >> 2);
    } else if (next_y - y) & (FIXED_ONE >> 1) != 0 {
      y_shift = 1;
    }

    let full_alpha = fixed_to_alpha(next_y - y);
    let mask_row = is_using_mask
      .then(|| target.mask_row(floor_to_int(y)))
      .flatten();

    while edges[current].upper_y <= y {
      winding += i32::from(edges[current].winding);

      let prev_in_interval = in_interval;

      in_interval = winding & winding_mask != 0;

      let is_left = in_interval && !prev_in_interval;
      let is_right = !in_interval && prev_in_interval;

      if is_right {
        let right = edges[current].x;

        edges[current].go_y_shift(next_y, y_shift);

        let next_left = left_clip.max(edges[left_edge].x);
        let right = right_clip.min(right);
        let next_right = right_clip.min(edges[current].x);
        let too_close = full_alpha == 0xFF
          && (right_edge_too_close(prev_right, left, edges[left_edge].x)
            || edges_too_close(edges, current as u32, edges[current].next, next_y));

        target.trapezoid_row(
          y >> 16,
          left,
          right,
          next_left,
          next_right,
          left_dy,
          edges[current].dy,
          full_alpha,
          mask_row,
          too_close,
        );
        prev_right = ceil_to_int(right.max(edges[current].x));
      } else {
        if is_left {
          left = edges[current].x.max(left_clip);
          left_dy = edges[current].dy;
          left_edge = current;
        }
        edges[current].go_y_shift(next_y, y_shift);
      }

      let next = edges[current].next as usize;

      while edges[current].lower_y <= next_y && edges[current].curve_count != 0 {
        edges[current].keep_continuous();
        if !edges[current].update() {
          break;
        }
      }

      if edges[current].lower_y <= next_y {
        remove_edge(edges, current);
      } else {
        update_next_next_y(edges[current].lower_y, next_y, &mut next_next_y);

        let new_x = edges[current].x;

        if new_x < prev_x {
          backward_insert_edge_based_on_x(edges, current);
        } else {
          prev_x = new_x;
        }
        if !skip_intersect {
          check_intersection(edges, current, next_y, &mut next_next_y);
        }
      }

      current = next;
    }

    // The right edge was culled away.
    if in_interval {
      let too_close = full_alpha == 0xFF
        && edges_too_close(edges, edges[left_edge].prev, left_edge as u32, next_y);

      target.trapezoid_row(
        y >> 16,
        left,
        right_clip,
        left_clip.max(edges[left_edge].x),
        right_clip,
        left_dy,
        0,
        full_alpha,
        mask_row,
        too_close,
      );
    }

    y = next_y;
    if y >= int_to_fixed(stop_y) {
      break;
    }

    insert_new_edges(edges, current, y, &mut next_next_y);
  }
}

/// `SkScan::AAAFillPath` for a non-inverse `path` with rounded-out bounds `ir`, clipped to `clip`.
pub(super) fn fill_path(
  path: &Path,
  fill_rule: FillRule,
  ir: IntBounds,
  clip: IntBounds,
  coverage: &mut Coverage,
) {
  let convex = is_convex(path);

  let additive = if MaskBlitter::can_handle(ir) {
    // `try_blit_fat_anti_rect`.
    if let Some(rect) = trivial_rect(path) {
      let Some(rect) = Rect::from_ltrb(
        rect.left.max(clip.left as f32),
        rect.top.max(clip.top as f32),
        rect.right.min(clip.right as f32),
        rect.bottom.min(clip.bottom as f32),
      ) else {
        return;
      };

      if round_out(rect).width() >= 3 {
        coverage.blit_fat_anti_rect(rect);
        return;
      }
    }

    Additive::Mask(MaskBlitter::new(ir))
  } else {
    // A convex fill never sums alphas above 255, so it skips `SafeRLEAdditiveBlitter`'s clamp.
    let sum = if convex {
      AlphaSum::Wrap
    } else {
      AlphaSum::Saturate
    };

    Additive::Runs(RunBlitter::new(ir.intersect(clip), sum))
  };

  let is_using_mask = matches!(additive, Additive::Mask(_));
  let mut target = Target { additive, coverage };

  fill_edges(
    path,
    fill_rule,
    ir,
    clip,
    convex,
    is_using_mask,
    &mut target,
  );
  target.additive.finish(target.coverage);
}

/// `aaa_fill_path`, clipping the edges to `clip` when the path bounds `ir` leave it.
fn fill_edges(
  path: &Path,
  fill_rule: FillRule,
  ir: IntBounds,
  clip: IntBounds,
  convex: bool,
  is_using_mask: bool,
  target: &mut Target,
) {
  let contained = clip.contains(ir);
  let clipper = if contained {
    None
  } else {
    // A convex path needs both edges even past the clip's right side.
    clip.to_rect().map(|rect| EdgeClipper::new(rect, !convex))
  };
  let mut edges: Vec<Edge> = build_edges(path, clipper.as_ref());
  let count = edges.len();
  let start_y = ir.top.max(clip.top);
  let stop_y = ir.bottom.min(clip.bottom);

  if count == 0 {
    return;
  }

  edges.sort_unstable_by(|a, b| {
    a.upper_y
      .cmp(&b.upper_y)
      .then(a.x.cmp(&b.x))
      .then(a.dx.cmp(&b.dx))
  });

  let (head, tail) = (count, count + 1);

  edges.push(Edge::sentinel(MIN_S32, MIN_S32));
  edges.push(Edge::sentinel(MAX_S32, MAX_S32));

  for (index, edge) in (0u32..).zip(&mut edges[..count]) {
    edge.prev = index.checked_sub(1).unwrap_or(head as u32);
    edge.next = if index as usize + 1 == count {
      tail as u32
    } else {
      index + 1
    };
  }
  edges[head].next = 0;
  edges[tail].prev = count as u32 - 1;

  let mut left_bound = int_to_fixed(clip.left);
  let mut right_bound = int_to_fixed(clip.right);

  if is_using_mask {
    // The mask holds only the path bounds, so edge drift must not leave them.
    left_bound = left_bound.max(int_to_fixed(ir.left));
    right_bound = right_bound.min(int_to_fixed(ir.right));
  }

  if convex && count >= 2 {
    walk_convex_edges(
      &mut edges,
      head,
      target,
      stop_y,
      left_bound,
      right_bound,
      is_using_mask,
    );
  } else {
    // Many points already give enough fractional scan lines, so intersections are skipped.
    let skip_intersect = path.points().len() > ((stop_y - start_y) * 2) as usize;

    walk_edges(
      &mut edges,
      head,
      tail,
      fill_rule,
      target,
      start_y,
      stop_y,
      left_bound,
      right_bound,
      is_using_mask,
      skip_intersect,
    );
  }
}

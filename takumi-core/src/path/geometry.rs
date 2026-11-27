//! The curve helpers of Skia's `src/core/SkGeometry.cpp` that bound, evaluate, and split paths.

use super::Point;

/// `valid_unit_divide`: `numer / denom` where it lies strictly inside 0 to 1.
pub fn valid_unit_divide(mut numer: f32, mut denom: f32) -> Option<f32> {
  if numer < 0.0 {
    numer = -numer;
    denom = -denom;
  }

  if denom == 0.0 || numer == 0.0 || numer >= denom {
    return None;
  }

  let ratio = numer / denom;

  (!ratio.is_nan() && ratio != 0.0).then_some(ratio)
}

/// `SkFindUnitQuadRoots`: the roots of `a t² + b t + c` strictly inside 0 to 1, ascending.
pub fn find_unit_quad_roots(a: f32, b: f32, c: f32) -> ([f32; 2], usize) {
  let mut roots = [0.0; 2];
  let mut count = 0;
  let mut push = |root: Option<f32>| {
    if let Some(root) = root {
      roots[count] = root;
      count += 1;
    }
  };

  if a == 0.0 {
    push(valid_unit_divide(-c, b));
    return (roots, count);
  }

  let discriminant = f64::from(b) * f64::from(b) - 4.0 * f64::from(a) * f64::from(c);

  if discriminant < 0.0 {
    return (roots, 0);
  }

  let r = discriminant.sqrt() as f32;

  if !r.is_finite() {
    return (roots, 0);
  }

  let q = if b < 0.0 {
    -(b - r) / 2.0
  } else {
    -(b + r) / 2.0
  };

  push(valid_unit_divide(q, a));
  push(valid_unit_divide(c, q));

  if count == 2 {
    if roots[0] > roots[1] {
      roots.swap(0, 1);
    } else if roots[0] == roots[1] {
      count = 1;
    }
  }

  (roots, count)
}

/// `SkFindQuadExtrema`: where the quad through `a`, `b`, `c` turns.
pub fn find_quad_extrema(a: f32, b: f32, c: f32) -> Option<f32> {
  valid_unit_divide(a - b, a - b - b + c)
}

/// `SkFindCubicExtrema`: where the cubic through `a` to `d` turns.
pub fn find_cubic_extrema(a: f32, b: f32, c: f32, d: f32) -> ([f32; 2], usize) {
  find_unit_quad_roots(d - a + 3.0 * (b - c), 2.0 * (a - b - b + c), b - a)
}

/// `SkEvalQuadAt`: the point at `t` on `quad`, from its polynomial coefficients.
pub fn eval_quad_at(quad: &[Point<f32>; 3], t: f32) -> Point<f32> {
  let axis = |p0: f32, p1: f32, p2: f32| {
    let a = p2 - (p1 + p1) + p0;
    let b = (p1 - p0) + (p1 - p0);

    (a * t + b) * t + p0
  };

  Point::new(
    axis(quad[0].x, quad[1].x, quad[2].x),
    axis(quad[0].y, quad[1].y, quad[2].y),
  )
}

/// `SkEvalCubicPosAt`: the point at `t` on `cubic`, from its polynomial coefficients.
pub fn eval_cubic_pos_at(cubic: &[Point<f32>; 4], t: f32) -> Point<f32> {
  let axis = |p0: f32, p1: f32, p2: f32, p3: f32| {
    let a = p3 + 3.0 * (p1 - p2) - p0;
    let b = 3.0 * (p2 - (p1 + p1) + p0);
    let c = 3.0 * (p1 - p0);

    ((a * t + b) * t + c) * t + p0
  };

  Point::new(
    axis(cubic[0].x, cubic[1].x, cubic[2].x, cubic[3].x),
    axis(cubic[0].y, cubic[1].y, cubic[2].y, cubic[3].y),
  )
}

fn lerp(a: Point<f32>, b: Point<f32>, t: f32) -> Point<f32> {
  Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// `SkChopQuadAt`: the two halves of `src` split at `t`, sharing their middle point.
pub fn chop_quad_at(src: [Point<f32>; 3], t: f32) -> [Point<f32>; 5] {
  let p01 = lerp(src[0], src[1], t);
  let p12 = lerp(src[1], src[2], t);

  [src[0], p01, lerp(p01, p12, t), p12, src[2]]
}

/// The control points `SkChopCubicAt` interpolates at `t`: `ab`, `bc`, `cd`, `abc`, `bcd`, `abcd`.
pub fn cubic_split(src: &[Point<f32>; 4], t: f32) -> [Point<f32>; 6] {
  let ab = lerp(src[0], src[1], t);
  let bc = lerp(src[1], src[2], t);
  let cd = lerp(src[2], src[3], t);
  let abc = lerp(ab, bc, t);
  let bcd = lerp(bc, cd, t);

  [ab, bc, cd, abc, bcd, lerp(abc, bcd, t)]
}

/// `SkChopCubicAt` into seven points.
pub fn chop_cubic_at(cubic: &[Point<f32>; 4], t: f32) -> [Point<f32>; 7] {
  if t == 1.0 {
    return [
      cubic[0], cubic[1], cubic[2], cubic[3], cubic[3], cubic[3], cubic[3],
    ];
  }

  let [ab, _, cd, abc, bcd, abcd] = cubic_split(cubic, t);

  [cubic[0], ab, abc, abcd, bcd, cd, cubic[3]]
}

//! Skia's polynomial root finders, `SkCubics` (`src/core/SkCubics.cpp`) and `SkQuads`
//! (`src/core/SkQuads.cpp`), with the double comparisons of `src/core/SkFloatingPoint.cpp`.

use std::f64::consts::PI;

/// `sk_double_nearly_zero`.
fn nearly_zero(value: f64) -> bool {
  value == 0.0 || value.abs() < f64::from(f32::EPSILON)
}

/// `sk_doubles_nearly_equal_ulps` with its default of 16 units in the last place.
fn nearly_equal_ulps(a: f64, b: f64) -> bool {
  // `magnitude`: the power of two a normal value lies at, zero for a subnormal one.
  let magnitude = |value: f64| f64::from_bits(value.to_bits() & 0x7ff0_0000_0000_0000);
  let max_magnitude = magnitude(a).max(f64::MIN_POSITIVE).max(magnitude(b));
  let tolerance = max_magnitude * (f64::EPSILON * 17.0);

  a == b || (b - a).abs() < tolerance
}

/// `SkCubics.cpp`'s `nearly_equal`.
fn nearly_equal(x: f64, y: f64) -> bool {
  if nearly_zero(x) {
    nearly_zero(y)
  } else {
    nearly_equal_ulps(x, y)
  }
}

/// `SkCubics::EvalAt`: `a t³ + b t² + c t + d`.
pub(super) fn eval(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
  t.mul_add(t.mul_add(t.mul_add(a, b), c), d)
}

/// `SkCubics::RootsValidT`: the roots of `a t³ + b t² + c t + d` in `0..=1`, with roots just
/// outside it moved onto its ends.
pub(super) fn roots_valid_t(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
  let mut found: Vec<f64> = Vec::with_capacity(3);

  for t in roots_real(a, b, c, d) {
    if t <= 1.00005 && (t >= 1.0 || nearly_equal_ulps(t, 1.0)) {
      if !found.iter().any(|&root| nearly_equal_ulps(root, 1.0)) {
        found.push(1.0);
      }
    } else if t >= -0.00005 && (t <= 0.0 || nearly_zero(t)) {
      if !found.iter().any(|&root| nearly_zero(root)) {
        found.push(0.0);
      }
    } else if t > 0.0 && t < 1.0 {
      found.push(t);
    }
  }

  found
}

/// `SkCubics::BinarySearchRootsValidT`: the roots in `0..=1`, bisected between the extrema.
pub(super) fn binary_search_roots_valid_t(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
  if ![a, b, c, d].iter().all(|value| value.is_finite()) {
    return Vec::new();
  }

  let mut regions = [0.0, 0.0, 0.0, 1.0];
  let extrema: Vec<f64> = quad_roots_real(3.0 * a, 2.0 * b, c)
    .into_iter()
    .filter(|t| (0.0..=1.0).contains(t))
    .collect();
  let start = 2 - extrema.len();

  match extrema[..] {
    [only] => regions[start + 1] = only,
    [first, second] => {
      regions[start + 1] = first.min(second);
      regions[start + 2] = first.max(second);
    }
    _ => {}
  }

  let mut found: Vec<f64> = Vec::with_capacity(3);

  for index in start..3 {
    let Some(root) = binary_search(a, b, c, d, regions[index], regions[index + 1]) else {
      continue;
    };

    if !found
      .iter()
      .take(2)
      .any(|&known| approximately_zero(known - root))
    {
      found.push(root);
    }
  }

  found
}

/// `SkCubics::RootsReal`: the real roots of `a t³ + b t² + c t + d`.
fn roots_real(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
  // `close_to_a_quadratic`.
  let close_to_quadratic = if nearly_zero(b) {
    nearly_zero(a)
  } else {
    (a / b).abs() < 1.0e-7
  };

  if close_to_quadratic {
    return quad_roots_real(b, c, d);
  }

  if nearly_zero(d) {
    let mut roots = quad_roots_real(a, b, c);

    if !roots.iter().any(|&root| nearly_zero(root)) {
      roots.push(0.0);
    }

    return roots;
  }

  if nearly_zero(a + b + c + d) {
    let mut roots = quad_roots_real(a, a + b, -d);

    if !roots.iter().any(|&root| nearly_equal_ulps(root, 1.0)) {
      roots.push(1.0);
    }

    return roots;
  }

  let inv_a = 1.0 / a;
  let (a, b, c) = (b * inv_a, c * inv_a, d * inv_a);
  let a2 = a * a;
  let q = (a2 - b * 3.0) / 9.0;
  let r = (2.0 * a2 * a - 9.0 * a * b + 27.0 * c) / 54.0;
  let r2 = r * r;
  let q3 = q * q * q;
  let r2_minus_q3 = r2 - q3;

  if !r2_minus_q3.is_finite() {
    return Vec::new();
  }

  let a_div_3 = a / 3.0;
  let mut roots = Vec::with_capacity(3);

  if r2_minus_q3 < 0.0 {
    let theta = (r / q3.sqrt()).clamp(-1.0, 1.0).acos();
    let neg_2_root_q = -2.0 * q.sqrt();

    roots.push(neg_2_root_q * (theta / 3.0).cos() - a_div_3);

    let second = neg_2_root_q * ((theta + 2.0 * PI) / 3.0).cos() - a_div_3;

    if !nearly_equal(roots[0], second) {
      roots.push(second);
    }

    let third = neg_2_root_q * ((theta - 2.0 * PI) / 3.0).cos() - a_div_3;

    if !nearly_equal(roots[0], third) && (roots.len() == 1 || !nearly_equal(roots[1], third)) {
      roots.push(third);
    }
  } else {
    let mut big_a = (r.abs() + r2_minus_q3.sqrt()).cbrt();

    if r > 0.0 {
      big_a = -big_a;
    }

    if !nearly_zero(big_a) {
      big_a += q / big_a;
    }

    roots.push(big_a - a_div_3);

    if !nearly_zero(r2) && nearly_equal_ulps(r2, q3) {
      let second = -big_a / 2.0 - a_div_3;

      if !nearly_equal(roots[0], second) {
        roots.push(second);
      }
    }
  }

  roots
}

/// `approximately_zero` in `SkCubics.cpp`, the bisection's tolerance.
fn approximately_zero(value: f64) -> bool {
  value.abs() < 0.000_000_01
}

/// `binary_search`: a root between `start` and `stop`, none unless the cubic changes sign there.
fn binary_search(a: f64, b: f64, c: f64, d: f64, mut start: f64, mut stop: f64) -> Option<f64> {
  let left = eval(a, b, c, d, start);

  if approximately_zero(left) {
    return Some(start);
  }

  let right = eval(a, b, c, d, stop);

  if !(left.is_finite() && right.is_finite())
    || (left > 0.0 && right > 0.0)
    || (left < 0.0 && right < 0.0)
  {
    return None;
  }

  for _ in 0..1000 {
    let step = (start + stop) / 2.0;
    let current = eval(a, b, c, d, step);

    if approximately_zero(current) {
      return Some(step);
    }

    if (current < 0.0 && left < 0.0) || (current > 0.0 && left > 0.0) {
      start = step;
    } else {
      stop = step;
    }
  }

  None
}

/// `SkQuads::RootsReal`: the real roots of `a t² + b t + c`.
fn quad_roots_real(a: f64, b: f64, c: f64) -> Vec<f64> {
  // `close_to_linear`: past this ratio the square term rounds away.
  if a == 0.0 || (b / a).abs() >= 1.0e16 {
    // `solve_linear`.
    if nearly_zero(b) {
      return if nearly_zero(c) {
        vec![0.0]
      } else {
        Vec::new()
      };
    }

    let root = -c / b;

    return if root.is_finite() {
      vec![root]
    } else {
      Vec::new()
    };
  }

  let (discriminant, root0, root1) = quad_roots(a, -0.5 * b, c);

  if !discriminant.is_finite() || discriminant < 0.0 {
    return Vec::new();
  }

  // `zero_if_tiny`.
  let mut roots: Vec<f64> = [root0, root1]
    .into_iter()
    .map(|root| if nearly_zero(root) { 0.0 } else { root })
    .filter(|root| root.is_finite())
    .collect();

  if roots.len() == 2 && nearly_equal_ulps(roots[0], roots[1]) {
    roots.truncate(1);
  }

  roots
}

/// `SkQuads::Roots`: the discriminant and the two roots of `a t² - 2 b t + c`, which are not a
/// number where the discriminant is negative.
fn quad_roots(a: f64, b: f64, c: f64) -> (f64, f64, f64) {
  let discriminant = quad_discriminant(a, b, c);

  if discriminant == 0.0 {
    return (discriminant, b / a, b / a);
  }

  if discriminant > 0.0 {
    let root = discriminant.sqrt();
    let r = if b > 0.0 { b + root } else { b - root };

    return (discriminant, r / a, c / r);
  }

  (discriminant, f64::NAN, f64::NAN)
}

/// `SkQuads::Discriminant`: `b² - a c`, with the rounding of both products added back where they
/// nearly cancel.
fn quad_discriminant(a: f64, b: f64, c: f64) -> f64 {
  let b2 = b * b;
  let ac = a * c;
  let rough = b2 - ac;

  if 3.0 * rough.abs() >= b2 + ac {
    return rough;
  }

  let b2_error = b.mul_add(b, -b2);
  let ac_error = a.mul_add(c, -ac);

  (b2 - ac) + (b2_error - ac_error)
}

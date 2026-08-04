//! Superellipse corner contours for the CSS `corner-shape` property.
//!
//! Corners are built in a normalized square where the curve runs from
//! `(0, 1)` to `(1, 0)`, the outer box corner sits at `(1, 1)` and the
//! corner center at `(0, 0)`. Callers map these coordinates into pixel
//! space per corner orientation.

use crate::style::Superellipse;

const KAPPA: f32 = 4.0 / 3.0 * (std::f32::consts::SQRT_2 - 1.0);

/// A corner outline in normalized coordinates, traversed from `(0, 1)` to `(1, 0)`.
pub(crate) enum CornerContour {
  /// Straight diagonal chord (`bevel`).
  Bevel,
  /// Two straight segments through the center point (`notch`).
  Notch,
  /// A single cubic Bézier: `[control1, control2, end]`.
  Cubic([[f32; 2]; 3]),
  /// Two cubic Bézier segments meeting at the exact superellipse midpoint.
  Cubics([[f32; 2]; 3], [[f32; 2]; 3]),
}

/// Builds the normalized contour for a corner shape.
///
/// `round` is intentionally excluded: callers keep the legacy quarter-ellipse
/// path for it, which this module would only reproduce approximately.
pub(crate) fn corner_contour(shape: Superellipse) -> CornerContour {
  if shape.is_fully_concave() {
    return CornerContour::Notch;
  }

  let exponent = shape.exponent();

  if exponent == 1.0 {
    return CornerContour::Bevel;
  }

  if exponent > 1.0 {
    convex_contour(exponent)
  } else {
    // A concave superellipse is the convex curve for the inverse exponent
    // mirrored across the corner chord: map every control point through
    // `(x, y) -> (1 - x, 1 - y)` and reverse the traversal.
    match convex_contour(1.0 / exponent) {
      CornerContour::Cubic([c1, c2, _]) => {
        CornerContour::Cubic([reflect(c2), reflect(c1), [1.0, 0.0]])
      }
      CornerContour::Cubics([c1, c2, mid], [c4, c5, _]) => CornerContour::Cubics(
        [reflect(c5), reflect(c4), reflect(mid)],
        [reflect(c2), reflect(c1), [1.0, 0.0]],
      ),
      contour => contour,
    }
  }
}

fn reflect(point: [f32; 2]) -> [f32; 2] {
  [1.0 - point[0], 1.0 - point[1]]
}

fn convex_contour(exponent: f32) -> CornerContour {
  if exponent == 2.0 {
    return CornerContour::Cubic([[KAPPA, 1.0], [1.0, KAPPA], [1.0, 0.0]]);
  }

  let (a, b, half) = approximate_half_corner(f64::from(exponent));

  CornerContour::Cubics(
    [[a, 1.0], [half - b, half + b], [half, half]],
    [[half + b, half - b], [1.0, a], [1.0, 0.0]],
  )
}

/// Cubic Bézier approximation of the first half (0° to 45°) of a convex
/// superellipse corner, ported from Chromium's fitted model:
/// <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/geometry/path_builder.cc>
///
/// Returns `(a, b, half)` for the control points `(a, 1)`,
/// `(half - b, half + b)` and the exact midpoint `(half, half)`; the second
/// half of the corner is this curve transposed.
fn approximate_half_corner(exponent: f64) -> (f32, f32, f32) {
  const P: [f64; 7] = [
    1.2430920942724248,
    2.010479023614843,
    0.32922901179443753,
    0.2823023142212073,
    1.3473704261055421,
    2.9149468637949814,
    0.9106507102917086,
  ];

  let s = exponent.log2();
  let slope = P[0] + (P[6] - P[0]) * 0.5 * (1.0 + (P[5] * (s - P[1])).tanh());
  let base = 1.0 / (1.0 + (slope * P[1]).exp());
  let logistic = 1.0 / (1.0 + (-slope * (s - P[1])).exp());
  let a = (logistic - base) / (1.0 - base);
  let b = P[2] * (-P[3] * s.powf(P[4])).exp();
  let half = 0.5f64.powf(1.0 / exponent);

  (a as f32, b as f32, half as f32)
}

/// Approximate length of a corner contour scaled to `radius_x`/`radius_y`,
/// by flattening each Bézier segment into chords.
pub(crate) fn contour_arc_length(contour: &CornerContour, radius_x: f32, radius_y: f32) -> f32 {
  const SEGMENTS: u32 = 8;

  let scaled_distance = |from: [f32; 2], to: [f32; 2]| {
    ((to[0] - from[0]) * radius_x).hypot((to[1] - from[1]) * radius_y)
  };

  match contour {
    CornerContour::Bevel => radius_x.hypot(radius_y),
    CornerContour::Notch => radius_x + radius_y,
    CornerContour::Cubic(cubic) => cubic_arc_length([0.0, 1.0], cubic, SEGMENTS, scaled_distance),
    CornerContour::Cubics(first, second) => {
      cubic_arc_length([0.0, 1.0], first, SEGMENTS, scaled_distance)
        + cubic_arc_length(first[2], second, SEGMENTS, scaled_distance)
    }
  }
}

fn cubic_arc_length(
  start: [f32; 2],
  [control1, control2, end]: &[[f32; 2]; 3],
  segments: u32,
  distance: impl Fn([f32; 2], [f32; 2]) -> f32,
) -> f32 {
  let point_at = |t: f32| {
    let u = 1.0 - t;
    let weight = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];

    [
      weight[0] * start[0] + weight[1] * control1[0] + weight[2] * control2[0] + weight[3] * end[0],
      weight[0] * start[1] + weight[1] * control1[1] + weight[2] * control2[1] + weight[3] * end[1],
    ]
  };

  let mut length = 0.0;
  let mut previous = start;

  for step in 1..=segments {
    let next = point_at(step as f32 / segments as f32);

    length += distance(previous, next);
    previous = next;
  }

  length
}

#[cfg(test)]
mod tests {
  use super::*;

  fn superellipse_error(exponent: f32, contour: &CornerContour) -> f32 {
    let cubics: Vec<([f32; 2], [[f32; 2]; 3])> = match contour {
      CornerContour::Cubic(cubic) => vec![([0.0, 1.0], *cubic)],
      CornerContour::Cubics(first, second) => {
        vec![([0.0, 1.0], *first), (first[2], *second)]
      }
      _ => panic!("expected curved contour"),
    };

    let mut max_error: f32 = 0.0;

    for (start, [c1, c2, end]) in cubics {
      for step in 0..=16 {
        let t = step as f32 / 16.0;
        let u = 1.0 - t;
        let x = u * u * u * start[0]
          + 3.0 * u * u * t * c1[0]
          + 3.0 * u * t * t * c2[0]
          + t * t * t * end[0];
        let y = u * u * u * start[1]
          + 3.0 * u * u * t * c1[1]
          + 3.0 * u * t * t * c2[1]
          + t * t * t * end[1];

        let residual = x.max(0.0).powf(exponent) + y.max(0.0).powf(exponent) - 1.0;

        max_error = max_error.max(residual.abs());
      }
    }

    max_error
  }

  #[test]
  fn squircle_contour_stays_near_superellipse() {
    let contour = corner_contour(crate::style::Superellipse::SQUIRCLE);

    assert!(superellipse_error(4.0, &contour) < 0.05);
  }

  #[test]
  fn convex_midpoint_is_exact() {
    for shape in [
      crate::style::Superellipse(0.5),
      crate::style::Superellipse::SQUIRCLE,
      crate::style::Superellipse(3.0),
    ] {
      let CornerContour::Cubics([_, _, [x, y]], _) = corner_contour(shape) else {
        panic!("expected two cubics");
      };
      let expected = 0.5f32.powf(1.0 / shape.exponent());

      assert!((x - expected).abs() < 1e-5);
      assert!((y - expected).abs() < 1e-5);
    }
  }

  #[test]
  fn scoop_mirrors_round_across_the_chord() {
    let CornerContour::Cubic([c1, c2, end]) = corner_contour(crate::style::Superellipse::SCOOP)
    else {
      panic!("expected single cubic");
    };

    assert_eq!(end, [1.0, 0.0]);
    assert_eq!(c1, [0.0, 1.0 - KAPPA]);
    assert_eq!(c2, [1.0 - KAPPA, 0.0]);
  }

  #[test]
  fn bevel_and_notch_are_straight() {
    assert!(matches!(
      corner_contour(crate::style::Superellipse::BEVEL),
      CornerContour::Bevel
    ));
    assert!(matches!(
      corner_contour(crate::style::Superellipse::NOTCH),
      CornerContour::Notch
    ));
  }

  #[test]
  fn arc_length_matches_known_shapes() {
    let bevel = contour_arc_length(&CornerContour::Bevel, 3.0, 4.0);

    assert!((bevel - 5.0).abs() < 1e-5);

    let quarter_circle = contour_arc_length(
      &CornerContour::Cubic([[KAPPA, 1.0], [1.0, KAPPA], [1.0, 0.0]]),
      10.0,
      10.0,
    );

    assert!((quarter_circle - std::f32::consts::FRAC_PI_2 * 10.0).abs() < 0.1);
  }
}

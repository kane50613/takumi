//! The linear and two-point conical gradients of Skia's `SkGradientBaseShader`, as its
//! high-precision raster pipeline runs them. Approximate: a shaded pixel rounds to 8 bits before it
//! blends, where Skia blends a gradient's pixels in floats.

use takumi_core::{geometry::Point, resources::image::SvgSpreadMethod, style::Affine};

/// How many pixels [`Gradient::shade_row`] carries through each stage at once.
const SHADE_CHUNK: usize = 64;

/// Skia's `SkGradientBaseShader::kDegenerateThreshold`.
const DEGENERATE_THRESHOLD: f32 = 1.0 / (1 << 15) as f32;

/// A gradient color stop: a position along the gradient and an unpremultiplied RGBA color.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GradientStop {
  pub(crate) position: f32,
  pub(crate) color: [f32; 4],
}

/// The shape a gradient's `t` follows.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GradientShape {
  /// From `start` (t = 0) to `end` (t = 1), Skia's `SkLinearGradient`.
  Linear { start: [f32; 2], end: [f32; 2] },
  /// From the circle at `start` of `start_radius` to the one at `end` of `end_radius`, Skia's
  /// two-point conical gradient.
  Conical {
    start: [f32; 2],
    start_radius: f32,
    end: [f32; 2],
    end_radius: f32,
  },
}

/// A gradient shader, built as `SkShaders::LinearGradient` and `SkShaders::TwoPointConicalGradient`
/// build theirs.
pub(crate) struct Gradient {
  /// Maps a device pixel center into the space `kind` reads `t` from.
  device_to_unit: Affine,
  kind: Kind,
  spread: SvgSpreadMethod,
  /// `ts[i]` starts interval `i`, whose colors are `factors[i] * t + biases[i]`, for `i >= 1`.
  /// Interval 0 holds the color before the first stop.
  ts: Vec<f32>,
  factors: Vec<[f32; 4]>,
  biases: Vec<[f32; 4]>,
}

/// How a gradient turns a point into `t`.
#[derive(Clone, Copy, Debug)]
enum Kind {
  /// `t` is `x`.
  Linear,
  /// `xy_to_radius`, then `t * scale + bias`.
  Radial { scale: f32, bias: f32 },
  /// `xy_to_2pt_conical_strip` with `r0` squared.
  Strip { r0_squared: f32 },
  /// The focal stages of `SkConicalGradient::appendGradientStages`.
  Focal(Focal),
}

/// Skia's `SkConicalGradient::FocalData`.
#[derive(Clone, Copy, Debug)]
struct Focal {
  r1: f32,
  focal_x: f32,
  swapped: bool,
}

impl Focal {
  /// `FocalData::set`: the focal data of radii `r0` and `r1` in units of the centers' distance,
  /// appending to `matrix` the map into the space its stages read.
  fn new(mut r0: f32, mut r1: f32, matrix: &mut Affine) -> Option<Self> {
    let mut swapped = false;
    let mut focal_x = r0 / (r0 - r1);

    if nearly_zero(focal_x - 1.0) {
      *matrix = Affine::scale(-1.0, 1.0) * Affine::translation(-1.0, 0.0) * *matrix;
      std::mem::swap(&mut r0, &mut r1);
      focal_x = 0.0;
      swapped = true;
    }

    // Maps {focal point, (1, 0)} to {(0, 0), (1, 0)}.
    let span = 1.0 - focal_x;

    if !span.is_finite() || span == 0.0 {
      return None;
    }

    *matrix = Affine::scale(1.0 / span, 1.0 / span) * Affine::translation(-focal_x, 0.0) * *matrix;

    let r1 = r1 / span.abs();
    let focal = Self {
      r1,
      focal_x,
      swapped,
    };

    *matrix = if focal.is_focal_on_circle() {
      Affine::scale(0.5, 0.5) * *matrix
    } else {
      Affine::scale(r1 / (r1 * r1 - 1.0), 1.0 / (r1 * r1 - 1.0).abs().sqrt()) * *matrix
    };
    *matrix = Affine::scale(span.abs(), span.abs()) * *matrix;

    Some(focal)
  }

  fn is_focal_on_circle(self) -> bool {
    nearly_zero(1.0 - self.r1)
  }

  fn is_well_behaved(self) -> bool {
    !self.is_focal_on_circle() && self.r1 > 1.0
  }

  fn is_natively_focal(self) -> bool {
    nearly_zero(self.focal_x)
  }

  /// The stages from `xy_to_2pt_conical_*` to `alter_2pt_conical_unswap`: `t` at `(x, y)`, or
  /// `None` where the gradient is undefined.
  fn t(self, x: f32, y: f32) -> Option<f32> {
    let inverse_r1 = 1.0 / self.r1;
    let mut t = if self.is_focal_on_circle() {
      x + y * y / x
    } else if self.is_well_behaved() {
      (x * x + y * y).sqrt() - x * inverse_r1
    } else if self.swapped || 1.0 - self.focal_x < 0.0 {
      -(x * x - y * y).sqrt() - x * inverse_r1
    } else {
      (x * x - y * y).sqrt() - x * inverse_r1
    };

    // `mask_2pt_conical_degenerates`.
    if !self.is_well_behaved() && (t <= 0.0 || t.is_nan()) {
      return None;
    }
    if 1.0 - self.focal_x < 0.0 {
      t = -t;
    }
    if !self.is_natively_focal() {
      t += self.focal_x;
    }
    if self.swapped {
      t = 1.0 - t;
    }

    Some(t)
  }
}

impl Gradient {
  /// A gradient of `shape` through `stops` spreading as `spread`, drawn under `transform` from its
  /// own space to device pixels. `Err` carries the solid color a degenerate gradient paints, and
  /// `Err(None)` paints nothing.
  pub(crate) fn new(
    shape: GradientShape,
    stops: &[GradientStop],
    spread: SvgSpreadMethod,
    transform: Affine,
  ) -> Result<Self, Option<[u8; 4]>> {
    let Some(&last) = stops.last() else {
      return Err(None);
    };

    if stops.len() == 1 {
      return Err(Some(premultiplied(last.color)));
    }

    let Some(device_to_local) = transform.invert() else {
      return Err(None);
    };
    let degenerate = || {
      Err(Some(match spread {
        SvgSpreadMethod::Pad => premultiplied(last.color),
        SvgSpreadMethod::Reflect | SvgSpreadMethod::Repeat => premultiplied(average_color(stops)),
      }))
    };
    let (local_to_unit, kind) = match shape {
      GradientShape::Linear { start, end } => {
        let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
        let length = dx.hypot(dy);

        if !length.is_finite() {
          return Err(None);
        }
        if nearly_zero_within(length, DEGENERATE_THRESHOLD) {
          return degenerate();
        }

        (points_to_unit(start, end)?, Kind::Linear)
      }
      GradientShape::Conical {
        start,
        start_radius,
        end,
        end_radius,
      } => {
        if start_radius < 0.0 || end_radius < 0.0 {
          return Err(None);
        }

        let concentric = nearly_zero_within(
          (start[0] - end[0]).hypot(start[1] - end[1]),
          DEGENERATE_THRESHOLD,
        );

        if concentric && nearly_zero_within(start_radius - end_radius, DEGENERATE_THRESHOLD) {
          // The interpolation region shrinks to a ring at the radius: the first color inside it and
          // the last outside, under `Pad`.
          if spread != SvgSpreadMethod::Pad || end_radius <= DEGENERATE_THRESHOLD {
            return degenerate();
          }

          let first = stops[0].color;
          let ring = [
            GradientStop {
              position: 0.0,
              color: first,
            },
            GradientStop {
              position: 1.0,
              color: first,
            },
            GradientStop {
              position: 1.0,
              color: last.color,
            },
          ];

          return Self::radial(start, end_radius, &ring, spread, device_to_local);
        }
        if concentric && nearly_zero_within(start_radius, DEGENERATE_THRESHOLD) {
          return Self::radial(start, end_radius, stops, spread, device_to_local);
        }

        Self::conical(start, start_radius, end, end_radius)?
      }
    };

    Ok(Self::from_parts(
      local_to_unit * device_to_local,
      kind,
      stops,
      spread,
    ))
  }

  /// `SkShaders::RadialGradient` around `center` of `radius`.
  fn radial(
    center: [f32; 2],
    radius: f32,
    stops: &[GradientStop],
    spread: SvgSpreadMethod,
    device_to_local: Affine,
  ) -> Result<Self, Option<[u8; 4]>> {
    if nearly_zero_within(radius, DEGENERATE_THRESHOLD) {
      let last = stops[stops.len() - 1].color;

      return Err(Some(match spread {
        SvgSpreadMethod::Pad => premultiplied(last),
        SvgSpreadMethod::Reflect | SvgSpreadMethod::Repeat => premultiplied(average_color(stops)),
      }));
    }

    let local_to_unit =
      Affine::scale(1.0 / radius, 1.0 / radius) * Affine::translation(-center[0], -center[1]);

    Ok(Self::from_parts(
      local_to_unit * device_to_local,
      Kind::Radial {
        scale: 1.0,
        bias: 0.0,
      },
      stops,
      spread,
    ))
  }

  /// `SkConicalGradient::Create` for centers that differ or radii that are not degenerate.
  fn conical(
    start: [f32; 2],
    start_radius: f32,
    end: [f32; 2],
    end_radius: f32,
  ) -> Result<(Affine, Kind), Option<[u8; 4]>> {
    let distance = (start[0] - end[0]).hypot(start[1] - end[1]);

    if nearly_zero(distance) {
      if nearly_zero(start_radius.max(end_radius)) || nearly_equal(start_radius, end_radius) {
        return Err(None);
      }

      // Concentric: radial, with `t` mapped from [0, r1] onto [r0, r1].
      let scale = 1.0 / start_radius.max(end_radius);
      let radius_span = end_radius - start_radius;

      return Ok((
        Affine::scale(scale, scale) * Affine::translation(-end[0], -end[1]),
        Kind::Radial {
          scale: start_radius.max(end_radius) / radius_span,
          bias: -start_radius / radius_span,
        },
      ));
    }

    let mut matrix = map_to_unit_x(start, end)?;

    if nearly_zero(end_radius - start_radius) {
      let scaled = start_radius / distance;

      return Ok((
        matrix,
        Kind::Strip {
          r0_squared: scaled * scaled,
        },
      ));
    }

    let focal =
      Focal::new(start_radius / distance, end_radius / distance, &mut matrix).ok_or(None)?;

    Ok((matrix, Kind::Focal(focal)))
  }

  fn from_parts(
    device_to_unit: Affine,
    kind: Kind,
    stops: &[GradientStop],
    spread: SvgSpreadMethod,
  ) -> Self {
    let (ts, factors, biases) = interval_equations(stops);

    Self {
      device_to_unit,
      kind,
      spread,
      ts,
      factors,
      biases,
    }
  }

  pub(crate) fn shade_row(&self, x: i32, y: i32, out: &mut [[u8; 4]]) {
    for (chunk, pixels) in out.chunks_mut(SHADE_CHUNK).enumerate() {
      let first = chunk * SHADE_CHUNK;
      let mut ts = [0.0; SHADE_CHUNK];
      let mut defined = [true; SHADE_CHUNK];
      let ts = &mut ts[..pixels.len()];

      // Each stage runs over the whole chunk, so its kind and spread are matched once.
      for (offset, (t, defined)) in ts.iter_mut().zip(&mut defined).enumerate() {
        let (ux, uy) = self
          .device_to_unit
          .transform_point(x as f32 + (first + offset) as f32 + 0.5, y as f32 + 0.5);

        match self.t(ux, uy) {
          Some(value) => *t = value,
          None => *defined = false,
        }
      }

      self.tile_all(ts);

      for ((pixel, &t), &defined) in pixels.iter_mut().zip(&*ts).zip(&defined) {
        *pixel = if defined {
          premultiplied(self.color_at(t))
        } else {
          [0; 4]
        };
      }
    }
  }

  /// [`Self::tile`] of every `t`, with the spread matched once.
  fn tile_all(&self, ts: &mut [f32]) {
    match self.spread {
      SvgSpreadMethod::Pad => {}
      SvgSpreadMethod::Repeat | SvgSpreadMethod::Reflect => {
        for t in ts {
          *t = self.tile(*t);
        }
      }
    }
  }

  /// The gradient's `t` at `(x, y)` of its unit space, `None` where a conical gradient is undefined.
  fn t(&self, x: f32, y: f32) -> Option<f32> {
    match self.kind {
      Kind::Linear => Some(x),
      Kind::Radial { scale, bias } => Some((x * x + y * y).sqrt() * scale + bias),
      Kind::Strip { r0_squared } => {
        let t = x + (r0_squared - y * y).sqrt();

        (!t.is_nan()).then_some(t)
      }
      Kind::Focal(focal) => focal.t(x, y),
    }
  }

  /// `repeat_x_1` or `mirror_x_1`. `Pad` leaves `t` as it is, as Skia does for stops that may be
  /// hard, since the outer intervals hold the end colors.
  fn tile(&self, t: f32) -> f32 {
    match self.spread {
      SvgSpreadMethod::Pad => t,
      SvgSpreadMethod::Repeat => (t - t.floor()).clamp(0.0, 1.0),
      SvgSpreadMethod::Reflect => (((t - 1.0) - 2.0 * ((t - 1.0) * 0.5).floor()) - 1.0)
        .abs()
        .clamp(0.0, 1.0),
    }
  }

  /// The `gradient` stage: the unpremultiplied color of the interval `t` falls in.
  fn color_at(&self, t: f32) -> [f32; 4] {
    let index = self.ts[1..].iter().filter(|&&start| t >= start).count();
    let (factor, bias) = (self.factors[index], self.biases[index]);

    std::array::from_fn(|channel| t * factor[channel] + bias[channel])
  }
}

/// `SkLinearGradient`'s `pts_to_unit_matrix`: `start` to the origin and `end` to `(1, 0)`.
fn points_to_unit(start: [f32; 2], end: [f32; 2]) -> Result<Affine, Option<[u8; 4]>> {
  let length = Point::new(end[0] - start[0], end[1] - start[1]).length();

  if length == 0.0 || !length.is_finite() {
    return Err(None);
  }

  let inverse = 1.0 / length;
  let (cos, sin) = ((end[0] - start[0]) * inverse, (end[1] - start[1]) * inverse);
  let [x, y] = start;
  let one_minus_cos = 1.0 - cos;

  // `setSinCos(-sin, cos, x, y)`, then `postTranslate(-x, -y)` and `postScale(inverse, inverse)`.
  Ok(Affine {
    a: cos * inverse,
    b: -sin * inverse,
    c: sin * inverse,
    d: cos * inverse,
    x: (-sin * y + one_minus_cos * x - x) * inverse,
    y: (sin * x + one_minus_cos * y - y) * inverse,
  })
}

/// `SkConicalGradient::MapToUnitX`: `start` to the origin and `end` to `(1, 0)`, through
/// `SkMatrix::PolyToPoly`'s `Poly2Proc` maps of both pairs of points.
fn map_to_unit_x(start: [f32; 2], end: [f32; 2]) -> Result<Affine, Option<[u8; 4]>> {
  let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
  let centers = Affine {
    a: dy,
    b: -dx,
    c: dx,
    d: dy,
    x: start[0],
    y: start[1],
  };
  let unit = Affine {
    a: 0.0,
    b: -1.0,
    c: 1.0,
    d: 0.0,
    x: 0.0,
    y: 0.0,
  };

  centers
    .skia_invert()
    .map(|inverse| unit * inverse)
    .ok_or(None)
}

/// The constructor's stop normalization and `AppendGradientFillStages`' arbitrary-stop equations.
fn interval_equations(stops: &[GradientStop]) -> (Vec<f32>, Vec<[f32; 4]>, Vec<[f32; 4]>) {
  let mut positions = Vec::with_capacity(stops.len() + 2);
  let mut colors = Vec::with_capacity(stops.len() + 2);

  if stops[0].position > 0.0 {
    positions.push(0.0);
    colors.push(stops[0].color);
  }

  let mut previous = 0.0;

  for stop in stops {
    let position = stop.position.clamp(previous, 1.0);

    positions.push(position);
    colors.push(stop.color);
    previous = position;
  }

  if *positions.last().unwrap_or(&1.0) != 1.0 {
    positions.push(1.0);
    colors.push(stops[stops.len() - 1].color);
  }

  positions[0] = 0.0;

  let count = colors.len();
  let first = if count > 2 && colors[0] == colors[1] {
    1
  } else {
    0
  };
  let last = if count > 2 && colors[count - 2] == colors[count - 1] {
    count - 2
  } else {
    count - 1
  };
  let mut ts = vec![0.0];
  let mut factors = vec![[0.0; 4]];
  let mut biases = vec![colors[first]];
  let (mut t_left, mut color_left) = (positions[first], colors[first]);

  for index in first..last {
    let (t_right, color_right) = (positions[index + 1], colors[index + 1]);

    if t_left < t_right {
      let scale = 1.0 / (t_right - t_left);

      if scale.is_finite() {
        let factor: [f32; 4] =
          std::array::from_fn(|channel| (color_right[channel] - color_left[channel]) * scale);

        ts.push(t_left);
        factors.push(factor);
        biases.push(std::array::from_fn(|channel| {
          color_left[channel] - factor[channel] * t_left
        }));
      }
    }

    t_left = t_right;
    color_left = color_right;
  }

  ts.push(t_left);
  factors.push([0.0; 4]);
  biases.push(color_left);

  (ts, factors, biases)
}

/// `average_gradient_color`: the color a gradient averages to across `t` in [0, 1].
fn average_color(stops: &[GradientStop]) -> [f32; 4] {
  let mut blend = [0.0; 4];

  for (index, pair) in stops.windows(2).enumerate() {
    let p0 = pair[0].position.clamp(0.0, 1.0);
    let p1 = pair[1].position.clamp(p0, 1.0);

    if index == 0 && p0 > 0.0 {
      for channel in 0..4 {
        blend[channel] += p0 * pair[0].color[channel];
      }
    }
    if index == stops.len() - 2 && p1 < 1.0 {
      for channel in 0..4 {
        blend[channel] += (1.0 - p1) * pair[1].color[channel];
      }
    }
    for channel in 0..4 {
      blend[channel] += 0.5 * (p1 - p0) * (pair[0].color[channel] + pair[1].color[channel]);
    }
  }

  blend
}

/// `premul` then `store_8888`: an unpremultiplied float color as premultiplied bytes, each
/// `to_unorm`'s `trunc(clamp(v, 0, 1) * 255 + 0.5)`.
fn premultiplied([red, green, blue, alpha]: [f32; 4]) -> [u8; 4] {
  let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;

  [
    byte(red * alpha),
    byte(green * alpha),
    byte(blue * alpha),
    byte(alpha),
  ]
}

/// `SkScalarNearlyZero` at its default tolerance.
fn nearly_zero(value: f32) -> bool {
  nearly_zero_within(value, 1.0 / (1 << 12) as f32)
}

fn nearly_zero_within(value: f32, tolerance: f32) -> bool {
  value.abs() <= tolerance
}

/// `SkScalarNearlyEqual` at its default tolerance.
fn nearly_equal(a: f32, b: f32) -> bool {
  nearly_zero(a - b)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn stop(position: f32, color: [f32; 4]) -> GradientStop {
    GradientStop { position, color }
  }

  const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
  const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

  fn row(gradient: &Gradient, y: i32, width: usize) -> Vec<[u8; 4]> {
    let mut out = vec![[0; 4]; width];

    gradient.shade_row(0, y, &mut out);
    out
  }

  #[test]
  fn a_linear_gradient_interpolates_between_its_stops() {
    let gradient = Gradient::new(
      GradientShape::Linear {
        start: [0.0, 0.0],
        end: [10.0, 0.0],
      },
      &[stop(0.0, RED), stop(1.0, BLUE)],
      SvgSpreadMethod::Pad,
      Affine::IDENTITY,
    )
    .unwrap();
    let pixels = row(&gradient, 0, 10);

    // Pixel 0's center is at t = 0.05.
    assert_eq!(pixels[0], [242, 0, 13, 255]);
    assert_eq!(pixels[9], [13, 0, 242, 255]);
  }

  #[test]
  fn pad_paints_the_first_stop_before_a_hard_stop_at_the_start() {
    let gradient = Gradient::new(
      GradientShape::Linear {
        start: [5.0, 0.0],
        end: [10.0, 0.0],
      },
      &[stop(0.0, RED), stop(0.0, BLUE), stop(1.0, BLUE)],
      SvgSpreadMethod::Pad,
      Affine::IDENTITY,
    )
    .unwrap();
    let pixels = row(&gradient, 0, 10);

    assert_eq!(pixels[0], [255, 0, 0, 255]);
    assert_eq!(pixels[9], [0, 0, 255, 255]);
  }

  #[test]
  fn spreads_repeat_and_reflect_past_the_ends() {
    let shape = GradientShape::Linear {
      start: [0.0, 0.0],
      end: [4.0, 0.0],
    };
    let stops = [stop(0.0, RED), stop(1.0, BLUE)];
    let repeat = Gradient::new(shape, &stops, SvgSpreadMethod::Repeat, Affine::IDENTITY).unwrap();
    let reflect = Gradient::new(shape, &stops, SvgSpreadMethod::Reflect, Affine::IDENTITY).unwrap();

    assert_eq!(row(&repeat, 0, 8)[4], row(&repeat, 0, 8)[0]);
    assert_eq!(row(&reflect, 0, 8)[4], row(&reflect, 0, 8)[3]);
  }

  #[test]
  fn a_degenerate_gradient_paints_one_color() {
    let shape = GradientShape::Linear {
      start: [3.0, 3.0],
      end: [3.0, 3.0],
    };
    let stops = [stop(0.0, RED), stop(1.0, BLUE)];

    assert_eq!(
      Gradient::new(shape, &stops, SvgSpreadMethod::Pad, Affine::IDENTITY).err(),
      Some(Some([0, 0, 255, 255]))
    );
    assert_eq!(
      Gradient::new(shape, &stops, SvgSpreadMethod::Repeat, Affine::IDENTITY).err(),
      Some(Some([128, 0, 128, 255]))
    );
  }

  #[test]
  fn a_radial_gradient_follows_the_distance_from_its_center() {
    let gradient = Gradient::new(
      GradientShape::Conical {
        start: [5.0, 5.0],
        start_radius: 0.0,
        end: [5.0, 5.0],
        end_radius: 5.0,
      },
      &[stop(0.0, RED), stop(1.0, BLUE)],
      SvgSpreadMethod::Pad,
      Affine::IDENTITY,
    )
    .unwrap();
    let pixels = row(&gradient, 4, 10);

    assert_eq!(pixels[4], pixels[5]);
    assert_eq!(pixels[0][3], 255);
    assert!(pixels[4][0] > pixels[0][0]);
  }

  /// The two-point conical gradient at the center of pixel `(x, y)`, solved in doubles for the
  /// largest `t` whose circle has a radius, padded, and interpolated between unpremultiplied stops:
  /// an oracle independent of the stage pipeline.
  fn conical_reference(shape: GradientShape, stops: &[GradientStop], x: u32, y: u32) -> [u8; 4] {
    let GradientShape::Conical {
      start,
      start_radius,
      end,
      end_radius,
    } = shape
    else {
      return [0; 4];
    };
    let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
    let (cx, cy) = (f64::from(start[0]), f64::from(start[1]));
    let (dx, dy) = (f64::from(end[0]) - cx, f64::from(end[1]) - cy);
    let (r0, dr) = (
      f64::from(start_radius),
      f64::from(end_radius - start_radius),
    );
    let (qx, qy) = (px - cx, py - cy);
    let a = dx * dx + dy * dy - dr * dr;
    let b = qx * dx + qy * dy + r0 * dr;
    let c = qx * qx + qy * qy - r0 * r0;
    let radius_at = |t: f64| r0 + t * dr;
    let t = if a.abs() < 1e-9 {
      (b != 0.0)
        .then(|| c / (2.0 * b))
        .filter(|&t| radius_at(t) >= 0.0)
    } else {
      let discriminant = b * b - a * c;

      (discriminant >= 0.0)
        .then(|| {
          let root = discriminant.sqrt();
          let (high, low) = ((b + root) / a, (b - root) / a);
          let (high, low) = if high >= low {
            (high, low)
          } else {
            (low, high)
          };

          [high, low].into_iter().find(|&t| radius_at(t) >= 0.0)
        })
        .flatten()
    };
    let Some(t) = t else {
      return [0; 4];
    };
    let t = t.clamp(0.0, 1.0) as f32;
    let next = stops
      .iter()
      .position(|stop| stop.position >= t)
      .unwrap_or(stops.len() - 1);
    let color = if next == 0 {
      stops[0].color
    } else {
      let (from, to) = (&stops[next - 1], &stops[next]);
      let span = to.position - from.position;
      let weight = if span > 0.0 {
        (t - from.position) / span
      } else {
        1.0
      };

      std::array::from_fn(|lane| from.color[lane] + (to.color[lane] - from.color[lane]) * weight)
    };
    let alpha = color[3];
    let unorm = |value: f32| (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;

    [
      unorm(color[0] * alpha),
      unorm(color[1] * alpha),
      unorm(color[2] * alpha),
      unorm(alpha),
    ]
  }

  #[test]
  fn conical_gradients_match_the_analytic_gradient_in_every_focal_case() {
    let stops = [
      stop(0.0, RED),
      stop(0.4, [0.0, 1.0, 0.0, 0.5]),
      stop(1.0, BLUE),
    ];
    let cases = [
      // Well behaved: the focal circle inside the end circle.
      ([16.0, 16.0], 2.0, [20.0, 18.0], 14.0),
      // Focal point on the end circle.
      ([4.0, 16.0], 0.0, [16.0, 16.0], 12.0),
      // Start circle outside the end circle, where only a cone is defined.
      ([2.0, 16.0], 3.0, [26.0, 16.0], 6.0),
      // Swapped radii.
      ([26.0, 16.0], 6.0, [2.0, 16.0], 3.0),
      // Equal radii: the strip.
      ([4.0, 10.0], 5.0, [24.0, 20.0], 5.0),
      // Concentric with an inner radius.
      ([16.0, 16.0], 4.0, [16.0, 16.0], 12.0),
    ];

    for (start, start_radius, end, end_radius) in cases {
      let shape = GradientShape::Conical {
        start,
        start_radius,
        end,
        end_radius,
      };
      let gradient = Gradient::new(shape, &stops, SvgSpreadMethod::Pad, Affine::IDENTITY).unwrap();

      for y in 0..32 {
        let ours = row(&gradient, y, 32);

        for (x, a) in ours.iter().enumerate() {
          let b = &conical_reference(shape, &stops, x as u32, y as u32);

          assert!(
            a.iter().zip(b).all(|(p, q)| p.abs_diff(*q) <= 1),
            "{shape:?} at ({x}, {y}): {a:?} against {b:?}"
          );
        }
      }
    }
  }
}

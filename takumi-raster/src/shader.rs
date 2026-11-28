//! What a path fill paints with, after Skia's shaders: a solid color, the linear and two-point
//! conical gradients of `SkGradientBaseShader` as its high-precision raster pipeline runs them, and
//! an image as `SkImageShader` samples one. Approximate: a shaded pixel rounds to 8 bits before it
//! blends, where Skia blends a gradient's pixels in floats.

use takumi_core::{geometry::Point, style::Affine};

use crate::blend::scale_premultiplied_pixel;

/// Skia's `SkGradientBaseShader::kDegenerateThreshold`.
const DEGENERATE_THRESHOLD: f32 = 1.0 / (1 << 15) as f32;

/// What a fill paints each pixel with.
pub(crate) enum Shader<'a> {
  /// One premultiplied color.
  Solid([u8; 4]),
  /// A gradient.
  Gradient(Gradient),
  /// An image.
  Image(ImageShader<'a>),
}

impl Shader<'_> {
  /// The premultiplied colors of the `out.len()` pixels that start at `(x, y)` along a row.
  pub(crate) fn shade_row(&self, x: i32, y: i32, out: &mut [[u8; 4]]) {
    match self {
      Self::Solid(color) => out.fill(*color),
      Self::Gradient(gradient) => gradient.shade_row(x, y, out),
      Self::Image(image) => image.shade_row(x, y, out),
    }
  }
}

/// How an image is sampled between its pixels, Skia's `SkFilterMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ImageFilter {
  Nearest,
  Linear,
}

/// An image shader, `SkImageShader` under a `Pad` (clamp) or `Repeat` tile mode.
pub(crate) struct ImageShader<'a> {
  pixels: &'a [[u8; 4]],
  width: u32,
  height: u32,
  /// Maps a device pixel center onto the image's pixel grid.
  device_to_image: Affine,
  spread: Spread,
  filter: ImageFilter,
  /// The paint alpha the samples scale by, `scale_1_float`.
  alpha: u8,
}

impl<'a> ImageShader<'a> {
  /// The premultiplied `width` by `height` `pixels`, drawn under `transform` from image pixels to
  /// device pixels at `opacity`. `None` for an empty image or a transform that does not invert.
  pub(crate) fn new(
    pixels: &'a [[u8; 4]],
    (width, height): (u32, u32),
    transform: Affine,
    spread: Spread,
    filter: ImageFilter,
    opacity: f32,
  ) -> Option<Self> {
    if width == 0 || height == 0 || pixels.len() != width as usize * height as usize {
      return None;
    }

    Some(Self {
      pixels,
      width,
      height,
      device_to_image: transform.invert()?,
      spread,
      filter,
      alpha: (opacity.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
    })
  }

  fn shade_row(&self, x: i32, y: i32, out: &mut [[u8; 4]]) {
    for (offset, pixel) in out.iter_mut().enumerate() {
      let (u, v) = self
        .device_to_image
        .transform_point(x as f32 + offset as f32 + 0.5, y as f32 + 0.5);
      let sample = match self.filter {
        ImageFilter::Nearest => self.nearest(u, v),
        ImageFilter::Linear => self.bilinear(u - 0.5, v - 0.5),
      };

      *pixel = scale_premultiplied_pixel(sample, self.alpha);
    }
  }

  /// `bilerp_clamp_8888` and its tiled form: the four texels around `(u, v)` weighted by distance.
  fn bilinear(&self, u: f32, v: f32) -> [u8; 4] {
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (u - x0, v - y0);
    let corners = [
      (self.texel(x0, y0), (1.0 - fx) * (1.0 - fy)),
      (self.texel(x0 + 1.0, y0), fx * (1.0 - fy)),
      (self.texel(x0, y0 + 1.0), (1.0 - fx) * fy),
      (self.texel(x0 + 1.0, y0 + 1.0), fx * fy),
    ];

    std::array::from_fn(|channel| {
      let value: f32 = corners
        .iter()
        .map(|(texel, weight)| f32::from(texel[channel]) * weight)
        .sum();

      (value + 0.5) as u8
    })
  }

  /// The texel at `(x, y)` of the pixel grid, its coordinates tiled as `spread` has them.
  fn texel(&self, x: f32, y: f32) -> [u8; 4] {
    let index = |coordinate: f32, size: u32| {
      self.tile(coordinate, size).clamp(0.0, size as f32 - 1.0) as usize
    };

    self.pixels[index(y, self.height) * self.width as usize + index(x, self.width)]
  }

  /// `gather_8888` under `roundDownAtInteger`, which `SkImageShader` sets for nearest sampling:
  /// `ix_and_ptr` clamps into `(0, size)` and steps one ulp down, so an integer picks the texel
  /// before it.
  fn nearest(&self, u: f32, v: f32) -> [u8; 4] {
    let index = |coordinate: f32, size: u32| {
      let limit = f32::from_bits((size as f32).to_bits() - 1);
      let clamped = self
        .tile(coordinate, size)
        .max(f32::MIN_POSITIVE)
        .min(limit);

      f32::from_bits(clamped.to_bits() - 1) as usize
    };

    self.pixels[index(v, self.height) * self.width as usize + index(u, self.width)]
  }

  /// `coordinate` tiled across `size` texels as `spread` has it.
  fn tile(&self, coordinate: f32, size: u32) -> f32 {
    match self.spread {
      Spread::Repeat => coordinate - (coordinate / size as f32).floor() * size as f32,
      Spread::Pad | Spread::Reflect => coordinate,
    }
  }
}

/// How a gradient repeats past its ends, Skia's `SkTileMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Spread {
  Pad,
  Reflect,
  Repeat,
}

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
  spread: Spread,
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
    spread: Spread,
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
        Spread::Pad => premultiplied(last.color),
        Spread::Reflect | Spread::Repeat => premultiplied(average_color(stops)),
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
          if spread != Spread::Pad || end_radius <= DEGENERATE_THRESHOLD {
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
    spread: Spread,
    device_to_local: Affine,
  ) -> Result<Self, Option<[u8; 4]>> {
    if nearly_zero_within(radius, DEGENERATE_THRESHOLD) {
      let last = stops[stops.len() - 1].color;

      return Err(Some(match spread {
        Spread::Pad => premultiplied(last),
        Spread::Reflect | Spread::Repeat => premultiplied(average_color(stops)),
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
    spread: Spread,
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

  fn shade_row(&self, x: i32, y: i32, out: &mut [[u8; 4]]) {
    for (offset, pixel) in out.iter_mut().enumerate() {
      let (ux, uy) = self
        .device_to_unit
        .transform_point(x as f32 + offset as f32 + 0.5, y as f32 + 0.5);

      *pixel = match self.t(ux, uy) {
        Some(t) => premultiplied(self.color_at(self.tile(t))),
        None => [0; 4],
      };
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
      Spread::Pad => t,
      Spread::Repeat => (t - t.floor()).clamp(0.0, 1.0),
      Spread::Reflect => (((t - 1.0) - 2.0 * ((t - 1.0) * 0.5).floor()) - 1.0)
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

  #[test]
  fn nearest_sampling_on_a_texel_edge_takes_the_texel_before_it() {
    let (red, blue) = ([255, 0, 0, 255], [0, 0, 255, 255]);
    let pixels = [red, blue, red, blue];
    let Some(shader) = ImageShader::new(
      &pixels,
      (4, 1),
      Affine::scale(0.5, 1.0),
      Spread::Pad,
      ImageFilter::Nearest,
      1.0,
    ) else {
      unreachable!("a half scale inverts");
    };
    let mut row = [[0; 4]; 2];

    shader.shade_row(0, 0, &mut row);

    assert_eq!(row, [red, red]);
  }

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
      Spread::Pad,
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
      Spread::Pad,
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
    let repeat = Gradient::new(shape, &stops, Spread::Repeat, Affine::IDENTITY).unwrap();
    let reflect = Gradient::new(shape, &stops, Spread::Reflect, Affine::IDENTITY).unwrap();

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
      Gradient::new(shape, &stops, Spread::Pad, Affine::IDENTITY).err(),
      Some(Some([0, 0, 255, 255]))
    );
    assert_eq!(
      Gradient::new(shape, &stops, Spread::Repeat, Affine::IDENTITY).err(),
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
      Spread::Pad,
      Affine::IDENTITY,
    )
    .unwrap();
    let pixels = row(&gradient, 4, 10);

    assert_eq!(pixels[4], pixels[5]);
    assert_eq!(pixels[0][3], 255);
    assert!(pixels[4][0] > pixels[0][0]);
  }

  /// tiny-skia ports the same Skia gradient code, so its pixels are the reference here.
  fn tiny_skia_pixels(shape: GradientShape, stops: &[GradientStop], size: u32) -> Vec<[u8; 4]> {
    use tiny_skia::{Color, GradientStop as TinyStop, Paint, Pixmap, SpreadMode};
    use tiny_skia::{Point, Rect, Transform};

    let tiny_stops = stops
      .iter()
      .map(|stop| {
        let [r, g, b, a] = stop.color;

        TinyStop::new(stop.position, Color::from_rgba(r, g, b, a).unwrap())
      })
      .collect();
    let shader = match shape {
      GradientShape::Linear { start, end } => tiny_skia::LinearGradient::new(
        Point::from_xy(start[0], start[1]),
        Point::from_xy(end[0], end[1]),
        tiny_stops,
        SpreadMode::Pad,
        Transform::identity(),
      ),
      GradientShape::Conical {
        start,
        start_radius,
        end,
        end_radius,
      } => tiny_skia::RadialGradient::new(
        Point::from_xy(start[0], start[1]),
        start_radius,
        Point::from_xy(end[0], end[1]),
        end_radius,
        tiny_stops,
        SpreadMode::Pad,
        Transform::identity(),
      ),
    }
    .unwrap();
    let mut pixmap = Pixmap::new(size, size).unwrap();

    pixmap.fill_rect(
      Rect::from_xywh(0.0, 0.0, size as f32, size as f32).unwrap(),
      &Paint {
        shader,
        anti_alias: false,
        ..Paint::default()
      },
      Transform::identity(),
      None,
    );
    pixmap
      .pixels()
      .iter()
      .map(|pixel| [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()])
      .collect()
  }

  #[test]
  fn conical_gradients_match_skia_in_every_focal_case() {
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
      let gradient = Gradient::new(shape, &stops, Spread::Pad, Affine::IDENTITY).unwrap();

      let reference = tiny_skia_pixels(shape, &stops, 32);

      for y in 0..32 {
        let ours = row(&gradient, y, 32);
        let theirs = &reference[y as usize * 32..][..32];

        for (x, (a, b)) in ours.iter().zip(theirs).enumerate() {
          assert!(
            a.iter().zip(b).all(|(p, q)| p.abs_diff(*q) <= 1),
            "{shape:?} at ({x}, {y}): {a:?} against {b:?}"
          );
        }
      }
    }
  }
}

// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{DiffuseLighting, LightSource, SpecularLighting};
use crate::{style::Color, svg::approx_eq::ApproxEq};

const FACTOR_1_2: f32 = 1.0 / 2.0;
const FACTOR_1_3: f32 = 1.0 / 3.0;
const FACTOR_1_4: f32 = 1.0 / 4.0;
const FACTOR_2_3: f32 = 2.0 / 3.0;

#[derive(Clone, Copy, Debug)]
struct Vector2 {
  x: f32,
  y: f32,
}

impl Vector2 {
  #[inline]
  fn new(x: f32, y: f32) -> Self {
    Vector2 { x, y }
  }

  #[inline]
  fn approx_zero(&self) -> bool {
    self.x.approx_eq(0.0) && self.y.approx_eq(0.0)
  }
}

impl core::ops::Mul<f32> for Vector2 {
  type Output = Self;

  #[inline]
  fn mul(self, c: f32) -> Self::Output {
    Vector2 {
      x: self.x * c,
      y: self.y * c,
    }
  }
}

#[derive(Clone, Copy, Debug)]
struct Vector3 {
  x: f32,
  y: f32,
  z: f32,
}

impl Vector3 {
  #[inline]
  fn new(x: f32, y: f32, z: f32) -> Self {
    Vector3 { x, y, z }
  }

  #[inline]
  fn dot(&self, other: &Self) -> f32 {
    self.x * other.x + self.y * other.y + self.z * other.z
  }

  #[inline]
  fn length(&self) -> f32 {
    (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
  }

  #[inline]
  fn normalized(&self) -> Option<Self> {
    let length = self.length();
    if !length.approx_eq(0.0) {
      Some(Vector3 {
        x: self.x / length,
        y: self.y / length,
        z: self.z / length,
      })
    } else {
      None
    }
  }
}

impl core::ops::Add<Vector3> for Vector3 {
  type Output = Self;

  #[inline]
  fn add(self, rhs: Vector3) -> Self::Output {
    Vector3 {
      x: self.x + rhs.x,
      y: self.y + rhs.y,
      z: self.z + rhs.z,
    }
  }
}

impl core::ops::Sub<Vector3> for Vector3 {
  type Output = Self;

  #[inline]
  fn sub(self, rhs: Vector3) -> Self::Output {
    Vector3 {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
      z: self.z - rhs.z,
    }
  }
}

/// The alpha channel of straight or premultiplied pixels.
#[derive(Clone, Copy)]
struct Alpha<'a> {
  pixels: &'a [[u8; 4]],
  width: usize,
  height: usize,
}

impl<'a> Alpha<'a> {
  fn new(pixels: &'a [[u8; 4]], width: usize) -> Self {
    Self {
      pixels,
      width,
      height: pixels.len() / width,
    }
  }

  fn at(&self, x: usize, y: usize) -> i16 {
    i16::from(self.pixels[self.width * y + x][3])
  }
}

#[derive(Clone, Copy, Debug)]
struct Normal {
  factor: Vector2,
  normal: Vector2,
}

impl Normal {
  #[inline]
  fn new(factor_x: f32, factor_y: f32, nx: i16, ny: i16) -> Self {
    Normal {
      factor: Vector2::new(factor_x, factor_y),
      normal: Vector2::new(-nx as f32, -ny as f32),
    }
  }
}

/// Blink's `FEDiffuseLighting` in `fe_diffuse_lighting.cc`, which Skia runs as
/// `SkImageFilters::DistantLitDiffuse`, `PointLitDiffuse`, and `SpotLitDiffuse`. Approximate: the
/// Sobel kernels follow resvg's port of Skia's.
///
/// Only the alpha of `src` is read, and `dest` gets straight alpha. Both are `width` pixels wide
/// and the same length, and nothing is done when they are less than 3x3.
pub(super) fn diffuse_lighting(
  fe: &DiffuseLighting,
  light_source: LightSource,
  lighting_color: Color,
  src: &[[u8; 4]],
  width: usize,
  dest: &mut [[u8; 4]],
) {
  let light_factor = |normal: Normal, light_vector: Vector3| {
    let k = if normal.normal.approx_zero() {
      light_vector.z
    } else {
      let mut n = normal.normal * (fe.surface_scale / 255.0);
      n.x *= normal.factor.x;
      n.y *= normal.factor.y;

      let normal = Vector3::new(n.x, n.y, 1.0);

      normal.dot(&light_vector) / normal.length()
    };

    fe.diffuse_constant * k
  };

  apply(
    light_source,
    fe.surface_scale,
    lighting_color,
    &light_factor,
    calc_diffuse_alpha,
    Alpha::new(src, width),
    dest,
  );
}

/// Blink's `FESpecularLighting` in `fe_specular_lighting.cc`, which Skia runs as
/// `SkImageFilters::DistantLitSpecular`, `PointLitSpecular`, and `SpotLitSpecular`. Approximate:
/// the Sobel kernels follow resvg's port of Skia's.
///
/// Only the alpha of `src` is read, and `dest` gets premultiplied alpha. Both are `width` pixels
/// wide and the same length, and nothing is done when they are less than 3x3.
pub(super) fn specular_lighting(
  fe: &SpecularLighting,
  light_source: LightSource,
  lighting_color: Color,
  src: &[[u8; 4]],
  width: usize,
  dest: &mut [[u8; 4]],
) {
  let light_factor = |normal: Normal, light_vector: Vector3| {
    let h = light_vector + Vector3::new(0.0, 0.0, 1.0);
    let h_length = h.length();

    if h_length.approx_eq(0.0) {
      return 0.0;
    }

    let k = if normal.normal.approx_zero() {
      let n_dot_h = h.z / h_length;
      if fe.specular_exponent.approx_eq(1.0) {
        n_dot_h
      } else {
        n_dot_h.powf(fe.specular_exponent)
      }
    } else {
      let mut n = normal.normal * (fe.surface_scale / 255.0);
      n.x *= normal.factor.x;
      n.y *= normal.factor.y;

      let normal = Vector3::new(n.x, n.y, 1.0);

      let n_dot_h = normal.dot(&h) / normal.length() / h_length;
      if fe.specular_exponent.approx_eq(1.0) {
        n_dot_h
      } else {
        n_dot_h.powf(fe.specular_exponent)
      }
    };

    fe.specular_constant * k
  };

  apply(
    light_source,
    fe.surface_scale,
    lighting_color,
    &light_factor,
    calc_specular_alpha,
    Alpha::new(src, width),
    dest,
  );
}

fn apply(
  light_source: LightSource,
  surface_scale: f32,
  lighting_color: Color,
  light_factor: &dyn Fn(Normal, Vector3) -> f32,
  calc_alpha: fn(u8, u8, u8) -> u8,
  src: Alpha,
  dest: &mut [[u8; 4]],
) {
  let Alpha { width, height, .. } = src;

  if width < 3 || height < 3 {
    return;
  }

  // `feDistantLight` has a fixed vector, so calculate it beforehand.
  let mut light_vector = match light_source {
    LightSource::Distant { azimuth, elevation } => {
      let azimuth = azimuth.to_radians();
      let elevation = elevation.to_radians();
      Vector3::new(
        azimuth.cos() * elevation.cos(),
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
      )
    }
    _ => Vector3::new(1.0, 1.0, 1.0),
  };

  let mut calc = |nx, ny, normal: Normal| {
    match light_source {
      LightSource::Distant { .. } => {}
      LightSource::Point { x, y, z } => {
        let nz = src.at(nx, ny) as f32 / 255.0 * surface_scale;
        let origin = Vector3::new(x, y, z);
        let v = origin - Vector3::new(nx as f32, ny as f32, nz);
        light_vector = v.normalized().unwrap_or(v);
      }
      LightSource::Spot(ref light) => {
        let nz = src.at(nx, ny) as f32 / 255.0 * surface_scale;
        let origin = Vector3::new(light.x, light.y, light.z);
        let v = origin - Vector3::new(nx as f32, ny as f32, nz);
        light_vector = v.normalized().unwrap_or(v);
      }
    }

    let light_color = light_color(&light_source, lighting_color, light_vector);
    let factor = light_factor(normal, light_vector);

    let compute = |x| ((x as f32 * factor).clamp(0.0, 255.0) + 0.5) as u8;

    let Color([red, green, blue, _]) = light_color;
    let r = compute(red);
    let g = compute(green);
    let b = compute(blue);
    let a = calc_alpha(r, g, b);

    dest[ny * width + nx] = [r, g, b, a];
  };

  calc(0, 0, top_left_normal(src));
  calc(width - 1, 0, top_right_normal(src));
  calc(0, height - 1, bottom_left_normal(src));
  calc(width - 1, height - 1, bottom_right_normal(src));

  for x in 1..width - 1 {
    calc(x, 0, top_row_normal(src, x));
    calc(x, height - 1, bottom_row_normal(src, x));
  }

  for y in 1..height - 1 {
    calc(0, y, left_column_normal(src, y));
    calc(width - 1, y, right_column_normal(src, y));
  }

  for y in 1..height - 1 {
    for x in 1..width - 1 {
      calc(x, y, interior_normal(src, x, y));
    }
  }
}

fn light_color(light: &LightSource, lighting_color: Color, light_vector: Vector3) -> Color {
  match *light {
    LightSource::Distant { .. } | LightSource::Point { .. } => lighting_color,
    LightSource::Spot(ref light) => {
      let origin = Vector3::new(light.x, light.y, light.z);
      let direction = Vector3::new(light.points_at_x, light.points_at_y, light.points_at_z);
      let direction = direction - origin;
      let direction = direction.normalized().unwrap_or(direction);
      let minus_l_dot_s = -light_vector.dot(&direction);
      if minus_l_dot_s <= 0.0 {
        return Color::black();
      }

      if let Some(limiting_cone_angle) = light.limiting_cone_angle
        && minus_l_dot_s < limiting_cone_angle.to_radians().cos()
      {
        return Color::black();
      }

      let factor = minus_l_dot_s.powf(light.specular_exponent);
      let compute = |x| ((x as f32 * factor).clamp(0.0, 255.0) + 0.5) as u8;

      let Color([red, green, blue, _]) = lighting_color;

      Color([compute(red), compute(green), compute(blue), 255])
    }
  }
}

fn top_left_normal(img: Alpha) -> Normal {
  let center = img.at(0, 0);
  let right = img.at(1, 0);
  let bottom = img.at(0, 1);
  let bottom_right = img.at(1, 1);

  Normal::new(
    FACTOR_2_3,
    FACTOR_2_3,
    -2 * center + 2 * right - bottom + bottom_right,
    -2 * center - right + 2 * bottom + bottom_right,
  )
}

fn top_right_normal(img: Alpha) -> Normal {
  let left = img.at(img.width - 2, 0);
  let center = img.at(img.width - 1, 0);
  let bottom_left = img.at(img.width - 2, 1);
  let bottom = img.at(img.width - 1, 1);

  Normal::new(
    FACTOR_2_3,
    FACTOR_2_3,
    -2 * left + 2 * center - bottom_left + bottom,
    -left - 2 * center + bottom_left + 2 * bottom,
  )
}

fn bottom_left_normal(img: Alpha) -> Normal {
  let top = img.at(0, img.height - 2);
  let top_right = img.at(1, img.height - 2);
  let center = img.at(0, img.height - 1);
  let right = img.at(1, img.height - 1);

  Normal::new(
    FACTOR_2_3,
    FACTOR_2_3,
    -top + top_right - 2 * center + 2 * right,
    -2 * top - top_right + 2 * center + right,
  )
}

fn bottom_right_normal(img: Alpha) -> Normal {
  let top_left = img.at(img.width - 2, img.height - 2);
  let top = img.at(img.width - 1, img.height - 2);
  let left = img.at(img.width - 2, img.height - 1);
  let center = img.at(img.width - 1, img.height - 1);

  Normal::new(
    FACTOR_2_3,
    FACTOR_2_3,
    -top_left + top - 2 * left + 2 * center,
    -top_left - 2 * top + left + 2 * center,
  )
}

fn top_row_normal(img: Alpha, x: usize) -> Normal {
  let left = img.at(x - 1, 0);
  let center = img.at(x, 0);
  let right = img.at(x + 1, 0);
  let bottom_left = img.at(x - 1, 1);
  let bottom = img.at(x, 1);
  let bottom_right = img.at(x + 1, 1);

  Normal::new(
    FACTOR_1_3,
    FACTOR_1_2,
    -2 * left + 2 * right - bottom_left + bottom_right,
    -left - 2 * center - right + bottom_left + 2 * bottom + bottom_right,
  )
}

fn bottom_row_normal(img: Alpha, x: usize) -> Normal {
  let top_left = img.at(x - 1, img.height - 2);
  let top = img.at(x, img.height - 2);
  let top_right = img.at(x + 1, img.height - 2);
  let left = img.at(x - 1, img.height - 1);
  let center = img.at(x, img.height - 1);
  let right = img.at(x + 1, img.height - 1);

  Normal::new(
    FACTOR_1_3,
    FACTOR_1_2,
    -top_left + top_right - 2 * left + 2 * right,
    -top_left - 2 * top - top_right + left + 2 * center + right,
  )
}

fn left_column_normal(img: Alpha, y: usize) -> Normal {
  let top = img.at(0, y - 1);
  let top_right = img.at(1, y - 1);
  let center = img.at(0, y);
  let right = img.at(1, y);
  let bottom = img.at(0, y + 1);
  let bottom_right = img.at(1, y + 1);

  Normal::new(
    FACTOR_1_2,
    FACTOR_1_3,
    -top + top_right - 2 * center + 2 * right - bottom + bottom_right,
    -2 * top - top_right + 2 * bottom + bottom_right,
  )
}

fn right_column_normal(img: Alpha, y: usize) -> Normal {
  let top_left = img.at(img.width - 2, y - 1);
  let top = img.at(img.width - 1, y - 1);
  let left = img.at(img.width - 2, y);
  let center = img.at(img.width - 1, y);
  let bottom_left = img.at(img.width - 2, y + 1);
  let bottom = img.at(img.width - 1, y + 1);

  Normal::new(
    FACTOR_1_2,
    FACTOR_1_3,
    -top_left + top - 2 * left + 2 * center - bottom_left + bottom,
    -top_left - 2 * top + bottom_left + 2 * bottom,
  )
}

fn interior_normal(img: Alpha, x: usize, y: usize) -> Normal {
  let top_left = img.at(x - 1, y - 1);
  let top = img.at(x, y - 1);
  let top_right = img.at(x + 1, y - 1);
  let left = img.at(x - 1, y);
  let right = img.at(x + 1, y);
  let bottom_left = img.at(x - 1, y + 1);
  let bottom = img.at(x, y + 1);
  let bottom_right = img.at(x + 1, y + 1);

  Normal::new(
    FACTOR_1_4,
    FACTOR_1_4,
    -top_left + top_right - 2 * left + 2 * right - bottom_left + bottom_right,
    -top_left - 2 * top - top_right + bottom_left + 2 * bottom + bottom_right,
  )
}

fn calc_diffuse_alpha(_: u8, _: u8, _: u8) -> u8 {
  255
}

fn calc_specular_alpha(r: u8, g: u8, b: u8) -> u8 {
  use core::cmp::max;
  max(max(r, g), b)
}

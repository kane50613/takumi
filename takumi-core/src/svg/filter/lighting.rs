use super::{BYTE_TO_FLOAT, DiffuseLighting, LightSource, SpecularLighting};
use crate::{geometry::Rect, style::Color};

/// How far past its cutoff a spotlight fades in, Skia's `kConeAAThreshold`.
const CONE_FADE: f32 = 0.016;

/// A lighting primitive in the layer's pixels, as Skia's `SkLightingImageFilter` takes it.
pub(super) struct Lighting {
  material: Material,
  light: Light,
  /// The light's color times the material's constant, in `0..=1` per unit of constant.
  color: [f32; 3],
  /// `surfaceScale` in the layer's pixels.
  depth: f32,
}

enum Material {
  Diffuse,
  Specular { shininess: f32 },
}

enum Light {
  /// The unit vector towards the light.
  Distant([f32; 3]),
  Point([f32; 3]),
  Spot {
    position: [f32; 3],
    /// The unit vector from the light towards what it points at.
    direction: [f32; 3],
    falloff: f32,
    cos_cutoff: f32,
  },
}

impl DiffuseLighting {
  /// The lighting of this primitive, with its light already in the layer's pixels.
  pub(super) fn lighting(&self, light: LightSource, color: Color, depth: f32) -> Lighting {
    Lighting::new(
      Material::Diffuse,
      light,
      color,
      self.diffuse_constant,
      self.surface_scale * depth,
    )
  }
}

impl SpecularLighting {
  /// The lighting of this primitive, with its light already in the layer's pixels.
  pub(super) fn lighting(&self, light: LightSource, color: Color, depth: f32) -> Lighting {
    Lighting::new(
      Material::Specular {
        shininess: self.specular_exponent,
      },
      light,
      color,
      self.specular_constant,
      self.surface_scale * depth,
    )
  }
}

impl Lighting {
  /// Blink's `FELighting::CreateImageFilter`, which hands the light to Skia.
  fn new(material: Material, light: LightSource, color: Color, k: f32, depth: f32) -> Self {
    let light = match light {
      LightSource::Distant { azimuth, elevation } => {
        let (azimuth, elevation) = (azimuth.to_radians(), elevation.to_radians());

        Light::Distant(unit_direction([
          azimuth.cos() * elevation.cos(),
          azimuth.sin() * elevation.cos(),
          elevation.sin(),
        ]))
      }
      LightSource::Point { x, y, z } => Light::Point([x, y, z]),
      LightSource::Spot(spot) => {
        let cone = spot
          .limiting_cone_angle
          .filter(|angle| *angle != 0.0 && (-90.0..=90.0).contains(angle))
          .unwrap_or(90.0);

        Light::Spot {
          position: [spot.x, spot.y, spot.z],
          direction: unit_direction([
            spot.points_at_x - spot.x,
            spot.points_at_y - spot.y,
            spot.points_at_z - spot.z,
          ]),
          falloff: spot.specular_exponent,
          cos_cutoff: cone.to_radians().cos(),
        }
      }
    };
    let Color([red, green, blue, _]) = color;
    let scale = k * BYTE_TO_FLOAT;

    Self {
      material,
      light,
      color: [red, green, blue].map(|channel| f32::from(channel) * scale),
      depth,
    }
  }

  /// Lights `window` from `source`'s alpha, as Skia's `sk_normal` and `sk_lighting` do.
  pub(super) fn apply(
    &self,
    source: &[[u8; 4]],
    width: usize,
    window: Rect<usize>,
    edges: Rect<isize>,
    out: &mut [[u8; 4]],
  ) {
    let span = window.right.saturating_sub(window.left);

    if span == 0 || window.bottom <= window.top {
      return;
    }

    let mut rows = [(); 3].map(|_| vec![0.0_f32; span + 2]);
    let mut row_labels = [None; 3];
    let mut normals = vec![[0.0_f32; 3]; span];
    let mut to_light = vec![[0.0_f32; 3]; span];
    let mut scales = vec![1.0_f32; span];

    for y in window.top..window.bottom {
      for dy in -1..=1 {
        let row = y as isize + dy;
        let slot = row.rem_euclid(3) as usize;

        if row_labels[slot] != Some(row) {
          alpha_row(
            source,
            width,
            edges,
            window.left as isize - 1,
            row,
            &mut rows[slot],
          );
          row_labels[slot] = Some(row);
        }
      }

      let slot = |dy: isize| (y as isize + dy).rem_euclid(3) as usize;
      let (above, middle, below) = (&rows[slot(-1)], &rows[slot(0)], &rows[slot(1)]);

      // `$normal_filter`: the Sobel kernel's right column less its left, and bottom row less its top.
      for (column, normal) in normals.iter_mut().enumerate() {
        let column_sum = |x: usize| 0.25 * above[x] + 0.5 * middle[x] + 0.25 * below[x];
        let row_sum =
          |row: &[f32]| 0.25 * row[column] + 0.5 * row[column + 1] + 0.25 * row[column + 2];
        let nx = column_sum(column + 2) - column_sum(column);
        let ny = row_sum(below) - row_sum(above);

        *normal = normalize([-self.depth * nx, -self.depth * ny, 1.0]);
      }

      // `$surface_to_light` and `$spotlight_scale`.
      let surface = |column: usize| {
        [
          (window.left + column) as f32 + 0.5,
          y as f32 + 0.5,
          self.depth * middle[column + 1],
        ]
      };

      match self.light {
        Light::Distant(direction) => to_light.fill(direction),
        Light::Point(position) => {
          for (column, to_light) in to_light.iter_mut().enumerate() {
            *to_light = normalize(sub(position, surface(column)));
          }
        }
        Light::Spot {
          position,
          direction,
          falloff,
          cos_cutoff,
        } => {
          for (column, (to_light, scale)) in to_light.iter_mut().zip(&mut scales).enumerate() {
            *to_light = normalize(sub(position, surface(column)));

            let cos_angle = -dot(*to_light, direction);

            *scale = if cos_angle < cos_cutoff {
              0.0
            } else if cos_angle < cos_cutoff + CONE_FADE {
              cos_angle.powf(falloff) * (cos_angle - cos_cutoff) / CONE_FADE
            } else {
              cos_angle.powf(falloff)
            };
          }
        }
      }

      // `$compute_lighting`.
      let line = &mut out[y * width + window.left..][..span];
      let lit = normals.iter().zip(&to_light).zip(&scales);

      match self.material {
        Material::Diffuse => {
          for (pixel, ((normal, to_light), scale)) in line.iter_mut().zip(lit) {
            let coefficient = dot(*normal, *to_light) * scale;
            let [red, green, blue] = self
              .color
              .map(|channel| (coefficient * channel).clamp(0.0, 1.0));

            *pixel = to_bytes([red, green, blue, 1.0]);
          }
        }
        Material::Specular { shininess } => {
          for (pixel, ((normal, to_light), scale)) in line.iter_mut().zip(lit) {
            let half_way = normalize([to_light[0], to_light[1], to_light[2] + 1.0]);
            let coefficient = dot(*normal, half_way).powf(shininess) * scale;
            let [red, green, blue] = self
              .color
              .map(|channel| (coefficient * channel).clamp(0.0, 1.0));

            *pixel = to_bytes([red, green, blue, red.max(green).max(blue)]);
          }
        }
      }
    }
  }
}

/// Fills `alpha` with the alpha of `source` along `row` from `left` on, clamped to `edges` and
/// transparent past the layer.
fn alpha_row(
  source: &[[u8; 4]],
  width: usize,
  edges: Rect<isize>,
  left: isize,
  row: isize,
  alpha: &mut [f32],
) {
  let height = (source.len() / width) as isize;
  let row = row.clamp(edges.top, edges.bottom - 1);

  if !(0..height).contains(&row) {
    alpha.fill(0.0);
    return;
  }

  let line = &source[row as usize * width..][..width];

  for (offset, alpha) in alpha.iter_mut().enumerate() {
    let x = (left + offset as isize).clamp(edges.left, edges.right - 1);

    *alpha = usize::try_from(x)
      .ok()
      .and_then(|x| line.get(x))
      .map_or(0.0, |pixel| f32::from(pixel[3]) * BYTE_TO_FLOAT);
  }
}

fn to_bytes(color: [f32; 4]) -> [u8; 4] {
  color.map(|channel| (channel * 255.0 + 0.5) as u8)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
  [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// SkSL's `normalize`, which multiplies by the inverse length.
fn normalize(vector: [f32; 3]) -> [f32; 3] {
  let inverse = 1.0 / dot(vector, vector).sqrt();

  vector.map(|component| component * inverse)
}

/// A light direction as Skia's `make_lighting_shader` sets it up: unit length, or zero for a zero
/// vector.
fn unit_direction(vector: [f32; 3]) -> [f32; 3] {
  let length = dot(vector, vector).sqrt();
  let inverse = if length > 0.0 { 1.0 / length } else { 0.0 };

  vector.map(|component| component * inverse)
}

//! What a path fill paints with, after Skia's shaders: a solid color, a gradient, or an image as
//! `SkImageShader` samples one.

use takumi_core::{resources::image::SvgSpreadMethod, style::Affine};

use crate::blend::scale_premultiplied_pixel;
#[cfg(feature = "svg")]
use crate::gradient::Gradient;

/// What a fill paints each pixel with.
pub(crate) enum Shader<'a> {
  /// One premultiplied color.
  #[cfg(feature = "svg")]
  Solid([u8; 4]),
  /// A gradient.
  #[cfg(feature = "svg")]
  Gradient(Gradient),
  /// An image.
  Image(ImageShader<'a>),
}

impl Shader<'_> {
  /// The premultiplied colors of the `out.len()` pixels that start at `(x, y)` along a row.
  pub(crate) fn shade_row(&self, x: i32, y: i32, out: &mut [[u8; 4]]) {
    match self {
      #[cfg(feature = "svg")]
      Self::Solid(color) => out.fill(*color),
      #[cfg(feature = "svg")]
      Self::Gradient(gradient) => gradient.shade_row(x, y, out),
      Self::Image(image) => image.shade_row(x, y, out),
    }
  }
}

/// How an image is sampled between its pixels, Skia's `SkFilterMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ImageFilter {
  #[cfg(feature = "svg")]
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
  spread: SvgSpreadMethod,
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
    spread: SvgSpreadMethod,
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
        #[cfg(feature = "svg")]
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
  #[cfg(feature = "svg")]
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
      SvgSpreadMethod::Repeat => coordinate - (coordinate / size as f32).floor() * size as f32,
      SvgSpreadMethod::Pad | SvgSpreadMethod::Reflect => coordinate,
    }
  }
}

#[cfg(all(test, feature = "svg"))]
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
      SvgSpreadMethod::Pad,
      ImageFilter::Nearest,
      1.0,
    ) else {
      unreachable!("a half scale inverts");
    };
    let mut row = [[0; 4]; 2];

    shader.shade_row(0, 0, &mut row);

    assert_eq!(row, [red, red]);
  }
}

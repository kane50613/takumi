//! What a path fill paints with, after Skia's shaders: a solid color, a gradient, or an image as
//! `SkImageShader` samples one.

use takumi_core::{paint::SpreadMethod, style::Affine};

use crate::blend::{scale_premultiplied_pixel, unorm_to_byte};
#[cfg(feature = "svg")]
use crate::gradient::Gradient;
use crate::pixmap::PixmapRef;

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
  spread: SpreadMethod,
  filter: ImageFilter,
  /// The paint alpha the samples scale by, `scale_1_float`.
  alpha: u8,
}

impl<'a> ImageShader<'a> {
  /// `image` drawn under `transform` from image pixels to device pixels at `opacity`. `None` for a
  /// transform that does not invert.
  pub(crate) fn new(
    image: PixmapRef<'a>,
    transform: Affine,
    spread: SpreadMethod,
    filter: ImageFilter,
    opacity: f32,
  ) -> Option<Self> {
    Some(Self {
      pixels: image.pixels(),
      width: image.width(),
      height: image.height(),
      device_to_image: transform.invert()?,
      spread,
      filter,
      alpha: unorm_to_byte(opacity),
    })
  }

  fn shade_row(&self, x: i32, y: i32, out: &mut [[u8; 4]]) {
    match self.filter {
      #[cfg(feature = "svg")]
      ImageFilter::Nearest => self.sample_row(x, y, out, |u, v| self.nearest(u, v)),
      ImageFilter::Linear => self.sample_row(x, y, out, |u, v| self.bilinear(u - 0.5, v - 0.5)),
    }
  }

  /// `sample` at each pixel center of the row from `(x, y)`, mapped onto the image.
  #[inline(always)]
  fn sample_row(&self, x: i32, y: i32, out: &mut [[u8; 4]], sample: impl Fn(f32, f32) -> [u8; 4]) {
    for (offset, pixel) in out.iter_mut().enumerate() {
      let (u, v) = self
        .device_to_image
        .transform_point(x as f32 + offset as f32 + 0.5, y as f32 + 0.5);

      *pixel = scale_premultiplied_pixel(sample(u, v), self.alpha);
    }
  }

  /// `bilerp_clamp_8888` and its tiled form: the four texels around `(u, v)` weighted by distance.
  fn bilinear(&self, u: f32, v: f32) -> [u8; 4] {
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (u - x0, v - y0);
    let (left, right) = (self.column(x0), self.column(x0 + 1.0));
    let (top, bottom) = (self.row_start(y0), self.row_start(y0 + 1.0));
    let corners = [
      (self.pixels[top + left], (1.0 - fx) * (1.0 - fy)),
      (self.pixels[top + right], fx * (1.0 - fy)),
      (self.pixels[bottom + left], (1.0 - fx) * fy),
      (self.pixels[bottom + right], fx * fy),
    ];

    std::array::from_fn(|channel| {
      let value: f32 = corners
        .iter()
        .map(|(texel, weight)| f32::from(texel[channel]) * weight)
        .sum();

      (value + 0.5) as u8
    })
  }

  /// The column of the pixel grid at `x`, tiled as `spread` has it.
  fn column(&self, x: f32) -> usize {
    self.tile(x, self.width).clamp(0.0, self.width as f32 - 1.0) as usize
  }

  /// The index of the first texel of the pixel grid's row at `y`, tiled as `spread` has it.
  fn row_start(&self, y: f32) -> usize {
    self
      .tile(y, self.height)
      .clamp(0.0, self.height as f32 - 1.0) as usize
      * self.width as usize
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
      SpreadMethod::Repeat => coordinate - (coordinate / size as f32).floor() * size as f32,
      SpreadMethod::Pad | SpreadMethod::Reflect => coordinate,
    }
  }
}

#[cfg(all(test, feature = "svg"))]
mod tests {
  use super::*;
  use takumi_core::geometry::Size;

  #[test]
  fn nearest_sampling_on_a_texel_edge_takes_the_texel_before_it() {
    let (red, blue) = ([255, 0, 0, 255], [0, 0, 255, 255]);
    let pixels = [red, blue, red, blue];
    let Some(image) = PixmapRef::from_bytes(pixels.as_flattened(), Size::new(4, 1)) else {
      unreachable!("four pixels make a 4 by 1 image");
    };
    let Some(shader) = ImageShader::new(
      image,
      Affine::scale(0.5, 1.0),
      SpreadMethod::Pad,
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

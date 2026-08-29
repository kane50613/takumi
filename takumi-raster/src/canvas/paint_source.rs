use image::Rgba;
use tiny_skia::{PixmapRef, PremultipliedColorU8};

use crate::{
  BackgroundTile, ColorTile, SampledBitmapView,
  blend::{premultiplied_from_pixel, premultiply_rgba},
  canvas::composite_premultiplied_over,
  style::{Color, ImageScalingAlgorithm},
};

#[derive(Clone, Copy)]
pub(crate) struct SamplingFootprint {
  pub(crate) x: f32,
  pub(crate) y: f32,
}

impl SamplingFootprint {
  pub(crate) fn new(x: f32, y: f32) -> Self {
    Self {
      x: x.max(0.0),
      y: y.max(0.0),
    }
  }

  fn is_minifying(self) -> bool {
    self.x > 1.0 || self.y > 1.0
  }

  fn box_span_x(self) -> f32 {
    self.x.max(1.0)
  }

  fn box_span_y(self) -> f32 {
    self.y.max(1.0)
  }
}

#[derive(Clone, Copy)]
pub(crate) enum PaintSource<'a> {
  Pixmap(PixmapRef<'a>),
  BackgroundTile(&'a BackgroundTile),
  ColorTile(&'a ColorTile),
}

impl<'a> PaintSource<'a> {
  pub(crate) fn width(self) -> u32 {
    match self {
      Self::Pixmap(pixmap) => pixmap.width(),
      Self::BackgroundTile(tile) => tile.width(),
      Self::ColorTile(tile) => tile.width(),
    }
  }

  pub(crate) fn height(self) -> u32 {
    match self {
      Self::Pixmap(pixmap) => pixmap.height(),
      Self::BackgroundTile(tile) => tile.height(),
      Self::ColorTile(tile) => tile.height(),
    }
  }

  pub(crate) fn get_pixel(self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
      Self::Pixmap(pixmap) => {
        let width = pixmap.width();
        let height = pixmap.height();
        if x >= width || y >= height {
          return PremultipliedColorU8::TRANSPARENT;
        }
        let index = (y * width + x) as usize;
        pixmap.pixels()[index]
      }
      Self::BackgroundTile(tile) => tile.get_pixel(x, y),
      Self::ColorTile(tile) => tile.get_pixel(x, y),
    }
  }

  /// Normalizes this source into the form pixel loops sample from, resolving
  /// everything that does not depend on the pixel exactly once. Tiles that
  /// already are a pixmap (including a bitmap drawn at its own size) come back
  /// as [`PaintSource::Pixmap`], and a scaled bitmap comes back as its hoisted
  /// [`SampledBitmapView`]. Every loop that reads more than one pixel must
  /// resolve first instead of sampling `self` directly.
  pub(crate) fn resolve(self) -> ResolvedSource<'a> {
    let Self::BackgroundTile(tile) = self else {
      return ResolvedSource::Direct(self);
    };

    match tile {
      BackgroundTile::Pixmap(source) => {
        ResolvedSource::Direct(Self::Pixmap(source.as_ref().as_ref()))
      }
      _ => match tile.sampled_bitmap_view() {
        Some(view) => match view.identity_source() {
          Some(source) => ResolvedSource::Direct(Self::Pixmap(source)),
          None => ResolvedSource::Bitmap(view),
        },
        None => ResolvedSource::Direct(self),
      },
    }
  }

  pub(crate) fn as_pixmap_ref(self) -> Option<PixmapRef<'a>> {
    match self.resolve() {
      ResolvedSource::Direct(Self::Pixmap(source)) => Some(source),
      _ => None,
    }
  }

  pub(crate) fn premultiplied_constant(self) -> Option<[u8; 4]> {
    match self {
      Self::ColorTile(tile) => Some(premultiplied_from_pixel(tile.get_pixel(0, 0))),
      Self::BackgroundTile(BackgroundTile::Color(tile)) => {
        Some(premultiplied_from_pixel(tile.get_pixel(0, 0)))
      }
      _ => None,
    }
  }

  fn write_premultiplied(self, dst: &mut [u8]) {
    let resolved = self.resolve();
    if let ResolvedSource::Direct(Self::Pixmap(source)) = resolved {
      dst.copy_from_slice(bytemuck::cast_slice(source.pixels()));
      return;
    }

    let width = self.width();
    let height = self.height();
    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(dst);
    for y in 0..height {
      for x in 0..width {
        pixels[(y * width + x) as usize] = premultiplied_from_pixel(resolved.get_pixel(x, y));
      }
    }
  }

  pub(crate) fn with_pixmap_ref<R>(self, f: impl FnOnce(PixmapRef<'_>) -> R) -> Option<R> {
    if let Some(source) = self.as_pixmap_ref() {
      return Some(f(source));
    }

    let width = self.width();
    let height = self.height();
    let source_len = width as usize * height as usize * 4;
    let mut premultiplied = vec![0; source_len];
    self.write_premultiplied(&mut premultiplied);

    PixmapRef::from_bytes(&premultiplied, width, height).map(f)
  }

  pub(crate) fn supports_rounded_fill_fast_path(self) -> bool {
    matches!(self, Self::Pixmap(_))
  }
}

/// A [`PaintSource`] after [`PaintSource::resolve`]: the form the interpolators
/// and pixel loops read from. Keeping them off the raw source is what
/// guarantees per-pixel work never re-derives sampling state.
#[derive(Clone, Copy)]
pub(crate) enum ResolvedSource<'a> {
  Direct(PaintSource<'a>),
  Bitmap(SampledBitmapView<'a>),
}

impl ResolvedSource<'_> {
  pub(crate) fn width(self) -> u32 {
    match self {
      Self::Direct(source) => source.width(),
      Self::Bitmap(view) => view.size().width,
    }
  }

  pub(crate) fn height(self) -> u32 {
    match self {
      Self::Direct(source) => source.height(),
      Self::Bitmap(view) => view.size().height,
    }
  }

  pub(crate) fn get_pixel(self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
      Self::Direct(source) => source.get_pixel(x, y),
      Self::Bitmap(view) => view.sample(x, y),
    }
  }
}

impl<'a> From<PixmapRef<'a>> for ResolvedSource<'a> {
  fn from(value: PixmapRef<'a>) -> Self {
    Self::Direct(PaintSource::Pixmap(value))
  }
}

impl<'a> From<PixmapRef<'a>> for PaintSource<'a> {
  fn from(value: PixmapRef<'a>) -> Self {
    Self::Pixmap(value)
  }
}

impl<'a> From<&'a tiny_skia::Pixmap> for PaintSource<'a> {
  fn from(value: &'a tiny_skia::Pixmap) -> Self {
    Self::Pixmap(value.as_ref())
  }
}

impl<'a> From<&'a BackgroundTile> for PaintSource<'a> {
  fn from(value: &'a BackgroundTile) -> Self {
    Self::BackgroundTile(value)
  }
}

impl<'a> From<&'a ColorTile> for PaintSource<'a> {
  fn from(value: &'a ColorTile) -> Self {
    Self::ColorTile(value)
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaskCompositeColor {
  SourceOnly,
  SourceOverColor([u8; 4]),
  ColorOverSource([u8; 4]),
}

impl MaskCompositeColor {
  pub(crate) fn source_over_color(color: Color) -> Self {
    Self::SourceOverColor(premultiply_rgba(Rgba(color.0)))
  }

  pub(crate) fn color_over_source(color: Color) -> Self {
    Self::ColorOverSource(premultiply_rgba(Rgba(color.0)))
  }
}

#[inline(always)]
pub(super) fn apply_mask_color_mode(src: [u8; 4], color_mode: MaskCompositeColor) -> [u8; 4] {
  match color_mode {
    MaskCompositeColor::SourceOnly => src,
    MaskCompositeColor::SourceOverColor(color) => {
      let mut out = color;
      composite_premultiplied_over(&mut out, src);
      out
    }
    MaskCompositeColor::ColorOverSource(color) => {
      let mut out = src;
      composite_premultiplied_over(&mut out, color);
      out
    }
  }
}

#[inline(always)]
pub(super) fn sample_paint_source(
  source: ResolvedSource<'_>,
  algorithm: ImageScalingAlgorithm,
  x: f32,
  y: f32,
  footprint: SamplingFootprint,
) -> Option<[u8; 4]> {
  interpolate_with_footprint(source, algorithm, x, y, footprint).map(premultiplied_from_pixel)
}

pub(crate) fn interpolate_with_footprint(
  image: ResolvedSource<'_>,
  algorithm: ImageScalingAlgorithm,
  x: f32,
  y: f32,
  footprint: SamplingFootprint,
) -> Option<PremultipliedColorU8> {
  if matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
    return interpolate_nearest(image, x, y);
  }

  if footprint.is_minifying() {
    return interpolate_box(image, x, y, footprint);
  }

  interpolate_bilinear(image, x, y)
}

#[inline(always)]
fn interpolate_nearest(image: ResolvedSource<'_>, x: f32, y: f32) -> Option<PremultipliedColorU8> {
  let w = image.width();
  let h = image.height();
  if w == 0 || h == 0 {
    return None;
  }

  let px = x.floor().max(0.0) as u32;
  let px = px.min(w.saturating_sub(1));
  let py = y.floor().max(0.0) as u32;
  let py = py.min(h.saturating_sub(1));

  Some(image.get_pixel(px, py))
}

#[inline(always)]
fn interpolate_bilinear(image: ResolvedSource<'_>, x: f32, y: f32) -> Option<PremultipliedColorU8> {
  let w = image.width();
  let h = image.height();
  if w == 0 || h == 0 {
    return None;
  }

  let x = (x - 0.5).clamp(0.0, w.saturating_sub(1) as f32);
  let y = (y - 0.5).clamp(0.0, h.saturating_sub(1) as f32);

  let uf = x.floor() as u32;
  let vf = y.floor() as u32;
  let uc = (uf + 1).min(w.saturating_sub(1));
  let vc = (vf + 1).min(h.saturating_sub(1));

  let p00 = image.get_pixel(uf, vf);
  if uf == uc && vf == vc {
    return Some(p00);
  }

  let u_ratio = ((x - uf as f32) * 256.0) as u32;
  let v_ratio = ((y - vf as f32) * 256.0) as u32;
  if u_ratio == 0 && v_ratio == 0 {
    return Some(p00);
  }

  let p01 = image.get_pixel(uf, vc);
  let p10 = image.get_pixel(uc, vf);
  let p11 = image.get_pixel(uc, vc);

  let u_opposite = 256 - u_ratio;
  let v_opposite = 256 - v_ratio;

  let w00 = u_opposite * v_opposite;
  let w01 = u_opposite * v_ratio;
  let w10 = u_ratio * v_opposite;
  let w11 = u_ratio * v_ratio;

  PremultipliedColorU8::from_rgba(
    ((p00.red() as u32 * w00
      + p10.red() as u32 * w10
      + p01.red() as u32 * w01
      + p11.red() as u32 * w11)
      >> 16) as u8,
    ((p00.green() as u32 * w00
      + p10.green() as u32 * w10
      + p01.green() as u32 * w01
      + p11.green() as u32 * w11)
      >> 16) as u8,
    ((p00.blue() as u32 * w00
      + p10.blue() as u32 * w10
      + p01.blue() as u32 * w01
      + p11.blue() as u32 * w11)
      >> 16) as u8,
    ((p00.alpha() as u32 * w00
      + p10.alpha() as u32 * w10
      + p01.alpha() as u32 * w01
      + p11.alpha() as u32 * w11)
      >> 16) as u8,
  )
}

fn interpolate_box(
  image: ResolvedSource<'_>,
  x: f32,
  y: f32,
  footprint: SamplingFootprint,
) -> Option<PremultipliedColorU8> {
  let width = image.width();
  let height = image.height();
  if width == 0 || height == 0 {
    return None;
  }

  let span_x = footprint.box_span_x();
  let span_y = footprint.box_span_y();
  let image_width = width as f32;
  let image_height = height as f32;

  let center_x = if span_x >= image_width {
    image_width * 0.5
  } else {
    x.clamp(span_x * 0.5, image_width - span_x * 0.5)
  };
  let center_y = if span_y >= image_height {
    image_height * 0.5
  } else {
    y.clamp(span_y * 0.5, image_height - span_y * 0.5)
  };

  let left = (center_x - span_x * 0.5).clamp(0.0, image_width);
  let right = (center_x + span_x * 0.5).clamp(0.0, image_width);
  let top = (center_y - span_y * 0.5).clamp(0.0, image_height);
  let bottom = (center_y + span_y * 0.5).clamp(0.0, image_height);

  let start_x = left.floor() as u32;
  let end_x = right.ceil().min(image_width) as u32;
  let start_y = top.floor() as u32;
  let end_y = bottom.ceil().min(image_height) as u32;

  let mut sum = [0.0; 4];
  let mut total_weight = 0.0;

  for source_y in start_y..end_y {
    let pixel_top = source_y as f32;
    let pixel_bottom = pixel_top + 1.0;
    let y_weight = (bottom.min(pixel_bottom) - top.max(pixel_top)).max(0.0);
    if y_weight == 0.0 {
      continue;
    }

    for source_x in start_x..end_x {
      let pixel_left = source_x as f32;
      let pixel_right = pixel_left + 1.0;
      let x_weight = (right.min(pixel_right) - left.max(pixel_left)).max(0.0);
      if x_weight == 0.0 {
        continue;
      }

      let weight = x_weight * y_weight;
      let pixel = image.get_pixel(source_x, source_y);
      sum[0] += pixel.red() as f32 * weight;
      sum[1] += pixel.green() as f32 * weight;
      sum[2] += pixel.blue() as f32 * weight;
      sum[3] += pixel.alpha() as f32 * weight;
      total_weight += weight;
    }
  }

  if total_weight == 0.0 {
    return None;
  }

  PremultipliedColorU8::from_rgba(
    (sum[0] / total_weight).round() as u8,
    (sum[1] / total_weight).round() as u8,
    (sum[2] / total_weight).round() as u8,
    (sum[3] / total_weight).round() as u8,
  )
}

#[cfg(test)]
mod tests {
  use tiny_skia::{Pixmap, PremultipliedColorU8};

  use super::{SamplingFootprint, interpolate_bilinear, interpolate_with_footprint};
  use crate::style::ImageScalingAlgorithm;

  /// Sampling takes pixel-centre coordinates: a draw that lands whole-pixel
  /// must read a texel exactly, not blend two of them.
  #[test]
  fn sampling_a_pixel_centre_reads_that_pixel() -> Result<(), &'static str> {
    let mut pixmap = Pixmap::new(2, 1).ok_or("failed to create pixmap")?;
    let pixels = pixmap.pixels_mut();
    pixels[0] = PremultipliedColorU8::from_rgba(0, 0, 0, 255).ok_or("black")?;
    pixels[1] = PremultipliedColorU8::from_rgba(255, 255, 255, 255).ok_or("white")?;

    let centre = interpolate_bilinear(pixmap.as_ref().into(), 0.5, 0.5).ok_or("sampled")?;
    assert_eq!(centre.red(), 0, "a pixel centre reads the pixel itself");

    let corner = interpolate_bilinear(pixmap.as_ref().into(), 0.0, 0.5).ok_or("sampled")?;
    assert_eq!(corner.red(), 0, "clamped at the left edge");

    let between = interpolate_bilinear(pixmap.as_ref().into(), 1.0, 0.5).ok_or("sampled")?;
    assert!(
      (between.red() as i16 - 128).abs() <= 1,
      "halfway between two pixels blends them"
    );

    Ok(())
  }

  #[test]
  fn minified_sampling_averages_high_frequency_content() -> Result<(), &'static str> {
    let mut pixmap = Pixmap::new(8, 1).ok_or("failed to create pixmap")?;
    for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
      let value = if index % 2 == 0 { 0 } else { 255 };
      *pixel = PremultipliedColorU8::from_rgba(value, value, value, 255)
        .ok_or("failed to create opaque grayscale pixel")?;
    }

    let sample = interpolate_with_footprint(
      pixmap.as_ref().into(),
      ImageScalingAlgorithm::Auto,
      4.0,
      0.5,
      SamplingFootprint::new(8.0, 1.0),
    )
    .ok_or("failed to sample minified image")?;

    assert!((sample.red() as i16 - 128).abs() <= 1);
    assert!((sample.green() as i16 - 128).abs() <= 1);
    assert!((sample.blue() as i16 - 128).abs() <= 1);
    assert_eq!(sample.alpha(), 255);
    Ok(())
  }
}

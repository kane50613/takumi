//! Path, gradient, decoration and image helpers translating takumi paint into krilla.

#[cfg(feature = "images")]
use takumi_core::resources::image::{ImageError, ImageSource, RenderedImage};
use takumi_core::{
  context::RenderContext,
  geometry::{ComputedLayout as Layout, PathCommand},
  style::{BlendMode, ComputedStyle, Overflow, ResolvedGradientStop},
};

use crate::{
  filter::ColorFilter,
  krilla::{
    blend::BlendMode as KrillaBlendMode,
    color::rgb,
    geom::{Path as KrillaPath, PathBuilder, Rect as KrillaRect},
    num::NormalizedF32,
    paint::{Fill, FillRule, SpreadMethod, Stop},
    surface::Surface,
  },
};
#[cfg(feature = "images")]
use crate::{krilla::image::Image as KrillaImage, raster::embedded_image};

/// A degenerate path, for a clip that must hide everything: an empty region is
/// what CSS asks for when a shape resolves to no area.
pub(crate) fn empty_path(x: f32, y: f32) -> Option<KrillaPath> {
  let mut builder = PathBuilder::new();

  builder.move_to(x, y);
  builder.line_to(x, y);
  builder.close();
  builder.finish()
}

/// A single-rectangle krilla path.
pub(crate) fn rect_path(rect: KrillaRect) -> Option<KrillaPath> {
  let mut builder = PathBuilder::new();

  builder.push_rect(rect);
  builder.finish()
}

/// Converts takumi-core path commands to a krilla path translated by `(x, y)`.
pub(crate) fn krilla_path(commands: &[PathCommand], x: f32, y: f32) -> Option<KrillaPath> {
  let mut builder = PathBuilder::new();

  for command in commands {
    match command {
      PathCommand::MoveTo(p) => builder.move_to(p.x + x, p.y + y),
      PathCommand::LineTo(p) => builder.line_to(p.x + x, p.y + y),
      PathCommand::QuadTo(c, p) => builder.quad_to(c.x + x, c.y + y, p.x + x, p.y + y),
      PathCommand::CubicTo(c1, c2, p) => {
        builder.cubic_to(c1.x + x, c1.y + y, c2.x + x, c2.y + y, p.x + x, p.y + y);
      }
      PathCommand::Close => builder.close(),
    }
  }
  builder.finish()
}

/// The rectangular overflow clip: each hidden axis bounds to the padding box,
/// a visible axis is left effectively unbounded.
pub(crate) fn overflow_clip_rect(
  style: &ComputedStyle,
  layout: Layout,
  x: f32,
  y: f32,
) -> Option<KrillaPath> {
  const UNBOUNDED: f32 = 1.0e6;
  let clip_x = style.overflow_x != Overflow::Visible;
  let clip_y = style.overflow_y != Overflow::Visible;

  let (left, right) = if clip_x {
    let padding_left = x + layout.border.left;
    let padding_right = (x + layout.size.width - layout.border.right).max(padding_left);
    (padding_left, padding_right)
  } else {
    (x - UNBOUNDED, x + layout.size.width + UNBOUNDED)
  };
  let (top, bottom) = if clip_y {
    let padding_top = y + layout.border.top;
    let padding_bottom = (y + layout.size.height - layout.border.bottom).max(padding_top);
    (padding_top, padding_bottom)
  } else {
    (y - UNBOUNDED, y + layout.size.height + UNBOUNDED)
  };
  KrillaRect::from_ltrb(left, top, right, bottom).and_then(rect_path)
}

/// Undoes the premultiplication takumi-core renders with: PDF image streams
/// carry straight alpha. Only the paths that go through core reach this;
/// embedded bytes never become premultiplied samples in the first place.
#[cfg(feature = "images")]
fn unpremultiply(data: &mut [u8]) {
  for pixel in data.as_chunks_mut::<4>().0 {
    let alpha = pixel[3];
    if alpha != 0 && alpha != 255 {
      let alpha16 = u16::from(alpha);
      pixel[0] = ((u16::from(pixel[0]) * 255 + alpha16 / 2) / alpha16).min(255) as u8;
      pixel[1] = ((u16::from(pixel[1]) * 255 + alpha16 / 2) / alpha16).min(255) as u8;
      pixel[2] = ((u16::from(pixel[2]) * 255 + alpha16 / 2) / alpha16).min(255) as u8;
    }
  }
}

#[cfg(feature = "images")]
fn encoded_bytes(source: &ImageSource) -> Option<&[u8]> {
  match source {
    ImageSource::Encoded(encoded) => Some(encoded.bytes()),
    _ => None,
  }
}

/// Why an image that had bytes could not be drawn.
#[cfg(feature = "images")]
fn undrawable_reason(filtered: bool, error: &ImageError) -> String {
  if filtered {
    format!("a filter needs the image's pixels: {error}")
  } else {
    error.to_string()
  }
}

/// Rasterizes an image source into a krilla image. Encoded bytes pass through
/// undecoded unless a filter has to run over the pixels; bitmap sources
/// rasterize at their intrinsic size; SVG sources at twice the target size for
/// print density.
///
/// `Ok(None)` is a source with nothing to draw, `Err` one that should have
/// drawn and could not.
#[cfg(feature = "images")]
pub(crate) fn rasterized_image(
  source: &ImageSource,
  context: &RenderContext,
  target: (f32, f32),
  filter: Option<&ColorFilter>,
) -> Result<Option<KrillaImage>, String> {
  if filter.is_none()
    && let Some(bytes) = encoded_bytes(source)
    && let Some(image) = embedded_image(bytes)
  {
    return Ok(Some(image));
  }

  let (width, height) = match source {
    ImageSource::Bitmap(bitmap) => (bitmap.width(), bitmap.height()),
    ImageSource::Animated(animated) => animated.dimensions(),
    ImageSource::Encoded(encoded) => encoded.dimensions(),
    #[cfg(feature = "svg")]
    ImageSource::Svg(_) => (
      (target.0 * 2.0).ceil() as u32,
      (target.1 * 2.0).ceil() as u32,
    ),
    _ => return Ok(None),
  };
  if width == 0 || height == 0 {
    return Ok(None);
  }
  let rendered = source
    .render_for_layout(
      width,
      height,
      context.style.image_rendering,
      0,
      context.current_color,
      Some(context.fonts()),
    )
    .map_err(|error| undrawable_reason(filter.is_some(), &error))?;
  let buffer = match &rendered {
    RenderedImage::Rasterized(buffer) => buffer.as_ref(),
    RenderedImage::Sampled { source, .. } => source.as_ref(),
  };
  let mut data = buffer.data().to_vec();

  unpremultiply(&mut data);
  if let Some(filter) = filter {
    for pixel in data.as_chunks_mut::<4>().0 {
      *pixel = filter.apply(*pixel);
    }
  }
  Ok(Some(KrillaImage::from_rgba8(
    data,
    buffer.width(),
    buffer.height(),
  )))
}

pub(crate) const fn spread(repeating: bool) -> SpreadMethod {
  if repeating {
    SpreadMethod::Repeat
  } else {
    SpreadMethod::Pad
  }
}

pub(crate) fn krilla_stop(offset: f32, rgba: [u8; 4]) -> Stop {
  Stop {
    offset: NormalizedF32::new(offset.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ZERO),
    color: rgb::Color::new(rgba[0], rgba[1], rgba[2]).into(),
    opacity: NormalizedF32::new(f32::from(rgba[3]) / 255.0).unwrap_or(NormalizedF32::ONE),
  }
}

pub(crate) fn krilla_stops(resolved: &[ResolvedGradientStop], base: f32, span: f32) -> Vec<Stop> {
  resolved
    .iter()
    .map(|stop| krilla_stop((stop.position - base) / span, stop.color.0))
    .collect()
}

/// Tiles one period of repeating stops across `extent` (the full gradient
/// radius), for shadings that cannot express a repeat natively.
pub(crate) fn expanded_radial_stops(resolved: &[ResolvedGradientStop], extent: f32) -> Vec<Stop> {
  let first = resolved.first().map_or(0.0, |s| s.position);
  let last = resolved.last().map_or(extent, |s| s.position);
  let period = (last - first).max(1e-6);
  // ponytail: a degenerate period (every stop at one position) would tile
  // millions of times. Past the cap the period stretches instead, so the stops
  // still reach the full radius rather than leaving the outer ring flat.
  const MAX_CYCLES: f32 = 512.0;
  let span = extent - first;
  let requested = (span / period).ceil().max(1.0);
  let cycles = requested.min(MAX_CYCLES);
  // Past the cap the period stretches, and the stops inside one period stretch
  // with it, so the last stop still reaches the full radius.
  let scale = if cycles < requested {
    (span / cycles) / period
  } else {
    1.0
  };
  let period = period * scale;
  let cycles = cycles as usize;
  let mut stops = Vec::with_capacity(cycles * resolved.len());

  for cycle in 0..cycles {
    let offset = first + cycle as f32 * period;

    for stop in resolved {
      stops.push(krilla_stop(
        (offset + (stop.position - first) * scale) / extent,
        stop.color.0,
      ));
    }
  }
  stops
}

pub(crate) const fn krilla_blend(mode: BlendMode) -> KrillaBlendMode {
  match mode {
    BlendMode::Multiply => KrillaBlendMode::Multiply,
    BlendMode::Screen => KrillaBlendMode::Screen,
    BlendMode::Overlay => KrillaBlendMode::Overlay,
    BlendMode::Darken => KrillaBlendMode::Darken,
    BlendMode::Lighten => KrillaBlendMode::Lighten,
    BlendMode::ColorDodge => KrillaBlendMode::ColorDodge,
    BlendMode::ColorBurn => KrillaBlendMode::ColorBurn,
    BlendMode::HardLight => KrillaBlendMode::HardLight,
    BlendMode::SoftLight => KrillaBlendMode::SoftLight,
    BlendMode::Difference => KrillaBlendMode::Difference,
    BlendMode::Exclusion => KrillaBlendMode::Exclusion,
    BlendMode::Hue => KrillaBlendMode::Hue,
    BlendMode::Saturation => KrillaBlendMode::Saturation,
    BlendMode::Color => KrillaBlendMode::Color,
    BlendMode::Luminosity => KrillaBlendMode::Luminosity,
    _ => KrillaBlendMode::Normal,
  }
}

pub(crate) fn pop_transforms(surface: &mut Surface, pushed: usize) {
  for _ in 0..pushed {
    surface.pop();
  }
}

pub(crate) fn fill_from_rgba(rgba: [u8; 4], opacity: f32) -> Fill {
  let alpha = (f32::from(rgba[3]) / 255.0) * opacity;

  Fill {
    paint: rgb::Color::new(rgba[0], rgba[1], rgba[2]).into(),
    opacity: NormalizedF32::new(alpha.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE),
    rule: FillRule::NonZero,
  }
}

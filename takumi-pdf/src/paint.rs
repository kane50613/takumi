//! Path, gradient, decoration and image helpers translating takumi paint into the writer.

#[cfg(feature = "images")]
use std::{cell::RefCell, sync::Arc};

#[cfg(feature = "images")]
use rustc_hash::FxHashMap;
#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext,
  resources::{
    image::{ImageError, ImageSource, RenderedImage},
    image_buffer::{ImageBuffer, demultiply_pixel},
  },
};
use takumi_core::{
  geometry::{PathCommand, Point, Size},
  painter::FillShape,
  style::{Affine, BlendMode, Color, FillRule as CoreFillRule, ResolvedGradientStop},
};

use crate::writer::{
  blend::BlendMode as PdfBlendMode,
  color::rgb,
  geom::{Path as PdfPath, PathBuilder, Rect as PdfRect, Transform},
  num::NormalizedF32,
  paint::{Fill, FillRule, SpreadMethod, Stop},
  stream::Stream,
  surface::Surface,
};
#[cfg(feature = "images")]
use crate::{filter::ColorFilter, raster::embedded_image, writer::image::Image as PdfImage};

/// A degenerate path, for a clip that must hide everything: an empty region is
/// what CSS asks for when a shape resolves to no area.
pub(crate) fn empty_path(origin: Point<f32>) -> Option<PdfPath> {
  let mut builder = PathBuilder::new();

  builder.move_to(origin.x, origin.y);
  builder.line_to(origin.x, origin.y);
  builder.close();
  builder.finish()
}

/// A single-rectangle writer path.
pub(crate) fn rect_path(rect: PdfRect) -> Option<PdfPath> {
  let mut builder = PathBuilder::new();

  builder.push_rect(rect);
  builder.finish()
}

/// Converts takumi-core path commands to a writer path translated by `origin`.
pub(crate) fn pdf_path(commands: &[PathCommand], origin: Point<f32>) -> Option<PdfPath> {
  let Point { x, y } = origin;
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

/// A fill shape as a writer path translated by `origin`.
pub(crate) fn shape_path(shape: &FillShape, origin: Point<f32>) -> Option<PdfPath> {
  match shape {
    FillShape::Rect(size) => {
      PdfRect::from_xywh(origin.x, origin.y, size.width, size.height).and_then(rect_path)
    }
    _ => pdf_path(&shape.to_commands(), origin),
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

/// An image source as a writer image, `Ok(None)` for no raster image, such as an SVG the vector
/// path draws.
#[cfg(feature = "images")]
pub(crate) fn rasterized_image(
  source: &ImageSource,
  context: &RenderContext,
  filter: Option<&ColorFilter>,
) -> Result<Option<PdfImage>, String> {
  if filter.is_none()
    && let ImageSource::Encoded(encoded) = source
    && let Some(image) = embedded_image(encoded.bytes())
  {
    return Ok(Some(image));
  }

  let (width, height) = match source {
    ImageSource::Bitmap(bitmap) => (bitmap.width(), bitmap.height()),
    ImageSource::Animated(animated) => animated.dimensions(),
    ImageSource::Encoded(encoded) => encoded.dimensions(),
    _ => return Ok(None),
  };
  if width == 0 || height == 0 {
    return Ok(None);
  }
  let rendered = source
    .render_for_layout(
      width,
      height,
      context.style.misc_inherited_data.image_rendering,
      0,
      context,
    )
    .map_err(|error| undrawable_reason(filter.is_some(), &error))?;
  let buffer = match &rendered {
    RenderedImage::Rasterized(buffer) => buffer.as_ref(),
    RenderedImage::Sampled { source, .. } => source.as_ref(),
  };

  Ok(Some(bitmap_image(buffer, filter)))
}

/// A decoded bitmap as a straight-alpha PDF image, recolored by `filter`.
#[cfg(feature = "images")]
pub(crate) fn bitmap_image(buffer: &ImageBuffer, filter: Option<&ColorFilter>) -> PdfImage {
  let mut data = buffer.data().to_vec();

  data
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .for_each(demultiply_pixel);
  if let Some(filter) = filter {
    for pixel in data.as_chunks_mut::<4>().0 {
      *pixel = filter.apply(*pixel);
    }
  }

  PdfImage::from_rgba8(data, buffer.width(), buffer.height())
}

/// What a rasterized image depends on: the `src` it resolved from and the filter.
#[cfg(feature = "images")]
#[derive(Hash, PartialEq, Eq)]
struct RasterKey {
  src: Arc<str>,
  filter: Vec<u32>,
}

/// Images [`rasterized_image`] made for one document, so an image drawn again from the same `src`
/// is decoded, filtered, and hashed once.
#[cfg(feature = "images")]
#[derive(Default)]
pub(crate) struct RasterizedImages {
  images: RefCell<FxHashMap<RasterKey, Option<PdfImage>>>,
}

#[cfg(feature = "images")]
impl RasterizedImages {
  /// [`rasterized_image`] of `source`, which `src` resolved to, reused when it was drawn the same
  /// way before.
  pub(crate) fn get(
    &self,
    src: &Arc<str>,
    source: &ImageSource,
    context: &RenderContext,
    filter: Option<&ColorFilter>,
  ) -> Result<Option<PdfImage>, String> {
    let key = RasterKey {
      src: Arc::clone(src),
      filter: filter.map(ColorFilter::key).unwrap_or_default(),
    };

    if let Some(image) = self.images.borrow().get(&key) {
      return Ok(image.clone());
    }

    let image = rasterized_image(source, context, filter)?;

    self.images.borrow_mut().insert(key, image.clone());
    Ok(image)
  }
}

pub(crate) const fn spread(repeating: bool) -> SpreadMethod {
  if repeating {
    SpreadMethod::Repeat
  } else {
    SpreadMethod::Pad
  }
}

pub(crate) fn pdf_stop(offset: f32, rgba: [u8; 4]) -> Stop {
  Stop {
    offset: NormalizedF32::new(offset.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ZERO),
    color: rgb::Color::new(rgba[0], rgba[1], rgba[2]).into(),
    opacity: normalized(f32::from(rgba[3]) / 255.0),
  }
}

pub(crate) fn pdf_stops(resolved: &[ResolvedGradientStop], base: f32, span: f32) -> Vec<Stop> {
  premultiplied_stops(resolved)
    .into_iter()
    .map(|(position, rgba)| pdf_stop((position - base) / span, rgba))
    .collect()
}

/// `resolved` with each transparent stop taking its neighbours' colours: the previous stop's
/// toward it and the next stop's past it, split in two where they differ. A PDF shading
/// interpolates colour apart from the alpha, where CSS interpolates premultiplied, which keeps a
/// colour toward a transparent stop instead of fading it through the stop's own colour.
fn premultiplied_stops(resolved: &[ResolvedGradientStop]) -> Vec<(f32, [u8; 4])> {
  let visible = |index: usize| resolved[index].color.0[3] != 0;
  let colour = |index: usize| {
    let [r, g, b, _] = resolved[index].color.0;

    [r, g, b, 0]
  };
  let mut stops = Vec::with_capacity(resolved.len());

  for (index, stop) in resolved.iter().enumerate() {
    if visible(index) {
      stops.push((stop.position, stop.color.0));
      continue;
    }

    let previous = (0..index).rev().find(|&other| visible(other)).map(colour);
    let next = (index + 1..resolved.len())
      .find(|&other| visible(other))
      .map(colour);

    match (previous, next) {
      (Some(previous), Some(next)) if previous != next => {
        stops.push((stop.position, previous));
        stops.push((stop.position, next));
      }
      (previous, next) => stops.push((stop.position, previous.or(next).unwrap_or(stop.color.0))),
    }
  }
  stops
}

/// The stretches of a gradient's axis, from `0` to `axis_length`, that `stops` paint opaque,
/// when each stop is opaque or transparent and the alpha changes only at hard stops. Drawing the
/// gradient opaque clipped to them then needs no soft mask, a construct Apple's PDF readers can
/// leave out, painting what lies under the transparent stops. `None` when the alpha blends or
/// never drops to zero.
pub(crate) fn opaque_runs(
  stops: &[ResolvedGradientStop],
  repeating: bool,
  axis_length: f32,
) -> Option<Vec<[f32; 2]>> {
  // How many periods a repeating gradient tiles before it keeps its soft mask instead.
  const MAX_RUNS: usize = 512;

  let opaque = |stop: &ResolvedGradientStop| stop.color.0[3] == u8::MAX;
  let binary = stops
    .iter()
    .all(|stop| matches!(stop.color.0[3], 0 | u8::MAX));
  let hard = stops
    .windows(2)
    .all(|pair| opaque(&pair[0]) == opaque(&pair[1]) || pair[0].position >= pair[1].position);
  let (first, last) = (stops.first()?, stops.last()?);

  if !binary || !hard || stops.iter().all(opaque) {
    return None;
  }

  // The runs of one pass along the stops, the padded ends reaching past the axis.
  let (before, after) = if repeating {
    (first.position, last.position)
  } else {
    (f32::NEG_INFINITY, f32::INFINITY)
  };
  let mut runs: Vec<[f32; 2]> = Vec::new();
  let mut start = opaque(first).then_some(before);

  for pair in stops.windows(2) {
    match (opaque(&pair[0]), opaque(&pair[1])) {
      (false, true) => start = Some(pair[1].position),
      (true, false) => runs.extend(start.take().map(|start| [start, pair[0].position])),
      _ => {}
    }
  }
  runs.extend(start.map(|start| [start, after]));

  if !repeating {
    return Some(clamp_runs(runs, axis_length));
  }

  let period = last.position - first.position;

  if period <= 0.0 {
    return None;
  }

  let periods = (first.position / period).ceil() as i64
    ..=((axis_length - first.position) / period).floor() as i64;
  let mut tiled = Vec::new();

  for shift in (periods.start() - 1)..=(periods.end() + 1) {
    for [start, end] in &runs {
      tiled.push([start + shift as f32 * period, end + shift as f32 * period]);
    }
    if tiled.len() > MAX_RUNS {
      return None;
    }
  }

  Some(clamp_runs(tiled, axis_length))
}

/// `runs` kept to the axis, a pixel past each end, with touching runs joined.
fn clamp_runs(runs: Vec<[f32; 2]>, axis_length: f32) -> Vec<[f32; 2]> {
  let mut clamped: Vec<[f32; 2]> = Vec::new();

  for [start, end] in runs {
    let [start, end] = [start.max(-1.0), end.min(axis_length + 1.0)];

    if start >= end {
      continue;
    }
    match clamped.last_mut() {
      Some(previous) if previous[1] >= start => previous[1] = previous[1].max(end),
      _ => clamped.push([start, end]),
    }
  }
  clamped
}

/// [`pdf_stops`] drawn opaque.
pub(crate) fn opaque_stops(resolved: &[ResolvedGradientStop], base: f32, span: f32) -> Vec<Stop> {
  premultiplied_stops(resolved)
    .into_iter()
    .map(|(position, [r, g, b, _])| pdf_stop((position - base) / span, [r, g, b, u8::MAX]))
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
  let period_stops = premultiplied_stops(resolved);
  let mut stops = Vec::with_capacity(cycles * period_stops.len());

  for cycle in 0..cycles {
    let offset = first + cycle as f32 * period;

    for &(position, rgba) in &period_stops {
      stops.push(pdf_stop(
        (offset + (position - first) * scale) / extent,
        rgba,
      ));
    }
  }
  stops
}

impl From<CoreFillRule> for FillRule {
  fn from(rule: CoreFillRule) -> Self {
    match rule {
      CoreFillRule::EvenOdd => Self::EvenOdd,
      _ => Self::NonZero,
    }
  }
}

/// An affine in takumi-core's column order, `[a, b, c, d, e, f]`.
impl From<Affine> for Transform {
  fn from(Affine { a, b, c, d, x, y }: Affine) -> Self {
    Self::from_row(a, b, c, d, x, y)
  }
}

impl From<Transform> for Affine {
  fn from(transform: Transform) -> Self {
    Self {
      a: transform.sx(),
      b: transform.ky(),
      c: transform.kx(),
      d: transform.sy(),
      x: transform.tx(),
      y: transform.ty(),
    }
  }
}

impl From<BlendMode> for PdfBlendMode {
  fn from(mode: BlendMode) -> Self {
    match mode {
      BlendMode::Multiply => Self::Multiply,
      BlendMode::Screen => Self::Screen,
      BlendMode::Overlay => Self::Overlay,
      BlendMode::Darken => Self::Darken,
      BlendMode::Lighten => Self::Lighten,
      BlendMode::ColorDodge => Self::ColorDodge,
      BlendMode::ColorBurn => Self::ColorBurn,
      BlendMode::HardLight => Self::HardLight,
      BlendMode::SoftLight => Self::SoftLight,
      BlendMode::Difference => Self::Difference,
      BlendMode::Exclusion => Self::Exclusion,
      BlendMode::Hue => Self::Hue,
      BlendMode::Saturation => Self::Saturation,
      BlendMode::Color => Self::Color,
      BlendMode::Luminosity => Self::Luminosity,
      _ => Self::Normal,
    }
  }
}

/// Fills the page box, an unset color leaving it empty as Chromium's print path does.
pub(crate) fn paint_page_background(color: Option<Color>, size: Size<f32>, surface: &mut Surface) {
  let Some(color) = color else {
    return;
  };
  let Some(path) = PdfRect::from_xywh(0.0, 0.0, size.width, size.height).and_then(rect_path) else {
    return;
  };

  surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
  surface.draw_path(&path);
}

/// Draws into a stream of its own, for a pattern tile or a mask.
pub(crate) fn draw_stream(surface: &mut Surface, draw: impl FnOnce(&mut Surface)) -> Stream {
  let mut builder = surface.stream_builder();
  let mut content = builder.surface();

  draw(&mut content);
  content.finish();
  builder.finish()
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
    opacity: normalized(alpha),
    rule: FillRule::NonZero,
  }
}

/// A value clamped into `0..=1`, with NaN read as one.
pub(crate) fn normalized(value: f32) -> NormalizedF32 {
  NormalizedF32::new(value.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE)
}

//! Path, gradient, decoration and image helpers translating takumi paint into the writer.

#[cfg(feature = "images")]
use std::{cell::RefCell, sync::Arc};

#[cfg(feature = "images")]
use rustc_hash::FxHashMap;
use takumi_core::resources::image::SvgSpreadMethod;
#[cfg(feature = "images")]
use takumi_core::{
  context::RenderContext,
  resources::{
    image::{ImageError, ImageSource, RenderedImage},
    image_buffer::{ImageBuffer, demultiply_pixel},
  },
};
use takumi_core::{geometry::Rect, path::Path, path::PathBuilder};
use takumi_core::{
  geometry::{PathCommand, Point, Size},
  painter::FillShape,
  style::{Color, FillRule, ResolvedGradientStop},
};

use crate::writer::{
  color::rgb,
  num::NormalizedF32,
  paint::{Fill, Stop},
  stream::Stream,
  surface::Surface,
};
#[cfg(feature = "images")]
use crate::{filter::ColorFilter, raster::embedded_image, writer::image::Image as PdfImage};

/// A degenerate path, for a clip that must hide everything: an empty region is
/// what CSS asks for when a shape resolves to no area.
pub(crate) fn empty_path(origin: Point<f32>) -> Option<Path> {
  let mut builder = PathBuilder::new();

  builder.move_to(origin.x, origin.y);
  builder.line_to(origin.x, origin.y);
  builder.close();
  builder.finish()
}

/// `commands` as a path translated by `origin`.
pub(crate) fn pdf_path(commands: &[PathCommand], origin: Point<f32>) -> Option<Path> {
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

/// `shape` as a path translated by `origin`.
pub(crate) fn shape_path(shape: &FillShape, origin: Point<f32>) -> Option<Path> {
  match shape {
    FillShape::Rect(size) => {
      Rect::from_xywh(origin.x, origin.y, size.width, size.height).map(Path::from_rect)
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

pub(crate) const fn spread(repeating: bool) -> SvgSpreadMethod {
  if repeating {
    SvgSpreadMethod::Repeat
  } else {
    SvgSpreadMethod::Pad
  }
}

pub(crate) fn pdf_stop(offset: f32, rgba: [u8; 4]) -> Stop {
  Stop {
    offset: NormalizedF32::new(offset.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ZERO),
    color: rgb::Color::new(rgba[0], rgba[1], rgba[2]).into(),
    opacity: normalized(f32::from(rgba[3]) / 255.0),
  }
}

/// Writer stops of `(position, rgba)` stops, placed from `base` over `span` and premultiplied.
pub(crate) fn pdf_stops(stops: &[(f32, [u8; 4])], base: f32, span: f32) -> Vec<Stop> {
  premultiplied_stops(stops)
    .into_iter()
    .map(|(position, rgba)| pdf_stop((position - base) / span, rgba))
    .collect()
}

pub(crate) fn positioned(resolved: &[ResolvedGradientStop]) -> Vec<(f32, [u8; 4])> {
  resolved
    .iter()
    .map(|stop| (stop.position, stop.color.0))
    .collect()
}

/// `stops` for a PDF shading, which interpolates colour apart from alpha where CSS interpolates
/// premultiplied, as Skia's `SkPDFGradientShader` does with a PostScript function. Takumi keeps
/// to stitched linear functions, which PDF/A exports allow, so a segment whose alpha changes
/// gains stops along the premultiplied curve, as [`premultiplied_curve`] approximates it, and a
/// transparent stop takes its neighbours' colours, the curve's limit there.
fn premultiplied_stops(stops: &[(f32, [u8; 4])]) -> Vec<(f32, [u8; 4])> {
  let visible = |index: usize| stops[index].1[3] != 0;
  let colour = |index: usize| {
    let [r, g, b, _] = stops[index].1;

    [r, g, b, 0]
  };
  let mut premultiplied = Vec::with_capacity(stops.len());

  for (index, &(position, rgba)) in stops.iter().enumerate() {
    if index > 0 {
      premultiplied_curve(stops[index - 1], stops[index], &mut premultiplied);
    }
    if visible(index) {
      premultiplied.push((position, rgba));
      continue;
    }

    let previous = (0..index).rev().find(|&other| visible(other)).map(colour);
    let next = (index + 1..stops.len())
      .find(|&other| visible(other))
      .map(colour);

    match (previous, next) {
      (Some(previous), Some(next)) if previous != next => {
        premultiplied.push((position, previous));
        premultiplied.push((position, next));
      }
      (previous, next) => premultiplied.push((position, previous.or(next).unwrap_or(rgba))),
    }
  }
  premultiplied
}

/// Pushes the stops strictly between `from` and `to` that bring a linear shading near their
/// premultiplied interpolation, none when the alpha holds or either end is transparent, where the
/// curve is the other end's colour. Approximate: each stretch is tested at its midpoint only,
/// against half an 8-bit level, the subdivision stops after `MAX_DEPTH` halvings, and the stops
/// round to bytes, so a steep alpha change near a nearly transparent end can stay further off.
fn premultiplied_curve(from: (f32, [u8; 4]), to: (f32, [u8; 4]), out: &mut Vec<(f32, [u8; 4])>) {
  // Halvings of a segment before its last stretch stays linear.
  const MAX_DEPTH: u32 = 8;

  let (alpha_from, alpha_to) = (f32::from(from.1[3]), f32::from(to.1[3]));

  if alpha_from == alpha_to || alpha_from == 0.0 || alpha_to == 0.0 {
    return;
  }

  // The unpremultiplied colour and the alpha at `u` along the segment.
  let at = |u: f32| {
    let alpha = alpha_from + (alpha_to - alpha_from) * u;
    let channel = |c: usize| {
      let premultiplied =
        f32::from(from.1[c]) * alpha_from * (1.0 - u) + f32::from(to.1[c]) * alpha_to * u;

      premultiplied / alpha
    };

    ([channel(0), channel(1), channel(2)], alpha)
  };
  let mut split = vec![(0.0f32, 1.0f32, 0u32)];

  while let Some((start, end, depth)) = split.pop() {
    let middle = (start + end) / 2.0;
    let (low, high, exact) = (at(start).0, at(end).0, at(middle).0);
    let straight = |c: usize| (low[c] + high[c]) / 2.0;

    if depth < MAX_DEPTH && (0..3).any(|c| (straight(c) - exact[c]).abs() > 0.5) {
      // The later half first, so the earlier one pops and lands first.
      split.push((middle, end, depth + 1));
      split.push((start, middle, depth + 1));
      continue;
    }
    if end < 1.0 {
      let (colour, alpha) = at(end);
      let byte = |value: f32| value.round().clamp(0.0, 255.0) as u8;

      out.push((
        from.0 + (to.0 - from.0) * end,
        [
          byte(colour[0]),
          byte(colour[1]),
          byte(colour[2]),
          byte(alpha),
        ],
      ));
    }
  }
}

/// The stretches of a gradient's axis, from `0` to `axis_length`, that `stops` paint opaque,
/// when each stop is opaque or transparent and the alpha changes only at hard stops. Drawing the
/// gradient opaque clipped to them then needs no soft mask, which Apple's PDF renderer can draw
/// wrong, as macOS does under a background clipped to text. `None` when the alpha blends or never
/// drops to zero.
pub(crate) fn opaque_runs(
  stops: &[(f32, [u8; 4])],
  repeating: bool,
  axis_length: f32,
) -> Option<Vec<[f32; 2]>> {
  // How many periods a repeating gradient tiles before it keeps its soft mask instead.
  const MAX_RUNS: usize = 512;

  let opaque = |stop: &(f32, [u8; 4])| stop.1[3] == u8::MAX;
  let binary = stops.iter().all(|stop| matches!(stop.1[3], 0 | u8::MAX));
  let hard = stops
    .windows(2)
    .all(|pair| opaque(&pair[0]) == opaque(&pair[1]) || pair[0].0 >= pair[1].0);
  let (first, last) = (stops.first()?, stops.last()?);

  if !binary || !hard || stops.iter().all(opaque) {
    return None;
  }

  // The runs of one pass along the stops, the padded ends reaching past the axis.
  let (before, after) = if repeating {
    (first.0, last.0)
  } else {
    (f32::NEG_INFINITY, f32::INFINITY)
  };
  let mut runs: Vec<[f32; 2]> = Vec::new();
  let mut start = opaque(first).then_some(before);

  for pair in stops.windows(2) {
    match (opaque(&pair[0]), opaque(&pair[1])) {
      (false, true) => start = Some(pair[1].0),
      (true, false) => runs.extend(start.take().map(|start| [start, pair[0].0])),
      _ => {}
    }
  }
  runs.extend(start.map(|start| [start, after]));

  if !repeating || runs.is_empty() {
    return Some(clamp_runs(runs, axis_length));
  }

  let period = last.0 - first.0;

  if period <= 0.0 {
    return None;
  }

  // From the first period whose end reaches the axis to the last whose start is still on it.
  let shifts = (-last.0 / period).floor() as i64..=((axis_length - first.0) / period).ceil() as i64;
  let mut tiled = Vec::new();

  for shift in shifts {
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
  let period_stops = premultiplied_stops(&positioned(resolved));
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

/// Fills the page box, an unset color leaving it empty as Chromium's print path does.
pub(crate) fn paint_page_background(color: Option<Color>, size: Size<f32>, surface: &mut Surface) {
  let Some(color) = color else {
    return;
  };
  let Some(path) = Rect::from_xywh(0.0, 0.0, size.width, size.height).map(Path::from_rect) else {
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

#[cfg(test)]
mod tests {
  use super::{opaque_runs, premultiplied_stops};

  const CLEAR: [u8; 4] = [0, 0, 0, 0];
  const YELLOW: [u8; 4] = [244, 239, 58, 255];

  #[test]
  fn a_highlighter_paints_only_past_its_hard_stops() {
    assert_eq!(
      opaque_runs(&[(34.0, CLEAR), (34.0, YELLOW)], false, 62.0),
      Some(vec![[34.0, 63.0]])
    );
    assert_eq!(
      opaque_runs(
        &[(58.0, CLEAR), (58.0, YELLOW), (92.0, YELLOW), (92.0, CLEAR)],
        false,
        100.0
      ),
      Some(vec![[58.0, 92.0]])
    );
  }

  #[test]
  fn an_alpha_that_blends_keeps_the_soft_mask() {
    let half = [244, 239, 58, 128];

    assert_eq!(
      opaque_runs(&[(0.0, YELLOW), (100.0, CLEAR)], false, 100.0),
      None
    );
    assert_eq!(
      opaque_runs(&[(0.0, half), (100.0, half)], false, 100.0),
      None
    );
    assert_eq!(
      opaque_runs(&[(0.0, YELLOW), (100.0, YELLOW)], false, 100.0),
      None
    );
  }

  #[test]
  fn repeating_stripes_tile_their_runs_across_the_axis() {
    let stripes = [(0.0, YELLOW), (10.0, YELLOW), (10.0, CLEAR), (20.0, CLEAR)];

    assert_eq!(
      opaque_runs(&stripes, true, 45.0),
      Some(vec![[0.0, 10.0], [20.0, 30.0], [40.0, 46.0]])
    );

    let late = [(20.0, YELLOW), (30.0, YELLOW), (30.0, CLEAR), (40.0, CLEAR)];

    assert_eq!(
      opaque_runs(&late, true, 45.0),
      Some(vec![[0.0, 10.0], [20.0, 30.0], [40.0, 46.0]])
    );
  }

  #[test]
  fn a_clear_repeating_gradient_has_no_runs_to_tile() {
    assert_eq!(
      opaque_runs(&[(0.0, CLEAR), (0.000_001, CLEAR)], true, 1000.0),
      Some(Vec::new())
    );
  }

  #[test]
  fn a_transparent_stop_keeps_the_colours_beside_it() {
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];

    assert_eq!(
      premultiplied_stops(&[(0.0, red), (100.0, CLEAR)]),
      [(0.0, red), (100.0, [255, 0, 0, 0])]
    );
    assert_eq!(
      premultiplied_stops(&[(0.0, red), (50.0, CLEAR), (100.0, blue)]),
      [
        (0.0, red),
        (50.0, [255, 0, 0, 0]),
        (50.0, [0, 0, 255, 0]),
        (100.0, blue)
      ]
    );
  }

  /// Half-transparent red to blue leans blue at the middle, premultiplied: `(85, 0, 170)`, where
  /// interpolating the colour alone gives `(128, 0, 128)`.
  #[test]
  fn a_fading_segment_follows_the_premultiplied_curve() {
    let stops = premultiplied_stops(&[(0.0, [255, 0, 0, 128]), (100.0, [0, 0, 255, 255])]);
    let colour_at = |position: f32| {
      let after = stops.iter().position(|stop| stop.0 >= position).unwrap();
      let ([before_at, after_at], [low, high]) = (
        [stops[after - 1].0, stops[after].0],
        [stops[after - 1].1, stops[after].1],
      );
      let u = (position - before_at) / (after_at - before_at);

      [0, 2].map(|c| f32::from(low[c]) + (f32::from(high[c]) - f32::from(low[c])) * u)
    };

    assert!(stops.len() > 2);
    assert!(stops.windows(2).all(|pair| pair[0].0 < pair[1].0));
    for (position, expected) in [(50.0, [85.0, 170.0]), (25.0, [153.0, 102.0])] {
      let [red, blue] = colour_at(position);

      assert!((red - expected[0]).abs() <= 1.0, "red {red} at {position}");
      assert!(
        (blue - expected[1]).abs() <= 1.0,
        "blue {blue} at {position}"
      );
    }
  }

  #[test]
  fn a_steady_alpha_needs_no_stops_between() {
    let half_red = [255, 0, 0, 128];
    let half_blue = [0, 0, 255, 128];

    assert_eq!(
      premultiplied_stops(&[(0.0, half_red), (100.0, half_blue)]),
      [(0.0, half_red), (100.0, half_blue)]
    );
  }
}

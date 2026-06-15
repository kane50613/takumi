//! CSS gradient → SVG paint emission.
//!
//! Linear and radial gradients map onto native `<linearGradient>` /
//! `<radialGradient>` (stops resolved with the same logic the raster backend
//! uses, so geometry and stop placement match). Conic gradients have no SVG 1.1
//! construct, so they are approximated by a fan of solid-color wedge `<path>`s
//! sampled from the gradient's color LUT and clipped to the box.

use std::f32::consts::TAU;

use takumi_core::context::RenderContext;
use takumi_core::layout::style::{
  BackgroundImage, ConicGradient, ConicGradientTile, LinearGradient, LinearGradientTile,
  RadialGradient, RadialGradientTile, ResolvedGradientStop, resolve_stops_along_axis,
};

use crate::{Affine, GradientStop, Rgba, SvgDocument};

const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
const CONIC_WEDGES: usize = 180;

/// Emits every background gradient for a node, painted bottom-up (CSS lists the
/// topmost layer first, so the slice is walked in reverse).
pub(crate) fn emit_background_images(
  images: &[BackgroundImage],
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) {
  if w <= 0.0 || h <= 0.0 {
    return;
  }
  for image in images.iter().rev() {
    match image {
      BackgroundImage::Linear(gradient) => emit_linear(gradient, context, x, y, w, h, doc),
      BackgroundImage::Radial(gradient) => emit_radial(gradient, context, x, y, w, h, doc),
      BackgroundImage::Conic(gradient) => emit_conic(gradient, context, x, y, w, h, doc),
      BackgroundImage::Url(_) | BackgroundImage::None => {}
    }
  }
}

fn svg_stops(stops: &[ResolvedGradientStop], base: f32, span: f32) -> Vec<GradientStop> {
  let span = span.max(1e-6);
  stops
    .iter()
    .map(|stop| GradientStop {
      offset: ((stop.position - base) / span).clamp(0.0, 1.0),
      color: Rgba(stop.color.0),
    })
    .collect()
}

fn emit_linear(
  gradient: &LinearGradient,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) {
  let tile = LinearGradientTile::new(
    gradient,
    w as u32,
    h as u32,
    &context.sizing,
    context.current_color,
  );
  let resolved = resolve_stops_along_axis(
    &gradient.stops,
    tile.axis_length.max(1e-6),
    &context.sizing,
    context.current_color,
  );
  if resolved.is_empty() {
    return;
  }

  // Map an axis position (px from one edge of the gradient line) to a point.
  let max_extent = tile.axis_length / 2.0;
  let (cx, cy) = (x + w / 2.0, y + h / 2.0);
  let point_at = |t: f32| {
    (
      cx + (t - max_extent) * tile.dir_x,
      cy + (t - max_extent) * tile.dir_y,
    )
  };

  let (t0, t1, base, span) = if gradient.repeating {
    let first = resolved.first().map_or(0.0, |s| s.position);
    let last = resolved.last().map_or(tile.axis_length, |s| s.position);
    (first, last, first, last - first)
  } else {
    (0.0, tile.axis_length, 0.0, tile.axis_length)
  };

  let stops = svg_stops(&resolved, base, span);
  let paint = doc.linear_gradient(point_at(t0), point_at(t1), gradient.repeating, &stops);
  doc.rect_paint(x, y, w, h, &paint);
}

fn emit_radial(
  gradient: &RadialGradient,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) {
  let tile = RadialGradientTile::new(
    gradient,
    w as u32,
    h as u32,
    &context.sizing,
    context.current_color,
  );
  let resolved = resolve_stops_along_axis(
    &gradient.stops,
    tile.radius_scale.max(1e-6),
    &context.sizing,
    context.current_color,
  );
  if resolved.is_empty() {
    return;
  }

  let radius_x = tile.inv_radius_x.recip();
  let radius_y = tile.inv_radius_y.recip();
  let (r, base, span) = if gradient.repeating {
    let first = resolved.first().map_or(0.0, |s| s.position);
    let last = resolved.last().map_or(tile.radius_scale, |s| s.position);
    ((last - first).max(1e-6), first, last - first)
  } else {
    (tile.radius_scale, 0.0, tile.radius_scale)
  };
  let scale = (
    (radius_x / tile.radius_scale.max(1e-6)).max(1e-6),
    (radius_y / tile.radius_scale.max(1e-6)).max(1e-6),
  );

  let stops = svg_stops(&resolved, base, span);
  let paint = doc.radial_gradient(
    (x + tile.cx, y + tile.cy),
    r,
    scale,
    gradient.repeating,
    &stops,
  );
  doc.rect_paint(x, y, w, h, &paint);
}

fn emit_conic(
  gradient: &ConicGradient,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) {
  let tile = ConicGradientTile::new(
    gradient,
    w as u32,
    h as u32,
    &context.sizing,
    context.current_color,
  );
  let lut_len = tile.color_lut.len();
  if lut_len == 0 {
    return;
  }

  let (ccx, ccy) = (x + tile.cx, y + tile.cy);
  let radius = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
    .into_iter()
    .map(|(px, py)| (px - ccx).hypot(py - ccy))
    .fold(0.0_f32, f32::max);

  let clip = doc.clip_path(&format!("M{x} {y} H{} V{} H{x} Z", x + w, y + h));
  let group = doc.begin_group(IDENTITY, 1.0, Some(&clip));
  for i in 0..CONIC_WEDGES {
    let a0 = i as f32 / CONIC_WEDGES as f32 * TAU;
    let a1 = (i + 1) as f32 / CONIC_WEDGES as f32 * TAU;
    let mid = (a0 + a1) / 2.0;
    let adjusted = (mid - tile.start_rad).rem_euclid(TAU);
    let idx = tile.lut_index_for_adjusted_angle_with_len(adjusted, lut_len);
    let color = tile.color_lut[idx].demultiply();
    let fill = Rgba([color.red(), color.green(), color.blue(), color.alpha()]);
    if fill.0[3] == 0 {
      continue;
    }
    let (x0, y0) = (ccx + radius * a0.sin(), ccy - radius * a0.cos());
    let (x1, y1) = (ccx + radius * a1.sin(), ccy - radius * a1.cos());
    doc.path(&format!("M{ccx} {ccy} L{x0} {y0} L{x1} {y1} Z"), fill);
  }
  doc.end_group(group);
}

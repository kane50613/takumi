//! CSS gradient / background-image → SVG paint emission.
//!
//! Linear and radial gradients map onto native `<linearGradient>` /
//! `<radialGradient>` (stops resolved with the same logic the raster backend
//! uses, so geometry and stop placement match). Conic gradients have no SVG 1.1
//! construct, so they are approximated by a fan of solid-color wedge `<path>`s
//! sampled from the gradient's color LUT and clipped to the tile.
//!
//! `background-size`/`background-position`/`background-repeat` are resolved with
//! the same logic the raster backend uses ([`BackgroundSize::resolve`] and the
//! tile-position collectors), producing a tile rectangle and a list of tile
//! origins. A single tile is painted directly; tiling repeats are emitted as an
//! SVG `<pattern>` filling the box.

use std::{f32::consts::TAU, io};

use taffy::Size;
use takumi_core::{
  context::RenderContext,
  layout::{
    node::resolve_image,
    style::{
      BackgroundImage, BackgroundPosition, BackgroundRepeat, BackgroundRepeatStyle, BackgroundSize,
      ColorInterpolationMethod, ConicGradient, ConicGradientTile, IntrinsicSizing, Length,
      LinearGradient, LinearGradientTile, PositionComponent, RadialGradient, RadialGradientTile,
      ResolvedGradientStop, SizingContext, build_color_lut_with_interpolation,
      resolve_stops_along_axis,
    },
  },
};

use crate::{
  APPROX_CHARS_PER_NUMBER, GradientStop, IDENTITY, Rgba, SvgDocument,
  box_model::{PathData, rect_path_data},
  image::{data_url_for_url, preserve_aspect_none},
};

const CONIC_WEDGES: usize = 180;

/// A resolved background/mask layer: the per-tile rect and the tile origins on
/// each axis (the cartesian product gives every tile placement).
struct LayerPlacement {
  tile_w: f32,
  tile_h: f32,
  xs: Vec<f32>,
  ys: Vec<f32>,
}

/// Emits every background gradient/image for a node, painted bottom-up (CSS lists
/// the topmost layer first, so the slice is walked in reverse). `background-size`,
/// `-position`, and `-repeat` are honored per layer (cycling the last value when
/// shorter than the image list, matching the raster backend).
pub(crate) fn emit_background_images(
  images: &[BackgroundImage],
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  emit_image_layers(
    images,
    &context.style.background_size,
    &context.style.background_position,
    &context.style.background_repeat,
    context,
    x,
    y,
    w,
    h,
    doc,
  )
}

/// Emits a list of background/mask image layers honoring per-layer size/position/
/// repeat. Shared by `background-image` and `mask-image`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_image_layers(
  images: &[BackgroundImage],
  sizes: &[BackgroundSize],
  positions: &[BackgroundPosition],
  repeats: &[BackgroundRepeat],
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if w <= 0.0 || h <= 0.0 {
    return Ok(());
  }
  let last_size = sizes.last().copied().unwrap_or_default();
  let last_position = positions.last().copied().unwrap_or_default();
  let last_repeat = repeats.last().copied().unwrap_or_default();

  for (index, image) in images.iter().enumerate().rev() {
    if matches!(image, BackgroundImage::None) {
      continue;
    }
    let size = sizes.get(index).copied().unwrap_or(last_size);
    let position = positions.get(index).copied().unwrap_or(last_position);
    let repeat = repeats.get(index).copied().unwrap_or(last_repeat);
    emit_layer(image, size, position, repeat, context, x, y, w, h, doc)?;
  }
  Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_layer(
  image: &BackgroundImage,
  size: BackgroundSize,
  position: BackgroundPosition,
  repeat: BackgroundRepeat,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let placement = resolve_placement(image, size, position, repeat, context, w, h);
  let Some(placement) = placement else {
    return Ok(());
  };
  if placement.tile_w <= 0.0 || placement.tile_h <= 0.0 {
    return Ok(());
  }

  // A single tile filling the box at the origin is the common case (no-repeat or
  // an exact fit); paint it directly. Otherwise tile via a <pattern>.
  if placement.xs.len() == 1 && placement.ys.len() == 1 {
    let tx = x + placement.xs[0];
    let ty = y + placement.ys[0];
    // A `cover`/positioned tile can extend past the box; clip it so it does not
    // bleed outside the element (the raster backend clips background to the box).
    let overflows = tx < x - 1e-3
      || ty < y - 1e-3
      || tx + placement.tile_w > x + w + 1e-3
      || ty + placement.tile_h > y + h + 1e-3;
    if overflows {
      let token = begin_box_clip(doc, x, y, w, h)?;
      emit_tile(
        image,
        context,
        tx,
        ty,
        placement.tile_w,
        placement.tile_h,
        doc,
      )?;
      return doc.end_group(token);
    }
    return emit_tile(
      image,
      context,
      tx,
      ty,
      placement.tile_w,
      placement.tile_h,
      doc,
    );
  }

  emit_tiled_pattern(image, context, x, y, w, h, &placement, doc)
}

/// Opens a group clipped to the layer box `(x, y, w, h)` so tiles that extend
/// past the edges (cover/positioned/repeated) don't bleed outside it.
fn begin_box_clip(
  doc: &mut SvgDocument,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
) -> io::Result<crate::GroupToken> {
  let clip = doc.clip_path(&rect_path_data(x, y, w, h))?;
  doc.begin_group(IDENTITY, 1.0, Some(&clip), None)
}

#[allow(clippy::too_many_arguments)]
fn emit_tiled_pattern(
  image: &BackgroundImage,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  placement: &LayerPlacement,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  // A `<pattern>` repeats on both axes, so only use it when both axes have
  // multiple evenly-spaced tiles; otherwise a single row/column would wrongly
  // repeat on the other axis, so emit the explicit grid.
  let even_x = is_even_step(&placement.xs, placement.tile_w);
  let even_y = is_even_step(&placement.ys, placement.tile_h);
  if let (Some(step_x), Some(step_y)) = (even_x, even_y)
    && placement.xs.len() > 1
    && placement.ys.len() > 1
  {
    let origin_x = x + placement.xs[0];
    let origin_y = y + placement.ys[0];
    let (token, paint) = doc.begin_pattern(origin_x, origin_y, step_x, step_y)?;
    emit_tile(
      image,
      context,
      0.0,
      0.0,
      placement.tile_w,
      placement.tile_h,
      doc,
    )?;
    doc.end_pattern(token)?;
    return doc.rect_paint(x, y, w, h, &paint);
  }

  // Explicit tile grid, clipped to the box so edge tiles don't bleed outside.
  let token = begin_box_clip(doc, x, y, w, h)?;
  for &ty in &placement.ys {
    for &tx in &placement.xs {
      emit_tile(
        image,
        context,
        x + tx,
        y + ty,
        placement.tile_w,
        placement.tile_h,
        doc,
      )?;
    }
  }
  doc.end_group(token)
}

/// Returns the uniform step between positions (tile size when there is a single
/// tile) if the positions are equally spaced, else `None`.
fn is_even_step(positions: &[f32], tile_size: f32) -> Option<f32> {
  match positions {
    [] => None,
    [_] => Some(tile_size),
    [first, second, ..] => {
      let step = second - first;
      positions
        .windows(2)
        .all(|w| (w[1] - w[0] - step).abs() < 0.5)
        .then_some(step)
    }
  }
}

/// Resolves a layer's tile size and per-axis tile origins (box-relative).
fn resolve_placement(
  image: &BackgroundImage,
  size: BackgroundSize,
  position: BackgroundPosition,
  repeat: BackgroundRepeat,
  context: &RenderContext,
  w: f32,
  h: f32,
) -> Option<LayerPlacement> {
  let area = Size {
    width: w.round().max(0.0) as u32,
    height: h.round().max(0.0) as u32,
  };
  let intrinsic = intrinsic_sizing(image, context);
  let resolved = size.resolve(area, &context.sizing, intrinsic);
  let tile_w = resolved.width as f32;
  let tile_h = resolved.height as f32;
  if tile_w <= 0.0 || tile_h <= 0.0 {
    return None;
  }

  let (xs, tile_w) = resolve_axis(repeat.0, position.0.x, tile_w, area.width, &context.sizing);
  let (ys, tile_h) = resolve_axis(repeat.1, position.0.y, tile_h, area.height, &context.sizing);

  Some(LayerPlacement {
    tile_w,
    tile_h,
    xs,
    ys,
  })
}

/// Resolves one axis' tile positions and (for `round`) adjusted tile size,
/// mirroring the raster backend's `resolve_axis_tiles`.
fn resolve_axis(
  repeat: BackgroundRepeatStyle,
  component: PositionComponent,
  tile_size: f32,
  area_size: u32,
  sizing: &SizingContext,
) -> (Vec<f32>, f32) {
  let tile = tile_size.round().max(0.0) as u32;
  match repeat {
    BackgroundRepeatStyle::NoRepeat => {
      let origin = resolve_position(component, tile, area_size, sizing);
      (vec![origin as f32], tile_size)
    }
    BackgroundRepeatStyle::Repeat => {
      let origin = resolve_position(component, tile, area_size, sizing);
      let positions = collect_repeat(area_size, tile, origin);
      (positions, tile_size)
    }
    BackgroundRepeatStyle::Space => {
      let positions = collect_spaced(area_size, tile);
      (positions, tile_size)
    }
    BackgroundRepeatStyle::Round => {
      let (positions, new_tile) = collect_round(area_size, tile);
      (positions, new_tile as f32)
    }
  }
}

/// `position% → (area − tile) × %`; lengths/keywords resolve against the
/// available space. Mirrors the raster backend's position resolution.
fn resolve_position(
  component: PositionComponent,
  tile_size: u32,
  area_size: u32,
  sizing: &SizingContext,
) -> i32 {
  let available = (area_size as i32).saturating_sub_unsigned(tile_size);
  let length = Length::from(component);
  match length {
    Length::Auto => available / 2,
    _ => length.to_px(sizing, available as f32) as i32,
  }
}

fn collect_repeat(area_size: u32, tile_size: u32, origin: i32) -> Vec<f32> {
  if tile_size == 0 {
    return Vec::new();
  }
  let mut start = origin;
  if start > 0 {
    let n = ((start as f32) / tile_size as f32).ceil() as i32;
    start -= n * tile_size as i32;
  }
  let mut positions = Vec::new();
  let mut x = start;
  while x < area_size as i32 {
    positions.push(x as f32);
    x += tile_size as i32;
  }
  positions
}

fn collect_spaced(area_size: u32, tile_size: u32) -> Vec<f32> {
  if tile_size == 0 {
    return Vec::new();
  }
  let count = area_size / tile_size;
  if count <= 1 {
    return vec![((area_size as i32 - tile_size as i32) / 2) as f32];
  }
  let gap = (area_size - count * tile_size) / (count - 1);
  let step = (tile_size + gap) as i32;
  (0..count as i32).map(|i| (i * step) as f32).collect()
}

fn collect_round(area_size: u32, tile_size: u32) -> (Vec<f32>, u32) {
  if tile_size == 0 || area_size == 0 {
    return (Vec::new(), tile_size);
  }
  let count = (area_size as f32 / tile_size as f32).max(1.0) as u32;
  let new_tile = (area_size as f32 / count as f32) as u32;
  let positions = (0..count as i32)
    .map(|i| (i * new_tile as i32) as f32)
    .collect();
  (positions, new_tile)
}

fn intrinsic_sizing(image: &BackgroundImage, context: &RenderContext) -> IntrinsicSizing {
  let BackgroundImage::Url(url) = image else {
    return IntrinsicSizing::default();
  };
  match resolve_image(url, context) {
    Ok(source) => source.intrinsic_sizing(&context.sizing),
    Err(_) => IntrinsicSizing::default(),
  }
}

/// Paints one tile of a layer into the rect `(tx, ty, tile_w, tile_h)`.
fn emit_tile(
  image: &BackgroundImage,
  context: &RenderContext,
  tx: f32,
  ty: f32,
  tile_w: f32,
  tile_h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  match image {
    BackgroundImage::Linear(gradient) => {
      emit_linear(gradient, context, tx, ty, tile_w, tile_h, doc)
    }
    BackgroundImage::Radial(gradient) => {
      emit_radial(gradient, context, tx, ty, tile_w, tile_h, doc)
    }
    BackgroundImage::Conic(gradient) => emit_conic(gradient, context, tx, ty, tile_w, tile_h, doc),
    BackgroundImage::Url(url) => emit_url(url, context, tx, ty, tile_w, tile_h, doc),
    BackgroundImage::None => Ok(()),
  }
}

fn emit_url(
  url: &str,
  context: &RenderContext,
  tx: f32,
  ty: f32,
  tile_w: f32,
  tile_h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some(href) = data_url_for_url(url, context) else {
    return Ok(());
  };
  doc.image(tx, ty, tile_w, tile_h, &href, Some(preserve_aspect_none()))
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

/// Number of stops sampled from the gradient color LUT for vector emission.
const GRADIENT_LUT_STOPS: usize = 64;

/// Builds dense SVG gradient stops by sampling takumi's interpolated color LUT,
/// baking the gradient's interpolation color space (e.g. OKLCH) into evenly-spaced
/// sRGB stops. SVG only interpolates between stops in sRGB, so sampling the LUT is
/// how the vector output matches the raster backend for non-sRGB interpolation.
///
/// CSS hard stops (two stops at the same position) are a discontinuity the uniform
/// LUT would smear into a ~1-cell ramp, so a coincident stop pair carrying the
/// exact before/after colors is injected at each hard-stop offset and the LUT
/// samples bridging it are dropped to keep the transition sharp.
fn lut_svg_stops(
  resolved: &[ResolvedGradientStop],
  axis_length: f32,
  interpolation: ColorInterpolationMethod,
) -> Vec<GradientStop> {
  let lut = build_color_lut_with_interpolation(
    resolved,
    axis_length.max(1e-6),
    GRADIENT_LUT_STOPS,
    interpolation.color_space,
    interpolation.hue_direction,
  );
  if lut.len() <= 1 {
    return svg_stops(resolved, 0.0, axis_length);
  }
  let span = axis_length.max(1e-6);
  let cell = 1.0 / (lut.len() - 1) as f32;
  let demul = |premultiplied: &tiny_skia::PremultipliedColorU8| {
    let color = premultiplied.demultiply();
    Rgba([color.red(), color.green(), color.blue(), color.alpha()])
  };

  // Hard stops: adjacent resolved stops with (near-)equal positions.
  let mut hard_stops = Vec::new();
  for pair in resolved.windows(2) {
    let (a, b) = (&pair[0], &pair[1]);
    if (b.position - a.position).abs() <= 1e-3 {
      hard_stops.push((
        (a.position / span).clamp(0.0, 1.0),
        Rgba(a.color.0),
        Rgba(b.color.0),
      ));
    }
  }

  let mut stops: Vec<GradientStop> = lut
    .iter()
    .enumerate()
    .filter(|(index, _)| {
      let offset = *index as f32 / (lut.len() - 1) as f32;
      // Drop LUT samples that straddle a hard stop; the injected pair covers it.
      !hard_stops
        .iter()
        .any(|(boundary, ..)| (offset - boundary).abs() < cell)
    })
    .map(|(index, premultiplied)| GradientStop {
      offset: index as f32 / (lut.len() - 1) as f32,
      color: demul(premultiplied),
    })
    .collect();

  for (boundary, before, after) in hard_stops {
    stops.push(GradientStop {
      offset: boundary,
      color: before,
    });
    stops.push(GradientStop {
      offset: boundary,
      color: after,
    });
  }
  stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
  stops
}

fn emit_linear(
  gradient: &LinearGradient,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
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
    return Ok(());
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

  let stops = if gradient.repeating {
    svg_stops(&resolved, base, span)
  } else {
    lut_svg_stops(&resolved, tile.axis_length, gradient.interpolation)
  };
  let paint = doc.linear_gradient(point_at(t0), point_at(t1), gradient.repeating, &stops)?;
  doc.rect_paint(x, y, w, h, &paint)
}

fn emit_radial(
  gradient: &RadialGradient,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
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
    return Ok(());
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

  let stops = if gradient.repeating {
    svg_stops(&resolved, base, span)
  } else {
    lut_svg_stops(&resolved, tile.radius_scale, gradient.interpolation)
  };
  let paint = doc.radial_gradient(
    (x + tile.cx, y + tile.cy),
    r,
    scale,
    gradient.repeating,
    &stops,
  )?;
  doc.rect_paint(x, y, w, h, &paint)
}

fn emit_conic(
  gradient: &ConicGradient,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let tile = ConicGradientTile::new(
    gradient,
    w as u32,
    h as u32,
    &context.sizing,
    context.current_color,
  );
  let lut_len = tile.color_lut.len();
  if lut_len == 0 {
    return Ok(());
  }

  let (ccx, ccy) = (x + tile.cx, y + tile.cy);
  let radius = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
    .into_iter()
    .map(|(px, py)| (px - ccx).hypot(py - ccy))
    .fold(0.0_f32, f32::max);

  let clip = doc.clip_path(&rect_path_data(x, y, w, h))?;
  let group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
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
    let mut wedge = PathData::with_capacity(6 * APPROX_CHARS_PER_NUMBER);
    wedge.command(b'M');
    wedge.pair(ccx, ccy);
    wedge.command(b'L');
    wedge.pair(x0, y0);
    wedge.pair(x1, y1);
    wedge.close();
    doc.path(&wedge.into_string(), fill)?;
  }
  doc.end_group(group)
}
